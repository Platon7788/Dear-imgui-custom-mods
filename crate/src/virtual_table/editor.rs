//! Inline cell-editor activation and per-frame editor rendering.
//!
//! Split out of `mod.rs` to keep files under 500 lines; extends
//! [`VirtualTable`](super::VirtualTable) via an `impl` block.

use super::*;

impl<T: VirtualTableRow> VirtualTable<T> {
    // ─── Internal: inline editor ────────────────────────────────────

    pub(super) fn try_activate_edit(&mut self, row_idx: usize, col_idx: usize) {
        if col_idx >= self.columns.len() {
            return;
        }
        if matches!(
            editor_kind(&self.columns[col_idx].editor),
            EditorKind::None
                | EditorKind::Checkbox
                | EditorKind::ComboBox
                | EditorKind::Button
                | EditorKind::ProgressBar
                | EditorKind::ColorEdit
        ) {
            return;
        }

        if let Some(row) = self.data.get(row_idx) {
            let value = row.cell_value(col_idx);
            self.edit_state.activate(row_idx, col_idx, &value);
        }
    }

    pub(super) fn render_editor_inline(&mut self, ui: &Ui, idx: usize, col_idx: usize) {
        ui.set_next_item_width(-1.0);

        // Clone the editor config to avoid borrow conflict with self.edit_state/self.data.
        let editor_snapshot = self.columns[col_idx].editor.clone();

        let first_frame = self.edit_state.just_activated();
        if first_frame {
            self.edit_state.set_activated(false);
        }

        let outcome = match &editor_snapshot {
            CellEditor::Custom => {
                let mut committed = false;
                if let Some(row) = self.data.get_mut(idx)
                    && row.render_editor(ui, col_idx)
                {
                    committed = true;
                }
                if ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                    edit_common::EditOutcome::Cancel
                } else if committed {
                    edit_common::EditOutcome::Commit
                } else {
                    edit_common::EditOutcome::Continue
                }
            }
            other => edit_common::render_editor_widget(
                ui,
                other,
                &mut self.edit_state.buf,
                first_frame,
                self.config.commit_on_focus_loss,
            ),
        };

        match outcome {
            edit_common::EditOutcome::Commit => {
                let value = self.edit_state.take_cell_value(&editor_snapshot);
                if let Some(row) = self.data.get_mut(idx) {
                    row.set_cell_value(col_idx, &value);
                }
                self.edit_state.deactivate();
            }
            edit_common::EditOutcome::Cancel | edit_common::EditOutcome::Custom => {
                self.edit_state.deactivate();
            }
            edit_common::EditOutcome::Continue => {}
        }
    }
}
