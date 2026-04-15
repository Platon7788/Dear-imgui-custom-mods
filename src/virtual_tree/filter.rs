//! Filter/search state with auto-expand for matching branches.
//!
//! When a filter is active, only nodes that match (or are ancestors of matches)
//! are shown. Matching ancestors are auto-expanded.
//!
//! ## Performance (10M nodes)
//!
//! - Pass 1 (scan all nodes): O(n) — unavoidable, but each `matches_filter` is user-defined
//! - Pass 2 (mark ancestors): O(matches × depth) — with early-break when not auto-expanding
//! - Uses `Vec<bool>` indexed by node slot for O(1) visibility checks (~10 MB at 10M nodes
//!   vs ~400 MB for HashSet)

use super::arena::{NodeId, TreeArena};
use super::node::VirtualTreeNode;

// ─── FilterState ────────────────────────────────────────────────────────────

/// Tracks the active filter query and the set of visible nodes.
pub struct FilterState {
    pub query: String,
    pub active: bool,
    /// Index-based visibility flags: `visible_set[node.index]` = true if visible.
    /// Sized to arena slot count during `set_filter`; avoids HashSet overhead at scale.
    visible_set: Vec<bool>,
    /// Reusable buffer for collecting matches (avoids re-allocation across filter calls).
    matching_buf: Vec<NodeId>,
}

impl Default for FilterState {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            active: false,
            visible_set: Vec::new(),
            matching_buf: Vec::new(),
        }
    }

    /// Check if a node is in the visible set. O(1) by slot index.
    #[inline]
    pub fn is_visible(&self, id: NodeId) -> bool {
        self.visible_set.get(id.index as usize).copied().unwrap_or(false)
    }

    /// Apply a filter query. Empty/whitespace-only query clears the filter.
    pub fn set_filter<T: VirtualTreeNode>(
        &mut self,
        query: &str,
        arena: &mut TreeArena<T>,
        auto_expand: bool,
    ) {
        let trimmed = query.trim();
        self.query.clear();
        self.query.push_str(trimmed);

        if trimmed.is_empty() {
            self.active = false;
            self.visible_set.clear();
            return;
        }

        self.active = true;

        // Resize to cover all arena slots and reset.
        let slot_count = arena.slot_len();
        self.visible_set.resize(slot_count, false);
        self.visible_set.fill(false);

        // Pass 1: find all matching nodes (reuse buffer to avoid allocation)
        self.matching_buf.clear();
        for (id, data) in arena.iter() {
            if data.matches_filter(trimmed) {
                self.matching_buf.push(id);
            }
        }

        // Pass 2: for each match, mark it and all ancestors as visible.
        //
        // When auto_expand is true, we must walk to root every time because
        // expand() has a side-effect that must reach every ancestor.
        //
        // When auto_expand is false, we can early-break if the ancestor is
        // already in visible_set (all ancestors above it are already marked).
        for i in 0..self.matching_buf.len() {
            let id = self.matching_buf[i];
            self.visible_set[id.index as usize] = true;
            let mut current = arena.parent(id);
            while let Some(pid) = current {
                let idx = pid.index as usize;
                let was_new = !self.visible_set[idx];
                self.visible_set[idx] = true;
                if auto_expand {
                    arena.expand(pid);
                } else if !was_new {
                    // Already in set — all ancestors above are already marked.
                    break;
                }
                current = arena.parent(pid);
            }
        }
    }

    /// Clear the filter.
    pub fn clear(&mut self) {
        self.query.clear();
        self.active = false;
        self.visible_set.clear();
        // Don't shrink matching_buf — keep capacity for next filter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtual_table::row::CellValue;
    use crate::virtual_tree::node::VirtualTreeNode;

    /// Minimal test node for filter tests.
    struct TestNode(String);

    impl VirtualTreeNode for TestNode {
        fn cell_value(&self, _col: usize) -> CellValue {
            CellValue::Text(self.0.clone())
        }
        fn set_cell_value(&mut self, _col: usize, value: &CellValue) {
            if let CellValue::Text(s) = value { self.0 = s.clone(); }
        }
        fn has_children(&self) -> bool { false }
        fn matches_filter(&self, query: &str) -> bool {
            self.0.to_lowercase().contains(&query.to_lowercase())
        }
    }

    #[test]
    fn empty_query_clears_filter() {
        let mut fs = FilterState::new();
        let mut arena = TreeArena::new();
        arena.insert_root(TestNode("hello".into()));
        fs.set_filter("hello", &mut arena, false);
        assert!(fs.active);
        fs.set_filter("  ", &mut arena, false);
        assert!(!fs.active);
    }

    #[test]
    fn filter_matches_nodes() {
        let mut fs = FilterState::new();
        let mut arena = TreeArena::new();
        let a = arena.insert_root(TestNode("apple".into())).unwrap();
        let b = arena.insert_root(TestNode("banana".into())).unwrap();
        let _c = arena.insert_root(TestNode("cherry".into())).unwrap();

        fs.set_filter("an", &mut arena, false);
        assert!(fs.active);
        assert!(!fs.is_visible(a)); // "apple" doesn't contain "an"
        assert!(fs.is_visible(b));  // "banana" contains "an"
    }

    #[test]
    fn filter_auto_expands_ancestors() {
        let mut fs = FilterState::new();
        let mut arena = TreeArena::new();
        let root = arena.insert_root(TestNode("root".into())).unwrap();
        let child = arena.insert_child(root, TestNode("child".into())).unwrap();
        let leaf = arena.insert_child(child, TestNode("target".into())).unwrap();

        // Root and child should not be expanded initially
        assert!(!arena.is_expanded(root));
        assert!(!arena.is_expanded(child));

        fs.set_filter("target", &mut arena, true); // auto_expand = true
        assert!(fs.is_visible(leaf));
        assert!(fs.is_visible(child)); // ancestor of match
        assert!(fs.is_visible(root));  // ancestor of match
        assert!(arena.is_expanded(root));  // auto-expanded
        assert!(arena.is_expanded(child)); // auto-expanded
    }

    #[test]
    fn filter_clear_resets_state() {
        let mut fs = FilterState::new();
        let mut arena = TreeArena::new();
        arena.insert_root(TestNode("hello".into()));
        fs.set_filter("hello", &mut arena, false);
        assert!(fs.active);

        fs.clear();
        assert!(!fs.active);
        assert!(fs.query.is_empty());
    }

    #[test]
    fn is_visible_out_of_bounds_returns_false() {
        let fs = FilterState::new();
        let fake_id = NodeId { index: 999, generation: 0 };
        assert!(!fs.is_visible(fake_id));
    }

    #[test]
    fn filter_no_auto_expand_early_break() {
        let mut fs = FilterState::new();
        let mut arena = TreeArena::new();
        let root = arena.insert_root(TestNode("root".into())).unwrap();
        let child = arena.insert_child(root, TestNode("child".into())).unwrap();
        let _leaf = arena.insert_child(child, TestNode("match_me".into())).unwrap();

        fs.set_filter("match_me", &mut arena, false); // auto_expand = false
        assert!(!arena.is_expanded(root));  // NOT expanded (auto_expand off)
        assert!(fs.is_visible(root));       // but still visible (ancestor of match)
    }
}
