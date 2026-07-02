//! # VirtualTree\<T\>
//!
//! High-performance hierarchical tree-table component for Dear ImGui,
//! inspired by DevExpress VirtualTreeList and Delphi VirtualStringTree.
//!
//! ## Key Features
//!
//! - **Slab/arena storage** with generational [`NodeId`] — O(1) insert/remove/lookup
//! - **Flattened view cache** — rebuilt only on expand/collapse, not every frame
//! - **ListClipper virtualization** — only visible rows rendered (100k+ nodes)
//! - **Multi-column** support reusing [`ColumnDef`]/[`CellEditor`] from `virtual_table`
//! - **Inline editing** (text, checkbox, combo, slider, color, button, custom)
//! - **Selection**: None, Single, Multi (Ctrl+Click, Shift+Click range on flat view)
//! - **Sibling-scoped sorting** via ImGui table headers
//! - **Drag-and-drop** reparenting between nodes
//! - **Filter/search** with auto-expand matching branches
//! - **Lazy children loading** via callback
//! - **Keyboard navigation**: Up/Down (flat), Left (collapse/parent), Right (expand/child)
//! - **Per-node icons**: glyph, color swatch, or custom-rendered
//!
//! ## Architecture
//!
//! ```text
//! virtual_tree/
//! ├── mod.rs        VirtualTree<T> struct, EditState, new()/lazy loader
//! ├── api.rs        Public data API: insert/remove/access/expand/move/select/sort/filter/export
//! ├── render.rs     Render entry points, table setup, header, sort handling
//! ├── row.rs        Per-row rendering (selectable, drag-drop, data cells)
//! ├── tree_cell.rs  Tree-column cell: indent, tree lines, expand glyph/arrow, icon
//! ├── edit.rs       Inline cell-editor activation and rendering
//! ├── input.rs      Selection, keyboard navigation, clipboard copy
//! ├── arena/        TreeArena<T> — slab storage with NodeId, parent/children
//! │   ├── mod.rs    types + core: alloc, insert, remove, access, expand
//! │   ├── ops.rs    reparenting, sibling sort, iteration
//! │   └── tests.rs  arena unit tests
//! ├── node.rs       VirtualTreeNode trait, NodeIcon
//! ├── config.rs     TreeConfig (wraps TableConfig)
//! ├── flat_view.rs  FlatView — cached linearization for ListClipper
//! ├── sort.rs       Sibling-scoped sort state
//! ├── filter.rs     FilterState — search with auto-expand
//! └── drag.rs       Drag-and-drop constants for node reparenting
//! ```

pub mod arena;
pub mod config;
mod drag;
pub mod filter;
pub mod flat_view;
pub mod node;
mod sort;

// Method impls split across files (CLAUDE.md: keep files < 500 lines).
// All operate on the same `VirtualTree<T>` via `impl` blocks; child modules
// see the parent-private fields of the struct above.
mod api;
mod edit;
mod input;
mod render;
mod row;
mod tree_cell;

pub use arena::{MAX_TREE_NODES, NodeId, NodeSlot};
pub use config::{ExpandStyle, TreeConfig};
pub use filter::FilterState;
pub use flat_view::{FlatRow, FlatView};
pub use node::{NodeIcon, VirtualTreeNode};

// Re-export shared types from virtual_table
pub use crate::virtual_table::column::{CellAlignment, CellEditor, ColumnDef, ColumnSizing};
pub use crate::virtual_table::config::{EditTrigger, RowDensity, SelectionMode};
pub use crate::virtual_table::row::{CellStyle, CellValue, RowStyle};

use crate::utils::clipboard::set_clipboard;
use crate::utils::text::calc_text_size;
use crate::virtual_table::column::{EditorKind, alignment_pad, editor_kind};
use dear_imgui_rs::{Key, ListClipper, MouseButton, SelectableFlags, TableRowFlags, Ui};

use std::collections::HashSet;

