//! Stateless geometry / colour helpers for the force-graph renderer.
//!
//! Split out of `render/draw.rs` to keep that file under the 500-line limit.
//! Everything here is pure (no ImGui state) and unit-testable without a UI
//! context.

use crate::utils::color::blend_color;

use super::super::config::{ColorMode, ForceConfig};
use super::super::data::{GraphData, NodeId};
use super::super::style::GraphColors;
use super::camera::Camera;
use super::visibility::VisibleSet;

// ─── Okabe-Ito palette (8 colors, colorblind-safe) ────────────────────────────

pub(super) const OKABE_ITO: [[f32; 4]; 8] = [
    [0.902, 0.624, 0.000, 1.0], // orange
    [0.337, 0.706, 0.914, 1.0], // sky blue
    [0.000, 0.620, 0.451, 1.0], // blue-green
    [0.941, 0.894, 0.259, 1.0], // yellow
    [0.000, 0.447, 0.698, 1.0], // blue
    [0.835, 0.369, 0.000, 1.0], // vermillion
    [0.800, 0.475, 0.655, 1.0], // reddish purple
    [0.600, 0.600, 0.600, 1.0], // gray (fallback)
];

/// `haystack.to_ascii_lowercase().contains(needle_lc)` without the per-call
/// allocation. `needle_lc` MUST already be lowercased (caller's responsibility
/// — typically pre-lowered once per frame). Used by the search-highlight path
/// in the per-node loop where allocating per node was a measured hot-path cost
/// on graphs >5k nodes.
pub(super) fn contains_ignore_ascii_case(haystack: &str, needle_lc: &str) -> bool {
    if needle_lc.is_empty() {
        return true;
    }
    if haystack.len() < needle_lc.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle_lc.len())
        .any(|w| w.eq_ignore_ascii_case(needle_lc.as_bytes()))
}

/// Draw a dotted background grid.
pub(super) fn draw_grid(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    camera: &Camera,
    canvas_min: [f32; 2],
    canvas_size: [f32; 2],
    color: u32,
) {
    let grid_step = 50.0_f32 * camera.zoom;
    if grid_step < 8.0 {
        return;
    }

    let origin_x = canvas_min[0] + camera.offset[0] % grid_step;
    let origin_y = canvas_min[1] + camera.offset[1] % grid_step;
    let canvas_max = [
        canvas_min[0] + canvas_size[0],
        canvas_min[1] + canvas_size[1],
    ];

    let mut x = origin_x;
    while x < canvas_max[0] {
        draw.add_line([x, canvas_min[1]], [x, canvas_max[1]], color)
            .build();
        x += grid_step;
    }
    let mut y = origin_y;
    while y < canvas_max[1] {
        draw.add_line([canvas_min[0], y], [canvas_max[0], y], color)
            .build();
        y += grid_step;
    }
}

/// Find the nearest visible node under the cursor (linear scan, Phase A).
pub(super) fn hit_test_nearest(
    graph: &GraphData,
    camera: &Camera,
    canvas_min: [f32; 2],
    mouse: [f32; 2],
    force_config: &ForceConfig,
    visible: &VisibleSet,
) -> Option<NodeId> {
    let world_mouse = camera.screen_to_world(mouse, canvas_min);
    let mut best: Option<(NodeId, f32)> = None;

    for (node_id, node) in graph.nodes.iter() {
        if !visible.contains(node_id) {
            continue;
        }
        let r = if force_config.radius_by_degree {
            let deg = graph.adjacency.get(&node_id).map_or(0, Vec::len);
            force_config.radius_base + force_config.radius_per_degree * deg as f32
        } else {
            node.style.radius.unwrap_or(force_config.radius_base)
        };

        let dx = world_mouse[0] - node.pos[0];
        let dy = world_mouse[1] - node.pos[1];
        let dist_sq = dx * dx + dy * dy;

        if dist_sq <= r * r && best.is_none_or(|(_, b)| dist_sq < b) {
            best = Some((node_id, dist_sq));
        }
    }

    best.map(|(id, _)| id)
}

