//! Inline cell editing state and logic (table). Value buffers + widget render
//! live in `super::edit_common`; this only adds the row/col key.

use super::column::CellEditor;
use super::edit_common::EditBuffers;
use super::row::CellValue;

/// Tracks the currently active inline editor, if any.
#[derive(Clone, Debug, Default)]
pub(crate) struct EditState {
    pub active: bool,
    pub row: usize,
    pub col: usize,
    pub buf: EditBuffers,
}

impl EditState {
    /// True on the first frame after activation.
    #[inline]
    pub(super) fn just_activated(&self) -> bool {
        self.buf.just_activated
    }

    #[inline]
    pub(super) fn set_activated(&mut self, v: bool) {
        self.buf.just_activated = v;
    }

    pub(super) fn activate(&mut self, row: usize, col: usize, value: &CellValue) {
        self.active = true;
        self.row = row;
        self.col = col;
        self.buf.copy_from_value(value);
    }

    pub(super) fn deactivate(&mut self) {
        self.active = false;
    }

    pub(super) fn take_cell_value(&mut self, editor: &CellEditor) -> CellValue {
        self.buf.take_cell_value(editor)
    }

    #[inline]
    pub(super) fn is_editing(&self, row: usize, col: usize) -> bool {
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
        assert!(es.just_activated());
        assert_eq!(es.row, 5);
        assert_eq!(es.col, 2);
        assert_eq!(es.buf.text_buf, "hello");
        assert!(es.is_editing(5, 2));
        assert!(!es.is_editing(5, 3));
    }

    #[test]
    fn activate_bool() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Bool(true));
        assert!(es.buf.bool_val);
    }

    #[test]
    fn activate_int() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Int(42));
        assert_eq!(es.buf.int_val, 42);
    }

    #[test]
    fn activate_int_clamped() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Int(i64::MAX));
        assert_eq!(es.buf.int_val, i32::MAX);
    }

    #[test]
    fn activate_float() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Float(3.25));
        assert!((es.buf.float_val - 3.25).abs() < 0.01);
    }

    #[test]
    fn activate_choice() {
        let mut es = EditState::default();
        es.activate(0, 0, &CellValue::Choice(7));
        assert_eq!(es.buf.choice_idx, 7);
    }

    #[test]
    fn activate_color() {
        let mut es = EditState::default();
        let c = [0.1, 0.2, 0.3, 0.4];
        es.activate(0, 0, &CellValue::Color(c));
        assert_eq!(es.buf.color_val, c);
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
        assert!(es.buf.text_buf.is_empty());
        assert!(es.buf.text_buf.capacity() >= 256);
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
        let val = es.take_cell_value(&CellEditor::SliderFloat {
            min: 0.0,
            max: 10.0,
        });
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
}
