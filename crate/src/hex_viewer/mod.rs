//! # HexViewer
//!
//! Standalone hex dump widget for raw memory / binary inspection.
//!
//! Classic 3-column layout: **offset | hex bytes | ASCII**.
//! Supports color regions (struct overlays), data inspector,
//! goto-address, wildcard byte search, selection with copy,
//! optional editing with undo/redo, navigation history,
//! semantic byte-category coloring, and auto-refresh for live data.
//!
//! ## Module layout
//!
//! Implementation is split across focused files (each impl block lives
//! next to the helpers it needs). Cross-file access uses `pub(super)`
//! visibility on struct fields — every sub-module is inside
//! `hex_viewer`, so `super` is the same crate-private boundary.
//!
//! - [`config`] — `HexViewerConfig`, `BytesPerRow`, and all config
//!   enums/structs with serde derives.
//! - [`provider`] — `HexDataProvider`, `VecDataProvider`, `ColorRegion`,
//!   `ByteCategory`.
//! - [`nav_history`] — `NavHistory` back/forward address stack.
//! - [`undo`] — `UndoEntry`, `UndoStack`.
//! - [`search`] — `Selection`, `PatternByte`, wildcard pattern matching,
//!   clipboard format conversion, `do_search`.
//! - [`input`] — keyboard/mouse handling, edit-mode state machine,
//!   commit-pending-nibble logic.
//! - [`draw`] — render pipeline, row drawing, byte color overrides,
//!   data inspector.
//! - [`popup`] — floating goto/search popups.
//! - [`tests`] — unit + property tests for all of the above.

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md

pub mod config;
mod draw;
mod input;
pub mod nav_history;
mod popup;
pub mod provider;
// `pub(crate)` so `disasm_view` can re-use the wildcard-aware byte
// search primitives (`PatternByte`, `parse_hex_pattern_masked`,
// `find_pattern_masked`) — both widgets must agree on byte-search
// semantics, so a single source of truth beats a clone.
pub(crate) mod search;
pub mod undo;

#[cfg(test)]
mod tests;

pub use config::{
    AddressWidth, ByteGrouping, BytesPerRow, CopyFormat, Endianness, HexSearchMode,
    HexViewerConfig, StringEncoding,
};
// `AddressFormatter` is defined below in this file but re-exported
// from the public surface so consumers can name the closure-trait type
// without reaching into private paths.
pub use input::EditColumn;
pub use nav_history::NavHistory;
pub use provider::{
    ArcVecDataProvider, ByteCategory, ColorRegion, HexDataProvider, VecDataProvider,
};
pub use search::{PatternByte, Selection};
pub use undo::{UndoEntry, UndoStack};

use std::sync::Arc;

use crate::utils::clipboard;

/// Closure type for the host-supplied Address-column formatter.
///
/// Takes the absolute address of a row and returns:
/// - `Some(string)` — show that string in the Address column instead
///   of the default absolute hex (typical use: `"kernel32+0x1234"` for
///   memory inside a known loaded module, or a known export name).
/// - `None` — fall back to the default absolute hex format
///   (`{:016X}` / `{:08X}` per [`HexViewerConfig::address_width`]).
///
/// `Send + Sync` so the boxed closure can be cloned freely between
/// threads if the host moves the widget across them. Performance: this
/// is invoked once per visible row per frame — back it with a
/// sorted-by-address vector + binary search, not a linear scan.
pub type AddressFormatter = Arc<dyn Fn(u64) -> Option<String> + Send + Sync>;

/// Boxed host callback fired once per **committed** single-byte edit.
///
/// Arguments are `(va, old, new)`: the absolute address of the edited
/// byte, its previous value, and the freshly committed value. See
/// [`HexViewer::set_byte_edit_callback`] for the full contract.
///
/// `Send + Sync` so the boxed closure can move with the widget across
/// threads.
pub type ByteEditCallback = Box<dyn FnMut(u64, u8, u8) + Send + Sync>;

// ── HexViewer ────────────────────────────────────────────────────────────────

/// Standalone hex dump widget.
///
/// Fields are `pub(super)` so the input/draw/search sub-modules can
/// touch state without going through getters — keeps the hot render
/// path allocation-free and the input handler ergonomic.
pub struct HexViewer {
    /// Host-supplied widget identifier — kept on the struct for
    /// debugging / future feature gating even though every
    /// per-frame ImGui-ID lookup goes through the cached
    /// `goto_popup_id` / `search_popup_id` / `child_id` /
    /// `splitter_id` / etc. strings below.
    #[allow(dead_code)]
    pub(super) id: String,
    /// BUG-128 (2026-05-15) — `Arc<Vec<u8>>` instead of `Vec<u8>` so
    /// hosts that already own an `Arc` (sliding-window debugger panes
    /// that also feed the same bytes into a disassembler worker) can
    /// hand off ownership without an O(N) memcpy. Internal mutations
    /// (edit mode, undo/redo) still work — they go through
    /// [`Arc::make_mut`], which is zero-cost when HexViewer is the sole
    /// owner (the common case) and only clones when the host is holding
    /// a parallel reference. Read-side accessors (`data()` returning
    /// `&[u8]`) are unchanged for callers; the new
    /// [`Self::set_data_arc`] / [`Self::data_arc`] surface lets
    /// zero-copy-aware hosts opt into ownership-sharing semantics.
    pub(super) data: Arc<Vec<u8>>,
    pub(super) reference: Vec<u8>,
    pub(super) regions: Vec<ColorRegion>,
    pub(super) config: HexViewerConfig,

