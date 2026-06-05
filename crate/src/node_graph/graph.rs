//! Graph data structure — nodes and wires.
//!
//! Framework-agnostic storage. `Graph<T>` holds nodes in a slab (Vec with
//! free-list for O(1) insert/remove) and wires in a `HashSet`.

use std::collections::HashSet;

use super::types::{Comment, InPinId, NodeId, OutPinId, Wire};

// ─── Node wrapper ────────────────────────────────────────────────────────────

/// A node in the graph: user payload `T` + position + visual state.
pub struct Node<T> {
    /// User-defined node data.
    pub value: T,
    /// Position in graph space.
    pub pos: [f32; 2],
    /// Whether the node body is expanded (true) or collapsed.
    pub open: bool,
}

// ─── Slab entry ──────────────────────────────────────────────────────────────

enum SlabEntry<T> {
    Occupied(Node<T>),
    Vacant(Option<u32>), // next free index
}

// ─── Graph ───────────────────────────────────────────────────────────────────

/// Core graph data: nodes (slab) + wires (hash set).
///
/// Generic over the user's node type `T`.
pub struct Graph<T> {
    nodes: Vec<SlabEntry<T>>,
    free_head: Option<u32>,
    node_count: u32,
    wires: HashSet<Wire>,
    /// Free-floating annotation rectangles drawn behind nodes.
    /// Addressed by index; independent of the node slab and `T`.
    comments: Vec<Comment>,
}

