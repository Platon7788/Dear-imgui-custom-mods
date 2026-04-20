//! Graph data structure — nodes and wires.
//!
//! Framework-agnostic storage. `Graph<T>` holds nodes in a slab (Vec with
//! free-list for O(1) insert/remove) and wires in a `HashSet`.

use std::collections::HashSet;

use super::types::{InPinId, NodeId, OutPinId, Wire};

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

    /// Clear the entire graph.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.wires.clear();
        self.free_head = None;
        self.node_count = 0;
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut g = Graph::new();
        let id = g.insert_node("hello", [10.0, 20.0]);
        assert_eq!(g.node_count(), 1);
        let node = g.get_node(id).unwrap();
        assert_eq!(node.value, "hello");
        assert_eq!(node.pos, [10.0, 20.0]);
        assert!(node.open);
    }

    #[test]
    fn remove_node_returns_value() {
        let mut g = Graph::new();
        let id = g.insert_node(42, [0.0, 0.0]);
        let val = g.remove_node(id);
        assert_eq!(val, Some(42));
        assert_eq!(g.node_count(), 0);
        assert!(g.get_node(id).is_none());
    }

    #[test]
    fn remove_nonexistent() {
        let mut g: Graph<i32> = Graph::new();
        let id = NodeId(99);
        assert!(g.remove_node(id).is_none());
    }

    #[test]
    fn slab_reuse() {
        let mut g = Graph::new();
        let a = g.insert_node("a", [0.0, 0.0]);
        let _b = g.insert_node("b", [1.0, 0.0]);
        g.remove_node(a);
        // Next insert should reuse slot 0
        let c = g.insert_node("c", [2.0, 0.0]);
        assert_eq!(c.index(), a.index());
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.get_node(c).unwrap().value, "c");
    }

    #[test]
    fn connect_disconnect() {
        let mut g = Graph::new();
        let a = g.insert_node("a", [0.0, 0.0]);
        let b = g.insert_node("b", [100.0, 0.0]);
        let out = OutPinId { node: a, output: 0 };
        let inp = InPinId { node: b, input: 0 };

        assert!(g.connect(out, inp));
        assert!(!g.connect(out, inp)); // duplicate
        assert_eq!(g.wire_count(), 1);
        assert!(g.has_wire(out, inp));

        assert!(g.disconnect(out, inp));
        assert_eq!(g.wire_count(), 0);
        assert!(!g.has_wire(out, inp));
    }

    #[test]
    fn remove_node_removes_wires() {
        let mut g = Graph::new();
        let a = g.insert_node("a", [0.0, 0.0]);
        let b = g.insert_node("b", [100.0, 0.0]);
        let c = g.insert_node("c", [200.0, 0.0]);
        g.connect(
            OutPinId { node: a, output: 0 },
            InPinId { node: b, input: 0 },
        );
        g.connect(
            OutPinId { node: b, output: 0 },
            InPinId { node: c, input: 0 },
        );
        assert_eq!(g.wire_count(), 2);
        g.remove_node(b);
        assert_eq!(g.wire_count(), 0);
    }

    #[test]
    fn input_output_remotes() {
        let mut g = Graph::new();
        let a = g.insert_node("a", [0.0, 0.0]);
        let b = g.insert_node("b", [100.0, 0.0]);
        let out = OutPinId { node: a, output: 0 };
        let inp = InPinId { node: b, input: 0 };
        g.connect(out, inp);

        assert_eq!(g.input_remotes(inp), vec![out]);
        assert_eq!(g.output_remotes(out), vec![inp]);
    }

    #[test]
    fn drop_inputs_outputs() {
        let mut g = Graph::new();
        let a = g.insert_node("a", [0.0, 0.0]);
        let b = g.insert_node("b", [100.0, 0.0]);
        let out0 = OutPinId { node: a, output: 0 };
        let out1 = OutPinId { node: a, output: 1 };
        let inp0 = InPinId { node: b, input: 0 };
        let inp1 = InPinId { node: b, input: 1 };
        g.connect(out0, inp0);
        g.connect(out1, inp1);
        assert_eq!(g.wire_count(), 2);

        g.drop_inputs(inp0);
        assert_eq!(g.wire_count(), 1);

        g.drop_outputs(out1);
        assert_eq!(g.wire_count(), 0);
    }

    #[test]
    fn nodes_iter() {
        let mut g = Graph::new();
        g.insert_node("a", [0.0, 0.0]);
        g.insert_node("b", [1.0, 0.0]);
        g.insert_node("c", [2.0, 0.0]);
        let ids: Vec<_> = g.nodes().map(|(id, _)| id).collect();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn node_ids_collect() {
        let mut g = Graph::new();
        let a = g.insert_node(1, [0.0, 0.0]);
        let _b = g.insert_node(2, [0.0, 0.0]);
        g.remove_node(a);
        let ids = g.node_ids();
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn clear_graph() {
        let mut g = Graph::new();
        g.insert_node("a", [0.0, 0.0]);
        let b = g.insert_node("b", [100.0, 0.0]);
        g.connect(
            OutPinId {
                node: NodeId(0),
                output: 0,
            },
            InPinId { node: b, input: 0 },
        );
        g.clear();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.wire_count(), 0);
    }

    #[test]
    fn get_node_mut() {
        let mut g = Graph::new();
        let id = g.insert_node("old", [0.0, 0.0]);
        g.get_node_mut(id).unwrap().value = "new";
        assert_eq!(g.get_node(id).unwrap().value, "new");
    }

    #[test]
    fn double_remove() {
        let mut g = Graph::new();
        let id = g.insert_node(1, [0.0, 0.0]);
        assert!(g.remove_node(id).is_some());
        assert!(g.remove_node(id).is_none());
        assert_eq!(g.node_count(), 0);
    }
}
