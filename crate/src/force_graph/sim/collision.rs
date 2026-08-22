//! Node-node overlap resolution (pushes nodes apart when they overlap).
//!
//! Uses the Rust 1.98 `algebraic_*` float methods for the same reasons as
//! [`super::spring`] — see that module's header.

/// Compute the push-apart impulse that node `a` (at `pos_a` with `radius_a`) receives
/// due to overlap with node `b` (at `pos_b` with `radius_b`).
///
/// Returns `Some([fx, fy])` with the force to apply to `a` (apply `-force` to `b`),
/// or `None` if nodes don't overlap.
pub(crate) fn collision_push(
    pos_a: [f32; 2],
    pos_b: [f32; 2],
    radius_a: f32,
    radius_b: f32,
    collision_strength: f32,
) -> Option<[f32; 2]> {
    const EPSILON: f32 = 0.01;
    let dx = pos_a[0].algebraic_sub(pos_b[0]);
    let dy = pos_a[1].algebraic_sub(pos_b[1]);
    let dist = dx
        .algebraic_mul(dx)
        .algebraic_add(dy.algebraic_mul(dy))
        .sqrt();
    let min_dist = radius_a + radius_b;
    if dist >= min_dist {
        return None;
    }
    // Exactly-overlapping nodes: push right to avoid NaN from div-by-zero.
    if dist < EPSILON {
        return Some([collision_strength, 0.0]);
    }
    let overlap = min_dist.algebraic_sub(dist);
    let force_mag = overlap
        .algebraic_mul(collision_strength)
        .algebraic_div(min_dist);
    Some([
        force_mag.algebraic_mul(dx).algebraic_div(dist),
        force_mag.algebraic_mul(dy).algebraic_div(dist),
    ])
}
