//! Hooke-spring attraction force between connected nodes.
//!
//! The arithmetic in these leaf kernels uses the Rust 1.98 `algebraic_*`
//! floating-point methods (`algebraic_add`/`sub`/`mul`/`div`). They compute the
//! same values as `+ - * /` but grant the compiler licence to reassociate and
//! contract (e.g. fuse into FMAs) — the classic physics-loop optimisation the
//! 1.98 release calls out. It is safe here because every caller consumes these
//! forces through an integrator with `EPSILON`/`MAX_SPEED` clamps and only ever
//! compares the two repulsion paths to a `1e-3` tolerance (see
//! `barnes_hut::force_law_matches_naive_repulsion`), far larger than any
//! reassociation rounding drift.

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
    let dx = pos_b[0].algebraic_sub(pos_a[0]);
    let dy = pos_b[1].algebraic_sub(pos_a[1]);
    let dist = dx
        .algebraic_mul(dx)
        .algebraic_add(dy.algebraic_mul(dy))
        .sqrt()
        .max(EPSILON);
    let force_mag = attraction
        .algebraic_mul(dist.algebraic_sub(rest_length))
        .algebraic_mul(weight);
    [
        force_mag.algebraic_mul(dx).algebraic_div(dist),
        force_mag.algebraic_mul(dy).algebraic_div(dist),
    ]
}

/// Coulomb-like repulsion force that `a` experiences away from `b`.
///
/// Force magnitude: `repulsion / (dist² + epsilon)`.
/// Returns `[f32; 2]` force to ADD to `a`'s velocity update (pointing away from `b`).
pub(crate) fn repulsion_force(pos_a: [f32; 2], pos_b: [f32; 2], repulsion: f32) -> [f32; 2] {
    const EPSILON: f32 = 100.0; // prevents division by zero at zero distance
    let dx = pos_a[0].algebraic_sub(pos_b[0]);
    let dy = pos_a[1].algebraic_sub(pos_b[1]);
    let dist_sq = dx
        .algebraic_mul(dx)
        .algebraic_add(dy.algebraic_mul(dy))
        .max(EPSILON);
    let force_mag = repulsion.algebraic_div(dist_sq);
    [force_mag.algebraic_mul(dx), force_mag.algebraic_mul(dy)]
}
