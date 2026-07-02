//! Column definition, sizing, alignment, and cell editor types.
//!
//! Each column in a [`VirtualTable`](super::VirtualTable) is described by a [`ColumnDef`]
//! that controls its width ([`ColumnSizing`]), content alignment ([`CellAlignment`]),
//! inline editor widget ([`CellEditor`]), and Dear ImGui column flags.
//!
//! # Builder Pattern
//!
//! ```rust,ignore
//! ColumnDef::new("Name")
//!     .stretch(1.0)                    // proportional width
//!     .align(CellAlignment::Left)      // cell content alignment
//!     .header_align(CellAlignment::Center)
//!     .editor(CellEditor::TextInput)   // inline editor type
//!     .no_sort()                        // disable sorting for this column
//! ```

use dear_imgui_rs::TableColumnFlags;

// ─── Sizing ─────────────────────────────────────────────────────────────────

/// How a column determines its width.
#[derive(Clone, Debug)]
pub enum ColumnSizing {
    /// Fixed width in pixels.
    Fixed(f32),
    /// Proportional stretch weight (fills remaining space).
    Stretch(f32),
    /// Auto-fit to content width (Dear ImGui `WidthFixed` with auto-fitting).
    AutoFit(f32),
}

// ─── Alignment ──────────────────────────────────────────────────────────────

/// Horizontal alignment for cell content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellAlignment {
    /// Text starts at the left edge of the cell (default).
    #[default]
    Left,
    /// Text is centered within the cell.
    Center,
    /// Text is right-aligned against the cell's edge.
    Right,
}

// ─── Cell editor ────────────────────────────────────────────────────────────

/// Which widget to show when a cell enters edit mode.
#[derive(Clone, Debug, Default)]
pub enum CellEditor {
    /// Read-only text display (default).
    #[default]
    None,
    /// Single-line `input_text`.
    TextInput,
    /// Boolean toggle checkbox.
    Checkbox,
    /// Dropdown combo box with fixed options.
    ComboBox {
        /// Selectable option labels shown in the dropdown, in display order.
        items: Vec<String>,
    },
    /// Integer slider with range.
    SliderInt {
        /// Minimum value the slider can reach (inclusive).
        min: i32,
        /// Maximum value the slider can reach (inclusive).
        max: i32,
    },
    /// Float slider with range.
    SliderFloat {
        /// Minimum value the slider can reach (inclusive).
        min: f32,
        /// Maximum value the slider can reach (inclusive).
        max: f32,
    },
    /// Integer spinner (`input_int` with step).
    SpinInt {
        /// Amount added/subtracted per regular step (arrow click).
        step: i32,
        /// Amount added/subtracted per fast step (Ctrl+click/hold).
        step_fast: i32,
    },
    /// Float spinner (`input_float` with step).
    SpinFloat {
        /// Amount added/subtracted per regular step (arrow click).
        step: f32,
        /// Amount added/subtracted per fast step (Ctrl+click/hold).
        step_fast: f32,
    },
    /// Progress bar (read-only visualization, 0.0..1.0).
    ProgressBar,
    /// Color picker (`color_edit4`).
    ColorEdit,
    /// Clickable button inside the cell.
    Button {
        /// Text drawn on the button.
        label: String,
    },
    /// User-rendered via `VirtualTableRow::render_cell` / `render_editor`.
    Custom,
}

// ─── ColumnDef ──────────────────────────────────────────────────────────────

/// Full description of a single table column.
#[derive(Clone, Debug)]
pub struct ColumnDef {
    /// Header text displayed for this column.
    pub name: String,
    /// Width policy: fixed pixels, proportional stretch, or auto-fit to content.
    pub sizing: ColumnSizing,
    /// Horizontal alignment applied to cell content.
    pub alignment: CellAlignment,
    /// Horizontal alignment applied to the header label.
    pub header_alignment: CellAlignment,
    /// Inline editor widget shown when a cell in this column enters edit mode.
    pub editor: CellEditor,
    /// Raw Dear ImGui column flags (resize/sort/reorder/hide behavior).
    pub flags: TableColumnFlags,
    /// Whether the column starts out visible; can be toggled later from the
    /// column-visibility UI.
    pub visible: bool,
    /// Opaque identifier reported back in sort/interaction callbacks so a
    /// column can be recognized independent of its display index.
    pub user_id: u32,
    /// Show a tooltip with the full cell text when it's clipped (wider than column).
    ///
    /// `None` inherits from `TableConfig::default_clip_tooltip` (the default).
    /// `Some(true/false)` overrides the table-level default for this column only.
    pub clip_tooltip: Option<bool>,
    /// Default sort direction for this column (None = not default-sorted).
    pub default_sort: Option<bool>,
}

