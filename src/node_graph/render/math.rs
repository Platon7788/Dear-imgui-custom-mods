//! Pure math and geometry for wire routing and hit testing.
//!
//! No rendering dependencies — all functions here deal with coordinates,
//! distances, and obstacle-aware path computation.

use super::super::config::NodeGraphConfig;
use super::super::graph::Graph;
use super::super::state::InteractionState;
use super::super::types::*;
use super::super::viewer::NodeGraphViewer;

// ─── Node AABB for obstacle avoidance ────────────────────────────────────────

/// Screen-space axis-aligned bounding box for a node.
pub(super) struct NodeAABB {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub id: NodeId,
}

/// Collect screen-space AABBs for all nodes into the provided buffer.
/// Clears and reuses the buffer to avoid per-frame allocation.
pub(super) fn collect_node_aabbs<T>(
    graph: &Graph<T>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
    aabbs: &mut Vec<NodeAABB>,
) {
    aabbs.clear();
    let vp = &state.viewport;
    for (nid, node) in graph.nodes() {
        let [sx, sy] = vp.graph_to_screen(node.pos);
        let w = viewer
            .node_width(&node.value)
            .unwrap_or(config.node_min_width)
            * vp.zoom;
        let h = config.node_height(
            viewer.inputs(&node.value),
            viewer.outputs(&node.value),
            viewer.has_body(&node.value),
            node.open,
            viewer.body_height(&node.value),
        ) * vp.zoom;
        aabbs.push(NodeAABB {
            x0: sx,
            y0: sy,
            x1: sx + w,
            y1: sy + h,
            id: nid,
        });
    }
}

/// Find node AABBs that a horizontal wire corridor would intersect,
/// excluding the source and target nodes.
pub(super) fn find_obstacles_in_corridor(
    aabbs: &[NodeAABB],
    from: [f32; 2],
    to: [f32; 2],
    margin: f32,
    skip_src: NodeId,
    skip_dst: NodeId,
) -> (f32, f32, bool) {
    let y_lo = from[1].min(to[1]) - margin;
    let y_hi = from[1].max(to[1]) + margin;
    let x_lo = from[0].min(to[0]);
    let x_hi = from[0].max(to[0]);

    let mut obs_y_min = f32::MAX;
    let mut obs_y_max = f32::MIN;
    let mut found = false;

    for aabb in aabbs {
        if aabb.id == skip_src || aabb.id == skip_dst {
            continue;
        }
        if aabb.x1 > x_lo && aabb.x0 < x_hi && aabb.y1 > y_lo && aabb.y0 < y_hi {
            obs_y_min = obs_y_min.min(aabb.y0);
            obs_y_max = obs_y_max.max(aabb.y1);
            found = true;
        }
    }

    (obs_y_min, obs_y_max, found)
}

// ─── Orthogonal polyline (shared by rendering + hit testing) ─────────────────

/// Orthogonal wire polyline (max 6 points).
pub(super) struct OrthoPolyline {
    pub points: [[f32; 2]; 6],
    pub len: u8,
}

/// Compute orthogonal wire polyline, with obstacle-aware routing.
///
/// This function is the single source of truth for orthogonal wire paths,
/// used by both `draw_wire_smart` and `wire_hit_test`.
pub(super) fn ortho_wire_points(
    from: [f32; 2],
    to: [f32; 2],
    has_obstacle: bool,
    obs_y_min: f32,
    obs_y_max: f32,
    margin: f32,
) -> OrthoPolyline {
    let mut pts = [[0.0_f32, 0.0]; 6];
    pts[0] = from;

    if to[0] > from[0] + 40.0 {
        if !has_obstacle {
            let mid_x = (from[0] + to[0]) * 0.5;
            pts[1] = [mid_x, from[1]];
            pts[2] = [mid_x, to[1]];
            pts[3] = to;
            OrthoPolyline {
                points: pts,
                len: 4,
            }
        } else {
            let go_above = (from[1] - obs_y_min).abs() < (from[1] - obs_y_max).abs();
            let detour_y = if go_above {
                obs_y_min - margin
            } else {
                obs_y_max + margin
            };
            let out_x = from[0] + 25.0;
            let in_x = to[0] - 25.0;
            pts[1] = [out_x, from[1]];
            pts[2] = [out_x, detour_y];
            pts[3] = [in_x, detour_y];
            pts[4] = [in_x, to[1]];
            pts[5] = to;
            OrthoPolyline {
                points: pts,
                len: 6,
            }
        }
    } else {
        let out_x = from[0] + 30.0;
        let in_x = to[0] - 30.0;
        let mid_y = if has_obstacle {
            let go_above = (from[1] - obs_y_min).abs() < (from[1] - obs_y_max).abs();
            if go_above {
                obs_y_min - margin
            } else {
                obs_y_max + margin
            }
        } else {
            (from[1] + to[1]) * 0.5
        };
        pts[1] = [out_x, from[1]];
        pts[2] = [out_x, mid_y];
        pts[3] = [in_x, mid_y];
        pts[4] = [in_x, to[1]];
        pts[5] = to;
        OrthoPolyline {
            points: pts,
            len: 6,
        }
    }
}

