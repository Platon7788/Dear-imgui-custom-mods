//! PageRank via power iteration for the knowledge-graph widget.

use std::collections::HashMap;

use super::super::data::{GraphData, NodeId};

/// Compute PageRank scores for all nodes.
///
/// Returns `(node_order, scores)` where `node_order[i]` is the [`NodeId`]
/// corresponding to `scores[i]`. The ordering is stable within a single call
/// but may differ between calls if nodes are added or removed.
///
/// # Parameters
///
/// * `damping`  – damping factor (standard value: `0.85`)
/// * `max_iter` – maximum number of power-iteration steps
/// * `tol`      – convergence threshold on max absolute delta
///
/// # Edge handling
///
/// * **Directed edges** (`edge.directed == true`): only `edge.to` receives
///   rank contribution from `edge.from`.
/// * **Undirected edges** (`edge.directed == false`): both endpoints
///   contribute rank to each other.
/// * **Dangling nodes** (out-degree 0): their accumulated rank is distributed
///   uniformly across all nodes before each iteration.
pub(crate) fn compute(
    graph: &GraphData,
    damping: f32,
    max_iter: u32,
    tol: f32,
) -> (Vec<NodeId>, Vec<f32>) {
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

    // ── Compute out-degrees (directed) ───────────────────────────────────────
    // out_deg[i] = number of edges that originate (or are effectively
    // originating) at node_order[i] for PageRank purposes.
    let mut out_deg: Vec<usize> = vec![0usize; n];
    for (_, edge) in graph.edges.iter() {
        let fi = index[&edge.from];
        let ti = index[&edge.to];
        if edge.directed {
            out_deg[fi] += 1;
        } else {
            out_deg[fi] += 1;
            out_deg[ti] += 1;
        }
    }

    // ── Initialise ranks ─────────────────────────────────────────────────────
    let init = 1.0_f32 / n as f32;
    let mut rank: Vec<f32> = vec![init; n];
    let mut next: Vec<f32> = vec![0.0_f32; n];

    // ── Power iteration ──────────────────────────────────────────────────────
    for _ in 0..max_iter {
        // Dangling-node mass: sum of rank[i] for nodes with out_deg[i] == 0.
        let dangling_sum: f32 = rank
            .iter()
            .enumerate()
            .filter(|&(i, _)| out_deg[i] == 0)
            .map(|(_, &r)| r)
            .sum();
        let dangling_contrib = damping * dangling_sum / n as f32;
        let teleport = (1.0 - damping) / n as f32;

        // Reset next buffer.
        for v in &mut next {
            *v = teleport + dangling_contrib;
        }

        // Distribute rank along edges.
        for (_, edge) in graph.edges.iter() {
            let fi = index[&edge.from];
            let ti = index[&edge.to];

            if edge.directed {
                // from → to only
                next[ti] += damping * rank[fi] / out_deg[fi] as f32;
            } else {
                // Both directions contribute
                next[ti] += damping * rank[fi] / out_deg[fi] as f32;
                next[fi] += damping * rank[ti] / out_deg[ti] as f32;
            }
        }

        // Check convergence.
        let max_delta = rank
            .iter()
            .zip(next.iter())
            .map(|(&r, &nx)| (r - nx).abs())
            .fold(0.0_f32, f32::max);

        std::mem::swap(&mut rank, &mut next);

        if max_delta < tol {
            break;
        }
    }

    (node_order, rank)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::style::{EdgeStyle, NodeStyle};

    fn es() -> EdgeStyle {
        EdgeStyle::new()
    }

    /// Single-node graph: rank should converge to 1.0.
    #[test]
    fn single_node_rank_is_one() {
        let mut g = GraphData::new();
        g.add_node(NodeStyle::new("A"));
        let (_, scores) = compute(&g, 0.85, 100, 1e-6);
        assert_eq!(scores.len(), 1);
        assert!(
            (scores[0] - 1.0).abs() < 1e-4,
            "expected ~1.0, got {}",
            scores[0]
        );
    }

    /// Two nodes connected by an undirected edge: ranks should be equal.
    #[test]
    fn two_node_symmetric_ranks() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        g.add_edge(a, b, es(), 1.0, false);
        let (order, scores) = compute(&g, 0.85, 100, 1e-6);
        assert_eq!(scores.len(), 2);
        let ia = order.iter().position(|&id| id == a).unwrap();
        let ib = order.iter().position(|&id| id == b).unwrap();
        assert!(
            (scores[ia] - scores[ib]).abs() < 1e-4,
            "expected equal ranks, got {} vs {}",
            scores[ia],
            scores[ib]
        );
    }

    /// Chain A-B-C (undirected): B is the middle node and should have strictly
    /// higher rank than the endpoints A and C.
    #[test]
    fn chain_middle_node_higher_rank() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        let c = g.add_node(NodeStyle::new("C"));
        g.add_edge(a, b, es(), 1.0, false);
        g.add_edge(b, c, es(), 1.0, false);

        let (order, scores) = compute(&g, 0.85, 100, 1e-6);
        let idx = |id: NodeId| order.iter().position(|&x| x == id).unwrap();
        let ra = scores[idx(a)];
        let rb = scores[idx(b)];
        let rc = scores[idx(c)];
        assert!(
            rb > ra && rb > rc,
            "middle B ({rb}) should outrank endpoints A ({ra}), C ({rc})"
        );
    }

    /// Empty graph: should return empty vecs without panicking.
    #[test]
    fn empty_graph_no_panic() {
        let g = GraphData::new();
        let (order, scores) = compute(&g, 0.85, 100, 1e-6);
        assert!(order.is_empty());
        assert!(scores.is_empty());
    }
}
