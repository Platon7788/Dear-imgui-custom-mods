//! Shared inline-editor value buffers + widget render, used by both
//! `VirtualTable` and `VirtualTree` so the editor semantics live in one place.

use super::column::CellEditor;
use super::row::CellValue;
use dear_imgui_rs::{Key, Ui};

/// The per-type value buffers behind an active inline editor. Widget-agnostic:
/// the owning widget adds its own key (`row: usize` / `node: NodeId`).
#[derive(Clone, Debug)]
pub(crate) struct EditBuffers {
    /// True on the very first frame after activation (drives focus grab).
    pub just_activated: bool,
    pub text_buf: String,
    pub bool_val: bool,
    pub int_val: i32,
    pub float_val: f32,
    pub choice_idx: usize,
    pub color_val: [f32; 4],
}

impl Default for EditBuffers {
    fn default() -> Self {
        Self {
            just_activated: false,
            text_buf: String::with_capacity(256),
            bool_val: false,
            int_val: 0,
            float_val: 0.0,
            choice_idx: 0,
            color_val: [1.0; 4],
        }
    }
}

impl EditBuffers {
    /// Copy a cell value into the buffers and arm `just_activated`.
    pub(crate) fn copy_from_value(&mut self, value: &CellValue) {
        self.just_activated = true;
        match value {
            CellValue::Text(s) => {
                self.text_buf.clear();
                self.text_buf.push_str(s);
            }
            CellValue::Bool(b) => self.bool_val = *b,
            CellValue::Int(v) => self.int_val = (*v).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            CellValue::Float(v) => self.float_val = (*v as f32).clamp(f32::MIN, f32::MAX),
            CellValue::Choice(idx) => self.choice_idx = *idx,
            CellValue::Color(c) => self.color_val = *c,
            CellValue::Progress(_) | CellValue::Custom => {}
        }
    }

    /// Build a `CellValue` from the buffers matching `editor`. For text, moves
    /// the string out (zero-copy) and leaves a fresh pre-allocated buffer.
    pub(crate) fn take_cell_value(&mut self, editor: &CellEditor) -> CellValue {
        match editor {
            CellEditor::None | CellEditor::TextInput => {
                let text = std::mem::replace(&mut self.text_buf, String::with_capacity(256));
                CellValue::Text(text)
            }
            CellEditor::Checkbox => CellValue::Bool(self.bool_val),
            CellEditor::ComboBox { .. } => CellValue::Choice(self.choice_idx),
            CellEditor::SliderInt { .. } | CellEditor::SpinInt { .. } => {
                CellValue::Int(self.int_val as i64)
            }
            CellEditor::SliderFloat { .. } | CellEditor::SpinFloat { .. } => {
                CellValue::Float(self.float_val as f64)
            }
            CellEditor::ColorEdit => CellValue::Color(self.color_val),
            CellEditor::ProgressBar => CellValue::Progress(self.float_val),
            CellEditor::Button { .. } | CellEditor::Custom => CellValue::Custom,
        }
    }
}

/// Result of rendering the editor widget for one frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EditOutcome {
    /// Still editing — keep the editor open.
    Continue,
    /// Commit the buffered value back to the cell.
    Commit,
    /// Discard the edit.
    Cancel,
    /// `CellEditor::Custom` — the caller must render via the node/row trait.
    Custom,
}

/// Render the built-in editor widget for `editor` into `buf`, returning the
/// frame's outcome. Focus/commit semantics are identical to the pre-refactor
/// per-module code; `Custom` is delegated back to the caller.
pub(crate) fn render_editor_widget(
    ui: &Ui,
    editor: &CellEditor,
    buf: &mut EditBuffers,
    first_frame: bool,
    commit_on_focus_loss: bool,
) -> EditOutcome {
    let mut outcome = EditOutcome::Continue;
    match editor {
        CellEditor::TextInput => {
            if first_frame {
                unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
            }
            let entered = ui
                .input_text("##edit", &mut buf.text_buf)
                .enter_returns_true(true)
                .build();
            if entered {
                outcome = EditOutcome::Commit;
            } else if !first_frame {
                if ui.is_item_deactivated_after_edit() {
                    outcome = if commit_on_focus_loss {
                        EditOutcome::Commit
                    } else {
                        EditOutcome::Cancel
                    };
                } else if ui.is_item_deactivated() {
                    outcome = EditOutcome::Cancel;
                }
            }
        }
        CellEditor::SliderInt { min, max } => {
            ui.slider_config("##edit", *min, *max)
                .build(&mut buf.int_val);
            if !first_frame && ui.is_item_deactivated_after_edit() {
                outcome = EditOutcome::Commit;
            }
        }
        CellEditor::SliderFloat { min, max } => {
            ui.slider_config("##edit", *min, *max)
                .build(&mut buf.float_val);
            if !first_frame && ui.is_item_deactivated_after_edit() {
                outcome = EditOutcome::Commit;
            }
        }
        CellEditor::SpinInt { step, step_fast } => {
            if first_frame {
                unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
            }
            unsafe {
                dear_imgui_rs::sys::igInputInt(
                    c"##edit".as_ptr(),
                    &mut buf.int_val,
                    *step,
                    *step_fast,
                    0,
                );
            }
            if !first_frame {
                if ui.is_item_deactivated_after_edit() {
                    outcome = if commit_on_focus_loss {
                        EditOutcome::Commit
                    } else {
                        EditOutcome::Cancel
                    };
                } else if ui.is_item_deactivated() {
                    outcome = EditOutcome::Cancel;
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
                    &mut buf.float_val,
                    *step,
                    *step_fast,
                    c"%.2f".as_ptr(),
                    0,
                );
            }
            if !first_frame {
                if ui.is_item_deactivated_after_edit() {
                    outcome = if commit_on_focus_loss {
                        EditOutcome::Commit
                    } else {
                        EditOutcome::Cancel
                    };
                } else if ui.is_item_deactivated() {
                    outcome = EditOutcome::Cancel;
                }
            }
        }
        CellEditor::Custom => {
            outcome = EditOutcome::Custom;
        }
        _ => {
            outcome = EditOutcome::Cancel;
        }
    }

    if ui.is_key_pressed(Key::Escape) {
        outcome = EditOutcome::Cancel;
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_copy_from_value_and_take_round_trip() {
        let mut b = EditBuffers::default();
        b.copy_from_value(&CellValue::Text("hi".into()));
        assert!(b.just_activated);
        match b.take_cell_value(&CellEditor::TextInput) {
            CellValue::Text(s) => assert_eq!(s, "hi"),
            _ => panic!("expected Text"),
        }
        assert!(b.text_buf.is_empty());
        assert!(b.text_buf.capacity() >= 256);
    }

    #[test]
    fn buffers_clamp_int_to_i32() {
        let mut b = EditBuffers::default();
        b.copy_from_value(&CellValue::Int(i64::MAX));
        assert_eq!(b.int_val, i32::MAX);
        match b.take_cell_value(&CellEditor::SpinInt {
            step: 1,
            step_fast: 10,
        }) {
            CellValue::Int(v) => assert_eq!(v, i32::MAX as i64),
            _ => panic!("expected Int"),
        }
    }
}