// ─── Bezier control points ───────────────────────────────────────────────────

/// Compute bezier control points for a wire.
/// Adaptive curvature: larger tangent when wire goes backwards to avoid node overlap.
#[inline]
pub(super) fn bezier_control_points(
    from: [f32; 2],
    to: [f32; 2],
    curvature: f32,
) -> ([f32; 2], [f32; 2]) {
    let horiz = to[0] - from[0];
    let vert = (to[1] - from[1]).abs();
    let dx = if horiz > 0.0 {
        horiz.abs() * curvature + vert * 0.15
    } else {
        let extent = horiz.abs() + vert;
        extent * curvature.max(0.4) + (extent * 0.15).max(40.0)
    };
    ([from[0] + dx, from[1]], [to[0] - dx, to[1]])
}

/// Compute obstacle-aware bezier control points (shared by rendering + hit test).
pub(super) fn obstacle_aware_bezier_cps(
    from: [f32; 2],
    to: [f32; 2],
    curvature: f32,
    has_obstacle: bool,
    obs_y_min: f32,
    obs_y_max: f32,
    margin: f32,
) -> ([f32; 2], [f32; 2]) {
    if !has_obstacle {
        bezier_control_points(from, to, curvature)
    } else {
        let go_above = (from[1] - obs_y_min).abs() < (from[1] - obs_y_max).abs();
        let detour_y = if go_above {
            obs_y_min - margin * 3.0
        } else {
            obs_y_max + margin * 3.0
        };
        let base = bezier_control_points(from, to, curvature);
        let blend = 0.7;
        (
            [base.0[0], base.0[1] * (1.0 - blend) + detour_y * blend],
            [base.1[0], base.1[1] * (1.0 - blend) + detour_y * blend],
        )
    }
}

// ─── Core math ───────────────────────────────────────────────────────────────

/// Evaluate cubic bezier at parameter `t`.
pub(super) fn cubic_bezier(
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    t: f32,
) -> [f32; 2] {
    let u = 1.0 - t;
    let uu = u * u;
    let tt = t * t;
    let uuu = uu * u;
    let ttt = tt * t;
    [
        uuu * p0[0] + 3.0 * uu * t * p1[0] + 3.0 * u * tt * p2[0] + ttt * p3[0],
        uuu * p0[1] + 3.0 * uu * t * p1[1] + 3.0 * u * tt * p2[1] + ttt * p3[1],
    ]
}

/// Distance from point `p` to line segment `a`–`b`.
pub(super) fn point_to_segment_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-6 {
        let ex = p[0] - a[0];
        let ey = p[1] - a[1];
        return (ex * ex + ey * ey).sqrt();
    }
    let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj_x = a[0] + t * dx;
    let proj_y = a[1] + t * dy;
    let ex = p[0] - proj_x;
    let ey = p[1] - proj_y;
    (ex * ex + ey * ey).sqrt()
}

// ─── Wire hit testing (obstacle-aware) ───────────────────────────────────────

