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
//! - [`config`] — `HexViewerConfig`, `BytesPerRow`, palettes,
//!   `UndoStack`, `NavHistory`.
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
mod popup;
mod search;

#[cfg(test)]
mod tests;

pub use config::{
    AddressWidth, ByteCategory, ByteGrouping, BytesPerRow, ColorRegion, CopyFormat, Endianness,
    HexDataProvider, HexSearchMode, HexViewerConfig, NavHistory, UndoEntry, UndoStack,
    VecDataProvider,
};
pub use input::EditColumn;
pub use search::{PatternByte, Selection};

use crate::utils::clipboard;

// ── HexViewer ────────────────────────────────────────────────────────────────

/// Standalone hex dump widget.
///
/// Fields are `pub(super)` so the input/draw/search sub-modules can
/// touch state without going through getters — keeps the hot render
/// path allocation-free and the input handler ergonomic.
pub struct HexViewer {
    pub(super) id: String,
    pub(super) data: Vec<u8>,
    pub(super) reference: Vec<u8>,
    pub(super) regions: Vec<ColorRegion>,
    pub(super) config: HexViewerConfig,

    // ── Cached ImGui IDs (built once at construction) ─────────
    pub(super) goto_popup_id: String,
    pub(super) search_popup_id: String,

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
}

impl HexViewer {
    pub fn new(id: impl Into<String>) -> Self {
        let id: String = id.into();
        let goto_popup_id = format!("##goto_{id}");
        let search_popup_id = format!("##search_{id}");
        let context_popup_id = format!("##ctx_{id}");
        let settings_popup_id = format!("##settings_{id}");
        Self {
            id,
            data: Vec::new(),
            reference: Vec::new(),
            regions: Vec::new(),
            config: HexViewerConfig::default(),
            goto_popup_id,
            search_popup_id,
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
        }
    }

    // ── Data management ─────────────────────────────────────────────

    pub fn set_data(&mut self, data: &[u8]) {
        self.data = data.to_vec();
        self.clamp_cursor();
        self.selection = Selection::default();
        self.stop_editing();
    }

    pub fn set_data_vec(&mut self, data: Vec<u8>) {
        self.data = data;
        self.clamp_cursor();
        self.selection = Selection::default();
        // Mirror set_data: cancel any in-progress edit so a stale layout
        // switch can't hold over a fresh buffer.
        self.stop_editing();
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }
    pub fn data_len(&self) -> usize {
        self.data.len()
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
        let new = offset.min(self.data.len().saturating_sub(1));
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

    pub fn nav_back(&mut self) {
        let current = self.config.base_address + self.cursor as u64;
        if let Some(addr) = self.nav.go_back(current) {
            let offset = addr.saturating_sub(self.config.base_address) as usize;
            self.cursor = offset.min(self.data.len().saturating_sub(1));
            self.scroll_to_cursor();
        }
    }

    pub fn nav_forward(&mut self) {
        let current = self.config.base_address + self.cursor as u64;
        if let Some(addr) = self.nav.go_forward(current) {
            let offset = addr.saturating_sub(self.config.base_address) as usize;
            self.cursor = offset.min(self.data.len().saturating_sub(1));
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
