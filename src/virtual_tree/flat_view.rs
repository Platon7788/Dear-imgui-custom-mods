//! Flattened view cache — linearizes the visible tree into a `Vec<FlatRow>`
//! for ListClipper integration.
//!
//! Rebuilt only when the tree structure or expand/collapse state changes
//! (not every frame). Collapsed subtrees are skipped entirely.
//!
//! ## Performance (500K–1M nodes)
//!
//! - Rebuild: O(visible_nodes) — typically much less than total when collapsed
//! - `index_of()`: O(1) via HashMap lookup (was O(n) linear scan)
//! - Zero per-visible-node allocations — children are counted/iterated without
//!   collecting into a temporary Vec
//! - **Iterative DFS** — no recursion, no stack overflow at any depth

use std::collections::HashMap;

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
    /// O(1) lookup: NodeId → flat index. Rebuilt together with `rows`.
    index_map: HashMap<NodeId, usize, foldhash::fast::FixedState>,
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
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            index_map: HashMap::with_hasher(foldhash::fast::FixedState::default()),
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
        self.index_map.clear();
        self.stack.clear();
        self.children_buf.clear();

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
                    if filter.active && !filter.visible_set.contains(&child_id) {
                        continue;
                    }
                    frame.visible_idx += 1;
                    let is_last = frame.visible_idx == frame.visible_total;
                    found = Some((child_id, slot.depth, slot.data.has_children() || !slot.children.is_empty(), slot.expanded, is_last));
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
            self.index_map.insert(node_id, flat_idx);

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
        self.children_buf[start..end].iter().filter(|&&id| {
            arena.get(id).is_some_and(|s| s.visible)
                && (!filter.active || filter.visible_set.contains(&id))
        }).count()
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

    /// Find the flat-view index of a node. O(1) via HashMap.
    #[inline]
    pub fn index_of(&self, id: NodeId) -> Option<usize> {
        self.index_map.get(&id).copied()
    }

    /// Mark as needing rebuild.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}
