//! Tree-level configuration.
//!
//! [`TreeConfig`] wraps [`TableConfig`](crate::virtual_table::config::TableConfig)
//! for column/table behavior, adding tree-specific settings (indentation, tree
//! lines, lazy loading, drag-and-drop, filter, keyboard).

use crate::virtual_table::config::TableConfig;

// ─── ExpandStyle ────────────────────────────────────────────────────────────

/// Visual style for expand/collapse indicators on branch nodes.
#[derive(Clone, Debug, Default)]
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
#[derive(Clone, Debug)]
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

    /// Enable lazy children loading via callback. Default: false.
    pub lazy_load: bool,

    /// Enable drag-and-drop node reparenting. Default: false.
    pub drag_drop_enabled: bool,

    /// Shift+Click range selection uses flat view indices. Default: true.
    pub multi_select_flat: bool,

    /// Alternate row background (zebra striping) for readability. Default: true.
    pub striped: bool,

    /// Maximum number of nodes the tree can hold.
    /// Must be in range `1..=MAX_TREE_NODES` (clamped automatically).
    /// Default: [`MAX_TREE_NODES`](super::arena::MAX_TREE_NODES) (10,000,000).
    pub max_nodes: usize,

    /// When `true` and the tree is at capacity, inserting a new node automatically
    /// removes the oldest root subtree (first root + all its descendants) to make room.
    /// When `false` (default), insert methods return `None` at capacity.
    pub evict_on_overflow: bool,
}

impl Default for TreeConfig {
    fn default() -> Self {
        Self {
            table: TableConfig::default(),
            tree_column: 0,
            indent_width: 20.0,
            show_tree_lines: false,
            tree_line_color: [0.35, 0.35, 0.35, 0.6],
            expand_style: ExpandStyle::default(),
            expand_on_double_click: true,
            auto_expand_on_filter: true,
            lazy_load: false,
            drag_drop_enabled: false,
            multi_select_flat: true,
            striped: true,
            max_nodes: super::arena::MAX_TREE_NODES,
            evict_on_overflow: false,
        }
    }
}
