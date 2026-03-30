//! Inline cell editing state and logic.

use super::column::CellEditor;
use super::row::CellValue;

/// Tracks the currently active inline editor, if any.
#[derive(Clone, Debug)]
pub(crate) struct EditState {
    pub active: bool,
    pub row: usize,
    pub col: usize,
    /// True on the very first frame after activation.
    pub just_activated: bool,
    /// How many frames the editor has been active (for safe dismissal).
    pub frames_active: u32,

    // Value buffers — one per editor type
    pub text_buf: String,
    pub bool_val: bool,
    pub int_val: i32,
    pub float_val: f32,
    pub choice_idx: usize,
    pub color_val: [f32; 4],
}

impl Default for EditState {
    fn default() -> Self {
        Self {
            active: false,
            row: 0,
            col: 0,
            just_activated: false,
            frames_active: 0,
            text_buf: String::with_capacity(256),
            bool_val: false,
            int_val: 0,
            float_val: 0.0,
            choice_idx: 0,
            color_val: [1.0; 4],
        }
    }
}

impl EditState {
    /// Activate editor for (row, col) by copying the current cell value into buffers.
    pub fn activate(&mut self, row: usize, col: usize, value: &CellValue) {
        self.active = true;
        self.row = row;
        self.col = col;
        self.just_activated = true;
        self.frames_active = 0;

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

    /// Deactivate the editor.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Build a `CellValue` from the current buffer state, matching the editor type.
    ///
    /// For `TextInput`: moves the string out of `text_buf` (zero-copy) and replaces
    /// it with a fresh pre-allocated buffer. The old capacity is not wasted because
    /// `set_cell_value` takes ownership of the String inside CellValue.
    pub fn take_cell_value(&mut self, editor: &CellEditor) -> CellValue {
        match editor {
            CellEditor::None | CellEditor::TextInput => {
                // Move the string out instead of cloning — saves one allocation.
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

    /// Check if editing this specific cell.
    #[inline]
    pub fn is_editing(&self, row: usize, col: usize) -> bool {
        self.active && self.row == row && self.col == col
    }
}