/// Compute a node's base radius in world space.
pub(super) fn node_radius(
    id: NodeId,
    node: &super::super::data::Node,
    graph: &GraphData,
    fc: &ForceConfig,
) -> f32 {
    if let Some(r) = node.style.radius {
        return r;
    }
    if fc.radius_by_degree {
        let deg = graph.adjacency.get(&id).map_or(0, Vec::len);
        fc.radius_base + fc.radius_per_degree * deg as f32
    } else {
        fc.radius_base
    }
}

/// Resolve the fill color for a node based on the current ColorMode.
pub(super) fn resolve_node_color(
    id: NodeId,
    style: &super::super::style::NodeStyle,
    graph: &GraphData,
    mode: &ColorMode,
    colors: &GraphColors,
) -> [f32; 4] {
    match mode {
        ColorMode::Static => style.color.unwrap_or(colors.node_default),
        ColorMode::ByTag => {
            if let Some(tag) = style.tags.first() {
                let hash = fnv1a_hash(tag.as_bytes()) as usize;
                OKABE_ITO[hash % OKABE_ITO.len()]
            } else {
                colors.node_default
            }
        }
        ColorMode::ByCommunity => style.color.unwrap_or(colors.node_default),
        ColorMode::ByPageRank => {
            let score = graph.pagerank_for(id);
            blend_color(
                colors.node_default,
                colors.node_selected,
                score.clamp(0.0, 1.0),
            )
        }
        ColorMode::ByBetweenness => {
            let score = graph.betweenness_for(id);
            blend_color(
                colors.node_default,
                colors.node_selected,
                score.clamp(0.0, 1.0),
            )
        }
        ColorMode::Custom(f) => f(style, graph),
    }
}

/// Compute the AABB of all node positions. Returns `None` when graph is empty.
pub(crate) fn graph_bounds(graph: &GraphData) -> Option<[[f32; 2]; 2]> {
    let mut it = graph.nodes.iter();
    let (_, first) = it.next()?;
    let mut min = first.pos;
    let mut max = first.pos;
    for (_, node) in it {
        min[0] = min[0].min(node.pos[0]);
        min[1] = min[1].min(node.pos[1]);
        max[0] = max[0].max(node.pos[0]);
        max[1] = max[1].max(node.pos[1]);
    }
    Some([min, max])
}

/// Simple FNV-1a hash for stable tag→color mapping.
fn fnv1a_hash(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Returns true if the line segment [a, b] is at least partially inside the
/// rect [min, max]. Used for edge frustum culling.
pub(super) fn segment_visible(a: [f32; 2], b: [f32; 2], min: [f32; 2], max: [f32; 2]) -> bool {
    let seg_min = [a[0].min(b[0]), a[1].min(b[1])];
    let seg_max = [a[0].max(b[0]), a[1].max(b[1])];
    seg_max[0] >= min[0] && seg_min[0] <= max[0] && seg_max[1] >= min[1] && seg_min[1] <= max[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_is_deterministic_and_distinct() {
        assert_eq!(fnv1a_hash(b"core"), fnv1a_hash(b"core"));
        assert_ne!(fnv1a_hash(b"core"), fnv1a_hash(b"ui"));
    }

    #[test]
    fn segment_visible_detects_overlap_and_rejection() {
        let min = [0.0, 0.0];
        let max = [100.0, 100.0];
        // Segment fully inside.
        assert!(segment_visible([10.0, 10.0], [50.0, 50.0], min, max));
        // Segment crossing the rect.
        assert!(segment_visible([-50.0, 50.0], [150.0, 50.0], min, max));
        // Segment entirely to the left — rejected.
        assert!(!segment_visible([-50.0, 50.0], [-10.0, 50.0], min, max));
        // Segment entirely below — rejected.
        assert!(!segment_visible([10.0, 200.0], [50.0, 250.0], min, max));
    }

    #[test]
    fn contains_ignore_ascii_case_matches() {
        assert!(contains_ignore_ascii_case("Hello World", "world"));
        assert!(contains_ignore_ascii_case("anything", ""));
        assert!(!contains_ignore_ascii_case("abc", "abcd"));
        assert!(!contains_ignore_ascii_case("xyz", "a"));
    }
}
