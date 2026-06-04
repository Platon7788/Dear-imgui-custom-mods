//! Public data API: insert / remove / access / expand / move / select / sort / filter / columns / export.
//!
//! Part of [`VirtualTree`](super::VirtualTree); split out of `mod.rs`
//! to keep files under 500 lines. See `mod.rs` for the struct.

use super::*;

impl<T: VirtualTreeNode> VirtualTree<T> {
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
        self.anchor = None;
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
        self.load_children_if_needed(id);
        self.arena.expand(id);
        self.flat_view.mark_dirty();
    }

    pub fn collapse(&mut self, id: NodeId) {
        self.arena.collapse(id);
        self.flat_view.mark_dirty();
    }

    pub fn toggle(&mut self, id: NodeId) {
        // Only materialize children lazily when transitioning into expanded.
        if !self.arena.is_expanded(id) {
            self.load_children_if_needed(id);
        }
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
        self.anchor = None;
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
        self.filter_state
            .set_filter(query, &mut self.arena, self.config.auto_expand_on_filter);
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
    pub(super) fn deselect_descendants(&mut self, id: NodeId) {
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

    // ─── Export / Import ────────────────────────────────────────────

    /// Export selected nodes (or all if none selected) to tree export format.
    ///
    /// When exporting selected nodes, each selected node exports with its
    /// full subtree (all descendants included).
    pub fn export_data(
        &self,
        scope: crate::utils::export::ExportScope,
    ) -> Vec<crate::utils::export::TreeExportNode>
    where
        T: crate::utils::export::Exportable,
    {
        match scope {
            crate::utils::export::ExportScope::Selected => {
                if self.selected_nodes.is_empty() {
                    // Nothing selected — export all roots.
                    return self.export_data(crate::utils::export::ExportScope::All);
                }
                // Export each selected node with subtree, but skip nodes
                // whose ancestors are already selected (avoid duplicates).
                let mut result = Vec::new();
                for &id in &self.selected_nodes {
                    let already_covered = self.is_ancestor_selected(id, &self.selected_nodes);
                    if !already_covered && let Some(node) = self.export_subtree(id) {
                        result.push(node);
                    }
                }
                result
            }
            crate::utils::export::ExportScope::All => self
                .arena
                .roots()
                .iter()
                .filter_map(|&id| self.export_subtree(id))
                .collect(),
        }
    }

    /// Export a single node with its subtree.
    pub(super) fn export_subtree(
        &self,
        id: crate::virtual_tree::arena::NodeId,
    ) -> Option<crate::utils::export::TreeExportNode>
    where
        T: crate::utils::export::Exportable,
    {
        let data = self.arena.get_data(id)?;
        let names = T::field_names();
        let fields: Vec<(String, crate::utils::export::FieldValue)> = (0..T::field_count())
            .map(|c| (names[c].to_string(), data.field_value(c)))
            .collect();

        let children: Vec<crate::utils::export::TreeExportNode> = self
            .arena
            .children(id)
            .iter()
            .filter_map(|&child_id| self.export_subtree(child_id))
            .collect();

        Some(crate::utils::export::TreeExportNode { fields, children })
    }

    /// Check if any ancestor of `id` is in the selected set.
    pub(super) fn is_ancestor_selected(&self, id: NodeId, selected: &NodeIdSet) -> bool {
        let mut current = self.arena.parent(id);
        while let Some(pid) = current {
            if selected.contains(&pid) {
                return true;
            }
            current = self.arena.parent(pid);
        }
        false
    }

    /// Export to string in the given format.
    pub fn export_string(
        &self,
        scope: crate::utils::export::ExportScope,
        format: crate::utils::export::ExportFormat,
    ) -> String
    where
        T: crate::utils::export::Exportable,
    {
        let nodes = self.export_data(scope);
        crate::utils::export::format_tree(&nodes, format)
    }

    /// Export to file. Format auto-detected from extension.
    pub fn export_to_file(
        &self,
        scope: crate::utils::export::ExportScope,
        path: &std::path::Path,
    ) -> std::io::Result<()>
    where
        T: crate::utils::export::Exportable,
    {
        let nodes = self.export_data(scope);
        crate::utils::export::export_tree_to_file(&nodes, path, None)
    }
}