/// Wire hit test using per-wire style, zoom-scaled distance, and obstacle-aware routing.
///
/// This mirrors `draw_wire_smart` exactly: the hit test path always matches
/// the drawn wire path (including obstacle-avoidance detours).
#[allow(clippy::too_many_arguments)]
pub(super) fn wire_hit_test(
    from: [f32; 2],
    to: [f32; 2],
    mouse: [f32; 2],
    max_dist: f32,
    style: WireStyle,
    config: &NodeGraphConfig,
    obstacles: &[NodeAABB],
    src_node: NodeId,
    dst_node: NodeId,
) -> bool {
    let margin = 10.0;
    let (obs_y_min, obs_y_max, has_obstacle) =
        find_obstacles_in_corridor(obstacles, from, to, margin * 2.0, src_node, dst_node);

    match style {
        WireStyle::Line => {
            if !has_obstacle {
                point_to_segment_dist(mouse, from, to) <= max_dist
            } else {
                let go_above = (from[1] - obs_y_min).abs() < (from[1] - obs_y_max).abs();
                let detour_y = if go_above {
                    obs_y_min - margin
                } else {
                    obs_y_max + margin
                };
                let mid_x = (from[0] + to[0]) * 0.5;
                let p1 = [mid_x, detour_y];
                point_to_segment_dist(mouse, from, p1).min(point_to_segment_dist(mouse, p1, to))
                    <= max_dist
            }
        }
        WireStyle::Bezier => {
            let (cp0, cp1) = obstacle_aware_bezier_cps(
                from,
                to,
                config.wire_curvature,
                has_obstacle,
                obs_y_min,
                obs_y_max,
                margin,
            );
            let samples = 20;
            let mut min_dist = f32::MAX;
            let mut prev = from;
            for i in 1..=samples {
                let t = i as f32 / samples as f32;
                let pt = cubic_bezier(from, cp0, cp1, to, t);
                let d = point_to_segment_dist(mouse, prev, pt);
                min_dist = min_dist.min(d);
                prev = pt;
            }
            min_dist <= max_dist
        }
        WireStyle::Orthogonal => {
            let poly = ortho_wire_points(from, to, has_obstacle, obs_y_min, obs_y_max, margin);
            let mut min_dist = f32::MAX;
            for i in 0..poly.len as usize - 1 {
                min_dist = min_dist.min(point_to_segment_dist(
                    mouse,
                    poly.points[i],
                    poly.points[i + 1],
                ));
            }
            min_dist <= max_dist
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── cubic_bezier ─────────────────────────────────────────────────────

    #[test]
    fn bezier_endpoints() {
        let p0 = [0.0, 0.0];
        let p1 = [10.0, 20.0];
        let p2 = [30.0, 20.0];
        let p3 = [40.0, 0.0];
        let start = cubic_bezier(p0, p1, p2, p3, 0.0);
        let end = cubic_bezier(p0, p1, p2, p3, 1.0);
        assert!((start[0] - p0[0]).abs() < 1e-5);
        assert!((start[1] - p0[1]).abs() < 1e-5);
        assert!((end[0] - p3[0]).abs() < 1e-5);
        assert!((end[1] - p3[1]).abs() < 1e-5);
    }

    #[test]
    fn bezier_midpoint() {
        let p = cubic_bezier([0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0], 0.5);
        assert!((p[0] - 15.0).abs() < 1e-4);
        assert!((p[1]).abs() < 1e-4);
    }

    // ── point_to_segment_dist ────────────────────────────────────────────

    #[test]
    fn dist_to_horizontal_segment() {
        let d = point_to_segment_dist([5.0, 3.0], [0.0, 0.0], [10.0, 0.0]);
        assert!((d - 3.0).abs() < 1e-5);
    }

    #[test]
    fn dist_to_segment_endpoint() {
        let d = point_to_segment_dist([15.0, 0.0], [0.0, 0.0], [10.0, 0.0]);
        assert!((d - 5.0).abs() < 1e-5);
    }

    #[test]
    fn dist_to_degenerate_segment() {
        let d = point_to_segment_dist([3.0, 4.0], [0.0, 0.0], [0.0, 0.0]);
        assert!((d - 5.0).abs() < 1e-4);
    }

    #[test]
    fn dist_point_on_segment() {
        let d = point_to_segment_dist([5.0, 0.0], [0.0, 0.0], [10.0, 0.0]);
        assert!(d < 1e-5);
    }

    // ── bezier_control_points ────────────────────────────────────────────

    #[test]
    fn bezier_cps_symmetry() {
        let from = [0.0, 0.0];
        let to = [100.0, 0.0];
        let (cp0, cp1) = bezier_control_points(from, to, 0.5);
        assert!((cp0[1] - from[1]).abs() < 1e-5);
        assert!((cp1[1] - to[1]).abs() < 1e-5);
        assert!(cp0[0] > from[0]);
        assert!(cp1[0] < to[0]);
    }

    #[test]
    fn bezier_cps_vertical() {
        let from = [50.0, 0.0];
        let to = [50.0, 100.0];
        let (cp0, _cp1) = bezier_control_points(from, to, 0.5);
        assert!(cp0[0].is_finite());
    }

    // ── ortho_wire_points ────────────────────────────────────────────────

    #[test]
    fn ortho_forward_no_obstacle() {
        let poly = ortho_wire_points([0.0, 50.0], [200.0, 100.0], false, 0.0, 0.0, 10.0);
        assert_eq!(poly.len, 4);
        assert_eq!(poly.points[0], [0.0, 50.0]);
        assert_eq!(poly.points[3], [200.0, 100.0]);
        assert!((poly.points[1][0] - 100.0).abs() < 1e-5);
    }

    #[test]
    fn ortho_forward_with_obstacle() {
        let poly = ortho_wire_points([0.0, 50.0], [200.0, 100.0], true, 60.0, 90.0, 10.0);
        assert_eq!(poly.len, 6);
        assert_eq!(poly.points[0], [0.0, 50.0]);
        assert_eq!(poly.points[5], [200.0, 100.0]);
    }

    #[test]
    fn ortho_backward() {
        let poly = ortho_wire_points([200.0, 50.0], [100.0, 100.0], false, 0.0, 0.0, 10.0);
        assert_eq!(poly.len, 6);
    }

    #[test]
    fn ortho_hit_test_matches_render() {
        let from = [0.0, 50.0];
        let to = [200.0, 100.0];
        let src = NodeId(0);
        let dst = NodeId(1);
        let mid_x = 100.0;
        let mid_y = 75.0;
        let mouse = [mid_x, mid_y];
        let hit = wire_hit_test(
            from,
            to,
            mouse,
            5.0,
            WireStyle::Orthogonal,
            &NodeGraphConfig::default(),
            &[],
            src,
            dst,
        );
        assert!(hit, "Point on vertical segment should be a hit");
    }

    #[test]
    fn ortho_hit_test_far_point_misses() {
        let from = [0.0, 50.0];
        let to = [200.0, 100.0];
        let src = NodeId(0);
        let dst = NodeId(1);
        let mouse = [100.0, 200.0];
        let hit = wire_hit_test(
            from,
            to,
            mouse,
            5.0,
            WireStyle::Orthogonal,
            &NodeGraphConfig::default(),
            &[],
            src,
            dst,
        );
        assert!(!hit, "Distant point should not be a hit");
    }

    #[test]
    fn ortho_hit_test_with_obstacle_detour() {
        let from = [0.0, 50.0];
        let to = [200.0, 100.0];
        let src = NodeId(0);
        let dst = NodeId(1);
        let obstacles = vec![NodeAABB {
            x0: 80.0,
            y0: 40.0,
            x1: 120.0,
            y1: 110.0,
            id: NodeId(2),
        }];
        let poly = ortho_wire_points(from, to, true, 40.0, 110.0, 10.0);
        let detour_y = poly.points[2][1];
        let mouse = [100.0, detour_y];
        let hit = wire_hit_test(
            from,
            to,
            mouse,
            5.0,
            WireStyle::Orthogonal,
            &NodeGraphConfig::default(),
            &obstacles,
            src,
            dst,
        );
        assert!(hit, "Point on detour segment should be a hit");

        let mouse_old = [100.0, 75.0];
        let hit_old = wire_hit_test(
            from,
            to,
            mouse_old,
            5.0,
            WireStyle::Orthogonal,
            &NodeGraphConfig::default(),
            &obstacles,
            src,
            dst,
        );
        assert!(
            !hit_old,
            "Point on old simple path should miss when obstacle reroutes"
        );
    }

    // ── obstacle_aware_bezier_cps ────────────────────────────────────────

    #[test]
    fn bezier_cps_no_obstacle_matches_simple() {
        let from = [0.0, 0.0];
        let to = [100.0, 50.0];
        let simple = bezier_control_points(from, to, 0.5);
        let aware = obstacle_aware_bezier_cps(from, to, 0.5, false, 0.0, 0.0, 10.0);
        assert!((simple.0[0] - aware.0[0]).abs() < 1e-5);
        assert!((simple.0[1] - aware.0[1]).abs() < 1e-5);
    }

    #[test]
    fn bezier_cps_with_obstacle_differs() {
        let from = [0.0, 50.0];
        let to = [200.0, 50.0];
        let simple = bezier_control_points(from, to, 0.5);
        let aware = obstacle_aware_bezier_cps(from, to, 0.5, true, 30.0, 70.0, 10.0);
        assert!((simple.0[1] - aware.0[1]).abs() > 1.0);
    }
}
