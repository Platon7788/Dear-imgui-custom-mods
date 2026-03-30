//! Sibling-scoped sort state for the tree.
//!
//! Unlike flat-table sorting that reorders the entire dataset,
//! tree sorting operates independently on each sibling group
//! (children of the same parent are sorted among themselves).

use std::cmp::Ordering;

use super::arena::TreeArena;
use super::node::VirtualTreeNode;

/// Cached sort specification from Dear ImGui table headers.
#[derive(Clone, Debug)]
pub(crate) struct SortSpec {
    pub column_index: usize,
    pub ascending: bool,
}

/// Manages sort specs and applies sibling-scoped sorting.
#[derive(Clone, Debug, Default)]
pub(crate) struct SortState {
    pub specs: Vec<SortSpec>,
}

impl SortState {
    /// Compare two nodes using the current sort specs.
    fn compare<T: VirtualTreeNode>(&self, a: &T, b: &T) -> Ordering {
        for spec in &self.specs {
            let ord = a.compare(b, spec.column_index);
            let ord = if spec.ascending { ord } else { ord.reverse() };
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    }

    /// Sort all sibling groups in the arena using current specs.
    pub fn sort_all<T: VirtualTreeNode>(&self, arena: &mut TreeArena<T>) {
        if self.specs.is_empty() {
            return;
        }
        let mut cmp = |a: &T, b: &T| self.compare(a, b);
        arena.sort_all_siblings(&mut cmp);
    }
}
