//! Public API surface: data management, cursor / selection, VA-native
//! navigation, inspector-height accessors, and the provider-driven
//! render entry points.
//!
//! Split out of `mod.rs` to keep that file (struct definition + `new` +
//! callback setters + locale) under the 500-line ceiling. Everything
//! here is an inherent method on `HexViewer`; the struct fields it
//! touches are `pub(super)` within the `hex_viewer` module.

use std::sync::Arc;

use super::HexViewer;
use super::config::HexViewerConfig;
use super::nav_history::NavHistory;
use super::provider::{self, ColorRegion};
use super::search::Selection;
use super::undo::UndoStack;

impl HexViewer {
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
