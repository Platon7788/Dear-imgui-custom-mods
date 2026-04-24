//! Row trait, cell values, and styling types.
//!
//! Implement [`VirtualTableRow`] for your data type to display it in a
//! [`VirtualTable`](super::VirtualTable). The trait has two required methods:
//!
//! - [`cell_value()`](VirtualTableRow::cell_value) — return a typed [`CellValue`] for each column
//! - [`set_cell_value()`](VirtualTableRow::set_cell_value) — accept edited values back
//!
//! All other methods have sensible defaults:
//!
//! | Method              | Purpose                                         | Default        |
//! |---------------------|-------------------------------------------------|----------------|
//! | `cell_display_text` | Custom text formatting per cell                 | Formats `CellValue` |
//! | `row_style`         | Per-row background, text color, custom height   | `None`         |
//! | `cell_style`        | Per-cell bg/text/alignment (overrides row style) | `None`         |
//! | `render_cell`       | Custom cell rendering (for `CellEditor::Custom`) | `false`        |
//! | `render_editor`     | Custom editor rendering                         | `false`        |
//! | `row_tooltip`       | Plain-text tooltip on row hover                 | empty          |
//! | `render_tooltip`    | Rich ImGui tooltip                              | `false`        |
//! | `compare`           | Sorting comparison on a given column            | `Equal`        |

use std::cmp::Ordering;
use std::fmt::Write;

use dear_imgui_rs::Ui;

use super::column::CellAlignment;

// ─── Cell value ─────────────────────────────────────────────────────────────

/// Typed value of a single cell.
#[derive(Clone, Debug)]
pub enum CellValue {
    /// Plain text.
    Text(String),
    /// Boolean (for Checkbox editor).
    Bool(bool),
    /// 64-bit integer.
    Int(i64),
    /// 64-bit float.
    Float(f64),
    /// Index into `CellEditor::ComboBox { items }`.
    Choice(usize),
    /// RGBA color (for ColorEdit).
    Color([f32; 4]),
    /// Progress fraction 0.0..1.0 (for ProgressBar).
    Progress(f32),
    /// User-drawn content (for CellEditor::Custom).
    Custom,
}

impl CellValue {
    /// Format the value as display text into `buf`.
    pub fn format_into(&self, buf: &mut String) {
        match self {
            CellValue::Text(s) => buf.push_str(s),
            CellValue::Bool(b) => {
                buf.push_str(if *b { "true" } else { "false" });
            }
            CellValue::Int(v) => {
                let _ = write!(buf, "{v}");
            }
            CellValue::Float(v) => {
                let _ = write!(buf, "{v:.2}");
            }
            CellValue::Choice(idx) => {
                let _ = write!(buf, "{idx}");
            }
            CellValue::Color(c) => {
                let _ = write!(
                    buf,
                    "#{:02X}{:02X}{:02X}{:02X}",
                    (c[0].clamp(0.0, 1.0) * 255.0) as u8,
                    (c[1].clamp(0.0, 1.0) * 255.0) as u8,
                    (c[2].clamp(0.0, 1.0) * 255.0) as u8,
                    (c[3].clamp(0.0, 1.0) * 255.0) as u8,
                );
            }
            CellValue::Progress(p) => {
                let _ = write!(buf, "{:.0}%", p * 100.0);
            }
            CellValue::Custom => {}
        }
    }
}

// ─── Styling ────────────────────────────────────────────────────────────────

/// Per-cell visual overrides.
#[derive(Clone, Debug, Default)]
pub struct CellStyle {
    pub text_color: Option<[f32; 4]>,
    pub bg_color: Option<[f32; 4]>,
    pub alignment: Option<CellAlignment>,
}

/// Per-row visual overrides.
///
/// Returned from `VirtualTableRow::row_style` (and `VirtualTreeNode::row_style`
/// by extension). `None` on any field means "use the table's defaults".
#[derive(Clone, Debug, Default)]
pub struct RowStyle {
    /// Background tint painted over the row when **not** selected.
    pub bg_color: Option<[f32; 4]>,
    /// Text color for every cell in the row.
    pub text_color: Option<[f32; 4]>,
    /// Override row height in pixels.
    pub height: Option<f32>,
    /// Background tint painted over the row when it **is** selected.
    ///
    /// `None` → use the table-wide `TableConfig::selection_color`.
    /// Useful when severity rows (e.g. errors) should keep their
    /// identity instead of flipping to the generic blue selection.
    pub selection_color: Option<[f32; 4]>,
    /// Text color for cells when the row is selected.
    ///
    /// `None` → use the table-wide `TableConfig::selection_text_color`.
    pub selection_text_color: Option<[f32; 4]>,
}

// ─── Row trait ──────────────────────────────────────────────────────────────

/// Implement this trait for any type displayed in a `VirtualTable`.
pub trait VirtualTableRow {
    /// Return the typed value of cell at `col`.
    fn cell_value(&self, col: usize) -> CellValue;

    /// Write an edited value back. Called when the user commits an edit.
    fn set_cell_value(&mut self, col: usize, value: &CellValue);

    /// Custom display text override. By default formats `cell_value()`.
    /// `buf` is pre-cleared before each call.
    fn cell_display_text(&self, col: usize, buf: &mut String) {
        self.cell_value(col).format_into(buf);
    }

    /// Per-row style (background, text color, height).
    fn row_style(&self) -> Option<RowStyle> {
        None
    }

    /// Per-cell style (overrides row_style for a specific column).
    fn cell_style(&self, _col: usize) -> Option<CellStyle> {
        None
    }

    /// Custom cell rendering (for `CellEditor::Custom`).
    /// Return `true` if you rendered something.
    fn render_cell(&self, _ui: &Ui, _col: usize) -> bool {
        false
    }

    /// Custom editor rendering (for `CellEditor::Custom` in edit mode).
    /// Return `true` if the edit should be committed.
    fn render_editor(&mut self, _ui: &Ui, _col: usize) -> bool {
        false
    }

    /// Plain-text tooltip shown on row hover.
    fn row_tooltip(&self, _buf: &mut String) {}

    /// Rich tooltip via Dear ImGui. Return `true` if rendered.
    fn render_tooltip(&self, _ui: &Ui) -> bool {
        false
    }

    /// Compare two rows for sorting on `col`.
    fn compare(&self, _other: &Self, _col: usize) -> Ordering {
        Ordering::Equal
    }
}
