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
    /// Suppress the default hover/active highlight on column-header
    /// captions. Useful for informational tables where headers are
    /// non-interactive (sort disabled) — keeps the caption row a flat
    /// strip instead of a button-like surface.
    ///
    /// Implementation: `render_header` wraps each `ui.table_header`
    /// call in a two-entry style-color stack (HeaderHovered +
    /// HeaderActive → transparent). The same colors are used elsewhere
    /// for the row selection highlight, so the style stack is strictly
    /// scoped to the individual header call to keep row feedback
    /// intact. Default: false.
    pub flat_headers: bool,
    pub context_menu: bool,
    pub freeze_cols: i32,
    pub freeze_rows: i32,
    pub sizing: SizingPolicy,

    // Extensions
    /// Row selection mode. `SelectionMode::Multi` enables **Ctrl+Click** (toggle)
    /// and **Shift+Click** (range) — works regardless of keyboard layout.
    pub selection_mode: SelectionMode,
    /// Background color painted over every selected row.
    ///
    /// Applied via `table_set_bg_color(RowBg1)` so it is clearly visible even
    /// when many rows are selected. Set alpha to 0 to disable the override and
    /// rely solely on Dear ImGui's default `Header` highlight.
    ///
    /// Default: `[0.20, 0.45, 0.85, 0.75]` — a vivid blue at 75% opacity.
    pub selection_color: [f32; 4],
    /// Text color override for selected rows. `None` uses default text color.
    /// Default: `Some([1.0, 1.0, 1.0, 1.0])` — white text on selection.
    pub selection_text_color: Option<[f32; 4]>,
    /// Ctrl+C copies selected rows as tab-separated text to the clipboard.
    /// Works regardless of keyboard layout. Default: `false`.
    pub copy_to_clipboard: bool,
    pub edit_trigger: EditTrigger,
    /// When `true`, losing focus on an editor commits the value.
    /// When `false`, losing focus cancels the edit (only Enter/widget commit applies).
    pub commit_on_focus_loss: bool,
    pub auto_scroll: bool,
    /// Row vertical density (Normal / Compact / Dense).
    pub row_density: RowDensity,
    pub default_row_height: Option<f32>,
    /// Quantize the table's outer height to a multiple of `row_height` so
    /// the last visible row is never clipped mid-pixel. The trade-off is a
    /// small gap below the last row (up to `row_height - 1` pixels) when
    /// the parent isn't an exact row-multiple. Default: `false` — matches
    /// ImGui's native table behavior.
    ///
    /// Useful for log / packet views where a half-visible last row looks
    /// broken. Requires `scroll_y = true` (the default).
    pub snap_last_row: bool,
    /// Extra Dear ImGui flags merged into the computed flags.
    pub extra_flags: TableFlags,

    /// Default value for `ColumnDef::clip_tooltip` when a column hasn't set an
    /// explicit value (`None`). Set to `false` to disable clip-tooltips globally;
    /// individual columns can still override with `.clip_tooltip(true)`.
    ///
    /// Default: `true`.
    pub default_clip_tooltip: bool,
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
            flat_headers: false,
            context_menu: true,
            freeze_cols: 0,
            freeze_rows: 1,
            sizing: SizingPolicy::FixedFit,

            selection_mode: SelectionMode::Single,
            selection_color: [0.20, 0.45, 0.85, 0.75],
            selection_text_color: Some([1.0, 1.0, 1.0, 1.0]),
            copy_to_clipboard: false,
            edit_trigger: EditTrigger::DoubleClick,
            commit_on_focus_loss: true,
            auto_scroll: false,
            row_density: RowDensity::Normal,
            default_row_height: None,
            snap_last_row: false,
            extra_flags: TableFlags::NONE,
            default_clip_tooltip: true,
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
        // `flat_headers` overrides `highlight_hovered`: the whole point of
        // flat headers is a calm, non-interactive caption row; letting
        // `HIGHLIGHT_HOVERED_COLUMN` paint a column-wide tint under the
        // header defeats the per-column transparent push in `render_header`.
        // This way callers only need to flip one flag.
        if self.highlight_hovered && !self.flat_headers {
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
