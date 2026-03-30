//! Filter/search state with auto-expand for matching branches.
//!
//! When a filter is active, only nodes that match (or are ancestors of matches)
//! are shown. Matching ancestors are auto-expanded.
//!
//! ## Performance (500K nodes)
//!
//! - Pass 1 (scan all nodes): O(n) — unavoidable, but each `matches_filter` is user-defined
//! - Pass 2 (mark ancestors): O(matches × depth) — with early-break when not auto-expanding
//! - Pre-allocated `matching` vec avoids repeated re-allocation

use std::collections::HashSet;

use super::arena::{NodeId, TreeArena};
use super::node::VirtualTreeNode;

// ─── FilterState ────────────────────────────────────────────────────────────

/// Tracks the active filter query and the set of visible nodes.
pub struct FilterState {
    pub query: String,
    pub active: bool,
    /// NodeIds that match the filter OR are ancestors of matches.
    pub visible_set: HashSet<NodeId, foldhash::fast::FixedState>,
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
            visible_set: HashSet::with_hasher(foldhash::fast::FixedState::default()),
            matching_buf: Vec::new(),
        }
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
        self.visible_set.clear();

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
            self.visible_set.insert(id);
            let mut current = arena.parent(id);
            while let Some(pid) = current {
                let was_new = self.visible_set.insert(pid);
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
