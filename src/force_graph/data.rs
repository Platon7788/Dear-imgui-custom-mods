//! Graph data model backed by [`slotmap`] for stable, O(1) node and edge
//! handles.
//!
//! [`GraphData`] is the *only* mutable state the caller needs to manage.
//! Build it up with [`GraphData::add_node`] / [`GraphData::add_edge`], then
//! hand an immutable reference to [`crate::force_graph::GraphViewer`] for
//! rendering. The physics simulation runs inside the viewer and updates
//! `pos` / `vel` through interior-mutable paths — the public API here is
//! purely structural.

use slotmap::{new_key_type, SlotMap};
use std::collections::{HashMap, HashSet};

use super::style::{Edge, EdgeStyle, NodeStyle};

// ─── Stable handles ───────────────────────────────────────────────────────────

new_key_type! {
    /// Stable handle to a node.
    ///
    /// Remains valid (and refers to the same node) after other nodes are
    /// inserted or removed. Invalidated only when *this* node is removed via
    /// [`GraphData::remove_node`].
    pub struct NodeId;
    /// Stable handle to an edge.
    ///
    /// Remains valid after other edges are inserted or removed. Invalidated
    /// when *this* edge is removed via [`GraphData::remove_edge`], or when
    /// either of its endpoint nodes is removed via
    /// [`GraphData::remove_node`].
    pub struct EdgeId;
}

// ─── Internal node record ─────────────────────────────────────────────────────

/// Internal per-node record: public style metadata + physics state.
///
/// The physics state (`pos`, `vel`) is private to the crate — callers interact
/// with it only indirectly through the viewer's simulation step.
pub(crate) struct Node {
    /// Visual and metadata style, fully owned.
    pub(crate) style: NodeStyle,
    /// Current canvas-space position (updated by the simulation).
    pub(crate) pos: [f32; 2],
    /// Current velocity (canvas units per simulation tick).
    pub(crate) vel: [f32; 2],
}

// ─── GraphData ────────────────────────────────────────────────────────────────

/// The graph model.
///
/// Build once (or stream updates) and hand to
/// [`crate::force_graph::GraphViewer`] for rendering and physics.
///
/// # Thread safety
///
/// `GraphData` is `Send` but not `Sync` — it is designed to live on the
/// render thread. If you need to build the graph on a background thread,
/// do so in a temporary `GraphData` and `std::mem::swap` it in before the
/// next frame.
pub struct GraphData {
    pub(crate) nodes: SlotMap<NodeId, Node>,
    pub(crate) edges: SlotMap<EdgeId, Edge>,
    /// Adjacency list: node → list of incident edge IDs (both directions).
    pub(crate) adjacency: HashMap<NodeId, Vec<EdgeId>>,
    /// Set when any structural mutation occurs — wakes the physics simulation.
    pub(crate) dirty: bool,
    /// Set when graph topology changes and metrics (PageRank, communities,
    /// betweenness) need recomputing.
    pub(crate) dirty_metrics: bool,
    /// Cached unique tag set (rebuilt lazily from `tag_cache_dirty`).
    pub(crate) tag_cache: Vec<&'static str>,
    pub(crate) tag_cache_dirty: bool,
    /// Cached metrics (computed lazily when dirty_metrics is set).
    pub(crate) metrics: Option<MetricsCache>,
}

/// Cached graph analytics computed by Phase-C metrics modules.
pub(crate) struct MetricsCache {
    /// PageRank score per node, indexed by `index`.
    pub(crate) pagerank: Vec<f32>,
    /// Betweenness centrality score per node, indexed by `index`.
    pub(crate) betweenness: Vec<f32>,
    /// O(1) NodeId → score-array index for `pagerank_for` / `betweenness_for`.
    pub(crate) index: HashMap<NodeId, usize>,
}

// ─── Initial position helper ──────────────────────────────────────────────────

/// Compute a deterministic initial canvas-space position for the `count`-th
/// node using a golden-angle spiral.
///
/// The spiral distributes nodes evenly in a disc that grows with `√count`,
/// avoiding the "all nodes start at origin" pathology that causes extreme
/// initial forces.
fn initial_pos(count: usize) -> [f32; 2] {
    // Golden angle ≈ 2.399 963 rad  (2π · (1 − 1/φ))
    const GOLDEN_ANGLE: f32 = 2.399_963;
    let r = (count as f32).sqrt() * 15.0;
    let angle = count as f32 * GOLDEN_ANGLE;
    [r * angle.cos(), r * angle.sin()]
}

// ─── impl GraphData ───────────────────────────────────────────────────────────

