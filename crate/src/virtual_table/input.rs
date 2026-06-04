//! Keyboard navigation, scroll handling, and click selection.
//!
//! Split out of `mod.rs` to keep files under 500 lines; extends
//! [`VirtualTable`](super::VirtualTable) via an `impl` block.

use super::*;

impl<T: VirtualTableRow> VirtualTable<T> {
    // ─── Internal: keyboard navigation ─────────────────────────────

    pub(super) fn handle_keyboard_nav(&mut self, ui: &Ui, row_count: usize) {
        if !ui.is_window_focused()
            || self.edit_state.active
            || self.config.selection_mode == SelectionMode::None
            || row_count == 0
        {
            return;
        }
        let current = self.selection_anchor.unwrap_or(0);
        let new_idx = if ui.is_key_pressed(Key::UpArrow) {
            Some(current.saturating_sub(1))
        } else if ui.is_key_pressed(Key::DownArrow) {
            Some((current + 1).min(row_count - 1))
        } else if ui.is_key_pressed(Key::Home) {
            Some(0)
        } else if ui.is_key_pressed(Key::End) {
            Some(row_count - 1)
        } else if ui.is_key_pressed(Key::PageUp) {
            Some(current.saturating_sub(20))
        } else if ui.is_key_pressed(Key::PageDown) {
            Some((current + 20).min(row_count - 1))
        } else {
            None
        };

        if let Some(idx) = new_idx
            && (idx != current || !self.selected_rows.contains(&idx))
        {
            self.selected_rows.clear();
            self.selected_rows.insert(idx);
            self.selection_anchor = Some(idx);
            self.pending_scroll_to = Some(idx);
        }
    }

    // ─── Internal: scroll ──────────────────────────────────────────

    pub(super) fn handle_scroll(&mut self, ui: &Ui, row_count: usize) {
        if let Some(target) = self.pending_scroll_to.take()
            && row_count > 0
        {
            let frac = target as f32 / (row_count - 1).max(1) as f32;
            ui.set_scroll_y(frac * ui.scroll_max_y());
        }
        if self.config.auto_scroll {
            let wheel = ui.io().mouse_wheel();
            if wheel > 0.0 && ui.is_window_hovered() {
                self.config.auto_scroll = false;
            }
            if self.config.auto_scroll && row_count > 0 {
                ui.set_scroll_here_y(1.0);
            }
        }
    }

    // ─── Internal: selection ────────────────────────────────────────

    /// Handle a click on row `idx`. `row_count` is the number of rows in the
    /// data source **being rendered this frame** — the ring buffer for
    /// `render()`, or the external length for `render_slice`/`render_lookup`.
    /// Used to clamp Shift+Click range selection; using the ring length here
    /// would collapse range selection to row 0 in external-render modes.
    pub(super) fn handle_selection(&mut self, ui: &Ui, idx: usize, row_count: usize) {
        match self.config.selection_mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                self.selected_rows.clear();
                self.selected_rows.insert(idx);
                self.selection_anchor = Some(idx);
            }
            SelectionMode::Multi => {
                let io = ui.io();
                let ctrl = io.key_ctrl();
                let shift = io.key_shift();

                if ctrl {
                    // Toggle: O(1) insert/remove via HashSet
                    if !self.selected_rows.remove(&idx) {
                        self.selected_rows.insert(idx);
                    }
                    self.selection_anchor = Some(idx);
                } else if shift {
                    let anchor = self.selection_anchor.unwrap_or(idx);
                    let max_idx = row_count.saturating_sub(1);
                    let (start, end) = if idx < anchor {
                        (idx, anchor.min(max_idx))
                    } else {
                        (anchor, idx.min(max_idx))
                    };
                    self.selected_rows.clear();
                    for r in start..=end {
                        self.selected_rows.insert(r);
                    }
                    // Keep anchor unchanged for consecutive shift-clicks
                } else {
                    self.selected_rows.clear();
                    self.selected_rows.insert(idx);
                    self.selection_anchor = Some(idx);
                }
            }
        }
    }
}
