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
    ByteCategory, ByteGrouping, BytesPerRow, ColorRegion, CopyFormat, Endianness, HexDataProvider,
    HexSearchMode, HexViewerConfig, NavHistory, UndoEntry, UndoStack, VecDataProvider,
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
    pub(super) scroll_to_row: Option<usize>,
    pub(super) char_advance: f32,
    pub(super) line_height: f32,
    pub(super) focused: bool,
    pub(super) frame_count: u32,

    // ── VK-fallback front-edge detection ─────────────────────
    // `GetAsyncKeyState` reports "key currently down", not "just
    // transitioned" — without prev-frame snapshots, every Ctrl+Z while
    // the key is held would fire a fresh undo. We track the previous
    // state per shortcut key here and compute edges in `handle_keyboard`.
    pub(super) vk_prev_a: bool,
    pub(super) vk_prev_c: bool,
    pub(super) vk_prev_f: bool,
    pub(super) vk_prev_g: bool,
    pub(super) vk_prev_y: bool,
    pub(super) vk_prev_z: bool,
}

impl HexViewer {
    pub fn new(id: impl Into<String>) -> Self {
        let id: String = id.into();
        let goto_popup_id = format!("##goto_{id}");
        let search_popup_id = format!("##search_{id}");
        Self {
            id,
            data: Vec::new(),
            reference: Vec::new(),
            regions: Vec::new(),
            config: HexViewerConfig::default(),
            goto_popup_id,
            search_popup_id,
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
            scroll_to_row: None,
            char_advance: 0.0,
            line_height: 0.0,
            focused: false,
            frame_count: 0,
            vk_prev_a: false,
            vk_prev_c: false,
            vk_prev_f: false,
            vk_prev_g: false,
            vk_prev_y: false,
            vk_prev_z: false,
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
