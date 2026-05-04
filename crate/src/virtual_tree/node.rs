//! Tree node trait, icon types, and re-exports from `virtual_table`.
//!
//! [`VirtualTreeNode`] mirrors [`VirtualTableRow`](crate::virtual_table::VirtualTableRow)
//! method-for-method where applicable, adding tree-specific extensions:
//! `has_children`, `icon`, `accepts_drop`, `is_draggable`, `matches_filter`.

use std::cmp::Ordering;

use dear_imgui_rs::Ui;

use crate::virtual_table::row::{CellStyle, CellValue, RowStyle};

use super::arena::NodeId;

// ─── NodeIcon ───────────────────────────────────────────────────────────────

/// Icon specification for the tree column (shown between expand arrow and text).
#[derive(Clone, Debug, Default)]
pub enum NodeIcon {
    /// No icon.
    #[default]
    None,
    /// Unicode codepoint (e.g. Material Design Icon from `crate::icons`).
    Glyph(char),
    /// Unicode codepoint with RGBA tint color.
    GlyphColored(char, [f32; 4]),
    /// RGBA color swatch displayed as a small square.
    ColorSwatch([f32; 4]),
    /// User-rendered via [`VirtualTreeNode::render_icon()`].
    Custom,
}

// ─── VirtualTreeNode trait ──────────────────────────────────────────────────

/// Implement this trait for any type displayed in a [`VirtualTree`](super::VirtualTree).
///
/// # Required methods
///
/// | Method | Purpose |
/// |--------|---------|
/// | `cell_value(col)` | Return typed cell value for column |
/// | `set_cell_value(col, value)` | Accept edited value back |
/// | `has_children()` | Whether to show expand arrow |
///
/// # Optional methods
///
/// All optional methods have sensible defaults (same as `VirtualTableRow`).
/// Tree-specific additions: `icon`, `render_icon`, `accepts_drop`,
/// `is_draggable`, `matches_filter`.
pub trait VirtualTreeNode {
    // ── Required (same contract as VirtualTableRow) ─────────────────

    /// Return the typed value of cell at `col`.
    /// Column 0 is typically the tree column (text shown next to expand arrow + icon).
    fn cell_value(&self, col: usize) -> CellValue;

    /// Write an edited value back. Called when the user commits an edit.
    fn set_cell_value(&mut self, col: usize, value: &CellValue);

    // ── Tree-specific (required) ────────────────────────────────────

    /// Whether this node conceptually has children, even if none are loaded yet.
    ///
    /// When `true`, an expand arrow is shown. When `false`, the node is a leaf.
    /// For eager trees, base this on whether children exist.
    /// For lazy trees, return `true` if children *could* be loaded.
    fn has_children(&self) -> bool;

    // ── Optional with defaults (mirrors VirtualTableRow) ────────────

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
    fn render_cell(&self, _ui: &Ui, _col: usize, _id: NodeId) -> bool {
        false
    }

    /// Custom editor rendering (for `CellEditor::Custom` in edit mode).
    /// Return `true` if the edit should be committed.
    fn render_editor(&mut self, _ui: &Ui, _col: usize, _id: NodeId) -> bool {
        false
    }

    /// Plain-text tooltip shown on row hover.
    fn row_tooltip(&self, _buf: &mut String) {}

    /// Rich tooltip via Dear ImGui. Return `true` if rendered.
    fn render_tooltip(&self, _ui: &Ui) -> bool {
        false
    }

    /// Compare two nodes for sorting within the same sibling group.
    fn compare(&self, _other: &Self, _col: usize) -> Ordering {
        Ordering::Equal
    }

    // ── Tree-specific (optional) ────────────────────────────────────

    /// Icon for the tree column (shown between expand arrow and text).
    fn icon(&self) -> NodeIcon {
        NodeIcon::None
    }

    /// Custom icon rendering (when [`icon()`](Self::icon) returns [`NodeIcon::Custom`]).
    /// Return `true` if rendered.
    fn render_icon(&self, _ui: &Ui) -> bool {
        false
    }

    /// Whether this node accepts a drag-drop of `dragged` as a child.
    /// Called during drag hover to show accept/reject cursor.
    fn accepts_drop(&self, _dragged: &Self) -> bool {
        true
    }

    /// Whether this node can be dragged.
    fn is_draggable(&self) -> bool {
        true
    }

    /// Called when filter is active. Return `true` if this node matches the query.
    /// Parent nodes auto-expand if any descendant matches.
    fn matches_filter(&self, _query: &str) -> bool {
        true
    }

    /// Optional badge text shown after the node label (e.g. children count).
    /// Return empty string for no badge.
    fn badge(&self) -> &str {
        ""
    }
}