    // ── Cached ImGui IDs (built once at construction) ─────────
    pub(super) goto_popup_id: String,
    pub(super) search_popup_id: String,
    /// Cached child-window ID (`##hv_child_<id>`) used by `draw.rs`.
    /// Pre-built so the per-frame draw path doesn't `format!` it.
    pub(super) child_id: String,
    /// Cached invisible-button ID (`##hv_splitter_<id>`) used by
    /// `render_splitter`. Same reason as `child_id`.
    pub(super) splitter_id: String,

    pub(super) nav: NavHistory,
    pub(super) undo: UndoStack,

    // ── Interaction state ────────────────────────────────────
    pub(super) cursor: usize,
    pub(super) selection: Selection,
    /// Which column is being edited (None = not editing).
    pub(super) edit_column: Option<EditColumn>,
    /// Partial hex digit during hex editing (first nibble typed).
    pub(super) edit_nibble: Option<u8>,
    /// Whether keyboard layout was switched for editing.
    pub(super) layout_switched: bool,
    pub(super) goto_buf: String,
    pub(super) search_buf: String,
    pub(super) search_pattern: Vec<PatternByte>,
    pub(super) search_results: Vec<usize>,
    pub(super) search_idx: usize,
    pub(super) show_goto: bool,
    pub(super) show_search: bool,
    /// One-shot trigger: opens the right-click context menu on the
    /// next render. Set by the right-mouse-button handler in
    /// [`Self::handle_mouse`]; consumed (and reset) by
    /// `render_context_menu`.
    pub(super) show_context_menu: bool,
    /// Sticky flag: while `true`, the Settings popup body renders
    /// each frame. Toggled by the Settings entry in the context
    /// menu and by the popup's own Close button.
    pub(super) show_settings: bool,
    /// Cached ImGui IDs for the new popups — built once at
    /// construction so the runtime never re-allocates the strings.
    pub(super) context_popup_id: String,
    pub(super) settings_popup_id: String,
    /// Screen-space anchor used by the next popup that opens.
    /// Captured by the keyboard / mouse handlers when the user
    /// triggers a popup so the floating window appears at the
    /// click / keypress location instead of `(0, 0)` (the
    /// default when ImGui's `BeginPopup` runs outside any
    /// window context — which is what happens here, since
    /// popups are dispatched **before** the child window's body).
    pub(super) popup_open_pos: [f32; 2],
    /// Screen-space centre of the hex-viewer child window —
    /// captured every frame inside `render()` from
    /// `ui.window_pos() + ui.window_size() * 0.5`. The modal-style
    /// popups (Goto, Search, Settings) anchor at this point with
    /// a `(0.5, 0.5)` pivot so they always sit at the visual
    /// centre of the viewer regardless of where the user
    /// triggered them. The right-click context menu deliberately
    /// keeps anchoring at `popup_open_pos` (the click location)
    /// because that's the standard context-menu UX.
    pub(super) component_center: [f32; 2],
    /// One-shot flag: when set, the goto-popup body grabs keyboard
    /// focus on its next render so the user can start typing
    /// without clicking into the input field. Set when the popup
    /// opens (Ctrl+G or [`Self::request_goto`]); cleared inside
    /// the popup body after `set_keyboard_focus_here`.
    pub(super) goto_focus_pending: bool,
    /// Same as [`Self::goto_focus_pending`] but for the search popup.
    pub(super) search_focus_pending: bool,
    pub(super) scroll_to_row: Option<usize>,
    pub(super) char_advance: f32,
    pub(super) line_height: f32,
    pub(super) focused: bool,
    pub(super) frame_count: u32,

    /// User-controlled inspector subview height in pixels. `0.0` means
    /// "auto" (`inspector_height()`). Set when the user drags the
    /// horizontal splitter between the hex area and the inspector.
    pub(super) inspector_h: f32,

    /// Visual flash for the address that was just copied via a left
    /// click in the address gutter. `(row_index, frames_remaining)` —
    /// the address text gets a background tint while
    /// `frames_remaining` is non-zero, decremented once per `render`
    /// frame. `None` when no flash is active.
    pub(super) address_flash: Option<(usize, u32)>,

    /// Width of the inner content area of the hex child-window, in
    /// pixels — captured at the start of every `render` frame via
    /// `ui.content_region_avail()[0]`. Used by [`Self::ascii_col_x`]
    /// to right-anchor the ASCII column to the child's right edge
    /// (rather than letting it float right after the hex column,
    /// which left a large dead zone on the right when the window was
    /// wider than the byte content). Both draw + hit-test paths read
    /// this so the ASCII column lines up with where mouse clicks
    /// expect it.
    pub(super) inner_content_w: f32,

