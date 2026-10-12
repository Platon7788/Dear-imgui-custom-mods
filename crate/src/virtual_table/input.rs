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
        if self.config.selection_mode == SelectionMode::Multi
            && ui.io().key_ctrl()
            && ui.is_key_pressed(Key::A)
            && !ui.is_any_item_active()
        {
            self.select_all(row_count);
            self.pause_for_multi_selection();
            return;
        }
        let current = self.focused_row.or(self.selection_anchor).unwrap_or(0);
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
            self.apply_selection(idx, row_count, false, ui.io().key_shift());
            self.pending_scroll_to = Some(idx);
        }
    }

    // ─── Internal: scroll ──────────────────────────────────────────

    pub(super) fn handle_scroll(&mut self, ui: &Ui, row_count: usize) {
        if let Some(target) = self.pending_scroll_to
            && row_count > 0
        {
            let maximum = ui.scroll_max_y();
            // ImGui establishes a new table's scroll extent after its first
            // frame. Keep offscreen requests until that extent is available.
            if maximum > 0.0 || target == 0 || row_count == 1 {
                let frac = scroll_fraction(target.min(row_count - 1), row_count);
                ui.set_scroll_y(frac * maximum);
                self.pending_scroll_to = None;
            }
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
        self.apply_selection(idx, row_count, ui.io().key_ctrl(), ui.io().key_shift());
    }
}
