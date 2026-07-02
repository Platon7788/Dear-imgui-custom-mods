//! Inline cell-editor activation and rendering.
//!
//! Part of [`VirtualTree`](super::VirtualTree); split out of `mod.rs`
//! to keep files under 500 lines. See `mod.rs` for the struct.

use super::*;

impl<T: VirtualTreeNode> VirtualTree<T> {
    // ─── Internal: inline editor ────────────────────────────────────

    pub(super) fn try_activate_edit(&mut self, flat_idx: usize, col_idx: usize) {
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

        let node_id = match self.flat_view.rows.get(flat_idx) {
            Some(r) => r.node_id,
            None => return,
        };

        if let Some(data) = self.arena.get_data(node_id) {
            let value = data.cell_value(col_idx);
            self.edit_state.activate(node_id, col_idx, &value);
        }
    }

    pub(super) fn render_editor_inline(&mut self, ui: &Ui, col_idx: usize, node_id: NodeId) {
        ui.set_next_item_width(-1.0);

        // Clone the editor config to avoid borrow conflict with self.edit_state/self.arena.
        let editor_snapshot = self.columns[col_idx].editor.clone();

        let first_frame = self.edit_state.just_activated();
        if first_frame {
            self.edit_state.set_activated(false);
        }

        let outcome = match &editor_snapshot {
            CellEditor::Custom => {
                let mut committed = false;
                if let Some(data) = self.arena.get_data_mut(node_id)
                    && data.render_editor(ui, col_idx, node_id)
                {
                    committed = true;
                }
                if ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                    crate::virtual_table::edit_common::EditOutcome::Cancel
                } else if committed {
                    crate::virtual_table::edit_common::EditOutcome::Commit
                } else {
                    crate::virtual_table::edit_common::EditOutcome::Continue
                }
            }
            other => crate::virtual_table::edit_common::render_editor_widget(
                ui,
                other,
                &mut self.edit_state.buf,
                first_frame,
                self.config.table.commit_on_focus_loss,
            ),
        };

        match outcome {
            crate::virtual_table::edit_common::EditOutcome::Commit => {
                let value = self.edit_state.take_cell_value(&editor_snapshot);
                if let Some(data) = self.arena.get_data_mut(node_id) {
                    data.set_cell_value(col_idx, &value);
                }
                self.edit_state.deactivate();
            }
            crate::virtual_table::edit_common::EditOutcome::Cancel => {
                self.edit_state.deactivate();
            }
            crate::virtual_table::edit_common::EditOutcome::Continue => {}
        }
    }
}
