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

use dear_imgui_rs::{TableFlags, TableOptions, TableSizingPolicy};

fn default_table_flags() -> TableFlags {
    TableFlags::NONE
}

fn default_card_border() -> bool {
    true
}

// ─── Enums ──────────────────────────────────────────────────────────────────

/// Row selection mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RowDensity {
    /// Comfortable: frame height + spacing (room for widgets).
    Normal,
    /// Compact: frame height only (widgets fit tightly).
    Compact,
    /// Dense: font size + 2px (text-only, no widget padding).
    /// Default — keeps high-row-count tables (10k+) on screen
    /// without scrolling and matches the typical
    /// reverse-engineering / observability use case.
    #[default]
    Dense,
}

/// Column sizing policy for the whole table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    /// intact. Default: `true` — keeps headers visually calm; flip to
    /// `false` for tables where the user is expected to interact with
    /// the header row (click-to-sort, drag-reorder, etc.).
    pub flat_headers: bool,
    /// Render the header row at all. When `false`, [`Self::freeze_rows`]
    /// is implicitly clamped to 0 (a frozen non-existent header would
    /// reserve dead pixels at the top of the body) and the header
    /// captions in [`ColumnDef::name`] are used only as ImGui IDs (the
    /// `##suffix` idiom keeps them unique without a visible caption).
    ///
    /// Useful for register dumps / status panes where the columns are
    /// self-explanatory and a header strip steals vertical real estate.
    /// Default: `true` (existing behavior).
    pub show_headers: bool,
    pub context_menu: bool,
    /// Suppress the right-click "Size column to fit / Size all columns
    /// to default" popup that Dear ImGui's `TableHeader` emits
    /// automatically when `Resizable` is on. When `false`, headers are
    /// rendered with raw `ui.text()` instead of `ui.table_header()`,
    /// so no native popup is registered for them at all. Defaults to
    /// `true` (preserves pre-existing behaviour for callers that don't
    /// opt in).
    ///
    /// `Sortable = true` is incompatible with this — `ui.text()` doesn't
    /// emit sort indicators or click-to-sort detection. Callers must
    /// flip `sortable = false` if they want the popup gone.
    #[serde(default = "default_header_popup")]
    pub header_popup: bool,
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
    #[serde(skip, default = "default_table_flags")]
    pub extra_flags: TableFlags,

    /// Default value for `ColumnDef::clip_tooltip` when a column hasn't set an
    /// explicit value (`None`). Set to `false` to disable clip-tooltips globally;
    /// individual columns can still override with `.clip_tooltip(true)`.
    ///
    /// Default: `true`.
    pub default_clip_tooltip: bool,

    /// Wrap the whole table render in a bordered child window so it
    /// reads as a rounded card (using the host's global `ChildRounding`
    /// / `Border` theme colors). The wrapper uses `WindowPadding [0, 0]`
    /// so the border sits flush against the table's outer cells — no
    /// extra inner gap, just the rounded corners. Default: `true`.
    ///
    /// Set to `false` for callers that already wrap the table in their
    /// own bordered child / panel, or for tables that should remain
    /// "flush" with the surrounding chrome (e.g. an embedded inline
    /// table inside a settings dialog).
    #[serde(default = "default_card_border")]
    pub card_border: bool,
}

fn default_header_popup() -> bool {
    true
}

impl Default for TableConfig {
    fn default() -> Self {
        let mut cfg: TableConfig = ron::from_str(include_str!("config.ron"))
            .expect("built-in virtual_table/config.ron is valid");
        cfg.extra_flags = TableFlags::NONE;
        cfg.header_popup = true;
        cfg
    }
}

impl TableConfig {
    /// Build Dear ImGui [`TableOptions`] (flags + single sizing policy) from
    /// this config. In dear-imgui-rs 0.13+ the sizing policy is no longer a
    /// bit in [`TableFlags`] (it's a mutually exclusive choice on
    /// [`TableSizingPolicy`]) — we surface a `TableOptions` instead so callers
    /// can pass it straight to `begin_table_with_flags`.
    pub(crate) fn to_table_options(&self) -> TableOptions {
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

        let sizing_policy = match self.sizing {
            SizingPolicy::FixedFit => TableSizingPolicy::FixedFit,
            SizingPolicy::FixedSame => TableSizingPolicy::FixedSame,
            SizingPolicy::StretchProp => TableSizingPolicy::StretchProp,
            SizingPolicy::StretchSame => TableSizingPolicy::StretchSame,
        };

        TableOptions::new()
            .flags(f | self.extra_flags)
            .sizing_policy(sizing_policy)
    }
}