impl<T> Default for Graph<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Graph<T> {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(32),
            free_head: None,
            node_count: 0,
            wires: HashSet::with_capacity(64),
            comments: Vec::new(),
        }
    }

    // ── Node operations ──────────────────────────────────────────────────

    /// Insert a node at the given position. Returns its [`NodeId`].
    pub fn insert_node(&mut self, value: T, pos: [f32; 2]) -> NodeId {
        let node = Node {
            value,
            pos,
            open: true,
        };
        let id = if let Some(idx) = self.free_head {
            // Reuse a vacant slot
            let entry = &mut self.nodes[idx as usize];
            let next = match entry {
                SlabEntry::Vacant(next) => *next,
                SlabEntry::Occupied(_) => unreachable!(),
            };
            *entry = SlabEntry::Occupied(node);
            self.free_head = next;
            NodeId(idx)
        } else {
            let idx = self.nodes.len() as u32;
            self.nodes.push(SlabEntry::Occupied(node));
            NodeId(idx)
        };
        self.node_count += 1;
        id
    }

    /// Remove a node and all its wires. Returns the user payload if the node existed.
    pub fn remove_node(&mut self, id: NodeId) -> Option<T> {
        let idx = id.0 as usize;
        if idx >= self.nodes.len() {
            return None;
        }
        match &self.nodes[idx] {
            SlabEntry::Vacant(_) => return None,
            SlabEntry::Occupied(_) => {}
        }

        // Remove all wires connected to this node
        self.wires
            .retain(|w| w.out_pin.node != id && w.in_pin.node != id);

        let old = std::mem::replace(&mut self.nodes[idx], SlabEntry::Vacant(self.free_head));
        self.free_head = Some(id.0);
        self.node_count -= 1;

        match old {
            SlabEntry::Occupied(n) => Some(n.value),
            SlabEntry::Vacant(_) => unreachable!(),
        }
    }

    /// Get a reference to a node.
    #[inline]
    pub fn get_node(&self, id: NodeId) -> Option<&Node<T>> {
        self.nodes.get(id.0 as usize).and_then(|e| match e {
            SlabEntry::Occupied(n) => Some(n),
            SlabEntry::Vacant(_) => None,
        })
    }

    /// Get a mutable reference to a node.
    #[inline]
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut Node<T>> {
        self.nodes.get_mut(id.0 as usize).and_then(|e| match e {
            SlabEntry::Occupied(n) => Some(n),
            SlabEntry::Vacant(_) => None,
        })
    }

    /// Number of live nodes.
    #[inline]
    pub fn node_count(&self) -> u32 {
        self.node_count
    }

    /// Iterate over all live `(NodeId, &Node<T>)` pairs.
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &Node<T>)> {
        self.nodes.iter().enumerate().filter_map(|(i, e)| match e {
            SlabEntry::Occupied(n) => Some((NodeId(i as u32), n)),
            SlabEntry::Vacant(_) => None,
        })
    }

    /// Iterate over all live `(NodeId, &mut Node<T>)` pairs.
    pub fn nodes_mut(&mut self) -> impl Iterator<Item = (NodeId, &mut Node<T>)> {
        self.nodes
            .iter_mut()
            .enumerate()
            .filter_map(|(i, e)| match e {
                SlabEntry::Occupied(n) => Some((NodeId(i as u32), n)),
                SlabEntry::Vacant(_) => None,
            })
    }

    /// Collect all live node IDs (allocates a Vec — use for iteration that mutates).
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                SlabEntry::Occupied(_) => Some(NodeId(i as u32)),
                SlabEntry::Vacant(_) => None,
            })
            .collect()
    }

    // ── Wire operations ──────────────────────────────────────────────────

    /// Built-in connection sanity check, independent of the user's
    /// [`NodeGraphViewer::can_connect`](super::NodeGraphViewer::can_connect).
    ///
    /// Rejects connections that are *never* meaningful regardless of node
    /// semantics, so the renderer can refuse them before emitting a
    /// [`GraphAction::Connected`](super::GraphAction::Connected):
    ///
    /// - **self-loops** — an output pin wired to an input pin on the *same*
    ///   node (`from.node == to.node`). egui-snarl forbids this by default and
    ///   the default `can_connect` (which returns `true`) would otherwise let
    ///   it through.
    /// - **dangling endpoints** — either pin's node is not live in the slab.
    ///
    /// Duplicate-wire rejection is handled separately by the `HashSet` in
    /// [`Self::connect`]; type/cycle policy stays the user's responsibility via
    /// `can_connect`.
    #[must_use]
    pub fn can_connect_basic(&self, from: OutPinId, to: InPinId) -> bool {
        from.node != to.node
            && self.get_node(from.node).is_some()
            && self.get_node(to.node).is_some()
    }

    /// Connect an output pin to an input pin. Returns `true` if new.
    pub fn connect(&mut self, from: OutPinId, to: InPinId) -> bool {
        self.wires.insert(Wire {
            out_pin: from,
            in_pin: to,
        })
    }

    /// Disconnect a specific wire. Returns `true` if it existed.
    pub fn disconnect(&mut self, from: OutPinId, to: InPinId) -> bool {
        self.wires.remove(&Wire {
            out_pin: from,
            in_pin: to,
        })
    }

    /// Remove all wires connected to an input pin.
    pub fn drop_inputs(&mut self, pin: InPinId) {
        self.wires.retain(|w| w.in_pin != pin);
    }

    /// Remove all wires connected to an output pin.
    pub fn drop_outputs(&mut self, pin: OutPinId) {
        self.wires.retain(|w| w.out_pin != pin);
    }

    /// All wires in the graph.
    #[inline]
    pub fn wires(&self) -> &HashSet<Wire> {
        &self.wires
    }

    /// Number of wires.
    #[inline]
    pub fn wire_count(&self) -> usize {
        self.wires.len()
    }

    /// Get all output pins connected to a given input pin.
    pub fn input_remotes(&self, pin: InPinId) -> Vec<OutPinId> {
        self.wires
            .iter()
            .filter(|w| w.in_pin == pin)
            .map(|w| w.out_pin)
            .collect()
    }

    /// Get all input pins connected to a given output pin.
    pub fn output_remotes(&self, pin: OutPinId) -> Vec<InPinId> {
        self.wires
            .iter()
            .filter(|w| w.out_pin == pin)
            .map(|w| w.in_pin)
            .collect()
    }

    /// Check if a specific wire exists.
    #[inline]
    pub fn has_wire(&self, from: OutPinId, to: InPinId) -> bool {
        self.wires.contains(&Wire {
            out_pin: from,
            in_pin: to,
        })
    }

    // ── Comment operations ───────────────────────────────────────────────

    /// Add a comment box. Returns its index in the comment list.
    pub fn add_comment(&mut self, comment: Comment) -> usize {
        let index = self.comments.len();
        self.comments.push(comment);
        index
    }

    /// All comment boxes, in index order.
    #[inline]
    pub fn comments(&self) -> &[Comment] {
        &self.comments
    }

    /// Mutable access to the comment list (for bulk load + by-index edit).
    ///
    /// Note: removing or reordering elements here shifts the indices used by
    /// [`GraphAction::CommentChanged`](super::GraphAction::CommentChanged) and
    /// [`GraphAction::CommentMenu`](super::GraphAction::CommentMenu).
    #[inline]
    pub fn comments_mut(&mut self) -> &mut Vec<Comment> {
        &mut self.comments
    }

    /// Remove a comment box by index. No-op if the index is out of range.
    ///
    /// Subsequent comments shift down by one index.
    pub fn remove_comment(&mut self, index: usize) {
        if index < self.comments.len() {
            self.comments.remove(index);
        }
    }

    /// Remove all comment boxes.
    pub fn clear_comments(&mut self) {
        self.comments.clear();
    }

    /// Clear the entire graph.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.wires.clear();
        self.free_head = None;
        self.node_count = 0;
        self.clear_comments();
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────
//
// The test suite lives in a sibling file to keep this module under the
// 500-line cap (CLAUDE.md). `tests` is still a child of `graph`, so its
// `use super::*` reaches private items directly.

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