use arena::TreeArena;
use sort::{SortSpec, SortState};

/// Fast hash set for NodeId. Uses `foldhash` for O(1) operations.
type NodeIdSet = HashSet<NodeId, foldhash::fast::FixedState>;

/// Boxed per-tree lazy children loader (see [`VirtualTree::set_lazy_loader`]).
/// Runtime payload — never serialized.
type LazyLoader<T> = Box<dyn FnMut(NodeId) -> Vec<T>>;

// ─── EditState (inline, mirrors virtual_table::edit) ────────────────────────

/// Tracks the currently active inline editor.
///
/// Keyed by [`NodeId`] (not flat-view index) so that an active edit follows
/// its node across flat-view rebuilds — inserting/removing/moving other nodes
/// while editing no longer commits into the wrong row.
#[derive(Clone, Debug, Default)]
struct EditState {
    active: bool,
    node: Option<NodeId>,
    col: usize,
    buf: crate::virtual_table::edit_common::EditBuffers,
}

impl EditState {
    fn just_activated(&self) -> bool {
        self.buf.just_activated
    }

    fn set_activated(&mut self, v: bool) {
        self.buf.just_activated = v;
    }

    fn activate(&mut self, node: NodeId, col: usize, value: &CellValue) {
        self.active = true;
        self.node = Some(node);
        self.col = col;
        self.buf.copy_from_value(value);
    }

    fn deactivate(&mut self) {
        self.active = false;
    }

    fn take_cell_value(&mut self, editor: &CellEditor) -> CellValue {
        self.buf.take_cell_value(editor)
    }

    #[inline]
    fn is_editing(&self, node: NodeId, col: usize) -> bool {
        self.active && self.node == Some(node) && self.col == col
    }
}

// ─── VirtualTree ────────────────────────────────────────────────────────────

/// Hierarchical tree-table widget with virtualization, inline editing,
/// drag-and-drop, and per-node icons.
///
/// Generic over `T: VirtualTreeNode` — your node data type.
///
/// # Per-frame output fields
///
/// After each `render()` call, check:
/// - `double_clicked_node` — NodeId if double-clicked this frame
/// - `button_clicked` — `(NodeId, col)` if a `CellEditor::Button` was clicked
/// - `context_node` / `context_col` — node/column of right-click
/// - `open_context_menu` — `true` when user right-clicked (reset after handling)
pub struct VirtualTree<T: VirtualTreeNode> {
    id: String,
    columns: Vec<ColumnDef>,
    /// Tree configuration. All fields are `pub` — modify freely between frames.
    pub config: TreeConfig,
    arena: TreeArena<T>,
    flat_view: FlatView,

    // Selection
    selected_nodes: NodeIdSet,
    /// Range-selection / keyboard-nav anchor, stored as a stable [`NodeId`]
    /// (not a flat-view index) so it survives expand/collapse/filter/sort
    /// rebuilds. Resolved to a flat-view row on demand.
    anchor: Option<NodeId>,
    /// Set to `Some(id)` when a node is double-clicked. Reset each frame.
    pub double_clicked_node: Option<NodeId>,
    /// Node of the last right-click (for context menu logic).
    pub context_node: Option<NodeId>,
    /// Column index of the last right-click (for per-column context menus).
    pub context_col: Option<usize>,
    /// `true` when the user right-clicked a node. Set to `false` after handling.
    pub open_context_menu: bool,
    /// Set to `Some((node, col))` when a `CellEditor::Button` is clicked. Reset each frame.
    pub button_clicked: Option<(NodeId, usize)>,

    // Internal state
    edit_state: EditState,
    sort_state: SortState,
    filter_state: FilterState,
    cell_buf: String,

    // Pending toggle (from ImGui TreeNode click — applied next frame)
    pending_toggle: Option<NodeId>,

    // Pending scroll-to-node (applied during next render)
    scroll_to_node: Option<NodeId>,

