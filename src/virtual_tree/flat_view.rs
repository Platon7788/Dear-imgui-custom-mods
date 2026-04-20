//! Flattened view cache — linearizes the visible tree into a `Vec<FlatRow>`
//! for ListClipper integration.
//!
//! Rebuilt only when the tree structure or expand/collapse state changes
//! (not every frame). Collapsed subtrees are skipped entirely.
//!
//! ## Performance (1M–10M nodes)
//!
//! - Rebuild: O(visible_nodes) — typically much less than total when collapsed
//! - `index_of()`: O(1) via direct slot-index lookup (`Vec<u32>`, no hashing)
//! - Zero per-visible-node allocations — children are counted/iterated without
//!   collecting into a temporary Vec
//! - **Iterative DFS** — no recursion, no stack overflow at any depth

use super::arena::{NodeId, TreeArena};
use super::filter::FilterState;
use super::node::VirtualTreeNode;

// ─── FlatRow ────────────────────────────────────────────────────────────────

/// One entry in the flattened visible-row list.
#[derive(Clone, Copy, Debug)]
pub struct FlatRow {
    pub node_id: NodeId,
    pub depth: u16,
    pub is_leaf: bool,
    pub is_expanded: bool,
    /// True if this node is the last child of its parent (for tree line rendering).
    pub is_last_child: bool,
    /// Bitmask: bit `d` is set if depth `d` ancestor is NOT the last child
    /// (i.e. a vertical continuation line should be drawn at that depth).
    /// Supports up to 64 levels of depth.
    pub continuation_mask: u64,
}

// ─── Stack frame for iterative DFS ──────────────────────────────────────────

/// Represents "process the i-th visible child of this parent".
struct StackFrame {
    children_end: usize,
    /// Current position within the children slice.
    cursor: usize,
    /// How many visible children this parent has (for last-child detection).
    visible_total: usize,
    /// Current visible child index (1-based).
    visible_idx: usize,
    /// Continuation mask at this parent's level.
    mask: u64,
}

// ─── FlatView ───────────────────────────────────────────────────────────────

/// Cached linearization of the tree. Rebuilt on structural changes.
pub struct FlatView {
    pub rows: Vec<FlatRow>,
    /// O(1) lookup: `index_map[NodeId.index]` → flat row index.
    /// Sentinel `u32::MAX` = not present. Sized to arena slot count.
    index_map: Vec<u32>,
    pub dirty: bool,
    /// Reusable DFS stack — avoids re-allocation across rebuilds.
    stack: Vec<StackFrame>,
    /// Scratch buffer for children snapshots (avoids borrow conflict with arena).
    children_buf: Vec<NodeId>,
}

