//! Betweenness centrality via Brandes' algorithm for the knowledge-graph widget.

use std::collections::HashMap;

use super::super::data::{GraphData, NodeId};

/// Compute normalized betweenness centrality for all nodes.
///
/// Returns `(node_order, scores)`. Scores are normalized to `[0, 1]` by
/// dividing by `(n-1)(n-2)/2` for undirected graphs (the maximum number of
/// shortest paths that can pass through a single node).
///
/// Uses Brandes' O(V·E) algorithm.
///
/// # Edge handling
///
/// The implementation treats the graph as **undirected** for BFS purposes
/// (shortest-path lengths are symmetric). For directed use-cases, the
/// adjacency list already encodes directionality in `edge.from`/`edge.to`,
/// but the normalization constant uses the undirected formula.
pub(crate) fn compute(graph: &GraphData) -> (Vec<NodeId>, Vec<f32>) {
    let n = graph.node_count();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }

    // ── Build index ──────────────────────────────────────────────────────────
    let node_order: Vec<NodeId> = graph.nodes.keys().collect();
    let index: HashMap<NodeId, usize> = node_order
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();

    // Pre-build undirected adjacency as index lists for BFS efficiency.
    let adj: Vec<Vec<usize>> = node_order
        .iter()
        .map(|&id| {
            graph
                .neighbors(id)
                .map(|nbr| index[&nbr])
                .collect()
        })
        .collect();

    let mut betweenness: Vec<f64> = vec![0.0_f64; n];

    // ── Brandes' algorithm ───────────────────────────────────────────────────
    for s in 0..n {
        // Stack of nodes in order of non-decreasing distance from s.
        let mut stack: Vec<usize> = Vec::with_capacity(n);
        // Predecessors on shortest paths from s.
        let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
        // Number of shortest paths from s to each node.
        let mut sigma: Vec<f64> = vec![0.0_f64; n];
        // Distance from s (-1 means unvisited).
        let mut dist: Vec<i64> = vec![-1_i64; n];

        sigma[s] = 1.0;
        dist[s] = 0;

        let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        queue.push_back(s);

        // Forward BFS.
        while let Some(v) = queue.pop_front() {
            stack.push(v);
            for &w in &adj[v] {
                // w found for first time?
                if dist[w] < 0 {
                    queue.push_back(w);
                    dist[w] = dist[v] + 1;
                }
                // Is this a shortest path to w via v?
                if dist[w] == dist[v] + 1 {
                    sigma[w] += sigma[v];
                    pred[w].push(v);
                }
            }
        }

        // Backward accumulation.
        let mut delta: Vec<f64> = vec![0.0_f64; n];
        while let Some(w) = stack.pop() {
            for &v in &pred[w] {
                let contribution = (sigma[v] / sigma[w]) * (1.0 + delta[w]);
                delta[v] += contribution;
            }
            if w != s {
                betweenness[w] += delta[w];
            }
        }
    }

    // ── Normalize ────────────────────────────────────────────────────────────
    // Undirected normalization: 2 / ((n-1)(n-2)).
    // For n <= 2 the denominator is zero; all scores stay at 0.
    let scores: Vec<f32> = if n > 2 {
        let norm = 2.0_f64 / ((n - 1) as f64 * (n - 2) as f64);
        betweenness
            .iter()
            .map(|&b| (b * norm).clamp(0.0, 1.0) as f32)
            .collect()
    } else {
        vec![0.0_f32; n]
    };

    (node_order, scores)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::style::{EdgeStyle, NodeStyle};

    fn es() -> EdgeStyle {
        EdgeStyle::new()
    }

    /// Path graph A-B-C: only B lies on paths between the two other nodes,
    /// so after normalization B should be 1.0 and A, C should be 0.0.
    #[test]
    fn path_graph_middle_node_max_betweenness() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        let c = g.add_node(NodeStyle::new("C"));
        g.add_edge(a, b, es(), 1.0, false);
        g.add_edge(b, c, es(), 1.0, false);

        let (order, scores) = compute(&g);
        let idx = |id: NodeId| order.iter().position(|&x| x == id).unwrap();

        let sb = scores[idx(b)];
        let sa = scores[idx(a)];
        let sc = scores[idx(c)];

        assert!(
            (sb - 1.0).abs() < 1e-5,
            "B should have betweenness 1.0, got {sb}"
        );
        assert!(
            sa.abs() < 1e-5,
            "A should have betweenness 0.0, got {sa}"
        );
        assert!(
            sc.abs() < 1e-5,
            "C should have betweenness 0.0, got {sc}"
        );
    }

    /// Star graph (center + N spokes): center should have maximum betweenness
    /// and all spokes should have lower (or equal zero) betweenness.
    #[test]
    fn star_graph_center_max_betweenness() {
        let mut g = GraphData::new();
        let center = g.add_node(NodeStyle::new("center"));
        let spokes: Vec<NodeId> = (0..4)
            .map(|i| g.add_node(NodeStyle::new(format!("spoke{i}"))))
            .collect();
        for &s in &spokes {
            g.add_edge(center, s, es(), 1.0, false);
        }

        let (order, scores) = compute(&g);
        let ic = order.iter().position(|&id| id == center).unwrap();
        let center_score = scores[ic];

        for &s in &spokes {
            let is = order.iter().position(|&id| id == s).unwrap();
            assert!(
                center_score >= scores[is],
                "center ({center_score}) should have >= betweenness than spoke ({})",
                scores[is]
            );
        }
        // Center betweenness must be strictly positive in a star with >= 3 spokes.
        assert!(center_score > 0.0, "center should have positive betweenness");
    }

    /// Single node: must not panic and returns zero score.
    #[test]
    fn single_node_no_panic() {
        let mut g = GraphData::new();
        g.add_node(NodeStyle::new("A"));
        let (order, scores) = compute(&g);
        assert_eq!(order.len(), 1);
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0], 0.0);
    }

    /// Two nodes: must not panic and returns zero scores (denominator is 0).
    #[test]
    fn two_nodes_no_panic() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        g.add_edge(a, b, es(), 1.0, false);
        let (_, scores) = compute(&g);
        assert_eq!(scores.len(), 2);
        for s in &scores {
            assert_eq!(*s, 0.0);
        }
    }

    /// Empty graph: must return empty vecs without panicking.
    #[test]
    fn empty_graph_no_panic() {
        let g = GraphData::new();
        let (order, scores) = compute(&g);
        assert!(order.is_empty());
        assert!(scores.is_empty());
    }
}
