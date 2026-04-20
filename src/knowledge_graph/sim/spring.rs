//! Hooke-spring attraction force between connected nodes.

/// Compute spring force vector (attraction) that node `a` at `pos_a` experiences
/// toward node `b` at `pos_b` along their shared edge.
///
/// Force magnitude: `attraction * (dist - rest_length) * weight`
/// Direction: toward `pos_b`.
/// Returns `[f32; 2]` force to ADD to `a`'s acceleration.
pub(crate) fn spring_force(
    pos_a: [f32; 2],
    pos_b: [f32; 2],
    attraction: f32,
    weight: f32,
    rest_length: f32,
) -> [f32; 2] {
    const EPSILON: f32 = 0.1;
    let dx = pos_b[0] - pos_a[0];
    let dy = pos_b[1] - pos_a[1];
    let dist = (dx * dx + dy * dy).sqrt().max(EPSILON);
    let force_mag = attraction * (dist - rest_length) * weight;
    [force_mag * dx / dist, force_mag * dy / dist]
}

/// Coulomb-like repulsion force that `a` experiences away from `b`.
///
/// Force magnitude: `repulsion / (dist² + epsilon)`.
/// Returns `[f32; 2]` force to ADD to `a`'s velocity update (pointing away from `b`).
pub(crate) fn repulsion_force(
    pos_a: [f32; 2],
    pos_b: [f32; 2],
    repulsion: f32,
) -> [f32; 2] {
    const EPSILON: f32 = 100.0; // prevents division by zero at zero distance
    let dx = pos_a[0] - pos_b[0];
    let dy = pos_a[1] - pos_b[1];
    let dist_sq = (dx * dx + dy * dy).max(EPSILON);
    let force_mag = repulsion / dist_sq;
    [force_mag * dx, force_mag * dy]
}
