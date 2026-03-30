//! Table-level configuration: behavior, features, layout.
//!
//! [`TableConfig`] controls all aspects of a [`VirtualTable`](super::VirtualTable):
//! Dear ImGui native features (resize, reorder, sort, borders, freeze) and
//! custom extensions (selection mode, edit trigger, row density, auto-scroll).
//!
//! All fields are `pub` and can be modified at runtime. Changes take effect
//! on the next `render()` call.
//!
//! # Defaults
//!
//! ```rust,ignore
//! TableConfig {
//!     resizable: true,      reorderable: false,
//!     hideable: true,       sortable: true,
//!     multi_sort: false,    borders: BorderStyle::Full,
//!     row_bg: true,         scroll_y: true,
//!     freeze_rows: 1,       // frozen header
//!     selection_mode: Single,
//!     edit_trigger: DoubleClick,
//!     row_density: Normal,
//!     commit_on_focus_loss: true,
//!     ..
//! }
//! ```

use dear_imgui_rs::TableFlags;

// ─── Enums ──────────────────────────────────────────────────────────────────

/// Row selection mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    /// No selection.
    None,
    /// Exactly one row at a time (default).
    #[default]
    Single,
    /// Multiple rows (Ctrl+Click, Shift+Click).
    Multi,
}

/// What triggers inline cell editing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditTrigger {
    /// Editing disabled.
    None,
    /// Double-click a cell to edit (default).
    #[default]
    DoubleClick,
    /// Single-click activates editor immediately.
    SingleClick,
    /// Press F2 on the selected row.
    F2Key,
}

/// Table border style presets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BorderStyle {
    /// No borders.
    None,
    /// Inner borders only.
    Inner,
    /// Outer borders only.
    Outer,
    /// All borders (default).
    #[default]
    Full,
    /// Vertical inner borders only.
    InnerV,
    /// Horizontal inner borders only.
    InnerH,
}

/// Row density / vertical padding mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowDensity {
    /// Comfortable: frame height + spacing (room for widgets).
    #[default]
    Normal,
    /// Compact: frame height only (widgets fit tightly).
    Compact,
    /// Dense: font size + 2px (text-only, no widget padding).
    Dense,
}

/// Column sizing policy for the whole table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SizingPolicy {
    /// Columns default to `WIDTH_FIXED` matching content.
    #[default]
    FixedFit,
    /// Columns default to `WIDTH_FIXED`, same width for all.
    FixedSame,
    /// Columns stretch proportionally to fill space.
    StretchProp,
    /// Columns stretch equally.
    StretchSame,
}

// ─── TableConfig ────────────────────────────────────────────────────────────

/// Complete configuration for a `VirtualTable`.
#[derive(Clone, Debug)]
pub struct TableConfig {
    // Dear ImGui native table features
    pub resizable: bool,
    pub reorderable: bool,
    pub hideable: bool,
    pub sortable: bool,
    pub multi_sort: bool,
    pub borders: BorderStyle,
    /// Show horizontal lines between rows.
    pub show_row_lines: bool,
    /// Show vertical lines between columns.
    pub show_column_lines: bool,
    pub row_bg: bool,
    pub scroll_x: bool,
    pub scroll_y: bool,
    pub highlight_hovered: bool,
    pub context_menu: bool,
    pub freeze_cols: i32,
    pub freeze_rows: i32,
    pub sizing: SizingPolicy,

    // Extensions
    pub selection_mode: SelectionMode,
    pub edit_trigger: EditTrigger,
    /// When `true`, losing focus on an editor commits the value.
    /// When `false`, losing focus cancels the edit (only Enter/widget commit applies).
    pub commit_on_focus_loss: bool,
    pub auto_scroll: bool,
    /// Row vertical density (Normal / Compact / Dense).
    pub row_density: RowDensity,
    pub default_row_height: Option<f32>,
    /// Extra Dear ImGui flags merged into the computed flags.
    pub extra_flags: TableFlags,
}

impl Default for TableConfig {
    fn default() -> Self {
        Self {
            resizable: true,
            reorderable: false,
            hideable: true,
            sortable: true,
            multi_sort: false,
            borders: BorderStyle::Full,
            show_row_lines: true,
            show_column_lines: true,
            row_bg: true,
            scroll_x: false,
            scroll_y: true,
            highlight_hovered: true,
            context_menu: true,
            freeze_cols: 0,
            freeze_rows: 1,
            sizing: SizingPolicy::FixedFit,

            selection_mode: SelectionMode::Single,
            edit_trigger: EditTrigger::DoubleClick,
            commit_on_focus_loss: true,
            auto_scroll: false,
            row_density: RowDensity::Normal,
            default_row_height: None,
            extra_flags: TableFlags::NONE,
        }
    }
}

impl TableConfig {
    /// Build Dear ImGui `TableFlags` from this config.
    pub(crate) fn to_table_flags(&self) -> TableFlags {
        let mut f = TableFlags::NONE;

        if self.resizable {
            f |= TableFlags::RESIZABLE;
        }
        if self.reorderable {
            f |= TableFlags::REORDERABLE;
        }
        if self.hideable {
            f |= TableFlags::HIDEABLE;
        }
        if self.sortable {
            f |= TableFlags::SORTABLE;
        }
        if self.multi_sort {
            f |= TableFlags::SORT_MULTI;
        }
        if self.row_bg {
            f |= TableFlags::ROW_BG;
        }
        if self.scroll_x {
            f |= TableFlags::SCROLL_X;
        }
        if self.scroll_y {
            f |= TableFlags::SCROLL_Y;
        }
        if self.highlight_hovered {
            f |= TableFlags::HIGHLIGHT_HOVERED_COLUMN;
        }
        if self.context_menu {
            f |= TableFlags::CONTEXT_MENU_IN_BODY;
        }

        // Borders — base preset
        match self.borders {
            BorderStyle::None => {}
            BorderStyle::Inner => {
                f |= TableFlags::BORDERS_INNER_H | TableFlags::BORDERS_INNER_V;
            }
            BorderStyle::Outer => {
                f |= TableFlags::BORDERS_OUTER_H | TableFlags::BORDERS_OUTER_V;
            }
            BorderStyle::Full => {
                f |= TableFlags::BORDERS_INNER_H
                    | TableFlags::BORDERS_INNER_V
                    | TableFlags::BORDERS_OUTER_H
                    | TableFlags::BORDERS_OUTER_V;
            }
            BorderStyle::InnerV => {
                f |= TableFlags::BORDERS_INNER_V;
            }
            BorderStyle::InnerH => {
                f |= TableFlags::BORDERS_INNER_H;
            }
        }

        // Override inner lines visibility
        if !self.show_row_lines {
            f &= !TableFlags::BORDERS_INNER_H;
        }
        if !self.show_column_lines {
            f &= !TableFlags::BORDERS_INNER_V;
        }

        // Sizing policy
        match self.sizing {
            SizingPolicy::FixedFit => f |= TableFlags::SIZING_FIXED_FIT,
            SizingPolicy::FixedSame => f |= TableFlags::SIZING_FIXED_SAME,
            SizingPolicy::StretchProp => f |= TableFlags::SIZING_STRETCH_PROP,
            SizingPolicy::StretchSame => f |= TableFlags::SIZING_STRETCH_SAME,
        }

        f | self.extra_flags
    }
}
