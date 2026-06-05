//! Unit tests for [`super::GraphData`].
//!
//! Split out of `data.rs` to keep that file under the 500-line limit. Included
//! via `#[cfg(test)] #[path = "data_tests.rs"] mod tests;` so `super::*`
//! resolves to the `data` module and private fields stay reachable.

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

// ── Metrics-cache index safety ──────────────────────────────────────────────

/// Regression: after removing a node, recomputing metrics must keep the
/// `pagerank`/`betweenness` arrays aligned with the cache `index` map, and
/// querying a *surviving* node must return its own (finite, in-range) score —
/// not a stale value from before the removal. This guards the
/// `recompute_metrics_if_needed` index-construction path.
#[test]
fn metrics_cache_index_stays_aligned_after_remove() {
    let mut g = GraphData::new();
    let a = g.add_node(NodeStyle::new("A"));
    let b = g.add_node(NodeStyle::new("B"));
    let c = g.add_node(NodeStyle::new("C"));
    let d = g.add_node(NodeStyle::new("D"));
    g.add_edge(a, b, make_edge_style(), 1.0, false);
    g.add_edge(b, c, make_edge_style(), 1.0, false);
    g.add_edge(c, d, make_edge_style(), 1.0, false);

    g.recompute_metrics_if_needed();
    // Remove an endpoint, forcing a full recompute over the smaller node set.
    g.remove_node(a);
    g.recompute_metrics_if_needed();

    // Every surviving node maps to a valid score slot.
    for (id, _) in g.nodes() {
        let pr = g.pagerank_for(id);
        let bt = g.betweenness_for(id);
        assert!(pr.is_finite() && pr >= 0.0, "pagerank out of range: {pr}");
        assert!((0.0..=1.0).contains(&bt), "betweenness out of range: {bt}");
    }
    // A removed node returns the not-found sentinel, never a panic.
    assert_eq!(g.pagerank_for(a), 0.0);
    assert_eq!(g.betweenness_for(a), 0.0);

    // PageRank of all surviving nodes sums to ~1.0 (mass conservation), proving
    // the cache holds a complete, correctly-indexed score vector.
    let sum: f32 = g.nodes().map(|(id, _)| g.pagerank_for(id)).sum();
    assert!((sum - 1.0).abs() < 1e-3, "pagerank sum drifted: {sum}");
}
