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
        let mut commit = false;
        let mut cancel = false;

        ui.set_next_item_width(-1.0);

        // Clone the editor config to avoid borrow conflict with self.edit_state/self.arena.
        let editor_snapshot = self.columns[col_idx].editor.clone();
        let first_frame = self.edit_state.just_activated;
        if first_frame {
            self.edit_state.just_activated = false;
        }

        match &editor_snapshot {
            CellEditor::TextInput => {
                if first_frame {
                    unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
                }
                let entered = ui
                    .input_text("##edit", &mut self.edit_state.text_buf)
                    .enter_returns_true(true)
                    .build();
                if entered {
                    commit = true;
                }
                if !first_frame && !entered {
                    if ui.is_item_deactivated_after_edit() {
                        if self.config.table.commit_on_focus_loss {
                            commit = true;
                        } else {
                            cancel = true;
                        }
                    } else if ui.is_item_deactivated() {
                        cancel = true;
                    }
                }
            }
            CellEditor::SliderInt { min, max } => {
                ui.slider_config("##edit", *min, *max)
                    .build(&mut self.edit_state.int_val);
                if !first_frame && ui.is_item_deactivated_after_edit() {
                    commit = true;
                }
            }
            CellEditor::SliderFloat { min, max } => {
                ui.slider_config("##edit", *min, *max)
                    .build(&mut self.edit_state.float_val);
                if !first_frame && ui.is_item_deactivated_after_edit() {
                    commit = true;
                }
            }
            CellEditor::SpinInt { step, step_fast } => {
                if first_frame {
                    unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
                }
                unsafe {
                    dear_imgui_rs::sys::igInputInt(
                        c"##edit".as_ptr(),
                        &mut self.edit_state.int_val,
                        *step,
                        *step_fast,
                        0,
                    );
                }
                if !first_frame {
                    if ui.is_item_deactivated_after_edit() {
                        if self.config.table.commit_on_focus_loss {
                            commit = true;
                        } else {
                            cancel = true;
                        }
                    } else if ui.is_item_deactivated() {
                        cancel = true;
                    }
                }
            }
            CellEditor::SpinFloat { step, step_fast } => {
                if first_frame {
                    unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
                }
                unsafe {
                    dear_imgui_rs::sys::igInputFloat(
                        c"##edit".as_ptr(),
                        &mut self.edit_state.float_val,
                        *step,
                        *step_fast,
                        c"%.2f".as_ptr(),
                        0,
                    );
                }
                if !first_frame {
                    if ui.is_item_deactivated_after_edit() {
                        if self.config.table.commit_on_focus_loss {
                            commit = true;
                        } else {
                            cancel = true;
                        }
                    } else if ui.is_item_deactivated() {
                        cancel = true;
                    }
                }
            }
            CellEditor::Custom => {
                if let Some(data) = self.arena.get_data_mut(node_id)
                    && data.render_editor(ui, col_idx, node_id)
                {
                    commit = true;
                }
            }
            _ => {
                self.edit_state.deactivate();
                return;
            }
        }

        if ui.is_key_pressed(Key::Escape) {
            cancel = true;
        }

        if cancel {
            self.edit_state.deactivate();
        } else if commit {
            let value = self.edit_state.take_cell_value(&editor_snapshot);
            if let Some(data) = self.arena.get_data_mut(node_id) {
                data.set_cell_value(col_idx, &value);
            }
            self.edit_state.deactivate();
        }
    }
}