    /// BUG-128 (2026-05-15) — first/last row indices of the rows that
    /// are currently visible inside the hex child-window. Populated
    /// every frame inside `render()` from the same `scroll_y +
    /// visible_h` math the virtualisation loop uses, so the public
    /// `viewport_first_va` / `viewport_last_va` getters return the
    /// REAL visible range (not the cursor row). Both fields are
    /// `usize::MAX` when the viewer hasn't rendered yet — callers use
    /// `viewport_first_va` / `viewport_last_va` which return `None`
    /// in that case.
    pub(super) viewport_first_row: usize,
    pub(super) viewport_last_row: usize,

    /// Phase 2 (provider-driven render): cached `usize`-projection of
    /// the active provider's `len()` for the current frame. Set at the
    /// top of [`Self::render_impl`] and consumed by layout / hit-test
    /// helpers that historically read `self.data.len()` directly
    /// (`offset_col_width`, `mouse_to_offset`, `mouse_to_address_row`).
    ///
    /// For the legacy [`Self::render`] entry point this equals
    /// `self.data.len()`. For hosts using
    /// [`Self::render_with_provider`] with a streaming provider this
    /// is the provider's clamped length — so the address-gutter digit
    /// count, partial-row hit-tests and total scrollbar extent reflect
    /// the *provider's* view of the buffer, not the viewer's stale
    /// internal `Arc<Vec<u8>>`. Outside the render frame this still
    /// reads `self.data.len()` (no provider available between frames),
    /// which is the right default for all the public getters
    /// (`cursor_address`, `viewport_first_va`, etc.).
    pub(super) effective_data_len: usize,

    /// BUG-128 (M2) — pre-computed 256-entry foreground colour palette,
    /// indexed by byte value. Rebuilt at the start of every `render()`
    /// from `HexViewerConfig::byte_fg_color`. The per-visible-byte hot
    /// path (`byte_fg_with_overrides`) reads this array instead of
    /// dispatching to the method, which was the dominant cost when
    /// `category_colors == true` (5-arm match per byte). Cost shifts
    /// from `O(visible_bytes × match)` to `O(256)` per frame.
    /// Pre-packed `u32` (not `[f32; 4]`) so the inner loop skips the
    /// `col32` conversion as well.
    pub(super) byte_palette: [u32; 256],

    /// Optional host-supplied Address-column formatter. When `Some`,
    /// invoked for every visible row's absolute address; if it returns
    /// `Some(string)` that string is rendered instead of the default
    /// hex format. `None` keeps every row in the default absolute hex
    /// rendering.
    ///
    /// Excluded from RON round-trip — it's runtime payload, not
    /// configuration. See [`Self::set_address_formatter`].
    pub(super) address_formatter: Option<AddressFormatter>,

    /// BUG-127 (2026-05-15) — host-supplied callback for VA-mode goto.
    ///
    /// When the user opens the goto popup (Ctrl+G) and enters an address
    /// that lies OUTSIDE `[base_address, base_address + data.len())`, the
    /// viewer's own `set_cursor` would clamp the offset to the last byte
    /// of the buffer — useless for hosts that drive a sliding-window
    /// view over a much larger address space (e.g. a debugger memory
    /// pane). When this callback is set, the popup invokes it with the
    /// parsed absolute VA instead, leaving the host free to re-anchor
    /// its window, queue a memory read, and (next frame) place the
    /// cursor at the new in-buffer offset.
    ///
    /// `None` keeps the legacy clamp-to-buffer behaviour, which is the
    /// correct semantics for hosts that show a single static buffer
    /// (binary editor, packet inspector, etc.).
    pub(super) va_goto_callback: Option<Box<dyn FnMut(u64) + Send + Sync>>,
    /// Host-supplied notification fired once per **committed** single-byte
    /// edit — see [`Self::set_byte_edit_callback`] for the contract.
    /// `None` keeps the legacy "local-only mutation" behaviour, which is
    /// the right semantics for hosts editing a freestanding buffer with
    /// no upstream backing store (binary file editor, packet inspector,
    /// ROM viewer).
    pub(super) byte_edit_callback: Option<ByteEditCallback>,
    // The active locale lives on `config.locale` so it round-trips
    // through `ron::to_string(&cfg)` / `ron::from_str` along with
    // every other display flag — see `with_locale` / `set_locale`.
}

