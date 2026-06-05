//! Unit tests for [`super`] — bezier evaluation, segment distance,
//! orthogonal routing, obstacle detection, and wire hit testing.

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

// ── find_obstacles_in_corridor ───────────────────────────────────────

/// The source and destination nodes never count as obstacles for their own
/// wire — only a *third* node in the corridor does.
#[test]
fn corridor_skips_src_and_dst() {
    let src = NodeId(0);
    let dst = NodeId(1);
    let aabbs = [
        NodeAABB {
            x0: -10.0,
            y0: 40.0,
            x1: 10.0,
            y1: 60.0,
            id: src,
        },
        NodeAABB {
            x0: 190.0,
            y0: 40.0,
            x1: 210.0,
            y1: 60.0,
            id: dst,
        },
    ];
    let (_, _, found) =
        find_obstacles_in_corridor(&aabbs, [0.0, 50.0], [200.0, 50.0], 20.0, src, dst);
    assert!(!found, "src/dst must be excluded from their own corridor");
}

#[test]
fn corridor_detects_third_node() {
    let src = NodeId(0);
    let dst = NodeId(1);
    let aabbs = [NodeAABB {
        x0: 90.0,
        y0: 45.0,
        x1: 110.0,
        y1: 65.0,
        id: NodeId(2),
    }];
    let (y_min, y_max, found) =
        find_obstacles_in_corridor(&aabbs, [0.0, 50.0], [200.0, 50.0], 20.0, src, dst);
    assert!(found, "third node in corridor must be detected");
    assert!((y_min - 45.0).abs() < 1e-5);
    assert!((y_max - 65.0).abs() < 1e-5);
}

#[test]
fn corridor_ignores_out_of_band_node() {
    let src = NodeId(0);
    let dst = NodeId(1);
    // Node well below the wire band (band = 50 ± margin).
    let aabbs = [NodeAABB {
        x0: 90.0,
        y0: 400.0,
        x1: 110.0,
        y1: 460.0,
        id: NodeId(2),
    }];
    let (_, _, found) =
        find_obstacles_in_corridor(&aabbs, [0.0, 50.0], [200.0, 50.0], 20.0, src, dst);
    assert!(!found, "node outside the wire band must not be an obstacle");
}

// ── Bezier wire hit test ─────────────────────────────────────────────

/// A point on the drawn bezier path registers as a hit; the bezier hit test
/// samples the *same* control points used for rendering.
#[test]
fn bezier_hit_test_on_curve() {
    let from = [0.0, 0.0];
    let to = [200.0, 0.0];
    let cfg = NodeGraphConfig::default();
    let (cp0, cp1) = bezier_control_points(from, to, cfg.wire_curvature);
    let on_curve = cubic_bezier(from, cp0, cp1, to, 0.5);
    let hit = wire_hit_test(
        from,
        to,
        on_curve,
        4.0,
        WireStyle::Bezier,
        &cfg,
        &[],
        NodeId(0),
        NodeId(1),
    );
    assert!(hit, "midpoint of the drawn bezier should be a hit");
}
