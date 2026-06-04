//! Tree-level configuration.
//!
//! [`TreeConfig`] wraps [`TableConfig`](crate::virtual_table::config::TableConfig)
//! for column/table behavior, adding tree-specific settings (indentation, tree
//! lines, lazy loading, drag-and-drop, filter, keyboard).

use crate::virtual_table::config::TableConfig;

// ─── ExpandStyle ────────────────────────────────────────────────────────────

/// Visual style for expand/collapse indicators on branch nodes.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum ExpandStyle {
    /// Standard ImGui TreeNode arrow (default filled triangle ▶/▼).
    #[default]
    Arrow,
    /// Custom Unicode glyphs for collapsed / expanded states.
    /// Example: `('\u{F0142}', '\u{F0140}')` for MDI chevron-right / chevron-down.
    Glyph {
        collapsed: char,
        expanded: char,
        /// Glyph color (RGBA). `None` = inherit current text color.
        color: Option<[f32; 4]>,
    },
}

// ─── TreeConfig ─────────────────────────────────────────────────────────────

/// Complete configuration for a [`VirtualTree`](super::VirtualTree).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TreeConfig {
    /// Embedded table configuration (columns, borders, selection, editing, etc.).
    pub table: TableConfig,

    /// Which column shows the tree hierarchy (expand arrow + indentation).
    /// Default: 0 (first column).
    pub tree_column: usize,

    /// Pixels of indentation per depth level. Default: 20.0.
    pub indent_width: f32,

    /// Show vertical/horizontal connector lines between parent and children.
    /// Default: false.
    pub show_tree_lines: bool,

    /// Tree line color (RGBA). Default: dim gray.
    pub tree_line_color: [f32; 4],

    /// Visual style for expand/collapse indicators. Default: Arrow (standard ImGui).
    pub expand_style: ExpandStyle,

    /// Expand/collapse on double-click (in addition to arrow click). Default: true.
    pub expand_on_double_click: bool,

    /// Auto-expand matching branches when filter is active. Default: true.
    pub auto_expand_on_filter: bool,

    /// Enable on-demand children loading. When `true`, register a loader via
    /// [`VirtualTree::set_lazy_loader`](super::VirtualTree::set_lazy_loader);
    /// each branch's children are materialized the first time it is expanded
    /// (and only once). Default: false.
    pub lazy_load: bool,

    /// Enable drag-and-drop node reparenting. Default: false.
    pub drag_drop_enabled: bool,

    /// Alternate row background (zebra striping) for readability. Default: true.
    pub striped: bool,

    /// Suppress the default hover/active highlight on column-header
    /// captions. Useful for informational tables where headers are
    /// non-interactive (sort disabled) — keeps the caption row a flat
    /// strip instead of a button-like surface.
    ///
    /// Implementation: `render_header` wraps each `ui.table_header`
    /// call in a two-entry style-color stack (HeaderHovered +
    /// HeaderActive → transparent). The same colors are used by
    /// `Selectable` rows for the selection state, so the style stack
    /// is strictly scoped to the header call to keep row feedback
    /// intact. Default: `true` — keeps headers visually calm; flip to
    /// `false` for trees where the user is expected to interact with
    /// the header row (click-to-sort, drag-reorder, etc.).
    pub flat_headers: bool,

    /// Maximum number of nodes the tree can hold.
    /// Must be in range `1..=MAX_TREE_NODES` (clamped automatically).
    /// Default: [`MAX_TREE_NODES`](super::arena::MAX_TREE_NODES) (10,000,000).
    pub max_nodes: usize,

    /// When `true` and the tree is at capacity, inserting a new node automatically
    /// removes the oldest root subtree (first root + all its descendants) to make room.
    /// When `false` (default), insert methods return `None` at capacity.
    pub evict_on_overflow: bool,

    /// Wrap the whole tree render in a bordered child window so it reads
    /// as a rounded card (uses the host's global `ChildRounding` /
    /// `Border` theme colors). Wrapper uses `WindowPadding [0, 0]` so the
    /// border sits flush against the inner table. Default: `true`.
    ///
    /// Note: `VirtualTree` calls `begin_table` directly (not
    /// `VirtualTable::render`), so the nested `table.card_border` does
    /// nothing here — only this top-level flag matters for tree
    /// wrapping.
    #[serde(default = "default_card_border_true")]
    pub card_border: bool,
}

fn default_card_border_true() -> bool {
    true
}

impl Default for TreeConfig {
    fn default() -> Self {
        ron::from_str(include_str!("config.ron"))
            .expect("built-in virtual_tree/config.ron is valid")
    }
}