impl HexViewer {
    pub fn new(id: impl Into<String>) -> Self {
        let id: String = id.into();
        let goto_popup_id = format!("##goto_{id}");
        let search_popup_id = format!("##search_{id}");
        let context_popup_id = format!("##ctx_{id}");
        let settings_popup_id = format!("##settings_{id}");
        let child_id = format!("##hv_child_{id}");
        let splitter_id = format!("##hv_splitter_{id}");
        Self {
            id,
            data: Arc::new(Vec::new()),
            reference: Vec::new(),
            regions: Vec::new(),
            config: HexViewerConfig::default(),
            goto_popup_id,
            search_popup_id,
            child_id,
            splitter_id,
            context_popup_id,
            settings_popup_id,
            // Hard-coded `64` per-direction back/forward stack depth.
            // 30 was attempted briefly on 2026-04-29 but the project
            // owner reverted — 64 is plenty for follow-the-pointer
            // debugging sessions and the `VecDeque` cost is negligible.
            nav: NavHistory::new(64),
            undo: UndoStack::default(),
            cursor: 0,
            selection: Selection::default(),
            edit_column: None,
            edit_nibble: None,
            layout_switched: false,
            goto_buf: String::new(),
            search_buf: String::new(),
            search_pattern: Vec::new(),
            search_results: Vec::new(),
            search_idx: 0,
            show_goto: false,
            show_search: false,
            show_context_menu: false,
            show_settings: false,
            goto_focus_pending: false,
            search_focus_pending: false,
            scroll_to_row: None,
            char_advance: 0.0,
            line_height: 0.0,
            focused: false,
            frame_count: 0,
            inspector_h: 0.0,
            address_flash: None,
            inner_content_w: 0.0,
            popup_open_pos: [0.0, 0.0],
            component_center: [0.0, 0.0],
            address_formatter: None,
            va_goto_callback: None,
            byte_edit_callback: None,
            viewport_first_row: usize::MAX,
            viewport_last_row: usize::MAX,
            effective_data_len: 0,
            byte_palette: [0; 256],
        }
    }

    /// BUG-127: install a host callback invoked when the goto-popup
    /// receives an address OUTSIDE the current buffer's
    /// `[base_address, base_address + data.len())` range. Typical use:
    /// hosts driving a sliding-window view (debugger memory pane) wire
    /// this to their `re-anchor-window + queue-read` path.
    ///
    /// ```rust,no_run
    /// # use dear_imgui_custom_mod::hex_viewer::HexViewer;
    /// let mut hv = HexViewer::new("dump");
    /// hv.set_va_goto_callback(Some(Box::new(|target_va: u64| {
    ///     // host: re-anchor window at target_va, queue ReadMem
    /// })));
    /// ```
    pub fn set_va_goto_callback(&mut self, f: Option<Box<dyn FnMut(u64) + Send + Sync>>) {
        self.va_goto_callback = f;
    }

    /// Install (or clear with `None`) a host-supplied notification fired
    /// once per **committed** single-byte edit — `(va, old_byte, new_byte)`.
    /// "Committed" means the user finished typing a hex / ASCII change
    /// (left the byte, pressed Tab/Enter, or moved the cursor); partial
    /// nibbles in progress do **not** fire the callback.
    ///
    /// The viewer's internal buffer has *already* been mutated to
    /// `new_byte` by the time the callback runs — the callback's job is
    /// to propagate the change to whatever backs the viewer (a file on
    /// disk, a debug-target's memory, a network packet under
    /// composition, …). The callback is also a natural place to record
    /// an external undo entry; the viewer's own undo stack is
    /// independent and remains the authoritative `Ctrl+Z` source for
    /// in-buffer rollback.
    ///
    /// Not invoked on:
    ///   * `set_data` / `set_data_vec` / `set_data_arc` — buffer
    ///     replacement, not user-driven editing.
    ///   * `undo` / `redo` — those replay already-committed edits.
    ///   * `stop_editing` — discards the pending nibble.
    ///
    /// ```rust,no_run
    /// # use dear_imgui_custom_mod::hex_viewer::HexViewer;
    /// let mut hv = HexViewer::new("mem");
    /// hv.set_byte_edit_callback(Some(Box::new(|va, _old, new| {
    ///     // host: queue WriteMem(va, &[new]) on its driver bus
    ///     let _ = (va, new);
    /// })));
    /// ```
    pub fn set_byte_edit_callback(&mut self, f: Option<ByteEditCallback>) {
        self.byte_edit_callback = f;
    }

    /// Install (or clear with `None`) the host-supplied Address-column
    /// formatter. See [`AddressFormatter`] for the contract and
    /// performance notes.
    ///
    /// ```rust,no_run
    /// # use dear_imgui_custom_mod::hex_viewer::HexViewer;
    /// # use std::sync::Arc;
    /// let mut hv = HexViewer::new("dump");
    /// hv.set_address_formatter(Some(Arc::new(|addr: u64| {
    ///     // Host-side module table lookup goes here.
    ///     if addr >= 0x7FF6_0000_0000 && addr < 0x7FF6_0010_0000 {
    ///         Some(format!("kernel32+0x{:X}", addr - 0x7FF6_0000_0000))
    ///     } else {
    ///         None  // fall back to default absolute hex
    ///     }
    /// })));
    /// ```
    pub fn set_address_formatter(&mut self, f: Option<AddressFormatter>) {
        self.address_formatter = f;
    }