impl GraphData {
    /// Create an empty graph with default internal capacities.
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            edges: SlotMap::with_key(),
            adjacency: HashMap::new(),
            dirty: false,
            dirty_metrics: false,
            tag_cache: Vec::new(),
            tag_cache_dirty: false,
            metrics: None,
        }
    }

    /// Create an empty graph with pre-allocated capacity hints.
    ///
    /// Avoids reallocation when the approximate final node and edge counts are
    /// known upfront.
    pub fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self {
            nodes: SlotMap::with_capacity_and_key(nodes),
            edges: SlotMap::with_capacity_and_key(edges),
            adjacency: HashMap::with_capacity(nodes),
            dirty: false,
            dirty_metrics: false,
            tag_cache: Vec::new(),
            tag_cache_dirty: false,
            metrics: None,
        }
    }

    // ── Mutation ─────────────────────────────────────────────────────────────

    /// Add a node with the given style and return its stable [`NodeId`].
    ///
    /// The node's initial canvas position is determined by a golden-angle
    /// spiral seeded on the current node count, giving a reasonable spread
    /// without a random number generator.
    pub fn add_node(&mut self, style: NodeStyle) -> NodeId {
        let count = self.nodes.len();
        let pos = initial_pos(count);
        let id = self.nodes.insert(Node {
            style,
            pos,
            vel: [0.0, 0.0],
        });
        self.adjacency.entry(id).or_default();
        self.dirty = true;
        self.dirty_metrics = true;
        self.tag_cache_dirty = true;
        id
    }

    /// Add a directed or undirected edge between nodes `a` and `b`.
    ///
    /// Returns `Some(EdgeId)` on success, or `None` if either node ID is
    /// invalid (has been removed). A `debug_assert!` fires in debug builds
    /// so callers catch invalid IDs during development; release builds degrade
    /// gracefully.
    ///
    /// The edge's directed flag and weight come from the `directed` and
    /// `weight` parameters; visual overrides (colour, dash, label) come from
    /// `style`.
    ///
    /// `weight` is clamped to `[0.0, 1.0]` — it drives edge thickness and
    /// spring-force strength; values outside that range are silently clamped.
    pub fn add_edge(
        &mut self,
        a: NodeId,
        b: NodeId,
        style: EdgeStyle,
        weight: f32,
        directed: bool,
    ) -> Option<EdgeId> {
        if !self.nodes.contains_key(a) || !self.nodes.contains_key(b) {
            return None;
        }
        let edge = Edge {
            from: a,
            to: b,
            directed,
            weight: weight.clamp(0.0, 1.0),
            style,
        };
        let eid = self.edges.insert(edge);
        self.adjacency.entry(a).or_default().push(eid);
        if a != b {
            self.adjacency.entry(b).or_default().push(eid);
        }
        self.dirty = true;
        self.dirty_metrics = true;
        Some(eid)
    }

    /// Remove a node and all edges incident to it.
    ///
    /// This is an O(degree) operation — it walks the node's adjacency list to
    /// clean up the opposite endpoint's adjacency list for each removed edge.
    ///
    /// If `id` is not a valid node handle, this is a no-op.
    pub fn remove_node(&mut self, id: NodeId) {
        if !self.nodes.contains_key(id) {
            return;
        }

        // Collect the edge IDs before mutating, to avoid borrow conflicts.
        let incident: Vec<EdgeId> = self
            .adjacency
            .get(&id)
            .cloned()
            .unwrap_or_default();

        for eid in incident {
            // Identify the *other* endpoint and remove `eid` from its list.
            if let Some(edge) = self.edges.get(eid) {
                let other = if edge.from == id { edge.to } else { edge.from };
                if let Some(adj) = self.adjacency.get_mut(&other) {
                    adj.retain(|&e| e != eid);
                }
            }
            self.edges.remove(eid);
        }

        self.adjacency.remove(&id);
        self.nodes.remove(id);

        self.dirty = true;
        self.dirty_metrics = true;
        self.tag_cache_dirty = true;
    }

    /// Remove a single edge.
    ///
    /// Updates both endpoints' adjacency lists. If `id` is not a valid edge
    /// handle, this is a no-op.
    pub fn remove_edge(&mut self, id: EdgeId) {
        if let Some(edge) = self.edges.remove(id) {
            if let Some(adj) = self.adjacency.get_mut(&edge.from) {
                adj.retain(|&e| e != id);
            }
            if edge.from != edge.to
                && let Some(adj) = self.adjacency.get_mut(&edge.to)
            {
                adj.retain(|&e| e != id);
            }
            self.dirty = true;
            self.dirty_metrics = true;
        }
    }

    /// Remove all nodes and edges and reset all dirty flags.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.adjacency.clear();
        self.tag_cache.clear();
        self.dirty = false;
        self.dirty_metrics = false;
        self.tag_cache_dirty = false;
    }

    // ── Read-only access ─────────────────────────────────────────────────────

    /// Return an immutable reference to the style of node `id`, or `None` if
    /// the ID is invalid.
    pub fn node(&self, id: NodeId) -> Option<&NodeStyle> {
        self.nodes.get(id).map(|n| &n.style)
    }

    /// Return a mutable reference to the style of node `id`, or `None` if the
    /// ID is invalid.
    ///
    /// Marks `dirty_metrics` and `tag_cache_dirty` so analytics and the tag
    /// cache are recomputed on the next render. Use [`Self::node_set_pinned`]
    /// when only toggling the `pinned` flag to avoid an unnecessary O(V·E)
    /// metrics recompute.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut NodeStyle> {
        self.dirty_metrics = true;
        self.tag_cache_dirty = true;
        self.nodes.get_mut(id).map(|n| &mut n.style)
    }

    /// Toggle the `pinned` flag on node `id` without invalidating metrics.
    ///
    /// Preferred over [`Self::node_mut`] when the only change is pinning/
    /// unpinning, since metrics (PageRank, betweenness) are unaffected by the
    /// pinned state.
    pub fn node_set_pinned(&mut self, id: NodeId, pinned: bool) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.style.pinned = pinned;
        }
    }

    /// Return an immutable reference to edge `id`, or `None` if invalid.
    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Iterate over all nodes, yielding `(NodeId, &NodeStyle)` pairs.
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &NodeStyle)> {
        self.nodes.iter().map(|(id, n)| (id, &n.style))
    }

    /// Iterate over all edges, yielding `(EdgeId, &Edge)` pairs.
    pub fn edges(&self) -> impl Iterator<Item = (EdgeId, &Edge)> {
        self.edges.iter()
    }

    /// Iterate over the [`NodeId`]s of all neighbours of `id`.
    ///
    /// A neighbour is any node that shares an edge with `id`, regardless of
    /// edge direction. Returns an empty iterator for invalid IDs.
    pub fn neighbors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.adjacency
            .get(&id)
            .into_iter()
            .flat_map(move |eids| {
                eids.iter().filter_map(move |&eid| {
                    self.edges.get(eid).map(|e| {
                        if e.from == id { e.to } else { e.from }
                    })
                })
            })
    }

    /// Return the degree (number of incident edges) of node `id`.
    ///
    /// Returns `0` for invalid IDs.
    pub fn degree(&self, id: NodeId) -> usize {
        self.adjacency.get(&id).map_or(0, Vec::len)
    }

    /// Total number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Total number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    // ── Metrics stubs (Phase B) ───────────────────────────────────────────────

    /// Ensure metrics are up to date, recomputing from scratch when
    /// `dirty_metrics` is set.
    ///
    /// Call before accessing [`Self::pagerank_for`] or [`Self::betweenness_for`].
    pub fn recompute_metrics_if_needed(&mut self) {
        if self.dirty_metrics || self.metrics.is_none() {
            use super::metrics::{centrality, pagerank};

            // Build node order + index once; both algorithms reuse it.
            let node_order: Vec<NodeId> = self.nodes.keys().collect();
            let index: HashMap<NodeId, usize> = node_order
                .iter()
                .enumerate()
                .map(|(i, &id)| (id, i))
                .collect();

            // Build index-based undirected adjacency once; centrality reuses it.
            let adj: Vec<Vec<usize>> = node_order
                .iter()
                .map(|&id| {
                    self.neighbors(id)
                        .filter_map(|nbr| index.get(&nbr).copied())
                        .collect()
                })
                .collect();

            let (_, pr) = pagerank::compute(self, 0.85, 100, 1e-6);
            let bt = centrality::compute_with_adj(&node_order, &adj);

            self.metrics = Some(MetricsCache { pagerank: pr, betweenness: bt, index });
            self.dirty_metrics = false;
        }
    }

    /// PageRank score for a specific node (0.0 if metrics not computed yet).
    pub fn pagerank_for(&self, id: NodeId) -> f32 {
        self.metrics.as_ref().and_then(|m| {
            m.index.get(&id).map(|&i| m.pagerank[i])
        }).unwrap_or(0.0)
    }

    /// Betweenness centrality score for a specific node (0.0 if not computed).
    pub fn betweenness_for(&self, id: NodeId) -> f32 {
        self.metrics.as_ref().and_then(|m| {
            m.index.get(&id).map(|&i| m.betweenness[i])
        }).unwrap_or(0.0)
    }

    /// Per-node community assignment (integer community ID).
    ///
    /// *Phase D stub* — Louvain implementation pending.
    pub fn community_assignment(&self) -> &[u32] {
        &[]
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    /// Return the cached set of unique tags across all nodes.
    ///
    /// Rebuilds the cache when `tag_cache_dirty` is set. The result is stable
    /// (same tags in the same order) between rebuilds — within a single rebuild
    /// order depends on SlotMap iteration order.
    #[allow(dead_code)] // Phase B sidebar tag-filter will call this
    pub(crate) fn unique_tags(&mut self) -> &[&'static str] {
        if self.tag_cache_dirty {
            self.tag_cache.clear();
            let mut seen: HashSet<&'static str> = HashSet::new();
            for (_, node) in &self.nodes {
                for &tag in &node.style.tags {
                    if seen.insert(tag) {
                        self.tag_cache.push(tag);
                    }
                }
            }
            self.tag_cache_dirty = false;
        }
        &self.tag_cache
    }
}