impl ColumnDef {
    /// Start building a column with the given header name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sizing: ColumnSizing::Fixed(100.0),
            alignment: CellAlignment::Left,
            header_alignment: CellAlignment::Center,
            editor: CellEditor::None,
            flags: TableColumnFlags::NONE,
            visible: true,
            user_id: 0,
            clip_tooltip: None,
            default_sort: None,
        }
    }

    /// Fixed width in pixels.
    pub fn fixed(mut self, width: f32) -> Self {
        self.sizing = ColumnSizing::Fixed(width);
        self
    }

    /// Stretch weight (proportional fill).
    pub fn stretch(mut self, weight: f32) -> Self {
        self.sizing = ColumnSizing::Stretch(weight);
        self
    }

    /// Cell content alignment.
    pub fn align(mut self, a: CellAlignment) -> Self {
        self.alignment = a;
        self
    }

    /// Header text alignment.
    pub fn header_align(mut self, a: CellAlignment) -> Self {
        self.header_alignment = a;
        self
    }

    /// Set the cell editor type.
    pub fn editor(mut self, e: CellEditor) -> Self {
        self.editor = e;
        self
    }

    /// Merge additional Dear ImGui column flags.
    pub fn flags(mut self, f: TableColumnFlags) -> Self {
        self.flags |= f;
        self
    }

    /// Mark column as not resizable.
    pub fn no_resize(mut self) -> Self {
        self.flags |= TableColumnFlags::NO_RESIZE;
        self
    }

    /// Mark column as not sortable.
    pub fn no_sort(mut self) -> Self {
        self.flags |= TableColumnFlags::NO_SORT;
        self
    }

    /// Mark column as not reorderable.
    pub fn no_reorder(mut self) -> Self {
        self.flags |= TableColumnFlags::NO_REORDER;
        self
    }

    /// Mark column as not hideable.
    pub fn no_hide(mut self) -> Self {
        self.flags |= TableColumnFlags::NO_HIDE;
        self
    }

    /// Set the user ID (used for sorting identification).
    pub fn user_id(mut self, id: u32) -> Self {
        self.user_id = id;
        self
    }

    /// Set initial visibility.
    pub fn visible(mut self, v: bool) -> Self {
        self.visible = v;
        self
    }

    /// Auto-fit width to content. `init_width` is the initial/minimum width.
    pub fn auto_fit(mut self, init_width: f32) -> Self {
        self.sizing = ColumnSizing::AutoFit(init_width);
        self
    }

    /// Override the table-level `default_clip_tooltip` for this column.
    /// `true` — always show; `false` — never show, regardless of the global default.
    pub fn clip_tooltip(mut self, enabled: bool) -> Self {
        self.clip_tooltip = Some(enabled);
        self
    }

    /// Set this column as the default sort column. `ascending = true` for A→Z.
    pub fn default_sort(mut self, ascending: bool) -> Self {
        self.default_sort = Some(ascending);
        self
    }

    /// Disable clip tooltips for this column, overriding the table-level default.
    pub fn no_clip_tooltip(mut self) -> Self {
        self.clip_tooltip = Some(false);
        self
    }

    /// Returns the Dear ImGui column flags. The width mode (Fixed/Stretch) is
    /// no longer a flag in dear-imgui-rs 0.13+ — it's a separate
    /// [`TableColumnWidth`] argument; see [`Self::column_width`].
    pub(crate) fn imgui_flags(&self) -> TableColumnFlags {
        let mut f = self.flags;
        if let Some(ascending) = self.default_sort {
            if ascending {
                f |= TableColumnFlags::PREFER_SORT_ASCENDING;
            } else {
                f |= TableColumnFlags::PREFER_SORT_DESCENDING;
            }
        }
        f
    }

    /// Width policy + initial value for Dear ImGui's column setup.
    pub(crate) fn column_width(&self) -> dear_imgui_rs::TableColumnWidth {
        match &self.sizing {
            ColumnSizing::Fixed(w) | ColumnSizing::AutoFit(w) => {
                dear_imgui_rs::TableColumnWidth::Fixed(*w)
            }
            ColumnSizing::Stretch(w) => dear_imgui_rs::TableColumnWidth::Stretch(*w),
        }
    }
}

// ─── Shared cell helpers ───────────────────────────────────────────────────
// Used by both virtual_table and virtual_tree to avoid code duplication.

/// Quick categorization of [`CellEditor`] variants (avoids full enum matching in hot paths).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EditorKind {
    None,
    Checkbox,
    ComboBox,
    Button,
    ProgressBar,
    ColorEdit,
    Custom,
    /// TextInput, SliderInt/Float, SpinInt/Float.
    Other,
}

/// Classify a [`CellEditor`] into its [`EditorKind`].
#[inline]
pub(crate) fn editor_kind(e: &CellEditor) -> EditorKind {
    match e {
        CellEditor::None => EditorKind::None,
        CellEditor::Checkbox => EditorKind::Checkbox,
        CellEditor::ComboBox { .. } => EditorKind::ComboBox,
        CellEditor::Button { .. } => EditorKind::Button,
        CellEditor::ProgressBar => EditorKind::ProgressBar,
        CellEditor::ColorEdit => EditorKind::ColorEdit,
        CellEditor::Custom => EditorKind::Custom,
        _ => EditorKind::Other,
    }
}

/// Compute horizontal padding for cell text alignment.
#[inline]
pub(crate) fn alignment_pad(alignment: CellAlignment, col_w: f32, text_w: f32) -> f32 {
    match alignment {
        CellAlignment::Left => 0.0,
        CellAlignment::Center => ((col_w - text_w) * 0.5).max(0.0),
        CellAlignment::Right => (col_w - text_w - 4.0).max(0.0),
    }
}
