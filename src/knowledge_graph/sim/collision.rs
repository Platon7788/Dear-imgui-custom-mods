//! Node-node overlap resolution (pushes nodes apart when they overlap).

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
    let dx = pos_a[0] - pos_b[0];
    let dy = pos_a[1] - pos_b[1];
    let dist = (dx * dx + dy * dy).sqrt();
    let min_dist = radius_a + radius_b;
    if dist >= min_dist || dist < EPSILON {
        return None;
    }
    let overlap = min_dist - dist;
    let force_mag = overlap * collision_strength / min_dist;
    Some([force_mag * dx / dist, force_mag * dy / dist])
}