impl Default for GraphData {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::force_graph::style::EdgeStyle;

    fn make_edge_style() -> EdgeStyle {
        EdgeStyle::new()
    }

    #[test]
    fn add_1000_nodes_remove_half_check_counts() {
        let mut g = GraphData::with_capacity(1000, 0);
        let ids: Vec<NodeId> = (0..1000)
            .map(|i| g.add_node(NodeStyle::new(format!("n{i}"))))
            .collect();

        assert_eq!(g.node_count(), 1000);

        for id in ids.iter().step_by(2) {
            g.remove_node(*id);
        }

        assert_eq!(g.node_count(), 500);
    }

    #[test]
    fn remove_node_removes_associated_edges() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        let c = g.add_node(NodeStyle::new("C"));

        g.add_edge(a, b, make_edge_style(), 1.0, false);
        g.add_edge(a, c, make_edge_style(), 1.0, false);
        assert_eq!(g.edge_count(), 2);

        g.remove_node(a);
        assert_eq!(g.edge_count(), 0);
        // b and c adjacency lists must be empty
        assert_eq!(g.degree(b), 0);
        assert_eq!(g.degree(c), 0);
    }

    #[test]
    fn id_stability_after_removes() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        let c = g.add_node(NodeStyle::new("C"));

        g.remove_node(b);

        // a and c must still be accessible
        assert!(g.node(a).is_some());
        assert!(g.node(c).is_some());
        assert!(g.node(b).is_none());
    }

    #[test]
    fn neighbors_on_star_graph() {
        let mut g = GraphData::new();
        let center = g.add_node(NodeStyle::new("center"));
        let spokes: Vec<NodeId> = (0..5)
            .map(|i| g.add_node(NodeStyle::new(format!("spoke{i}"))))
            .collect();

        for &s in &spokes {
            g.add_edge(center, s, make_edge_style(), 1.0, false);
        }

        let mut nbrs: Vec<NodeId> = g.neighbors(center).collect();
        nbrs.sort_unstable();

        assert_eq!(nbrs.len(), 5);

        // Each spoke should have exactly one neighbor: center
        for &s in &spokes {
            let n: Vec<NodeId> = g.neighbors(s).collect();
            assert_eq!(n.len(), 1);
            assert_eq!(n[0], center);
        }
    }

    #[test]
    fn add_edge_with_invalid_node_id_does_not_panic() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        g.remove_node(a); // a is now invalid

        let b = g.add_node(NodeStyle::new("B"));
        let result = g.add_edge(a, b, make_edge_style(), 1.0, false);
        assert!(result.is_none());
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn clear_resets_counts_to_zero() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        g.add_edge(a, b, make_edge_style(), 0.5, true);

        g.clear();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn degree_matches_adjacency_len() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        let c = g.add_node(NodeStyle::new("C"));

        g.add_edge(a, b, make_edge_style(), 1.0, false);
        g.add_edge(a, c, make_edge_style(), 1.0, false);

        assert_eq!(g.degree(a), 2);
        assert_eq!(g.degree(b), 1);
        assert_eq!(g.degree(c), 1);
    }

    #[test]
    fn remove_edge_cleans_both_adjacency_lists() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        let eid = g.add_edge(a, b, make_edge_style(), 1.0, false).unwrap();

        g.remove_edge(eid);
        assert_eq!(g.edge_count(), 0);
        assert_eq!(g.degree(a), 0);
        assert_eq!(g.degree(b), 0);
    }

    #[test]
    fn unique_tags_deduplicates() {
        let mut g = GraphData::new();
        g.add_node(NodeStyle::new("A").with_tag("core").with_tag("ui"));
        g.add_node(NodeStyle::new("B").with_tag("core").with_tag("data"));

        let tags = g.unique_tags().to_vec();
        assert!(tags.contains(&"core"));
        assert!(tags.contains(&"ui"));
        assert!(tags.contains(&"data"));
        // "core" must appear only once
        assert_eq!(tags.iter().filter(|&&t| t == "core").count(), 1);
    }

    #[test]
    fn initial_pos_zero_node_is_origin_adjacent() {
        let pos = super::initial_pos(0);
        // r = 0.0 * 15.0 = 0.0 → both components zero
        assert_eq!(pos, [0.0, 0.0]);
    }
}