impl Default for FlatView {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatView {
    /// Sentinel value: node is not present in the flat view.
    const NO_INDEX: u32 = u32::MAX;

    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            index_map: Vec::new(),
            dirty: true,
            stack: Vec::new(),
            children_buf: Vec::new(),
        }
    }

    /// Rebuild the flat view from the arena.
    /// O(visible nodes) — collapsed subtrees are skipped.
    /// Iterative DFS — safe at any tree depth.
    pub fn rebuild<T: VirtualTreeNode>(&mut self, arena: &TreeArena<T>, filter: &FilterState) {
        self.rows.clear();
        self.stack.clear();
        self.children_buf.clear();

        // Resize index_map to cover all arena slots; fill with sentinel.
        let slot_count = arena.slot_len();
        self.index_map.resize(slot_count, Self::NO_INDEX);
        self.index_map.fill(Self::NO_INDEX);

        let roots = arena.roots();
        if roots.is_empty() {
            self.dirty = false;
            return;
        }

        // Snapshot root IDs into children_buf so we can iterate without borrowing arena.
        let roots_start = 0;
        let roots_end = roots.len();
        self.children_buf.extend_from_slice(roots);

        // Count visible roots
        let visible_roots = self.count_visible(arena, filter, roots_start, roots_end);
        if visible_roots == 0 {
            self.dirty = false;
            return;
        }

        // Push root-level frame
        self.stack.push(StackFrame {
            children_end: roots_end,
            cursor: roots_start,
            visible_total: visible_roots,
            visible_idx: 0,
            mask: 0,
        });

        while let Some(frame) = self.stack.last_mut() {
            // Find next visible child in current frame
            let mut found = None;
            while frame.cursor < frame.children_end {
                let child_id = self.children_buf[frame.cursor];
                frame.cursor += 1;

                if let Some(slot) = arena.get(child_id) {
                    if !slot.visible {
                        continue;
                    }
                    if filter.active && !filter.is_visible(child_id) {
                        continue;
                    }
                    frame.visible_idx += 1;
                    let is_last = frame.visible_idx == frame.visible_total;
                    found = Some((
                        child_id,
                        slot.depth,
                        slot.data.has_children() || !slot.children.is_empty(),
                        slot.expanded,
                        is_last,
                    ));
                    break;
                }
            }

            let Some((node_id, depth, has_children, expanded, is_last)) = found else {
                // Frame exhausted — pop
                self.stack.pop();
                continue;
            };

            // Build continuation mask
            let parent_mask = frame.mask;
            let mut mask = parent_mask;
            if !is_last && depth < 64 {
                mask |= 1u64 << depth;
            } else if depth < 64 {
                mask &= !(1u64 << depth);
            }

            // Emit row
            let flat_idx = self.rows.len();
            self.rows.push(FlatRow {
                node_id,
                depth,
                is_leaf: !has_children,
                is_expanded: expanded,
                is_last_child: is_last,
                continuation_mask: mask,
            });
            self.index_map[node_id.index as usize] = flat_idx as u32;

            // If expanded, push children frame
            if expanded {
                let children = arena.children(node_id);
                if !children.is_empty() {
                    let start = self.children_buf.len();
                    self.children_buf.extend_from_slice(children);
                    let end = self.children_buf.len();

                    let visible_count = self.count_visible(arena, filter, start, end);
                    if visible_count > 0 {
                        self.stack.push(StackFrame {
                            children_end: end,
                            cursor: start,
                            visible_total: visible_count,
                            visible_idx: 0,
                            mask,
                        });
                    }
                }
            }
        }

        self.dirty = false;
    }

    /// Count visible children in children_buf[start..end] without allocation.
    #[inline]
    fn count_visible<T: VirtualTreeNode>(
        &self,
        arena: &TreeArena<T>,
        filter: &FilterState,
        start: usize,
        end: usize,
    ) -> usize {
        self.children_buf[start..end]
            .iter()
            .filter(|&&id| {
                arena.get(id).is_some_and(|s| s.visible)
                    && (!filter.active || filter.is_visible(id))
            })
            .count()
    }

    /// Number of visible rows.
    #[inline]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the flat view is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Find the flat-view index of a node. O(1) via direct slot-index lookup.
    #[inline]
    pub fn index_of(&self, id: NodeId) -> Option<usize> {
        let idx = *self.index_map.get(id.index as usize)?;
        if idx == Self::NO_INDEX {
            None
        } else {
            Some(idx as usize)
        }
    }

    /// Mark as needing rebuild.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtual_table::row::CellValue;
    use crate::virtual_tree::node::VirtualTreeNode;

    /// Minimal test node for flat view tests.
    struct FvNode {
        name: &'static str,
        is_parent: bool,
    }

    impl VirtualTreeNode for FvNode {
        fn cell_value(&self, _col: usize) -> CellValue {
            CellValue::Text(self.name.to_string())
        }
        fn set_cell_value(&mut self, _col: usize, _value: &CellValue) {}
        fn has_children(&self) -> bool {
            self.is_parent
        }
    }

    fn node(name: &'static str, parent: bool) -> FvNode {
        FvNode {
            name,
            is_parent: parent,
        }
    }

    #[test]
    fn empty_tree() {
        let arena: TreeArena<FvNode> = TreeArena::new();
        let filter = FilterState::new();
        let mut fv = FlatView::new();
        fv.rebuild(&arena, &filter);
        assert!(fv.is_empty());
        assert_eq!(fv.len(), 0);
    }

    #[test]
    fn roots_only() {
        let mut arena = TreeArena::new();
        let a = arena.insert_root(node("a", false)).unwrap();
        let b = arena.insert_root(node("b", false)).unwrap();
        let filter = FilterState::new();
        let mut fv = FlatView::new();
        fv.rebuild(&arena, &filter);

        assert_eq!(fv.len(), 2);
        assert_eq!(fv.rows[0].node_id, a);
        assert_eq!(fv.rows[1].node_id, b);
        assert!(fv.rows[0].is_leaf);
        assert_eq!(fv.rows[0].depth, 0);
    }

    #[test]
    fn collapsed_children_hidden() {
        let mut arena = TreeArena::new();
        let root = arena.insert_root(node("root", true)).unwrap();
        arena.insert_child(root, node("child", false)).unwrap();
        let filter = FilterState::new();
        let mut fv = FlatView::new();
        fv.rebuild(&arena, &filter);

        // Root collapsed → only root visible
        assert_eq!(fv.len(), 1);
        assert_eq!(fv.rows[0].node_id, root);
        assert!(!fv.rows[0].is_expanded);
    }

    #[test]
    fn expanded_children_visible() {
        let mut arena = TreeArena::new();
        let root = arena.insert_root(node("root", true)).unwrap();
        let child = arena.insert_child(root, node("child", false)).unwrap();
        arena.expand(root);
        let filter = FilterState::new();
        let mut fv = FlatView::new();
        fv.rebuild(&arena, &filter);

        assert_eq!(fv.len(), 2);
        assert_eq!(fv.rows[0].node_id, root);
        assert_eq!(fv.rows[1].node_id, child);
        assert_eq!(fv.rows[1].depth, 1);
        assert!(fv.rows[1].is_leaf);
    }

    #[test]
    fn index_of_lookup() {
        let mut arena = TreeArena::new();
        let a = arena.insert_root(node("a", false)).unwrap();
        let b = arena.insert_root(node("b", false)).unwrap();
        let filter = FilterState::new();
        let mut fv = FlatView::new();
        fv.rebuild(&arena, &filter);

        assert_eq!(fv.index_of(a), Some(0));
        assert_eq!(fv.index_of(b), Some(1));
    }

    #[test]
    fn index_of_missing_node() {
        let arena: TreeArena<FvNode> = TreeArena::new();
        let filter = FilterState::new();
        let mut fv = FlatView::new();
        fv.rebuild(&arena, &filter);

        let fake = NodeId {
            index: 42,
            generation: 0,
        };
        assert!(fv.index_of(fake).is_none());
    }

    #[test]
    fn deep_tree_no_stack_overflow() {
        let mut arena = TreeArena::new();
        let mut parent = arena.insert_root(node("r", true)).unwrap();
        // Build a chain 200 deep
        for _ in 0..200 {
            let child = arena.insert_child(parent, node("c", true)).unwrap();
            arena.expand(parent);
            parent = child;
        }
        let filter = FilterState::new();
        let mut fv = FlatView::new();
        fv.rebuild(&arena, &filter);
        assert_eq!(fv.len(), 201); // 1 root + 200 children
    }

    #[test]
    fn dirty_flag_cleared_after_rebuild() {
        let arena: TreeArena<FvNode> = TreeArena::new();
        let filter = FilterState::new();
        let mut fv = FlatView::new();
        assert!(fv.dirty);
        fv.rebuild(&arena, &filter);
        assert!(!fv.dirty);
        fv.mark_dirty();
        assert!(fv.dirty);
    }

    #[test]
    fn is_last_child_flag() {
        let mut arena = TreeArena::new();
        let root = arena.insert_root(node("root", true)).unwrap();
        arena.insert_child(root, node("a", false)).unwrap();
        arena.insert_child(root, node("b", false)).unwrap();
        arena.expand(root);
        let filter = FilterState::new();
        let mut fv = FlatView::new();
        fv.rebuild(&arena, &filter);

        assert!(!fv.rows[1].is_last_child); // "a" is not last
        assert!(fv.rows[2].is_last_child); // "b" is last
    }
}
