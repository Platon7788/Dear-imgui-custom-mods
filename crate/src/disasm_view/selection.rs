//! Selection state + cursor accessors for `DisasmView`.
//!
//! Split out of `mod.rs` (audit session 043) to keep every file under
//! the 500-line ceiling. The `DisasmView` struct + its fields stay in
//! `mod.rs`; this file only carries an `impl DisasmView { ... }` block.

use super::*;

impl DisasmView {
    /// Currently focused (cursor) instruction index.
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        self.cursor_idx
    }

    /// All selected instruction indices.
    #[must_use]
    pub fn selected_indices(&self) -> &BTreeSet<usize> {
        &self.selection
    }

    /// Number of selected instructions.
    #[must_use]
    pub fn selected_count(&self) -> usize {
        self.selection.len()
    }

    /// Whether a specific index is selected.
    #[must_use]
    pub fn is_selected(&self, idx: usize) -> bool {
        self.selection.contains(&idx)
    }

    /// Set the cursor and single-select one instruction.
    pub fn select(&mut self, idx: usize) {
        self.cursor_idx = Some(idx);
        self.selection.clear();
        self.selection.insert(idx);
        self.sel_anchor = Some(idx);
        self.scroll_to = Some(idx);
    }

    /// Clear all selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.sel_anchor = None;
    }

    /// Scroll the viewport so the row at `addr` becomes visible —
    /// WITHOUT mutating cursor / selection / nav-history / origin
    /// breadcrumb. Companion to [`Self::goto_address`] for callers
    /// that want a "soft" navigation (host placing an initial anchor
    /// at entry-point, viewport re-centring on pause, etc.) — the
    /// user keeps whatever they had clicked previously (or no
    /// selection at all on a fresh tab).
    ///
    /// No-op when `addr` doesn't resolve through
    /// [`DisasmDataProvider::index_of_address`].
    pub fn scroll_to_address(&mut self, addr: u64, provider: &dyn DisasmDataProvider) {
        if let Some(idx) = provider.index_of_address(addr) {
            self.scroll_to = Some(idx);
        }
    }

    // ── Selection helpers ─────────────────────

    /// Select a contiguous range [lo..=hi].
    ///
    /// `pub(super)` because sibling modules (`nav::select_function`,
    /// `input`, `popup`) drive range selection — the method moved out
    /// of `mod.rs` during the session-043 split, and a sibling
    /// submodule can no longer reach a bare-private `fn`.
    pub(super) fn select_range(&mut self, a: usize, b: usize) {
        let lo = a.min(b);
        let hi = a.max(b);
        self.selection.clear();
        for i in lo..=hi {
            self.selection.insert(i);
        }
    }

    /// Whether the view is focused.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }
}
