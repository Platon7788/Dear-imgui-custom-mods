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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_inactive() {
        let es = EditState::default();
        assert!(!es.active);
        assert!(!es.is_editing(0, 0));
    }

    #[test]
    fn activate_text() {
        let mut es = EditState::default();
        es.activate(5, 2, &CellValue::Text("hello".into()));
        assert!(es.active);
        assert!(es.just_activated);
        assert_eq!(es.row, 5);
        assert_eq!(es.col, 2);
        assert_eq!(es.text_buf, "hello");
        assert!(es.is_editing(5, 2));
        assert!(!es.is_editing(5, 3));
    }

    #[test]
    fn activate_bool() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Bool(true));
        assert!(es.bool_val);
    }

    #[test]
    fn activate_int() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Int(42));
        assert_eq!(es.int_val, 42);
    }

    #[test]
    fn activate_int_clamped() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Int(i64::MAX));
        assert_eq!(es.int_val, i32::MAX);
    }

    #[test]
    fn activate_float() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Float(3.14));
        assert!((es.float_val - 3.14).abs() < 0.01);
    }

    #[test]
    fn activate_choice() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Choice(7));
        assert_eq!(es.choice_idx, 7);
    }

    #[test]
    fn activate_color() {
        let mut es = EditState::default();
        let c = [0.1, 0.2, 0.3, 0.4];
        es.activate(0, 0, &CellValue::Color(c));
        assert_eq!(es.color_val, c);
    }

    #[test]
    fn deactivate() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Text("x".into()));
        assert!(es.active);
        es.deactivate();
        assert!(!es.active);
    }

    #[test]
    fn take_cell_value_text_zero_copy() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Text("original".into()));
        let val = es.take_cell_value(&CellEditor::TextInput);
        match val {
            CellValue::Text(s) => assert_eq!(s, "original"),
            _ => panic!("expected Text"),
        }
        // text_buf should be replaced with fresh allocation
        assert!(es.text_buf.is_empty());
        assert!(es.text_buf.capacity() >= 256);
    }

    #[test]
    fn take_cell_value_slider_int() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Int(99));
        let val = es.take_cell_value(&CellEditor::SliderInt { min: 0, max: 100 });
        match val {
            CellValue::Int(v) => assert_eq!(v, 99),
            _ => panic!("expected Int"),
        }
    }

    #[test]
    fn take_cell_value_slider_float() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Float(1.5));
        let val = es.take_cell_value(&CellEditor::SliderFloat { min: 0.0, max: 10.0 });
        match val {
            CellValue::Float(v) => assert!((v - 1.5).abs() < 0.01),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn take_cell_value_checkbox() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Bool(true));
        let val = es.take_cell_value(&CellEditor::Checkbox);
        match val {
            CellValue::Bool(b) => assert!(b),
            _ => panic!("expected Bool"),
        }
    }

    #[test]
    fn take_cell_value_combo() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Choice(3));
        let val = es.take_cell_value(&CellEditor::ComboBox { items: vec![] });
        match val {
            CellValue::Choice(idx) => assert_eq!(idx, 3),
            _ => panic!("expected Choice"),
        }
    }

    #[test]
    fn take_cell_value_color() {
        let mut es = EditState::default();
        let c = [0.5, 0.6, 0.7, 0.8];
        es.activate(0, 0, &CellValue::Color(c));
        let val = es.take_cell_value(&CellEditor::ColorEdit);
        match val {
            CellValue::Color(v) => assert_eq!(v, c),
            _ => panic!("expected Color"),
        }
    }

    #[test]
    fn frames_active_counter() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Text("".into()));
        assert_eq!(es.frames_active, 0);
        es.frames_active += 1;
        assert_eq!(es.frames_active, 1);
    }
}
