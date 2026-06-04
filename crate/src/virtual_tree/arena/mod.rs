//! Generational slab arena for tree nodes.
//!
//! O(1) insert, O(1) remove, O(1) lookup by [`NodeId`].
//! Each node stores parent/children links, expand state, and cached depth.
//!
//! ## Capacity
//!
//! Hard limit: [`MAX_TREE_NODES`] (10,000,000). Insertions beyond this return `None`.

mod ops;

/// Maximum number of nodes a single tree can hold.
/// At 10M nodes the arena consumes ~560 MB, flat view ~120 MB (~680 MB total).
/// Flat-view rebuild: O(visible_nodes); filter: O(n) + O(matches × depth).
pub const MAX_TREE_NODES: usize = 10_000_000;

// ─── NodeId ─────────────────────────────────────────────────────────────────

/// Opaque handle into the arena. Copy + Eq + Hash.
/// Generational — a stale ID from a removed node safely returns `None`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

// ─── NodeSlot ───────────────────────────────────────────────────────────────

/// Internal slot storing user data alongside tree metadata.
pub struct NodeSlot<T> {
    pub data: T,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub expanded: bool,
    pub depth: u16,
    /// Lazy loading: `true` once this branch's children have been materialized
    /// by the `set_lazy_loader` callback, so it is never reloaded. Always
    /// `false` for eager trees.
    pub children_loaded: bool,
}

// ─── TreeArena ──────────────────────────────────────────────────────────────

/// Generational slab arena with parent/children links.
pub struct TreeArena<T> {
    slots: Vec<Option<NodeSlot<T>>>,
    generations: Vec<u32>,
    free_list: Vec<u32>,
    roots: Vec<NodeId>,
    count: usize,
    /// Per-instance capacity limit (≤ MAX_TREE_NODES).
    capacity: usize,
    /// When true, evict oldest root subtree on overflow instead of returning None.
    evict_on_overflow: bool,
}