    /// Last completed drag-drop reparent: `(dragged_id, new_parent_id, position)`.
    ///
    /// Set when a drag-drop completes successfully during `render()`.
    /// The consumer should `take()` this after each render frame.
    pub last_reparent: Option<(NodeId, Option<NodeId>, usize)>,

    /// Set to `Some(text)` when **Ctrl+C** copies selected nodes this frame.
    /// Requires [`TreeConfig::table::copy_to_clipboard`] = `true`. Reset each frame.
    pub copied_text: Option<String>,

    /// On-demand children loader for `lazy_load` trees. Invoked the first time
    /// a branch node (`has_children() == true`) is expanded, to materialize its
    /// children. Runtime payload — never serialized. See [`set_lazy_loader`].
    ///
    /// [`set_lazy_loader`]: VirtualTree::set_lazy_loader
    lazy_loader: Option<LazyLoader<T>>,
}

impl<T: VirtualTreeNode> VirtualTree<T> {
    /// Create a new tree with the given columns and configuration.
    pub fn new(id: impl Into<String>, columns: Vec<ColumnDef>, config: TreeConfig) -> Self {
        let max_nodes = config.max_nodes;
        let evict = config.evict_on_overflow;
        Self {
            id: id.into(),
            columns,
            config,
            arena: {
                let mut a = TreeArena::with_capacity(max_nodes);
                a.set_evict_on_overflow(evict);
                a
            },
            flat_view: FlatView::new(),
            selected_nodes: NodeIdSet::default(),
            anchor: None,
            double_clicked_node: None,
            context_node: None,
            context_col: None,
            open_context_menu: false,
            button_clicked: None,
            edit_state: EditState::default(),
            sort_state: SortState::default(),
            filter_state: FilterState::new(),
            cell_buf: String::with_capacity(256),
            pending_toggle: None,
            scroll_to_node: None,
            last_reparent: None,
            copied_text: None,
            lazy_loader: None,
        }
    }

    // ─── Lazy loading ───────────────────────────────────────────────

    /// Register an on-demand children loader for `lazy_load` trees.
    ///
    /// The closure is invoked the first time a branch node (one whose
    /// [`VirtualTreeNode::has_children`] returns `true`) is expanded and has no
    /// materialized children yet. It receives the branch [`NodeId`] and returns
    /// the children to insert. Each branch is loaded at most once.
    ///
    /// Requires [`TreeConfig::lazy_load`] = `true`; otherwise the loader is
    /// never called and the tree behaves eagerly.
    pub fn set_lazy_loader(&mut self, loader: impl FnMut(NodeId) -> Vec<T> + 'static) {
        self.lazy_loader = Some(Box::new(loader));
    }