    /// Override the user-visible language on construction. Default is
    /// English; pass [`crate::i18n::Locale::Ru`] to switch to Russian.
    /// The host is responsible for baking `GlyphRanges::Cyrillic`
    /// into the active font atlas — without that, non-ASCII
    /// characters render as `?` placeholders.
    ///
    /// The locale is stored on [`HexViewerConfig::locale`], so it
    /// round-trips through `ron::to_string` / `ron::from_str` along
    /// with every other display setting.
    pub fn with_locale(mut self, locale: crate::i18n::Locale) -> Self {
        self.config.locale = locale;
        self
    }

    /// Mid-flight language switch. Same caveat as [`Self::with_locale`]
    /// regarding font atlas glyph ranges.
    pub fn set_locale(&mut self, locale: crate::i18n::Locale) {
        self.config.locale = locale;
    }

    /// Currently-active locale (mirror of `self.config.locale`).
    pub fn locale(&self) -> crate::i18n::Locale {
        self.config.locale
    }

    /// Static catalogue lookup for the current locale. Convenience
    /// for the per-frame popup / draw paths.
    #[inline]
    pub(super) fn strings(&self) -> &'static crate::i18n::hex_viewer::Strings {
        crate::i18n::hex_viewer::strings(self.config.locale)
    }

    // ── Data management ─────────────────────────────────────────────

    /// Copy `data` into a fresh internal buffer. Existing API —
    /// preserved for hosts that hand in a borrowed slice.
    pub fn set_data(&mut self, data: &[u8]) {
        self.data = Arc::new(data.to_vec());
        self.effective_data_len = self.data.len();
        self.clamp_cursor();
        self.selection = Selection::default();
        self.stop_editing();
    }

    /// Take ownership of `data`. Wraps the `Vec` in a fresh `Arc`.
    /// Preferred over [`Self::set_data`] when the host already owns
    /// the bytes (avoids the `.to_vec()` copy).
    pub fn set_data_vec(&mut self, data: Vec<u8>) {
        self.data = Arc::new(data);
        self.effective_data_len = self.data.len();
        self.clamp_cursor();
        self.selection = Selection::default();
        // Mirror set_data: cancel any in-progress edit so a stale layout
        // switch can't hold over a fresh buffer.
        self.stop_editing();
    }

    /// BUG-128 (2026-05-15) — zero-copy data swap. Hosts that already
    /// own an `Arc<Vec<u8>>` (typical for debugger memory panes that
    /// also feed the same bytes into a disassembler worker via
    /// `Arc::clone`) call this to install the buffer without ANY
    /// memcpy. The previous internal `Arc` is dropped; if HexViewer
    /// was the sole owner the inner `Vec` is freed.
    pub fn set_data_arc(&mut self, data: Arc<Vec<u8>>) {
        self.data = data;
        self.effective_data_len = self.data.len();
        self.clamp_cursor();
        self.selection = Selection::default();
        self.stop_editing();
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Mutable byte buffer. Triggers `Arc::make_mut` — if the host is
    /// also holding an `Arc` clone (e.g. a parked disasm submission),
    /// THIS path clones the inner `Vec` so the host's clone is left
    /// untouched (copy-on-write semantics). When HexViewer is the sole
    /// owner (the typical case while editing) this is zero-cost.
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        Arc::make_mut(&mut self.data)
    }
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    /// BUG-128 — return a cheap `Arc::clone` of the current data
    /// buffer. Hosts can forward this directly to a worker (e.g.
    /// `DisasmCmd::Decode { bytes: hex.data_arc(), .. }`) without
    /// copying the bytes. Subsequent edits in HexViewer go through
    /// `Arc::make_mut` so the worker's clone stays a stable snapshot.
    pub fn data_arc(&self) -> Arc<Vec<u8>> {
        Arc::clone(&self.data)
    }

    pub fn set_reference(&mut self, reference: &[u8]) {
        self.reference = reference.to_vec();
    }
    pub fn clear_reference(&mut self) {
        self.reference.clear();
    }

    pub fn set_regions(&mut self, regions: Vec<ColorRegion>) {
        self.regions = regions;
    }
    pub fn add_region(&mut self, region: ColorRegion) {
        self.regions.push(region);
    }
    pub fn clear_regions(&mut self) {
        self.regions.clear();
    }

    // ── Cursor & Selection ───────────────────────────────────────────

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Move the cursor to `offset` as an explicit navigation jump.
    ///
    /// Always records the previous position in the back/forward history
    /// (so `goto` followed by Alt+Left returns to the prior spot) and
    /// clears any active selection — that matches user expectations for
    /// a "go here" command. Also commits any half-typed hex nibble on
    /// the previous byte and exits edit mode — `goto` is a deliberate
    /// "leave this byte" gesture, just like a single click on another
    /// byte.
    pub fn set_cursor(&mut self, offset: usize) {
        self.commit_pending_edit();
        let old = self.cursor;
        // Clamp against the larger of internal buffer and last-frame
        // projection — keeps the cursor valid even mid-frame when the
        // legacy `render()` path has moved `self.data` into its
        // provider wrapper.
        let len = self.data.len().max(self.effective_data_len);
        let new = offset.min(len.saturating_sub(1));
        if new != old {
            self.nav.push(self.config.base_address + old as u64);
        }
        self.cursor = new;
        self.selection = Selection::default();
        self.stop_editing();
        let bpr = self.config.bytes_per_row.value();
        self.scroll_to_row = Some(self.cursor / bpr);
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn selected_bytes(&self) -> &[u8] {
        if self.selection.is_empty() {
            return &[];
        }
        let (lo, hi) = self.selection.ordered();
        let lo = lo.min(self.data.len());
        let hi = hi.min(self.data.len());
        &self.data[lo..hi]
    }

    pub fn goto(&mut self, offset: usize) {
        self.set_cursor(offset);
    }

    // ─── BUG-128 (2026-05-15): VA-native API parity with DisasmView ───
    //
    // The host previously had to repeat `(va - base_address) as usize`
    // at every goto/contains/viewport callsite. These wrappers move
    // that boilerplate into the library so:
    //   * Hosts driving a sliding-window view get the same VA-first
    //     mental model they already use with DisasmView.
    //   * The base-address bookkeeping stays as a single source of truth
    //     inside `HexViewerConfig`.
    //   * Future changes to the base-address contract (paged buffers,
    //     multi-segment maps) need to touch only this file.
    //
    // The legacy offset-based methods (`goto`, `cursor`, `data_len`)
    // remain — file-editor / binary-dump hosts that don't care about VAs
    // keep their pre-existing API.

    /// Absolute virtual address of the current cursor byte.
    /// Computed as `config.base_address + cursor`. When the buffer is
    /// empty returns `base_address`.
    pub fn cursor_address(&self) -> u64 {
        self.config.base_address.saturating_add(self.cursor as u64)
    }

    /// `true` when `va` lies within `[base_address, base_address + buffer_len)`.
    /// Empty-buffer or fully-zero range → always `false`.
    ///
    /// Phase 2: `buffer_len` is the larger of `self.data.len()` and the
    /// last-frame provider projection (`effective_data_len`). That
    /// keeps hosts using [`Self::render_with_provider`] honest — a
    /// debugger memory pane streams bytes through the provider so its
    /// `self.data` may be empty, but `contains_va` still has to answer
    /// "yes, this VA is inside the currently-visible window".
    pub fn contains_va(&self, va: u64) -> bool {
        let effective = self.data.len().max(self.effective_data_len);
        if effective == 0 {
            return false;
        }
        let base = self.config.base_address;
        let end = base.saturating_add(effective as u64);
        va >= base && va < end
    }

    /// VA-native goto. If `va` is inside the current buffer the cursor
    /// jumps to the corresponding byte. If outside AND a
    /// `va_goto_callback` is installed, fires the callback so the host
    /// can re-anchor the buffer. Without a callback and a VA outside
    /// the buffer, clamps to the last byte (legacy behaviour).
    pub fn goto_address(&mut self, va: u64) {
        let base = self.config.base_address;
        if self.contains_va(va) {
            let offset = (va - base) as usize;
            self.set_cursor(offset);
        } else if let Some(cb) = self.va_goto_callback.as_mut() {
            cb(va);
        } else {
            let offset = va.saturating_sub(base) as usize;
            self.set_cursor(offset);
        }
    }

    /// VA at the start of the FIRST currently-visible row in the
    /// hex pane. Driven by `viewport_first_row` cached during the
    /// last `render()` from `scroll_y / line_height` — same source
    /// of truth as the virtualisation loop, so this never disagrees
    /// with what the user actually sees. Returns `None` while the
    /// buffer is empty or before the first render.
    pub fn viewport_first_va(&self) -> Option<u64> {
        let effective = self.data.len().max(self.effective_data_len);
        if effective == 0 || self.viewport_first_row == usize::MAX {
            return None;
        }
        let bpr = self.config.bytes_per_row.value() as u64;
        Some(self.config.base_address + self.viewport_first_row as u64 * bpr)
    }

    /// VA at the start of the LAST currently-visible row (inclusive
    /// of the partial bottom row). Same caching contract as
    /// [`Self::viewport_first_va`]. Together they let hosts driving
    /// a sliding-window view answer "is the user's current scroll
    /// position still inside the buffer?" without dipping into
    /// ImGui state.
    pub fn viewport_last_va(&self) -> Option<u64> {
        let effective = self.data.len().max(self.effective_data_len);
        if effective == 0 || self.viewport_last_row == usize::MAX {
            return None;
        }
        let bpr = self.config.bytes_per_row.value() as u64;
        // `viewport_last_row` is the exclusive end (loop ran `..last_row`),
        // so clamp to `len-1` row index for the address of the last
        // VISIBLE row.
        let total_rows = effective.div_ceil(self.config.bytes_per_row.value());
        let last_row_idx = self
            .viewport_last_row
            .saturating_sub(1)
            .min(total_rows.saturating_sub(1));
        Some(self.config.base_address + last_row_idx as u64 * bpr)
    }

    /// **Scroll-only** sibling of [`Self::goto_address`]: pin the row
    /// containing `va` to the top of the viewport on the next render
    /// without touching `cursor` or `selection`. Use this when a sibling
    /// pane (e.g. a disassembler) wants the hex view to *follow* its
    /// cursor without grabbing input focus — clicking an instruction in
    /// disasm should reframe the bytes around it, not steal the byte
    /// cursor away from whatever the user was inspecting.
    ///
    /// If `va` is outside the loaded buffer AND a `va_goto_callback` is
    /// installed, fires the callback so the host can re-anchor the
    /// buffer (same fallback contract as `goto_address`); otherwise this
    /// is a no-op.
    pub fn set_viewport_first_va(&mut self, va: u64) {
        let base = self.config.base_address;
        if self.contains_va(va) {
            let offset = (va - base) as usize;
            let bpr = self.config.bytes_per_row.value();
            // Round DOWN to the row that holds `va` — pinning that row
            // (not the byte's row mid-line) is what "place at top" means.
            self.scroll_to_row = Some(offset / bpr);
        } else if let Some(cb) = self.va_goto_callback.as_mut() {
            cb(va);
        }
    }

    pub fn nav_back(&mut self) {
        let current = self.config.base_address + self.cursor as u64;
        if let Some(addr) = self.nav.go_back(current) {
            let offset = addr.saturating_sub(self.config.base_address) as usize;
            // Clamp against the larger of `self.data.len()` and the
            // last-frame projection. The legacy [`Self::render`] path
            // temporarily moves `self.data` out into its provider
            // wrapper, so during that frame `self.data.len()` is 0;
            // `effective_data_len` keeps the cursor inside the visible
            // window.
            let len = self.data.len().max(self.effective_data_len);
            self.cursor = offset.min(len.saturating_sub(1));
            self.scroll_to_cursor();
        }
    }

    pub fn nav_forward(&mut self) {
        let current = self.config.base_address + self.cursor as u64;
        if let Some(addr) = self.nav.go_forward(current) {
            let offset = addr.saturating_sub(self.config.base_address) as usize;
            let len = self.data.len().max(self.effective_data_len);
            self.cursor = offset.min(len.saturating_sub(1));
            self.scroll_to_cursor();
        }
    }

    /// Open the **goto-address** popup on the next frame, mirroring
    /// the `Ctrl+G` shortcut handled internally.
    ///
    /// Useful when the host app wires its own global hotkey (e.g.
    /// from a menu / toolbar / outer keybinding system) and wants
    /// to trigger goto without depending on the hex viewer being
    /// the focused widget. Clears the input buffer first so stale
    /// content from a previous open doesn't appear pre-filled.
    ///
    /// ```rust,ignore
    /// // Inside your app's hotkey handler:
    /// if global_hotkey == "Ctrl+G" {
    ///     viewer.request_goto();
    /// }
    /// // Then on the next frame, render() opens the popup.
    /// ```
    pub fn request_goto(&mut self) {
        self.show_goto = true;
        self.goto_buf.clear();
        // No mouse position to anchor at (caller is host-side, not
        // ImGui-handled). Leave `popup_open_pos` at whatever the
        // last in-viewer event captured — guarantees the popup
        // appears in a sane place relative to the viewer's last
        // interaction. If never set (fresh viewer, never clicked),
        // it stays at `(0, 0)` — the popup-render path will fall
        // back to "centre on screen" in that case.
    }

    /// Open the **search** popup on the next frame, mirroring the
    /// `Ctrl+F` shortcut handled internally. Same use case as
    /// [`Self::request_goto`] — bridge from a host-side global
    /// hotkey when the viewer isn't the focused widget.
    pub fn request_search(&mut self) {
        self.show_search = true;
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }
    pub fn config(&self) -> &HexViewerConfig {
        &self.config
    }
    pub fn config_mut(&mut self) -> &mut HexViewerConfig {
        &mut self.config
    }
    pub fn undo_stack(&self) -> &UndoStack {
        &self.undo
    }
    pub fn nav_history(&self) -> &NavHistory {
        &self.nav
    }

    /// Inspector subview height in pixels, or `None` when the user
    /// hasn't manually resized it (the panel uses its natural auto
    /// height calculated from the line metrics).
    pub fn inspector_height_px(&self) -> Option<f32> {
        if self.inspector_h > 0.0 {
            Some(self.inspector_h)
        } else {
            None
        }
    }

    /// Programmatically set the inspector subview height (in pixels).
    /// The next `render` call will clamp the value against the runtime
    /// min/max envelope (at least `2 × line_height`, leaves at least
    /// 5 rows of hex visible). Pass `0.0` to revert to auto-sizing.
    pub fn set_inspector_height_px(&mut self, h: f32) {
        self.inspector_h = h.max(0.0);
    }

    /// Reset the user-controlled inspector height back to auto.
    /// Equivalent to `set_inspector_height_px(0.0)`.
    pub fn reset_inspector_height(&mut self) {
        self.inspector_h = 0.0;
    }

    // ── Render entry points (Phase 2: provider-driven) ──────────────

    /// Legacy render entry — keeps the zero-argument signature every
    /// existing caller depends on. Wraps `self.data` in a read-only
    /// [`ArcVecDataProvider`] (single `Arc::clone`, refcount bump)
    /// and dispatches to the generic [`Self::render_impl`].
    ///
    /// Cost model:
    ///   * **Reads** — every visible byte goes through `provider.read()`
    ///     which copies one row at a time into a stack scratch buffer.
    ///     Net: one extra per-row memcpy of `bpr` bytes (≤ 64 B / row)
    ///     vs the pre-Phase-2 direct-slice path.
    ///   * **Writes** — the wrapper returns `false` from `write()`, so
    ///     in-frame edits (`Ctrl+Z`, hex/ASCII type, double-click +
    ///     type) fall through to `Arc::make_mut(&mut self.data)`.
    ///     Because the wrapper holds an additional `Arc::clone`, the
    ///     refcount is ≥ 2 throughout the frame — `Arc::make_mut`
    ///     therefore COW-clones the inner `Vec` once on the first
    ///     in-frame edit. Subsequent edits in the same frame are
    ///     zero-copy (the wrapper's clone now points at the OLD
    ///     buffer, and `self.data` points at the new one which has
    ///     refcount 1).
    ///
    /// The first-edit COW clone is a measurable regression for hosts
    /// editing multi-megabyte buffers; those hosts should migrate to
    /// [`Self::render_with_provider`] with a custom writable
    /// provider (e.g. a `VecMutProvider` over their own `&mut Vec<u8>`).
    ///
    /// Use this when the host hands the viewer a static buffer via
    /// `set_data*`. For sliding-window / streaming sources implement
    /// [`HexDataProvider`] and call [`Self::render_with_provider`].
    pub fn render(&mut self, ui: &dear_imgui_rs::Ui) {
        let mut wrapper = provider::ArcVecDataProvider::from_arc(Arc::clone(&self.data));
        self.render_impl(ui, &mut wrapper);
    }

    /// Provider-driven render entry. The viewer asks `provider` for
    /// every visible byte (one batched `read()` per row + one for the
    /// data inspector), routes hex/ASCII edits through `provider.write()`
    /// when accepted, and treats `provider.len()` as the authoritative
    /// buffer extent (incl. address-gutter digit count and scrollbar
    /// thumb size). Hosts driving a debugger memory pane / raw-disk
    /// view / network packet stream pass their own implementation —
    /// `set_data*` calls become irrelevant (and `self.data` stays
    /// empty), the provider IS the source of truth.
    ///
    /// Generic over `P: HexDataProvider`. For trait objects pass the
    /// reference directly (`&mut *boxed_provider` or
    /// `&mut **arc_mutex_guard`) — the impl dispatches through
    /// `&mut dyn HexDataProvider` internally so the dyn-safety cost
    /// is paid once at the call site.
    ///
    /// ```rust,ignore
    /// // Custom streaming provider over a debug-target memory pane:
    /// let mut my_provider = MemoryPaneProvider::new(window_base, length);
    /// hex.render_with_provider(ui, &mut my_provider);
    /// ```
    pub fn render_with_provider<P: provider::HexDataProvider>(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut P,
    ) {
        self.render_impl(ui, provider);
    }

    /// Trait-object overload of [`Self::render_with_provider`]. Lets
    /// callers pass a `&mut dyn HexDataProvider` directly (typical
    /// when the provider is selected at runtime from several
    /// candidate impls held in a `Box<dyn HexDataProvider>`).
    pub fn render_with_dyn_provider(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn provider::HexDataProvider,
    ) {
        self.render_impl(ui, provider);
    }

    /// Returns `true` exactly once each time the auto-refresh counter
    /// (driven by [`HexViewerConfig::auto_refresh_frames`]) wraps. Caller
    /// should respond by re-fetching live data and pushing it via
    /// `set_data*`. Returns `false` when the feature is disabled
    /// (`auto_refresh_frames == 0`).
    ///
    /// **Note:** the internal counter is only advanced inside
    /// [`HexViewer::render`]. If the widget is not rendered (hidden tab,
    /// minimised window, headless test), this method will never return
    /// `true` — by design, since there is nothing to refresh anyway.
    pub fn take_refresh_pending(&mut self) -> bool {
        let interval = self.config.auto_refresh_frames;
        if interval == 0 {
            return false;
        }
        if self.frame_count >= interval {
            self.frame_count = 0;
            true
        } else {
            false
        }
    }
}

impl Drop for HexViewer {
    fn drop(&mut self) {
        // Ensure keyboard layout is restored if we're dropped while editing.
        if self.layout_switched {
            clipboard::restore_keyboard_layout();
        }
    }
}
