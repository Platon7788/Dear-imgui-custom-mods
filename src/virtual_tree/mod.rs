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
//! ├── mod.rs        VirtualTree<T> widget, render loop, public API
//! ├── arena.rs      TreeArena<T> — slab storage with NodeId, parent/children
//! ├── node.rs       VirtualTreeNode trait, NodeIcon
//! ├── config.rs     TreeConfig (wraps TableConfig)
//! ├── flat_view.rs  FlatView — cached linearization for ListClipper
//! ├── sort.rs       Sibling-scoped sort state
//! ├── filter.rs     FilterState — search with auto-expand
//! └── drag.rs       DragDropState for node reparenting
//! ```

pub mod arena;
pub mod config;
mod drag;
pub mod filter;
pub mod flat_view;
pub mod node;
mod sort;

pub use arena::{NodeId, NodeSlot, MAX_TREE_NODES};
pub use config::{ExpandStyle, TreeConfig};
pub use filter::FilterState;
pub use flat_view::{FlatRow, FlatView};
pub use node::{NodeIcon, VirtualTreeNode};

// Re-export shared types from virtual_table
pub use crate::virtual_table::column::{CellAlignment, CellEditor, ColumnDef, ColumnSizing};
pub use crate::virtual_table::config::{EditTrigger, RowDensity, SelectionMode};
pub use crate::virtual_table::row::{CellStyle, CellValue, RowStyle};

use crate::utils::text::calc_text_size;
use dear_imgui_rs::{
    Key, ListClipper, MouseButton, SelectableFlags, TableBgTarget,
    TableRowFlags, Ui,
};

use std::collections::HashSet;

use arena::TreeArena;
use drag::DragDropState;
use sort::{SortSpec, SortState};

/// Fast hash set for NodeId. Uses `foldhash` for O(1) operations.
type NodeIdSet = HashSet<NodeId, foldhash::fast::FixedState>;

// ─── EditState (inline, mirrors virtual_table::edit) ────────────────────────

/// Tracks the currently active inline editor.
/// `row` is the flat-view index, resolved to NodeId on commit.
#[derive(Clone, Debug, Default)]
struct EditState {
    active: bool,
    row: usize,   // flat-view index
    col: usize,
    just_activated: bool,
    frames_active: u32,
    text_buf: String,
    bool_val: bool,
    int_val: i32,
    float_val: f32,
    choice_idx: usize,
    color_val: [f32; 4],
}

impl EditState {
    fn activate(&mut self, row: usize, col: usize, value: &CellValue) {
        self.active = true;
        self.row = row;
        self.col = col;
        self.just_activated = true;
        self.frames_active = 0;
        match value {
            CellValue::Text(s) => { self.text_buf.clear(); self.text_buf.push_str(s); }
            CellValue::Bool(b) => self.bool_val = *b,
            CellValue::Int(v) => self.int_val = (*v).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            CellValue::Float(v) => self.float_val = (*v as f32).clamp(f32::MIN, f32::MAX),
            CellValue::Choice(idx) => self.choice_idx = *idx,
            CellValue::Color(c) => self.color_val = *c,
            CellValue::Progress(_) | CellValue::Custom => {}
        }
    }

    fn deactivate(&mut self) { self.active = false; }

    fn take_cell_value(&mut self, editor: &CellEditor) -> CellValue {
        match editor {
            CellEditor::None | CellEditor::TextInput => {
                // Move string out instead of cloning — zero-copy commit.
                let text = std::mem::replace(&mut self.text_buf, String::with_capacity(256));
                CellValue::Text(text)
            }
            CellEditor::Checkbox => CellValue::Bool(self.bool_val),
            CellEditor::ComboBox { .. } => CellValue::Choice(self.choice_idx),
            CellEditor::SliderInt { .. } | CellEditor::SpinInt { .. } => CellValue::Int(self.int_val as i64),
            CellEditor::SliderFloat { .. } | CellEditor::SpinFloat { .. } => CellValue::Float(self.float_val as f64),
            CellEditor::ColorEdit => CellValue::Color(self.color_val),
            CellEditor::ProgressBar => CellValue::Progress(self.float_val),
            CellEditor::Button { .. } | CellEditor::Custom => CellValue::Custom,
        }
    }

    #[inline]
    fn is_editing(&self, row: usize, col: usize) -> bool {
        self.active && self.row == row && self.col == col
    }
}

// ─── EditorKind ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum EditorKind {
    None, Checkbox, ComboBox, Button, ProgressBar, ColorEdit, Custom, Other,
}