    /// Materialize a branch's children via the registered loader if the tree is
    /// in `lazy_load` mode and this branch has never been loaded. No-op
    /// otherwise. Safe to call on every expand — the `children_loaded` flag
    /// makes it idempotent.
    fn load_children_if_needed(&mut self, id: NodeId) {
        if !self.config.lazy_load {
            return;
        }
        let needs_load = self
            .arena
            .get(id)
            .is_some_and(|s| !s.children_loaded && s.children.is_empty() && s.data.has_children());
        if !needs_load {
            return;
        }
        // Take the loader out to avoid aliasing `&mut self` while it borrows the
        // arena for inserts, then restore it.
        if let Some(mut loader) = self.lazy_loader.take() {
            for child in loader(id) {
                self.arena.insert_child(id, child);
            }
            self.lazy_loader = Some(loader);
            if let Some(slot) = self.arena.slot_mut(id) {
                slot.children_loaded = true;
            }
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// Minimal node: a name + whether it is a branch (`has_children`).
    struct TNode {
        name: String,
        branch: bool,
    }
    impl TNode {
        fn leaf(name: &str) -> Self {
            Self {
                name: name.into(),
                branch: false,
            }
        }
        fn branch(name: &str) -> Self {
            Self {
                name: name.into(),
                branch: true,
            }
        }
    }
    impl VirtualTreeNode for TNode {
        fn cell_value(&self, _col: usize) -> CellValue {
            CellValue::Text(self.name.clone())
        }
        fn set_cell_value(&mut self, _col: usize, value: &CellValue) {
            if let CellValue::Text(s) = value {
                self.name = s.clone();
            }
        }
        fn has_children(&self) -> bool {
            self.branch
        }
    }

    fn one_col_tree(config: TreeConfig) -> VirtualTree<TNode> {
        VirtualTree::new("t", vec![ColumnDef::new("name")], config)
    }

    #[test]
    fn tree_config_theme_values_from_ron() {
        let cfg = TreeConfig::default();
        assert_eq!(cfg.striped_color, [1.0, 1.0, 1.0, 0.02]);
        assert_eq!(cfg.arrow_color, [0.65, 0.68, 0.72, 1.0]);
        assert_eq!(cfg.badge_color, [0.50, 0.55, 0.62, 1.0]);
    }

    #[test]
    fn tree_config_round_trips_through_ron() {
        let cfg = TreeConfig::default();
        let s = ron::to_string(&cfg).expect("serialize");
        let back: TreeConfig = ron::from_str(&s).expect("deserialize");
        assert_eq!(back.striped_color, cfg.striped_color);
        assert_eq!(back.arrow_color, cfg.arrow_color);
        assert_eq!(back.badge_color, cfg.badge_color);
    }

    #[test]
    fn tree_config_theme_colors_optional_in_ron() {
        // Simulate an older config.ron saved before the theme-colour fields
        // existed: strip those three lines and confirm it still parses, with the
        // `#[serde(default = ...)]` fallbacks supplying the values.
        let full = include_str!("config.ron");
        let old: String = full
            .lines()
            .filter(|l| {
                !l.contains("striped_color")
                    && !l.contains("arrow_color")
                    && !l.contains("badge_color")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let cfg: TreeConfig =
            ron::from_str(&old).expect("old ron without theme colours must parse");
        assert_eq!(cfg.striped_color, [1.0, 1.0, 1.0, 0.02]);
        assert_eq!(cfg.arrow_color, [0.65, 0.68, 0.72, 1.0]);
        assert_eq!(cfg.badge_color, [0.50, 0.55, 0.62, 1.0]);
    }

    #[test]
    fn tree_config_default_parses() {
        // Guards that the built-in `config.ron` stays valid (the `Default`
        // impl `.expect()`s on it) and pins a few headline values.
        let cfg = TreeConfig::default();
        assert_eq!(cfg.tree_column, 0);
        assert!(cfg.striped);
        assert!(!cfg.lazy_load);
        assert_eq!(cfg.indent_width, 20.0);
    }

    #[test]
    fn lazy_loader_materializes_children_once_on_expand() {
        let mut tree = one_col_tree(TreeConfig {
            lazy_load: true,
            ..Default::default()
        });
        let root = tree.insert_root(TNode::branch("root")).unwrap();

        let calls = Rc::new(Cell::new(0u32));
        let calls_c = calls.clone();
        tree.set_lazy_loader(move |_id| {
            calls_c.set(calls_c.get() + 1);
            vec![TNode::leaf("a"), TNode::leaf("b")]
        });

        assert_eq!(
            tree.children(root).len(),
            0,
            "children not loaded before expand"
        );
        tree.expand(root);
        assert_eq!(tree.children(root).len(), 2, "loader populated children");
        assert_eq!(calls.get(), 1);

        // Re-expanding must NOT reload (children_loaded latches).
        tree.collapse(root);
        tree.expand(root);
        assert_eq!(tree.children(root).len(), 2);
        assert_eq!(calls.get(), 1, "loader invoked exactly once per branch");
    }

    #[test]
    fn lazy_loader_inert_when_flag_disabled() {
        let mut tree = one_col_tree(TreeConfig::default()); // lazy_load = false
        let root = tree.insert_root(TNode::branch("root")).unwrap();
        tree.set_lazy_loader(|_id| vec![TNode::leaf("x")]);
        tree.expand(root);
        assert_eq!(
            tree.children(root).len(),
            0,
            "no load when lazy_load is off"
        );
    }

    #[test]
    fn copy_includes_collapsed_subtree() {
        let mut tree = one_col_tree(TreeConfig::default());
        let root = tree.insert_root(TNode::branch("root")).unwrap();
        tree.insert_child(root, TNode::leaf("a")).unwrap();
        tree.insert_child(root, TNode::leaf("b")).unwrap();
        // root is collapsed → its children are absent from the flat view.
        tree.flat_view.rebuild(&tree.arena, &tree.filter_state);
        tree.select(root);

        let text = tree.build_copy_text();
        assert!(text.contains("root"));
        assert!(
            text.contains('a'),
            "collapsed child 'a' must still copy: {text:?}"
        );
        assert!(
            text.contains('b'),
            "collapsed child 'b' must still copy: {text:?}"
        );
    }

    #[test]
    fn copy_leaf_only_selection() {
        let mut tree = one_col_tree(TreeConfig::default());
        let root = tree.insert_root(TNode::branch("root")).unwrap();
        let a = tree.insert_child(root, TNode::leaf("alpha")).unwrap();
        tree.insert_child(root, TNode::leaf("beta")).unwrap();
        tree.select(a);

        let text = tree.build_copy_text();
        assert!(text.contains("alpha"));
        assert!(
            !text.contains("beta"),
            "unselected sibling must not copy: {text:?}"
        );
        assert!(
            !text.contains("root"),
            "unselected parent must not copy: {text:?}"
        );
    }

    #[test]
    fn copy_no_duplicate_when_ancestor_selected() {
        let mut tree = one_col_tree(TreeConfig::default());
        let root = tree.insert_root(TNode::branch("root")).unwrap();
        let child = tree.insert_child(root, TNode::leaf("child")).unwrap();
        tree.select(root);
        tree.select(child); // both selected — child is covered by root's subtree

        let text = tree.build_copy_text();
        assert_eq!(
            text.matches("child").count(),
            1,
            "no duplicate emission: {text:?}"
        );
    }

    #[test]
    fn edit_state_is_keyed_by_node_not_row() {
        let mut es = EditState::default();
        let n1 = NodeId {
            index: 1,
            generation: 0,
        };
        let n2 = NodeId {
            index: 2,
            generation: 0,
        };
        es.activate(n1, 0, &CellValue::Text("x".into()));
        assert!(es.is_editing(n1, 0));
        assert!(!es.is_editing(n2, 0), "different node must not match");
        assert!(!es.is_editing(n1, 1), "different column must not match");
        es.deactivate();
        assert!(!es.is_editing(n1, 0));
    }

    #[test]
    fn anchor_survives_flat_view_rebuild() {
        // Regression for stale-index anchors: the anchor is a NodeId, so after
        // structural change the same node still resolves to its (new) row.
        let mut tree = one_col_tree(TreeConfig::default());
        let r0 = tree.insert_root(TNode::leaf("r0")).unwrap();
        let r1 = tree.insert_root(TNode::leaf("r1")).unwrap();
        tree.select(r1);
        tree.anchor = Some(r1);

        // Insert a new root at the front → r1 shifts down by one row.
        tree.insert_root_at(0, TNode::leaf("new")).unwrap();
        tree.flat_view.rebuild(&tree.arena, &tree.filter_state);

        let r1_row = tree.flat_view.index_of(r1).unwrap();
        let anchor_row = tree.anchor.and_then(|n| tree.flat_view.index_of(n));
        assert_eq!(anchor_row, Some(r1_row), "anchor still tracks its node");
        let _ = (r0,);
    }
}
