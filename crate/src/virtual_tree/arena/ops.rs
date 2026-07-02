//! `TreeArena<T>` structural ops: reparenting, sibling sorting, iteration.
//!
//! Split out of `arena/mod.rs` (CLAUDE.md: keep files < 500 lines). These are
//! inherent methods on [`TreeArena`](super::TreeArena); as a child module they
//! access the parent-private slab fields directly.

use super::*;

impl<T> TreeArena<T> {
    // ─── Move / Reparent ────────────────────────────────────────────

    /// Move a node to a new parent at the given position.
    /// Pass `None` for `new_parent` to make it a root.
    pub fn move_node(&mut self, id: NodeId, new_parent: Option<NodeId>, position: usize) -> bool {
        // Validate id exists
        if self.get(id).is_none() {
            return false;
        }

        // Prevent moving a node into its own subtree
        if let Some(np) = new_parent
            && (np == id || self.is_ancestor_of(id, np))
        {
            return false;
        }

        // Detach from old parent — position + remove to preserve sibling order.
        let old_parent = self.get(id).and_then(|s| s.parent);
        if let Some(op) = old_parent {
            if let Some(ps) = self.slot_mut(op)
                && let Some(pos) = ps.children.iter().position(|&c| c == id)
            {
                ps.children.remove(pos);
            }
        } else {
            if let Some(pos) = self.roots.iter().position(|&r| r == id) {
                self.roots.remove(pos);
            }
        }

        // Attach to new parent
        if let Some(np) = new_parent {
            let new_depth = self.get(np).map_or(0, |s| s.depth).saturating_add(1);
            if let Some(ps) = self.slot_mut(np) {
                let pos = position.min(ps.children.len());
                ps.children.insert(pos, id);
            }
            if let Some(s) = self.slot_mut(id) {
                s.parent = Some(np);
                s.depth = new_depth;
            }
            // Update depths of all descendants
            self.update_subtree_depth(id);
        } else {
            let pos = position.min(self.roots.len());
            self.roots.insert(pos, id);
            if let Some(s) = self.slot_mut(id) {
                s.parent = None;
                s.depth = 0;
            }
            self.update_subtree_depth(id);
        }

        true
    }

    /// Check if `ancestor` is an ancestor of `descendant`.
    fn is_ancestor_of(&self, ancestor: NodeId, descendant: NodeId) -> bool {
        let mut current = self.get(descendant).and_then(|s| s.parent);
        while let Some(pid) = current {
            if pid == ancestor {
                return true;
            }
            current = self.get(pid).and_then(|s| s.parent);
        }
        false
    }

    /// Update depth of a node's entire subtree after reparenting.
    /// Iterative DFS (LIFO stack) to avoid stack overflow on deep trees.
    fn update_subtree_depth(&mut self, id: NodeId) {
        let mut queue = vec![id];
        while let Some(nid) = queue.pop() {
            let depth = match self.get(nid) {
                Some(s) => s.depth,
                None => continue,
            };
            // Take children to avoid borrow conflict, then restore.
            let children = match self.slot_mut(nid) {
                Some(s) => std::mem::take(&mut s.children),
                None => continue,
            };
            for &child_id in &children {
                if let Some(cs) = self.slot_mut(child_id) {
                    cs.depth = depth.saturating_add(1);
                }
                queue.push(child_id);
            }
            // Restore children vec
            if let Some(s) = self.slot_mut(nid) {
                s.children = children;
            }
        }
    }

    // ─── Sort siblings ──────────────────────────────────────────────

    /// Sort the children of a node (or roots if `parent` is None) using a comparator.
    pub fn sort_children(
        &mut self,
        parent: Option<NodeId>,
        cmp: &mut impl FnMut(&T, &T) -> std::cmp::Ordering,
    ) {
        // Take the children vec out to avoid borrow conflict with self.get_data().
        let mut children = if let Some(pid) = parent {
            match self.slot_mut(pid) {
                Some(s) => std::mem::take(&mut s.children),
                None => return,
            }
        } else {
            std::mem::take(&mut self.roots)
        };

        children.sort_by(|&a, &b| {
            let da = self.get_data(a);
            let db = self.get_data(b);
            match (da, db) {
                (Some(da), Some(db)) => cmp(da, db),
                _ => std::cmp::Ordering::Equal,
            }
        });

        // Put the sorted children back.
        if let Some(pid) = parent {
            if let Some(s) = self.slot_mut(pid) {
                s.children = children;
            }
        } else {
            self.roots = children;
        }
    }

    /// Sort all sibling groups recursively.
    pub fn sort_all_siblings(&mut self, cmp: &mut impl FnMut(&T, &T) -> std::cmp::Ordering) {
        // Collect all node ids that have children
        let parents: Vec<Option<NodeId>> = std::iter::once(None)
            .chain(self.slots.iter().enumerate().filter_map(|(i, slot)| {
                let s = slot.as_ref()?;
                if s.children.is_empty() {
                    None
                } else {
                    Some(Some(NodeId {
                        index: i as u32,
                        generation: self.generations[i],
                    }))
                }
            }))
            .collect();

        for parent in parents {
            self.sort_children(parent, cmp);
        }
    }

    // ─── Iteration ──────────────────────────────────────────────────

    /// Iterate over all live (node_id, &T) pairs. Order is unspecified.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, slot)| {
            let s = slot.as_ref()?;
            Some((
                NodeId {
                    index: i as u32,
                    generation: self.generations[i],
                },
                &s.data,
            ))
        })
    }
}