fn editor_kind(e: &CellEditor) -> EditorKind {
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

fn alignment_pad(alignment: CellAlignment, col_w: f32, text_w: f32) -> f32 {
    match alignment {
        CellAlignment::Left => 0.0,
        CellAlignment::Center => ((col_w - text_w) * 0.5).max(0.0),
        CellAlignment::Right => (col_w - text_w - 4.0).max(0.0),
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
    pub config: TreeConfig,
    arena: TreeArena<T>,
    flat_view: FlatView,

    // Selection
    selected_nodes: NodeIdSet,
    selection_anchor: Option<usize>, // flat-view index
    pub double_clicked_node: Option<NodeId>,
    pub context_node: Option<NodeId>,
    pub context_col: Option<usize>,
    pub open_context_menu: bool,
    pub button_clicked: Option<(NodeId, usize)>,

    // Internal state
    edit_state: EditState,
    sort_state: SortState,
    drag_state: DragDropState,
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
}

impl<T: VirtualTreeNode> VirtualTree<T> {
    /// Create a new tree with the given columns and configuration.
    pub fn new(
        id: impl Into<String>,
        columns: Vec<ColumnDef>,
        config: TreeConfig,
    ) -> Self {
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
            selection_anchor: None,
            double_clicked_node: None,
            context_node: None,
            context_col: None,
            open_context_menu: false,
            button_clicked: None,
            edit_state: EditState::default(),
            sort_state: SortState::default(),
            drag_state: DragDropState::new(),
            filter_state: FilterState::new(),
            cell_buf: String::with_capacity(256),
            pending_toggle: None,
            scroll_to_node: None,
            last_reparent: None,
        }
    }

    // ─── Node insertion ─────────────────────────────────────────────

    /// Insert a root node at the end of the root list.
    /// Returns `None` if the tree is at capacity ([`MAX_TREE_NODES`]).
    pub fn insert_root(&mut self, data: T) -> Option<NodeId> {
        let id = self.arena.insert_root(data)?;
        self.flat_view.mark_dirty();
        Some(id)
    }

    /// Insert a root node at a specific position.
    /// Returns `None` if the tree is at capacity ([`MAX_TREE_NODES`]).
    pub fn insert_root_at(&mut self, index: usize, data: T) -> Option<NodeId> {
        let id = self.arena.insert_root_at(index, data)?;
        self.flat_view.mark_dirty();
        Some(id)
    }

    /// Insert a child node at the end of parent's children.
    pub fn insert_child(&mut self, parent: NodeId, data: T) -> Option<NodeId> {
        let id = self.arena.insert_child(parent, data)?;
        self.flat_view.mark_dirty();
        Some(id)
    }

    /// Insert a child node at a specific position among siblings.
    pub fn insert_child_at(&mut self, parent: NodeId, index: usize, data: T) -> Option<NodeId> {
        let id = self.arena.insert_child_at(parent, index, data)?;
        self.flat_view.mark_dirty();
        Some(id)
    }

    // ─── Node removal ───────────────────────────────────────────────

    /// Remove a node and all descendants. Returns the removed node's data.
    pub fn remove(&mut self, id: NodeId) -> Option<T> {
        self.edit_state.deactivate();
        self.selected_nodes.remove(&id);
        // Remove any selected descendants without allocating a result vec.
        self.deselect_descendants(id);
        let data = self.arena.remove(id)?;
        self.flat_view.mark_dirty();
        Some(data)
    }

    /// Remove all nodes.
    pub fn clear(&mut self) {
        self.arena.clear();
        self.selected_nodes.clear();
        self.selection_anchor = None;
        self.edit_state.deactivate();
        self.flat_view.mark_dirty();
    }

    // ─── Node access ────────────────────────────────────────────────

    /// Get a reference to node data.
    #[inline]
    pub fn get(&self, id: NodeId) -> Option<&T> {
        self.arena.get_data(id)
    }

    /// Get a mutable reference to node data.
    #[inline]
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut T> {
        self.arena.get_data_mut(id)
    }

    /// Number of live nodes in the tree.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.arena.node_count()
    }

    /// Current capacity limit.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.arena.capacity()
    }

    /// Set a new capacity limit (clamped to `1..=MAX_TREE_NODES`).
    /// Does **not** evict existing nodes if count already exceeds the new limit.
    pub fn set_capacity(&mut self, capacity: usize) {
        self.arena.set_capacity(capacity);
    }

    /// Enable or disable automatic eviction of the oldest root subtree on overflow.
    pub fn set_evict_on_overflow(&mut self, enabled: bool) {
        self.arena.set_evict_on_overflow(enabled);
    }

    /// Whether eviction-on-overflow is enabled.
    #[inline]
    pub fn evict_on_overflow(&self) -> bool {
        self.arena.evict_on_overflow()
    }

    /// Parent of a node.
    #[inline]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.arena.parent(id)
    }

    /// Children of a node.
    #[inline]
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.arena.children(id)
    }

    /// Top-level root nodes.
    #[inline]
    pub fn roots(&self) -> &[NodeId] {
        self.arena.roots()
    }

    /// Cached depth of a node (0 = root).
    #[inline]
    pub fn depth(&self, id: NodeId) -> Option<u16> {
        self.arena.depth(id)
    }

    /// Whether a node is expanded.
    #[inline]
    pub fn is_expanded(&self, id: NodeId) -> bool {
        self.arena.is_expanded(id)
    }

    /// Access the underlying arena (for advanced iteration).
    pub fn arena(&self) -> &TreeArena<T> {
        &self.arena
    }

    // ─── Expand / Collapse ──────────────────────────────────────────

    pub fn expand(&mut self, id: NodeId) {
        self.arena.expand(id);
        self.flat_view.mark_dirty();
    }

    pub fn collapse(&mut self, id: NodeId) {
        self.arena.collapse(id);
        self.flat_view.mark_dirty();
    }

    pub fn toggle(&mut self, id: NodeId) {
        self.arena.toggle(id);
        self.flat_view.mark_dirty();
    }

    pub fn expand_all(&mut self) {
        self.arena.expand_all();
        self.flat_view.mark_dirty();
    }

    pub fn collapse_all(&mut self) {
        self.arena.collapse_all();
        self.flat_view.mark_dirty();
    }

    /// Expand all ancestors so that `id` becomes visible.
    pub fn ensure_visible(&mut self, id: NodeId) {
        self.arena.ensure_visible(id);
        self.flat_view.mark_dirty();
    }

    /// Expand ancestors + scroll to the node on next render.
    pub fn scroll_to_node(&mut self, id: NodeId) {
        self.arena.ensure_visible(id);
        self.flat_view.mark_dirty();
        self.scroll_to_node = Some(id);
    }

    /// Number of direct children of a node.
    pub fn children_count(&self, id: NodeId) -> usize {
        self.arena.children(id).len()
    }

    // ─── Move / Reparent ────────────────────────────────────────────

    /// Move a node to a new parent at position. Pass `None` to make root.
    pub fn move_node(&mut self, id: NodeId, new_parent: Option<NodeId>, position: usize) -> bool {
        let ok = self.arena.move_node(id, new_parent, position);
        if ok {
            self.flat_view.mark_dirty();
        }
        ok
    }

    // ─── Selection ──────────────────────────────────────────────────

    pub fn selected_nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.selected_nodes.iter().copied()
    }

    pub fn selected_count(&self) -> usize {
        self.selected_nodes.len()
    }

    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selected_nodes.contains(&id)
    }

    pub fn select(&mut self, id: NodeId) {
        self.selected_nodes.insert(id);
    }

    pub fn deselect(&mut self, id: NodeId) {
        self.selected_nodes.remove(&id);
    }

    pub fn clear_selection(&mut self) {
        self.selected_nodes.clear();
        self.selection_anchor = None;
    }

    /// Convenience for single-select: returns the one selected node.
    pub fn selected_node(&self) -> Option<NodeId> {
        self.selected_nodes.iter().next().copied()
    }

    // ─── Sorting ────────────────────────────────────────────────────

    /// Sort children of a specific parent (or roots if None).
    pub fn sort_children(&mut self, parent: Option<NodeId>, col: usize, ascending: bool) {
        let mut cmp = |a: &T, b: &T| {
            let ord = a.compare(b, col);
            if ascending { ord } else { ord.reverse() }
        };
        self.arena.sort_children(parent, &mut cmp);
        self.flat_view.mark_dirty();
    }

    // ─── Filter ─────────────────────────────────────────────────────

    pub fn set_filter(&mut self, query: &str) {
        self.filter_state.set_filter(query, &mut self.arena, self.config.auto_expand_on_filter);
        self.flat_view.mark_dirty();
    }

    pub fn clear_filter(&mut self) {
        self.filter_state.clear();
        self.flat_view.mark_dirty();
    }

    pub fn is_filtered(&self) -> bool {
        self.filter_state.active
    }

    // ─── Column access ──────────────────────────────────────────────

    pub fn columns(&self) -> &[ColumnDef] {
        &self.columns
    }

    pub fn columns_mut(&mut self) -> &mut [ColumnDef] {
        &mut self.columns
    }

    // ─── Flat view queries ──────────────────────────────────────────

    /// Number of visible (flattened) rows.
    pub fn flat_row_count(&self) -> usize {
        self.flat_view.len()
    }

    /// Find the flat-view index of a node.
    pub fn flat_index_of(&self, id: NodeId) -> Option<usize> {
        self.flat_view.index_of(id)
    }

    // ─── Editing ────────────────────────────────────────────────────

    pub fn is_editing(&self) -> bool {
        self.edit_state.active
    }

    pub fn cancel_edit(&mut self) {
        self.edit_state.deactivate();
    }

    // ─── Internal helpers ───────────────────────────────────────────

    /// Remove all descendants of `id` from selected_nodes set.
    /// Uses iterative DFS without allocating a result vec — directly removes from set.
    fn deselect_descendants(&mut self, id: NodeId) {
        // Fast path: if nothing is selected, skip traversal.
        if self.selected_nodes.is_empty() {
            return;
        }
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            for &child in self.arena.children(current) {
                self.selected_nodes.remove(&child);
                stack.push(child);
            }
        }
    }

    // ─── Render ─────────────────────────────────────────────────────

    /// Render the tree. Call once per frame inside an ImGui window.
    pub fn render(&mut self, ui: &Ui) {
        // Apply pending toggle from previous frame
        if let Some(id) = self.pending_toggle.take() {
            self.toggle(id);
        }

        // Reset per-frame outputs
        self.double_clicked_node = None;
        self.button_clicked = None;
        self.last_reparent = None;

        // Rebuild flat view if dirty
        if self.flat_view.dirty {
            self.flat_view.rebuild(&self.arena, &self.filter_state);
        }

        let col_count = self.columns.len();
        if col_count == 0 {
            return;
        }

        let flags = self.config.table.to_table_flags();
        let _table = match ui.begin_table_with_flags(&self.id, col_count, flags) {
            Some(t) => t,
            None => return,
        };

        // Column setup
        for i in 0..col_count {
            let col = &self.columns[i];
            ui.table_setup_column(
                &col.name,
                col.imgui_flags(),
                col.init_width_or_weight(),
                col.user_id.max(i as u32),
            );
            if !col.visible {
                ui.table_set_column_enabled(i as i32, false);
            }
        }

        ui.table_setup_scroll_freeze(
            self.config.table.freeze_cols,
            self.config.table.freeze_rows,
        );

        // Header
        self.render_header(ui);

        // Sort
        self.handle_sort(ui);

        // Rows via ListClipper
        let row_count = self.flat_view.len();
        let clip = ListClipper::new(row_count as i32);
        let tok = clip.begin(ui);

        let scroll_target = self.scroll_to_node.take()
            .and_then(|id| self.flat_view.index_of(id));

        for flat_idx in tok.iter() {
            let idx = flat_idx as usize;
            self.render_row(ui, idx);

            // Scroll to target node
            if scroll_target == Some(idx) {
                unsafe { dear_imgui_rs::sys::igSetScrollHereY(0.5) };
            }
        }

        // Keyboard navigation
        self.handle_keyboard(ui);
    }

    // ─── Internal: header ───────────────────────────────────────────

    fn render_header(&self, ui: &Ui) {
        ui.table_next_row_with_flags(TableRowFlags::HEADERS, 0.0);
        for i in 0..self.columns.len() {
            if !ui.table_set_column_index(i as i32) {
                continue;
            }
            let col = &self.columns[i];
            let col_w = ui.current_column_width();
            let text_w = calc_text_size(&col.name)[0];
            let pad = alignment_pad(col.header_alignment, col_w, text_w);
            if pad > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0] + pad, cursor[1]]);
            }
            ui.table_header(&col.name);
        }
    }

    // ─── Internal: sort ─────────────────────────────────────────────

    fn handle_sort(&mut self, ui: &Ui) {
        if !self.config.table.sortable {
            return;
        }
        if let Some(mut specs) = ui.table_get_sort_specs()
            && specs.is_dirty()
        {
            self.sort_state.specs.clear();
            for s in specs.iter() {
                self.sort_state.specs.push(SortSpec {
                    column_index: s.column_index as usize,
                    ascending: s.sort_direction
                        == dear_imgui_rs::SortDirection::Ascending,
                });
            }
            specs.clear_dirty();

            self.sort_state.sort_all(&mut self.arena);
            self.flat_view.mark_dirty();
            self.edit_state.deactivate();
        }
    }

    // ─── Internal: row rendering ────────────────────────────────────

    fn render_row(&mut self, ui: &Ui, flat_idx: usize) {
        let flat_row = match self.flat_view.rows.get(flat_idx) {
            Some(r) => *r,
            None => return,
        };
        let node_id = flat_row.node_id;

        // Extract row-level data
        let row_style = self.arena.get_data(node_id).and_then(|d| d.row_style());

        // Row height
        let auto_h = unsafe {
            match self.config.table.row_density {
                RowDensity::Normal => dear_imgui_rs::sys::igGetFrameHeightWithSpacing(),
                RowDensity::Compact => dear_imgui_rs::sys::igGetFrameHeight() + 2.0,
                RowDensity::Dense => dear_imgui_rs::sys::igGetFontSize() + 2.0,
            }
        };
        let row_height = row_style
            .as_ref()
            .and_then(|s| s.height)
            .or(self.config.table.default_row_height)
            .unwrap_or(auto_h);

        ui.table_next_row_with_flags(TableRowFlags::NONE, row_height);

        // Row background (striped + custom)
        if let Some(ref style) = row_style
            && let Some(bg) = style.bg_color
        {
            ui.table_set_bg_color(TableBgTarget::RowBg1, bg, -1);
        } else if self.config.striped && flat_idx % 2 == 1 {
            ui.table_set_bg_color(
                TableBgTarget::RowBg1,
                [1.0, 1.0, 1.0, 0.02],
                -1,
            );
        }

        let is_selected = self.selected_nodes.contains(&node_id);

        let _row_id = ui.push_id(flat_idx);

        // Selectable spanning all columns
        ui.table_next_column();
        if ui
            .selectable_config("##sel")
            .flags(
                SelectableFlags::ALLOW_DOUBLE_CLICK
                    | SelectableFlags::SPAN_ALL_COLUMNS
                    | SelectableFlags::ALLOW_OVERLAP,
            )
            .selected(is_selected)
            .size([0.0, row_height])
            .build()
        {
            self.handle_selection(ui, flat_idx);

            if ui.is_mouse_double_clicked(MouseButton::Left) {
                self.double_clicked_node = Some(node_id);
                if self.config.expand_on_double_click && !flat_row.is_leaf {
                    self.pending_toggle = Some(node_id);
                }
            }

            // Edit trigger
            let activate_edit = match self.config.table.edit_trigger {
                EditTrigger::DoubleClick => ui.is_mouse_double_clicked(MouseButton::Left),
                EditTrigger::SingleClick => true,
                _ => false,
            };
            if activate_edit {
                let hovered_col = ui.table_get_hovered_column();
                if hovered_col >= 0 {
                    self.try_activate_edit(flat_idx, hovered_col as usize);
                }
            }
        }

        // Tooltip
        if ui.is_item_hovered() {
            if let Some(data) = self.arena.get_data(node_id) {
                if !data.render_tooltip(ui) {
                    self.cell_buf.clear();
                    data.row_tooltip(&mut self.cell_buf);
                    if !self.cell_buf.is_empty() {
                        ui.tooltip_text(&self.cell_buf);
                    }
                }
            }
        }

        // Context menu
        if ui.is_item_hovered() && ui.is_mouse_clicked(MouseButton::Right) {
            self.handle_selection(ui, flat_idx);
            self.context_node = Some(node_id);
            let hovered = ui.table_get_hovered_column();
            self.context_col = if hovered >= 0 { Some(hovered as usize) } else { None };
            self.open_context_menu = true;
        }

        // ── Drag-and-drop ───────────────────────────────────────────
        if self.config.drag_drop_enabled {
            // Drag source
            let is_draggable = self.arena.get_data(node_id).map_or(false, |d| d.is_draggable());
            if is_draggable {
                if let Some(tooltip) = ui.drag_drop_source_config(drag::DRAG_DROP_TYPE)
                    .begin_payload(node_id)
                {
                    self.drag_state.dragging = Some(node_id);
                    // Show node name as drag tooltip
                    if let Some(data) = self.arena.get_data(node_id) {
                        self.cell_buf.clear();
                        data.cell_display_text(self.config.tree_column, &mut self.cell_buf);
                        ui.text(&self.cell_buf);
                    }
                    tooltip.end();
                }
            }

            // Drop target
            if let Some(target) = ui.drag_drop_target() {
                if let Some(Ok(payload)) = target.accept_payload::<NodeId, _>(
                    drag::DRAG_DROP_TYPE,
                    dear_imgui_rs::DragDropFlags::NONE,
                ) {
                    if payload.delivery {
                        let dragged_id = payload.data;
                        // Check if target accepts this drop
                        let accepted = self.arena.get_data(node_id)
                            .and_then(|target_data| {
                                self.arena.get_data(dragged_id)
                                    .map(|dragged_data| target_data.accepts_drop(dragged_data))
                            })
                            .unwrap_or(false);

                        if accepted {
                            // Move dragged node as child of target
                            let pos = self.arena.children(node_id).len();
                            self.arena.move_node(dragged_id, Some(node_id), pos);
                            self.arena.expand(node_id);
                            self.flat_view.mark_dirty();
                            // Record event for consumers
                            self.last_reparent = Some((dragged_id, Some(node_id), pos));
                        }
                    }
                }
                target.pop();
            }
        }

        // ── Render cells ────────────────────────────────────────────
        let row_text_color = row_style.as_ref().and_then(|s| s.text_color);
        let col_count = self.columns.len();
        let tree_col = self.config.tree_column.min(col_count.saturating_sub(1));

        let widget_h = unsafe { dear_imgui_rs::sys::igGetFrameHeight() };
        let vert_offset = ((row_height - widget_h) * 0.5).max(0.0);

        for col_idx in 0..col_count {
            if col_idx == 0 {
                ui.same_line_with_spacing(0.0, 0.0);
            } else {
                ui.table_next_column();
            }

            if vert_offset > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0], cursor[1] + vert_offset]);
            }

            let _cell_id = ui.push_id(col_idx);

            // Tree column: indent + expand arrow + icon + text
            if col_idx == tree_col {
                self.render_tree_cell(ui, flat_idx, &flat_row, node_id, row_text_color);
                continue;
            }

            // Non-tree column: same as VirtualTable
            if self.edit_state.is_editing(flat_idx, col_idx) {
                self.render_editor_inline(ui, flat_idx, col_idx, node_id);
                continue;
            }

            self.render_data_cell(ui, node_id, col_idx, row_text_color);
        }
    }

    // ─── Internal: tree cell ────────────────────────────────────────

    fn render_tree_cell(
        &mut self,
        ui: &Ui,
        flat_idx: usize,
        flat_row: &flat_view::FlatRow,
        node_id: NodeId,
        row_text_color: Option<[f32; 4]>,
    ) {
        let indent = flat_row.depth as f32 * self.config.indent_width;
        let tree_col = self.config.tree_column.min(self.columns.len().saturating_sub(1));
        let indent_w = self.config.indent_width;

        // ── Tree lines (vertical + horizontal connectors) ────────────
        if self.config.show_tree_lines && flat_row.depth > 0 {
            let draw_list = ui.get_window_draw_list();
            let cursor_screen = ui.cursor_screen_pos();
            let row_h = unsafe { dear_imgui_rs::sys::igGetFrameHeightWithSpacing() };
            let line_color = crate::utils::color::rgba_f32(
                self.config.tree_line_color[0],
                self.config.tree_line_color[1],
                self.config.tree_line_color[2],
                self.config.tree_line_color[3],
            );

            // Vertical continuation lines at ancestor depths
            for d in 1..flat_row.depth {
                if flat_row.continuation_mask & (1u64 << d) != 0 {
                    let x = cursor_screen[0] + (d as f32) * indent_w + indent_w * 0.5;
                    draw_list.add_line(
                        [x, cursor_screen[1]],
                        [x, cursor_screen[1] + row_h],
                        line_color,
                    ).build();
                }
            }

            // This node's connector: vertical stub + horizontal branch
            let x = cursor_screen[0] + (flat_row.depth as f32) * indent_w + indent_w * 0.5;
            let mid_y = cursor_screen[1] + row_h * 0.5;

            // Vertical stub: from top of row to mid-y (or full if not last child)
            let vert_end = if flat_row.is_last_child { mid_y } else { cursor_screen[1] + row_h };
            draw_list.add_line(
                [x, cursor_screen[1]],
                [x, vert_end],
                line_color,
            ).build();

            // Horizontal branch: from vertical line to arrow/icon
            let arrow_space = unsafe { dear_imgui_rs::sys::igGetTreeNodeToLabelSpacing() };
            let h_end = cursor_screen[0] + indent + if flat_row.is_leaf { arrow_space * 0.5 } else { 0.0 };
            draw_list.add_line(
                [x, mid_y],
                [h_end, mid_y],
                line_color,
            ).build();
        }

        // ── Editing the tree column? ────────────────────────────────
        if self.edit_state.is_editing(flat_idx, tree_col) {
            if indent > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0] + indent, cursor[1]]);
            }
            self.render_editor_inline(ui, flat_idx, tree_col, node_id);
            return;
        }

        if flat_row.is_leaf {
            // Leaf: indent + (arrow space) + icon + text
            let arrow_width = unsafe { dear_imgui_rs::sys::igGetTreeNodeToLabelSpacing() };
            let total_indent = indent + arrow_width;
            if total_indent > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0] + total_indent, cursor[1]]);
            }
        } else {
            match &self.config.expand_style {
                config::ExpandStyle::Glyph { collapsed, expanded, color } => {
                    // Custom glyph expand/collapse indicator
                    let glyph = if flat_row.is_expanded { *expanded } else { *collapsed };
                    let glyph_color = *color;

                    // Indent
                    if indent > 0.0 {
                        let cursor = ui.cursor_pos();
                        ui.set_cursor_pos([cursor[0] + indent, cursor[1]]);
                    }

                    // Render glyph as clickable button (zero allocation)
                    self.cell_buf.clear();
                    self.cell_buf.push(glyph);
                    let font_size = unsafe { dear_imgui_rs::sys::igGetFontSize() };
                    let glyph_sz = crate::utils::text::calc_text_size(&self.cell_buf);
                    let btn_w = glyph_sz[0].max(font_size);

                    // Write button ID into tail of cell_buf to avoid format! allocation.
                    let glyph_len = self.cell_buf.len();
                    let _ = std::fmt::Write::write_fmt(
                        &mut self.cell_buf,
                        format_args!("##xp{}", flat_idx),
                    );
                    // SAFETY: btn_id borrows from cell_buf tail; cell_buf is not mutated
                    // until after invisible_button returns.
                    let btn_id_ptr = self.cell_buf[glyph_len..].as_ptr();
                    let btn_id_len = self.cell_buf.len() - glyph_len;
                    let btn_id = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(btn_id_ptr, btn_id_len)) };

                    if ui.invisible_button(btn_id, [btn_w, font_size]) {
                        self.pending_toggle = Some(node_id);
                    }
                    // Draw glyph over the invisible button (use only the glyph portion)
                    let btn_min = ui.item_rect_min();
                    let draw_list = ui.get_window_draw_list();
                    let glyph_x = btn_min[0] + (btn_w - glyph_sz[0]) * 0.5;
                    let glyph_y = btn_min[1];
                    let c = glyph_color.unwrap_or(row_text_color
                        .unwrap_or([0.85, 0.88, 0.92, 1.0]));
                    let color_u32 = crate::utils::color::rgba_f32(c[0], c[1], c[2], c[3]);
                    draw_list.add_text([glyph_x, glyph_y], color_u32, &self.cell_buf[..glyph_len]);

                    ui.same_line_with_spacing(0.0, 4.0);
                }
                config::ExpandStyle::Arrow => {
                    // Standard ImGui TreeNode arrow
                    if indent > 0.0 {
                        let cursor = ui.cursor_pos();
                        ui.set_cursor_pos([cursor[0] + indent, cursor[1]]);
                    }

                    self.cell_buf.clear();
                    let _ = std::fmt::Write::write_fmt(
                        &mut self.cell_buf,
                        format_args!("##tn{}", flat_idx),
                    );
                    let node = ui.tree_node_config(&self.cell_buf)
                        .open_on_arrow(true)
                        .span_full_width(true)
                        .no_tree_push_on_open(true)
                        .frame_padding(true)
                        .default_open(flat_row.is_expanded);
                    let token = node.push();
                    // NO_TREE_PUSH_ON_OPEN means TreePush was NOT called,
                    // so we must NOT TreePop.
                    if let Some(t) = token {
                        std::mem::forget(t);
                    }

                    if ui.is_item_toggled_open() {
                        self.pending_toggle = Some(node_id);
                    }

                    ui.same_line_with_spacing(0.0, 0.0);
                }
            }
        }

        // ── Render icon ─────────────────────────────────────────────
        if let Some(data) = self.arena.get_data(node_id) {
            match data.icon() {
                NodeIcon::None => {}
                NodeIcon::Glyph(ch) => {
                    self.cell_buf.clear();
                    self.cell_buf.push(ch);
                    ui.text(&self.cell_buf);
                    ui.same_line_with_spacing(0.0, 4.0);
                }
                NodeIcon::GlyphColored(ch, color) => {
                    self.cell_buf.clear();
                    self.cell_buf.push(ch);
                    ui.text_colored(color, &self.cell_buf);
                    ui.same_line_with_spacing(0.0, 4.0);
                }
                NodeIcon::ColorSwatch(c) => {
                    let size = unsafe { dear_imgui_rs::sys::igGetFontSize() };
                    let cursor_screen = ui.cursor_screen_pos();
                    let draw_list = ui.get_window_draw_list();
                    let color = crate::utils::color::rgba_f32(c[0], c[1], c[2], c[3]);
                    draw_list
                        .add_rect(
                            cursor_screen,
                            [cursor_screen[0] + size, cursor_screen[1] + size],
                            color,
                        )
                        .filled(true)
                        .build();
                    ui.dummy([size, size]);
                    ui.same_line_with_spacing(0.0, 4.0);
                }
                NodeIcon::Custom => {
                    data.render_icon(ui);
                    ui.same_line_with_spacing(0.0, 4.0);
                }
            }
        }

        // ── Render text + badge ─────────────────────────────────────
        if let Some(data) = self.arena.get_data(node_id) {
            self.cell_buf.clear();
            data.cell_display_text(self.config.tree_column, &mut self.cell_buf);

            let color = data.cell_style(self.config.tree_column)
                .and_then(|s| s.text_color)
                .or(row_text_color);

            if let Some(c) = color {
                ui.text_colored(c, &self.cell_buf);
            } else {
                ui.text(&self.cell_buf);
            }

            // Clip tooltip for tree cell text
            if self.columns[self.config.tree_column].clip_tooltip
                && !self.cell_buf.is_empty()
                && ui.is_item_hovered()
            {
                let col_w = ui.current_column_width();
                let text_w = calc_text_size(&self.cell_buf)[0];
                // Account for indent + arrow + icon width
                let arrow_width = unsafe { dear_imgui_rs::sys::igGetTreeNodeToLabelSpacing() };
                let used_w = indent + arrow_width + 20.0; // approximate icon + spacing
                if text_w + used_w > col_w {
                    ui.tooltip_text(&self.cell_buf);
                }
            }

            // Badge (e.g. children count)
            let badge = data.badge();
            if !badge.is_empty() {
                ui.same_line_with_spacing(0.0, 6.0);
                ui.text_colored([0.50, 0.55, 0.62, 1.0], badge);
            }
        }
    }

    // ─── Internal: data cell (non-tree) ─────────────────────────────

    fn render_data_cell(
        &mut self,
        ui: &Ui,
        node_id: NodeId,
        col_idx: usize,
        row_text_color: Option<[f32; 4]>,
    ) {
        let ek = editor_kind(&self.columns[col_idx].editor);

        match ek {
            EditorKind::Checkbox => {
                if let Some(data) = self.arena.get_data(node_id) {
                    let val = data.cell_value(col_idx);
                    if let CellValue::Bool(mut b) = val
                        && ui.checkbox("##cb", &mut b)
                    {
                        if let Some(data) = self.arena.get_data_mut(node_id) {
                            data.set_cell_value(col_idx, &CellValue::Bool(b));
                        }
                    }
                }
            }
            EditorKind::ComboBox => {
                // SAFETY: pointer avoids borrow conflict — self.columns is not mutated
                // while self.arena is borrowed mutably below.
                let items = match &self.columns[col_idx].editor {
                    CellEditor::ComboBox { items } => unsafe { &*(items as *const Vec<String>) },
                    _ => { self.edit_state.deactivate(); return; }
                };
                if let Some(data) = self.arena.get_data(node_id) {
                    let val = data.cell_value(col_idx);
                    if let CellValue::Choice(mut choice) = val {
                        ui.set_next_item_width(-1.0);
                        if ui.combo_simple_string("##combo", &mut choice, items) {
                            if let Some(data) = self.arena.get_data_mut(node_id) {
                                data.set_cell_value(col_idx, &CellValue::Choice(choice));
                            }
                        }
                    }
                }
            }
            EditorKind::ColorEdit => {
                if let Some(data) = self.arena.get_data(node_id) {
                    let val = data.cell_value(col_idx);
                    if let CellValue::Color(mut c) = val {
                        ui.set_next_item_width(-1.0);
                        if ui.color_edit4_config("##color", &mut c)
                            .flags(dear_imgui_rs::ColorEditFlags::NO_INPUTS)
                            .build()
                        {
                            if let Some(data) = self.arena.get_data_mut(node_id) {
                                data.set_cell_value(col_idx, &CellValue::Color(c));
                            }
                        }
                    }
                }
            }
            EditorKind::Button => {
                // SAFETY: same as ComboBox above — pointer avoids borrow conflict.
                let label = match &self.columns[col_idx].editor {
                    CellEditor::Button { label } => unsafe { &*(label as *const String) },
                    _ => { self.edit_state.deactivate(); return; }
                };
                if ui.button(label) {
                    self.button_clicked = Some((node_id, col_idx));
                }
            }
            EditorKind::ProgressBar => {
                if let Some(data) = self.arena.get_data(node_id) {
                    let val = data.cell_value(col_idx);
                    if let CellValue::Progress(p) = val {
                        self.cell_buf.clear();
                        let _ = std::fmt::Write::write_fmt(
                            &mut self.cell_buf,
                            format_args!("{:.0}%", p * 100.0),
                        );
                        ui.progress_bar(p)
                            .size([-1.0, 0.0])
                            .overlay_text(&self.cell_buf)
                            .build();
                    }
                }
            }
            EditorKind::Custom => {
                if let Some(data) = self.arena.get_data(node_id) {
                    data.render_cell(ui, col_idx, node_id);
                }
            }
            EditorKind::Other | EditorKind::None => {
                if let Some(data) = self.arena.get_data(node_id) {
                    self.cell_buf.clear();
                    data.cell_display_text(col_idx, &mut self.cell_buf);

                    let cell_style = data.cell_style(col_idx);
                    let col_alignment = self.columns[col_idx].alignment;
                    let cell_alignment = cell_style
                        .as_ref()
                        .and_then(|s| s.alignment)
                        .unwrap_or(col_alignment);

                    if let Some(ref style) = cell_style
                        && let Some(bg) = style.bg_color
                    {
                        ui.table_set_bg_color(TableBgTarget::CellBg, bg, -1);
                    }

                    if !self.cell_buf.is_empty() {
                        let col_w = ui.current_column_width();
                        let text_w = calc_text_size(&self.cell_buf)[0];
                        let pad = alignment_pad(cell_alignment, col_w, text_w);
                        if pad > 0.0 {
                            let cursor = ui.cursor_pos();
                            ui.set_cursor_pos([cursor[0] + pad, cursor[1]]);
                        }
                    }

                    let color = cell_style
                        .as_ref()
                        .and_then(|s| s.text_color)
                        .or(row_text_color);

                    if let Some(c) = color {
                        ui.text_colored(c, &self.cell_buf);
                    } else {
                        ui.text(&self.cell_buf);
                    }

                    // Clip tooltip: show full text when hovered and clipped
                    if self.columns[col_idx].clip_tooltip
                        && !self.cell_buf.is_empty()
                        && ui.is_item_hovered()
                    {
                        let col_w = ui.current_column_width();
                        let text_w = calc_text_size(&self.cell_buf)[0];
                        if text_w > col_w {
                            ui.tooltip_text(&self.cell_buf);
                        }
                    }
                }
            }
        }
    }

    // ─── Internal: inline editor ────────────────────────────────────

    fn try_activate_edit(&mut self, flat_idx: usize, col_idx: usize) {
        if col_idx >= self.columns.len() {
            return;
        }
        if matches!(
            editor_kind(&self.columns[col_idx].editor),
            EditorKind::None
                | EditorKind::Checkbox
                | EditorKind::ComboBox
                | EditorKind::Button
                | EditorKind::ProgressBar
                | EditorKind::ColorEdit
        ) {
            return;
        }

        let node_id = match self.flat_view.rows.get(flat_idx) {
            Some(r) => r.node_id,
            None => return,
        };

        if let Some(data) = self.arena.get_data(node_id) {
            let value = data.cell_value(col_idx);
            self.edit_state.activate(flat_idx, col_idx, &value);
        }
    }

    fn render_editor_inline(&mut self, ui: &Ui, _flat_idx: usize, col_idx: usize, node_id: NodeId) {
        let mut commit = false;
        let mut cancel = false;

        ui.set_next_item_width(-1.0);

        // SAFETY: We take a pointer to avoid cloning CellEditor (which may contain Vec<String>).
        // The pointer is valid for the duration of this function because self.columns is not
        // mutated during editor rendering.
        let editor_ptr = &self.columns[col_idx].editor as *const CellEditor;
        let editor_snapshot = unsafe { &*editor_ptr };
        let first_frame = self.edit_state.just_activated;
        if first_frame {
            self.edit_state.just_activated = false;
        }
        self.edit_state.frames_active += 1;

        match editor_snapshot {
            CellEditor::TextInput => {
                if first_frame {
                    unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
                }
                let entered = ui
                    .input_text("##edit", &mut self.edit_state.text_buf)
                    .enter_returns_true(true)
                    .build();
                if entered {
                    commit = true;
                }
                if !first_frame && !entered {
                    if ui.is_item_deactivated_after_edit() {
                        if self.config.table.commit_on_focus_loss {
                            commit = true;
                        } else {
                            cancel = true;
                        }
                    } else if ui.is_item_deactivated() {
                        cancel = true;
                    }
                }
            }
            CellEditor::SliderInt { min, max } => {
                ui.slider_config("##edit", *min, *max)
                    .build(&mut self.edit_state.int_val);
                if !first_frame && ui.is_item_deactivated_after_edit() {
                    commit = true;
                }
            }
            CellEditor::SliderFloat { min, max } => {
                ui.slider_config("##edit", *min, *max)
                    .build(&mut self.edit_state.float_val);
                if !first_frame && ui.is_item_deactivated_after_edit() {
                    commit = true;
                }
            }
            CellEditor::SpinInt { step, step_fast } => {
                if first_frame {
                    unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
                }
                unsafe {
                    dear_imgui_rs::sys::igInputInt(
                        c"##edit".as_ptr(),
                        &mut self.edit_state.int_val,
                        *step,
                        *step_fast,
                        0,
                    );
                }
                if !first_frame {
                    if ui.is_item_deactivated_after_edit() {
                        if self.config.table.commit_on_focus_loss {
                            commit = true;
                        } else {
                            cancel = true;
                        }
                    } else if ui.is_item_deactivated() {
                        cancel = true;
                    }
                }
            }
            CellEditor::SpinFloat { step, step_fast } => {
                if first_frame {
                    unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
                }
                unsafe {
                    dear_imgui_rs::sys::igInputFloat(
                        c"##edit".as_ptr(),
                        &mut self.edit_state.float_val,
                        *step,
                        *step_fast,
                        c"%.2f".as_ptr(),
                        0,
                    );
                }
                if !first_frame {
                    if ui.is_item_deactivated_after_edit() {
                        if self.config.table.commit_on_focus_loss {
                            commit = true;
                        } else {
                            cancel = true;
                        }
                    } else if ui.is_item_deactivated() {
                        cancel = true;
                    }
                }
            }
            CellEditor::Custom => {
                if let Some(data) = self.arena.get_data_mut(node_id)
                    && data.render_editor(ui, col_idx, node_id)
                {
                    commit = true;
                }
            }
            _ => {
                self.edit_state.deactivate();
                return;
            }
        }

        if ui.is_key_pressed(Key::Escape) {
            cancel = true;
        }

        if cancel {
            self.edit_state.deactivate();
        } else if commit {
            let value = self.edit_state.take_cell_value(editor_snapshot);
            if let Some(data) = self.arena.get_data_mut(node_id) {
                data.set_cell_value(col_idx, &value);
            }
            self.edit_state.deactivate();
        }
    }

    // ─── Internal: selection ────────────────────────────────────────

    fn handle_selection(&mut self, ui: &Ui, flat_idx: usize) {
        let node_id = match self.flat_view.rows.get(flat_idx) {
            Some(r) => r.node_id,
            None => return,
        };

        match self.config.table.selection_mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                self.selected_nodes.clear();
                self.selected_nodes.insert(node_id);
                self.selection_anchor = Some(flat_idx);
            }
            SelectionMode::Multi => {
                let io = ui.io();
                let ctrl = io.key_ctrl();
                let shift = io.key_shift();

                if ctrl {
                    if !self.selected_nodes.remove(&node_id) {
                        self.selected_nodes.insert(node_id);
                    }
                    self.selection_anchor = Some(flat_idx);
                } else if shift {
                    let anchor = self.selection_anchor.unwrap_or(flat_idx);
                    let (start, end) = if flat_idx < anchor {
                        (flat_idx, anchor)
                    } else {
                        (anchor, flat_idx)
                    };
                    self.selected_nodes.clear();
                    for i in start..=end {
                        if let Some(r) = self.flat_view.rows.get(i) {
                            self.selected_nodes.insert(r.node_id);
                        }
                    }
                } else {
                    self.selected_nodes.clear();
                    self.selected_nodes.insert(node_id);
                    self.selection_anchor = Some(flat_idx);
                }
            }
        }
    }

    // ─── Internal: keyboard ─────────────────────────────────────────

    fn handle_keyboard(&mut self, ui: &Ui) {
        if !ui.is_window_focused() {
            return;
        }

        if ui.is_key_pressed(Key::DownArrow) {
            if let Some(anchor) = self.selection_anchor {
                let next = (anchor + 1).min(self.flat_view.len().saturating_sub(1));
                self.select_flat_row(next);
            } else if !self.flat_view.rows.is_empty() {
                self.select_flat_row(0);
            }
        }

        if ui.is_key_pressed(Key::UpArrow) {
            if let Some(anchor) = self.selection_anchor {
                let prev = anchor.saturating_sub(1);
                self.select_flat_row(prev);
            } else if !self.flat_view.rows.is_empty() {
                self.select_flat_row(0);
            }
        }

        if ui.is_key_pressed(Key::RightArrow) {
            if let Some(anchor) = self.selection_anchor {
                if let Some(row) = self.flat_view.rows.get(anchor) {
                    let node_id = row.node_id;
                    if !row.is_leaf && !row.is_expanded {
                        self.pending_toggle = Some(node_id);
                    } else if row.is_expanded && anchor + 1 < self.flat_view.len() {
                        self.select_flat_row(anchor + 1);
                    }
                }
            }
        }

        if ui.is_key_pressed(Key::LeftArrow) {
            if let Some(anchor) = self.selection_anchor {
                if let Some(row) = self.flat_view.rows.get(anchor) {
                    let node_id = row.node_id;
                    if !row.is_leaf && row.is_expanded {
                        self.pending_toggle = Some(node_id);
                    } else if let Some(parent_id) = self.arena.parent(node_id) {
                        if let Some(parent_flat) = self.flat_view.index_of(parent_id) {
                            self.select_flat_row(parent_flat);
                        }
                    }
                }
            }
        }

        // Delete
        if ui.is_key_pressed(Key::Delete) && !self.selected_nodes.is_empty() {
            // Collect to avoid borrow conflict
            let to_remove: Vec<NodeId> = self.selected_nodes.iter().copied().collect();
            for id in to_remove {
                self.arena.remove(id);
            }
            self.selected_nodes.clear();
            self.selection_anchor = None;
            self.flat_view.mark_dirty();
        }

        // Ctrl+A
        if ui.io().key_ctrl() && ui.is_key_pressed(Key::A) {
            self.selected_nodes.clear();
            for row in &self.flat_view.rows {
                self.selected_nodes.insert(row.node_id);
            }
        }

        // F2
        if ui.is_key_pressed(Key::F2)
            && self.config.table.edit_trigger == EditTrigger::F2Key
        {
            if let Some(anchor) = self.selection_anchor {
                for c in 0..self.columns.len() {
                    if !matches!(
                        editor_kind(&self.columns[c].editor),
                        EditorKind::None
                            | EditorKind::Checkbox
                            | EditorKind::ComboBox
                            | EditorKind::Button
                            | EditorKind::ProgressBar
                            | EditorKind::ColorEdit
                            | EditorKind::Custom
                    ) {
                        self.try_activate_edit(anchor, c);
                        break;
                    }
                }
            }
        }
    }

    fn select_flat_row(&mut self, flat_idx: usize) {
        if let Some(row) = self.flat_view.rows.get(flat_idx) {
            self.selected_nodes.clear();
            self.selected_nodes.insert(row.node_id);
            self.selection_anchor = Some(flat_idx);
        }
    }
}