impl<T> TreeArena<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            generations: Vec::new(),
            free_list: Vec::new(),
            roots: Vec::new(),
            count: 0,
            capacity: MAX_TREE_NODES,
            evict_on_overflow: false,
        }
    }

    /// Create an arena pre-allocated for `capacity` nodes.
    /// The capacity is clamped to `1..=MAX_TREE_NODES`.
    /// Avoids repeated re-allocation during bulk inserts.
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.clamp(1, MAX_TREE_NODES);
        Self {
            slots: Vec::with_capacity(cap),
            generations: Vec::with_capacity(cap),
            free_list: Vec::new(),
            roots: Vec::with_capacity(cap / 10 + 1),
            count: 0,
            capacity: cap,
            evict_on_overflow: false,
        }
    }

    /// Set the maximum number of nodes this arena can hold.
    /// Clamped to `1..=MAX_TREE_NODES`. Does **not** evict existing nodes
    /// if the current count already exceeds the new limit.
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.clamp(1, MAX_TREE_NODES);
    }

    /// Current capacity limit.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Enable or disable automatic eviction of the oldest root subtree
    /// when the arena is at capacity.
    pub fn set_evict_on_overflow(&mut self, enabled: bool) {
        self.evict_on_overflow = enabled;
    }

    /// Whether eviction-on-overflow is enabled.
    #[inline]
    pub fn evict_on_overflow(&self) -> bool {
        self.evict_on_overflow
    }

    // ─── Allocation ─────────────────────────────────────────────────

    /// Allocate a slot, returning a valid NodeId.
    ///
    /// If at capacity and `evict_on_overflow` is enabled, the oldest root subtree
    /// is removed first. Otherwise returns `None`.
    fn alloc(&mut self, data: T, parent: Option<NodeId>, depth: u16) -> Option<NodeId> {
        if self.count >= self.capacity {
            if self.evict_on_overflow {
                self.evict_oldest_root();
                // After eviction, if still at capacity (shouldn't happen normally),
                // give up to avoid infinite loops.
                if self.count >= self.capacity {
                    return None;
                }
            } else {
                return None;
            }
        }
        let slot = NodeSlot {
            data,
            parent,
            children: Vec::new(),
            expanded: false,
            depth,
            children_loaded: false,
        };

        if let Some(idx) = self.free_list.pop() {
            let i = idx as usize;
            self.generations[i] = self.generations[i].wrapping_add(1);
            self.slots[i] = Some(slot);
            self.count += 1;
            Some(NodeId {
                index: idx,
                generation: self.generations[i],
            })
        } else {
            let idx = self.slots.len() as u32;
            self.slots.push(Some(slot));
            self.generations.push(0);
            self.count += 1;
            Some(NodeId {
                index: idx,
                generation: 0,
            })
        }
    }

    // ─── Insert ─────────────────────────────────────────────────────

    /// Insert a new root node at the end of the roots list.
    /// Returns `None` if the arena is at capacity ([`MAX_TREE_NODES`]).
    pub fn insert_root(&mut self, data: T) -> Option<NodeId> {
        let id = self.alloc(data, None, 0)?;
        self.roots.push(id);
        Some(id)
    }

    /// Insert a new root node at a specific position.
    /// Returns `None` if the arena is at capacity ([`MAX_TREE_NODES`]).
    pub fn insert_root_at(&mut self, index: usize, data: T) -> Option<NodeId> {
        let pos = index.min(self.roots.len());
        let id = self.alloc(data, None, 0)?;
        self.roots.insert(pos, id);
        Some(id)
    }

    /// Insert a child node at the end of parent's children.
    /// Returns `None` if parent is invalid or arena is at capacity.
    pub fn insert_child(&mut self, parent: NodeId, data: T) -> Option<NodeId> {
        let parent_depth = self.get(parent)?.depth;
        let id = self.alloc(data, Some(parent), parent_depth.saturating_add(1))?;
        self.slot_mut(parent)?.children.push(id);
        Some(id)
    }

    /// Insert a child node at a specific position among siblings.
    /// Returns `None` if parent is invalid or arena is at capacity.
    pub fn insert_child_at(&mut self, parent: NodeId, index: usize, data: T) -> Option<NodeId> {
        let parent_depth = self.get(parent)?.depth;
        let id = self.alloc(data, Some(parent), parent_depth.saturating_add(1))?;
        let children = &mut self.slot_mut(parent)?.children;
        let pos = index.min(children.len());
        children.insert(pos, id);
        Some(id)
    }

    // ─── Eviction ──────────────────────────────────────────────────

    /// Remove the oldest root subtree (first root + all its descendants).
    /// Used internally when `evict_on_overflow` is enabled.
    fn evict_oldest_root(&mut self) {
        if let Some(&oldest_root) = self.roots.first() {
            self.remove(oldest_root);
        }
    }

    // ─── Remove ─────────────────────────────────────────────────────

    /// Remove a node and all its descendants. Returns the removed node's data.
    ///
    /// Uses iterative DFS to avoid stack overflow on deep trees.
    pub fn remove(&mut self, id: NodeId) -> Option<T> {
        // Detach from parent first — use position + swap_remove for O(1).
        if let Some(parent_id) = self.get(id)?.parent {
            if let Some(parent_slot) = self.slot_mut(parent_id)
                && let Some(pos) = parent_slot.children.iter().position(|&c| c == id)
            {
                parent_slot.children.swap_remove(pos);
            }
        } else {
            // It's a root — swap_remove is OK since root order may change.
            if let Some(pos) = self.roots.iter().position(|&r| r == id) {
                self.roots.swap_remove(pos);
            }
        }

        // Iterative DFS to free the node and all descendants.
        let mut stack = vec![id];
        let mut root_data: Option<T> = None;

        while let Some(nid) = stack.pop() {
            // Take children and push them onto the stack
            if let Some(slot) = self.slot_mut(nid) {
                let children = std::mem::take(&mut slot.children);
                stack.extend(children);
            }
            // Free the slot
            if let Some(slot) = self
                .slots
                .get_mut(nid.index as usize)
                .and_then(|s| s.take())
            {
                self.free_list.push(nid.index);
                self.count -= 1;
                if nid == id {
                    root_data = Some(slot.data);
                }
            }
        }

        root_data
    }

    /// Remove all nodes.
    pub fn clear(&mut self) {
        self.slots.clear();
        self.generations.clear();
        self.free_list.clear();
        self.roots.clear();
        self.count = 0;
    }

    // ─── Access ─────────────────────────────────────────────────────

    /// Get a reference to the node slot (generation-checked).
    #[inline]
    pub(crate) fn get(&self, id: NodeId) -> Option<&NodeSlot<T>> {
        let i = id.index as usize;
        if i >= self.slots.len() || self.generations[i] != id.generation {
            return None;
        }
        self.slots[i].as_ref()
    }

    /// Get a mutable reference to the node slot (generation-checked).
    #[inline]
    pub(crate) fn slot_mut(&mut self, id: NodeId) -> Option<&mut NodeSlot<T>> {
        let i = id.index as usize;
        if i >= self.slots.len() || self.generations[i] != id.generation {
            return None;
        }
        self.slots[i].as_mut()
    }

    /// Get a reference to the user data.
    #[inline]
    pub fn get_data(&self, id: NodeId) -> Option<&T> {
        self.get(id).map(|s| &s.data)
    }

    /// Get a mutable reference to the user data.
    #[inline]
    pub fn get_data_mut(&mut self, id: NodeId) -> Option<&mut T> {
        self.slot_mut(id).map(|s| &mut s.data)
    }

    /// Parent of a node.
    #[inline]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.get(id)?.parent
    }

    /// Children of a node (slice).
    #[inline]
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.get(id).map_or(&[], |s| &s.children)
    }

    /// Top-level root nodes.
    #[inline]
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// Cached depth of a node (0 = root).
    #[inline]
    pub fn depth(&self, id: NodeId) -> Option<u16> {
        self.get(id).map(|s| s.depth)
    }

    /// Whether the node is expanded.
    #[inline]
    pub fn is_expanded(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|s| s.expanded)
    }

    /// Number of live nodes.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.count
    }

    /// Total slot count (live + freed). Used to size index-based data structures.
    #[inline]
    pub fn slot_len(&self) -> usize {
        self.slots.len()
    }

    // ─── Expand / Collapse ──────────────────────────────────────────

    /// Expand a node (show children in flat view).
    pub fn expand(&mut self, id: NodeId) {
        if let Some(slot) = self.slot_mut(id) {
            slot.expanded = true;
        }
    }

    /// Collapse a node (hide children in flat view).
    pub fn collapse(&mut self, id: NodeId) {
        if let Some(slot) = self.slot_mut(id) {
            slot.expanded = false;
        }
    }

    /// Toggle expand/collapse.
    pub fn toggle(&mut self, id: NodeId) {
        if let Some(slot) = self.slot_mut(id) {
            slot.expanded = !slot.expanded;
        }
    }

    /// Expand all ancestors so that `id` becomes visible.
    pub fn ensure_visible(&mut self, id: NodeId) {
        let mut current = self.get(id).and_then(|s| s.parent);
        while let Some(pid) = current {
            if let Some(slot) = self.slot_mut(pid) {
                slot.expanded = true;
                current = slot.parent;
            } else {
                break;
            }
        }
    }

    /// Expand all nodes recursively.
    pub fn expand_all(&mut self) {
        for s in self.slots.iter_mut().flatten() {
            s.expanded = true;
        }
    }

    /// Collapse all nodes.
    pub fn collapse_all(&mut self) {
        for s in self.slots.iter_mut().flatten() {
            s.expanded = false;
        }
    }
}

impl<T> Default for TreeArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
