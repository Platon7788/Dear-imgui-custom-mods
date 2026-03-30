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
    #[default]
    Left,
    Center,
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
    ComboBox { items: Vec<String> },
    /// Integer slider with range.
    SliderInt { min: i32, max: i32 },
    /// Float slider with range.
    SliderFloat { min: f32, max: f32 },
    /// Integer spinner (`input_int` with step).
    SpinInt { step: i32, step_fast: i32 },
    /// Float spinner (`input_float` with step).
    SpinFloat { step: f32, step_fast: f32 },
    /// Progress bar (read-only visualization, 0.0..1.0).
    ProgressBar,
    /// Color picker (`color_edit4`).
    ColorEdit,
    /// Clickable button inside the cell.
    Button { label: String },
    /// User-rendered via `VirtualTableRow::render_cell` / `render_editor`.
    Custom,
}

// ─── ColumnDef ──────────────────────────────────────────────────────────────

/// Full description of a single table column.
#[derive(Clone, Debug)]
pub struct ColumnDef {
    pub name: String,
    pub sizing: ColumnSizing,
    pub alignment: CellAlignment,
    pub header_alignment: CellAlignment,
    pub editor: CellEditor,
    pub flags: TableColumnFlags,
    pub visible: bool,
    pub user_id: u32,
    /// Show a tooltip with the full cell text when it's clipped (wider than column).
    /// Default: true.
    pub clip_tooltip: bool,
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
            clip_tooltip: true,
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

    /// Show tooltip with full text when cell content is clipped. Default: true.
    pub fn clip_tooltip(mut self, enabled: bool) -> Self {
        self.clip_tooltip = enabled;
        self
    }

    /// Set this column as the default sort column. `ascending = true` for A→Z.
    pub fn default_sort(mut self, ascending: bool) -> Self {
        self.default_sort = Some(ascending);
        self
    }

    /// Disable clip tooltips for this column (e.g. for checkbox/button columns).
    pub fn no_clip_tooltip(mut self) -> Self {
        self.clip_tooltip = false;
        self
    }

    /// Returns the Dear ImGui column flags with sizing flags applied.
    pub(crate) fn imgui_flags(&self) -> TableColumnFlags {
        let mut f = self.flags;
        match &self.sizing {
            ColumnSizing::Fixed(_) | ColumnSizing::AutoFit(_) => {
                f |= TableColumnFlags::WIDTH_FIXED;
            }
            ColumnSizing::Stretch(_) => f |= TableColumnFlags::WIDTH_STRETCH,
        }
        if let Some(ascending) = self.default_sort {
            if ascending {
                f |= TableColumnFlags::PREFER_SORT_ASCENDING;
            } else {
                f |= TableColumnFlags::PREFER_SORT_DESCENDING;
            }
        }
        f
    }

    /// Returns the init_width_or_weight value for Dear ImGui.
    pub(crate) fn init_width_or_weight(&self) -> f32 {
        match &self.sizing {
            ColumnSizing::Fixed(w) | ColumnSizing::AutoFit(w) => *w,
            ColumnSizing::Stretch(w) => *w,
        }
    }
}
