//! Visibility computation for the knowledge-graph renderer.
//!
//! Determines which nodes should be drawn each frame based on the active
//! [`FilterState`]. All logic is pure (no ImGui calls) and runs as a pre-render
//! pass before any draw-list commands.

use std::collections::{HashSet, VecDeque};

use super::super::data::{GraphData, NodeId};
use super::super::filter::FilterState;
use super::super::style::NodeKind;

/// The result of the visibility pass: either all nodes are visible, or a
/// specific subset.
#[derive(Default)]
pub(crate) enum VisibleSet {
    /// No filter is active — every node is visible.
    #[default]
    All,
    /// Only nodes in this set are visible.
    Some(HashSet<NodeId>),
}

impl VisibleSet {
    /// Returns `true` if `id` is in the visible set.
    #[inline]
    pub(crate) fn contains(&self, id: NodeId) -> bool {
        match self {
            Self::All => true,
            Self::Some(s) => s.contains(&id),
        }
    }
}

/// Compute which nodes are visible given the current filter state.
///
/// `search_highlight` — when `true`, the search-query pass is skipped so the
/// render layer can dim non-matching nodes instead of hiding them.
///
/// Returns [`VisibleSet::All`] when the filter is an identity (nothing active).
pub(crate) fn compute(
    graph: &GraphData,
    filter: &FilterState,
    search_highlight: bool,
) -> VisibleSet {
    if filter.is_identity() {
        return VisibleSet::All;
    }

    let mut set: HashSet<NodeId> = graph.nodes().map(|(id, _)| id).collect();

    // ── Tag whitelist (enabled_tags) ────────────────────────────────────────
    if !filter.enabled_tags.is_empty() {
        set.retain(|id| {
            let Some(node) = graph.node(*id) else {
                return false;
            };
            node.tags.iter().any(|t| filter.enabled_tags.contains(t))
        });
    }

    // ── Search filter (skipped when search_highlight is on) ─────────────────
    if !search_highlight && !filter.search_query.is_empty() {
        let q = filter.search_query.to_ascii_lowercase();
        set.retain(|id| {
            let Some(node) = graph.node(*id) else {
                return false;
            };
            let label_match = node.label.to_ascii_lowercase().contains(&q);
            if label_match {
                return true;
            }
            if filter.search_match_tags {
                node.tags
                    .iter()
                    .any(|t| t.to_ascii_lowercase().contains(&q))
            } else {
                false
            }
        });
    }

    // ── Time-travel filter ──────────────────────────────────────────────────
    if filter.time_threshold.is_finite() {
        set.retain(|id| {
            let Some(node) = graph.node(*id) else {
                return false;
            };
            node.created_at <= filter.time_threshold
        });
    }

    // ── Node-kind visibility ────────────────────────────────────────────────
    if filter.hide_unresolved || filter.hide_tags || filter.hide_attachments {
        set.retain(|id| {
            let Some(node) = graph.node(*id) else {
                return false;
            };
            match node.kind {
                NodeKind::Unresolved if filter.hide_unresolved => false,
                NodeKind::Tag if filter.hide_tags => false,
                NodeKind::Attachment if filter.hide_attachments => false,
                _ => true,
            }
        });
    }

    // ── Orphan filter ───────────────────────────────────────────────────────
    if !filter.show_orphans {
        set.retain(|id| graph.degree(*id) > 0);
    }

    // ── Minimum degree filter ───────────────────────────────────────────────
    if filter.min_degree > 0 {
        set.retain(|id| graph.degree(*id) >= filter.min_degree as usize);
    }

    // ── Depth filter (BFS from focused_node) ───────────────────────────────
    if let (Some(depth), Some(focus)) = (filter.depth, filter.focused_node) {
        let reachable = bfs_reachable(graph, focus, depth);
        set.retain(|id| reachable.contains(id));
    }

    VisibleSet::Some(set)
}

/// BFS from `start`, exploring undirected edges up to `max_hops` hops.
fn bfs_reachable(graph: &GraphData, start: NodeId, max_hops: u32) -> HashSet<NodeId> {
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut queue: VecDeque<(NodeId, u32)> = VecDeque::new();

    if !graph.nodes.contains_key(start) {
        return visited;
    }

    visited.insert(start);
    queue.push_back((start, 0));

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_hops {
            continue;
        }
        for neighbor in graph.neighbors(current) {
            if visited.insert(neighbor) {
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    visited
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::force_graph::data::GraphData;
    use crate::force_graph::filter::FilterState;
    use crate::force_graph::style::{EdgeStyle, NodeStyle};

    #[test]
    fn identity_filter_returns_all() {
        let mut g = GraphData::new();
        g.add_node(NodeStyle::new("a"));
        let f = FilterState::new();
        assert!(matches!(compute(&g, &f), VisibleSet::All));
    }

    #[test]
    fn search_filter_hides_non_matching() {
        let mut g = GraphData::new();
        g.add_node(NodeStyle::new("alpha"));
        g.add_node(NodeStyle::new("beta"));
        let mut f = FilterState::new();
        f.search_query = "alp".into();
        let vis = compute(&g, &f);
        let nodes: Vec<_> = g.nodes().map(|(id, _)| id).collect();
        // exactly 1 should be visible
        let visible_count = nodes.iter().filter(|id| vis.contains(**id)).count();
        assert_eq!(visible_count, 1);
    }

    #[test]
    fn orphan_filter_hides_isolated_nodes() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("a"));
        let b = g.add_node(NodeStyle::new("b"));
        g.add_node(NodeStyle::new("orphan"));
        g.add_edge(a, b, EdgeStyle::new(), 1.0, false);
        let mut f = FilterState::new();
        f.show_orphans = false;
        let vis = compute(&g, &f);
        let visible: Vec<_> = g
            .nodes()
            .map(|(id, _)| id)
            .filter(|id| vis.contains(*id))
            .collect();
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn depth_bfs_limits_hops() {
        // chain: a -> b -> c -> d
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("a"));
        let b = g.add_node(NodeStyle::new("b"));
        let c = g.add_node(NodeStyle::new("c"));
        let d = g.add_node(NodeStyle::new("d"));
        g.add_edge(a, b, EdgeStyle::new(), 1.0, false);
        g.add_edge(b, c, EdgeStyle::new(), 1.0, false);
        g.add_edge(c, d, EdgeStyle::new(), 1.0, false);

        let mut f = FilterState::new();
        f.focused_node = Some(a);
        f.depth = Some(2); // should reach a, b, c but NOT d

        let vis = compute(&g, &f);
        assert!(vis.contains(a));
        assert!(vis.contains(b));
        assert!(vis.contains(c));
        assert!(!vis.contains(d));
    }
}
