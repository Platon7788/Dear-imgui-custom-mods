//! Barnes-Hut quadtree for O(N log N) repulsion forces.
//!
//! Approximates the N-body repulsion problem by grouping distant particles
//! into aggregate "super-particles" whose center-of-mass is used in place
//! of individual particle forces.

// ── Data structures ──────────────────────────────────────────────────────────

/// Axis-aligned bounding box with a square footprint.
#[derive(Clone, Debug)]
struct Aabb {
    center: [f32; 2],
    /// Half-width (and half-height — always square).
    half: f32,
}

impl Aabb {
    /// Returns the quadrant index (0=NW, 1=NE, 2=SW, 3=SE) for a point.
    fn quadrant(&self, pos: [f32; 2]) -> usize {
        let east = pos[0] >= self.center[0];
        let south = pos[1] >= self.center[1];
        match (east, south) {
            (false, false) => 0, // NW
            (true, false) => 1,  // NE
            (false, true) => 2,  // SW
            (true, true) => 3,   // SE
        }
    }

    /// Returns the child AABB for the given quadrant index.
    fn child_aabb(&self, quadrant: usize) -> Aabb {
        let q = self.half / 2.0;
        let cx = match quadrant {
            0 | 2 => self.center[0] - q, // west
            _ => self.center[0] + q,     // east
        };
        let cy = match quadrant {
            0 | 1 => self.center[1] - q, // north
            _ => self.center[1] + q,     // south
        };
        Aabb {
            center: [cx, cy],
            half: q,
        }
    }
}

/// A node in the quadtree — can be empty, a single leaf, or an internal cell.
enum QuadNode {
    Empty,
    Leaf {
        pos: [f32; 2],
        mass: f32,
    },
    Internal {
        aabb: Aabb,
        center_of_mass: [f32; 2],
        total_mass: f32,
        children: Box<[QuadNode; 4]>,
    },
}

/// Minimum cell half-width. Once a cell shrinks below this, coincident (or
/// near-coincident) particles are merged into a single super-leaf instead of
/// being subdivided further. Without this guard, two particles at the *exact*
/// same position would recurse until the `half` value underflows to zero —
/// an unbounded recursion / stack overflow. The `1e-4` repulsion softening in
/// [`repulsion`] makes a merged super-leaf physically indistinguishable from
/// two coincident leaves, so this is lossless for the force computation.
const MIN_CELL_HALF: f32 = 1e-3;

impl QuadNode {
    /// Insert a particle into this node (or its subtree).  `aabb` is this
    /// node's bounding box.
    fn insert(&mut self, pos: [f32; 2], mass: f32, aabb: Aabb) {
        match self {
            QuadNode::Empty => {
                *self = QuadNode::Leaf { pos, mass };
            }
            QuadNode::Leaf {
                pos: lpos,
                mass: lmass,
            } => {
                // Coincident / sub-resolution particles: merge their mass into
                // a single leaf at the mass-weighted centroid rather than
                // subdividing forever (see `MIN_CELL_HALF`).
                let dx = lpos[0] - pos[0];
                let dy = lpos[1] - pos[1];
                if aabb.half <= MIN_CELL_HALF
                    || (dx * dx + dy * dy) < (MIN_CELL_HALF * MIN_CELL_HALF)
                {
                    let total_mass = *lmass + mass;
                    if total_mass > 0.0 {
                        lpos[0] = (lpos[0] * *lmass + pos[0] * mass) / total_mass;
                        lpos[1] = (lpos[1] * *lmass + pos[1] * mass) / total_mass;
                    }
                    *lmass = total_mass;
                    return;
                }

                // Promote this leaf to an internal node, re-insert the old
                // leaf and the new particle into the appropriate children.
                let old_pos = *lpos;
                let old_mass = *lmass;
                let children = Box::new([
                    QuadNode::Empty,
                    QuadNode::Empty,
                    QuadNode::Empty,
                    QuadNode::Empty,
                ]);
                let total_mass = old_mass + mass;
                let com = [
                    (old_pos[0] * old_mass + pos[0] * mass) / total_mass,
                    (old_pos[1] * old_mass + pos[1] * mass) / total_mass,
                ];
                *self = QuadNode::Internal {
                    aabb: aabb.clone(),
                    center_of_mass: com,
                    total_mass,
                    children,
                };
                // Re-insert both particles into children.
                if let QuadNode::Internal {
                    aabb: iabb,
                    children,
                    ..
                } = self
                {
                    let q_old = iabb.quadrant(old_pos);
                    let aabb_old = iabb.child_aabb(q_old);
                    children[q_old].insert(old_pos, old_mass, aabb_old);
                    let q_new = iabb.quadrant(pos);
                    let aabb_new = iabb.child_aabb(q_new);
                    children[q_new].insert(pos, mass, aabb_new);
                }
            }
            QuadNode::Internal {
                aabb: iabb,
                center_of_mass,
                total_mass,
                children,
            } => {
                // Update aggregate values.
                let new_total = *total_mass + mass;
                center_of_mass[0] = (center_of_mass[0] * (*total_mass) + pos[0] * mass) / new_total;
                center_of_mass[1] = (center_of_mass[1] * (*total_mass) + pos[1] * mass) / new_total;
                *total_mass = new_total;
                // Recurse into the appropriate quadrant.
                let q = iabb.quadrant(pos);
                let child_aabb = iabb.child_aabb(q);
                children[q].insert(pos, mass, child_aabb);
            }
        }
    }

    /// Compute the net repulsion force on a particle at `pos` from this node.
    fn traverse(&self, pos: [f32; 2], k: f32, theta: f32) -> [f32; 2] {
        match self {
            QuadNode::Empty => [0.0, 0.0],
            QuadNode::Leaf { pos: lpos, mass } => repulsion(pos, *lpos, k * mass),
            QuadNode::Internal {
                aabb,
                center_of_mass,
                total_mass,
                children,
            } => {
                let dx = pos[0] - center_of_mass[0];
                let dy = pos[1] - center_of_mass[1];
                let dist = (dx * dx + dy * dy).sqrt();
                let size = aabb.half * 2.0;
                if dist > 0.0 && size / dist < theta {
                    // Far enough — treat whole cell as single particle.
                    repulsion(pos, *center_of_mass, k * total_mass)
                } else {
                    // Too close — recurse into all four children.
                    let mut fx = 0.0_f32;
                    let mut fy = 0.0_f32;
                    for child in children.iter() {
                        let f = child.traverse(pos, k, theta);
                        fx += f[0];
                        fy += f[1];
                    }
                    [fx, fy]
                }
            }
        }
    }
}

/// Coulomb-like repulsion force at `a` away from `b`, scaled by `k`.
///
/// `k` should already incorporate the aggregate particle mass at `b`.
///
/// This MUST stay numerically identical (up to the aggregation `k` carries) to
/// [`super::spring::repulsion_force`], which the naïve O(N²) path uses for
/// graphs ≤ 50 nodes. Both produce a force whose magnitude falls off as
/// `k / dist` (i.e. `[k/dist_sq * dx, k/dist_sq * dy]`, an unnormalised
/// direction vector). Earlier this function normalised the direction, giving a
/// `k / dist_sq` magnitude — a *different* force law — so the layout changed
/// character the instant a graph crossed the 50-node Barnes-Hut threshold.
///
/// The `EPSILON` clamp on `dist_sq` matches the naïve path and prevents both
/// division-by-zero and an unbounded force spike between coincident nodes.
#[inline]
fn repulsion(a: [f32; 2], b: [f32; 2], k: f32) -> [f32; 2] {
    const EPSILON: f32 = 100.0;
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dist_sq = (dx * dx + dy * dy).max(EPSILON);
    let mag = k / dist_sq;
    [mag * dx, mag * dy]
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Barnes-Hut quadtree for approximate O(N log N) repulsion force computation.
pub(crate) struct BarnesHutTree {
    root: QuadNode,
    theta: f32,
}

impl BarnesHutTree {
    /// Build a new tree from a slice of `(position, mass)` pairs.
    ///
    /// `theta` is the Barnes-Hut approximation parameter (typically 0.5–1.0;
    /// higher values are faster but less accurate).  `theta = 0.9` is a
    /// reasonable default for interactive simulations.
    pub(crate) fn new(particles: &[([f32; 2], f32)], theta: f32) -> Self {
        if particles.is_empty() {
            return Self {
                root: QuadNode::Empty,
                theta,
            };
        }

        // Compute the axis-aligned bounding box of all particles.
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for (pos, _) in particles {
            if pos[0] < min_x {
                min_x = pos[0];
            }
            if pos[0] > max_x {
                max_x = pos[0];
            }
            if pos[1] < min_y {
                min_y = pos[1];
            }
            if pos[1] > max_y {
                max_y = pos[1];
            }
        }

        // Make it square with a small padding so boundary particles are inside.
        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;
        let half = ((max_x - min_x).max(max_y - min_y) / 2.0) + 1.0;
        let root_aabb = Aabb {
            center: [cx, cy],
            half,
        };

        let mut root = QuadNode::Empty;
        for &(pos, mass) in particles {
            root.insert(pos, mass, root_aabb.clone());
        }

        Self { root, theta }
    }

    /// Compute the net repulsion force on a particle at `pos` with repulsion
    /// strength `k`.
    pub(crate) fn force(&self, pos: [f32; 2], k: f32) -> [f32; 2] {
        self.root.traverse(pos, k, self.theta)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty tree returns zero force.
    #[test]
    fn empty_tree_returns_zero() {
        let tree = BarnesHutTree::new(&[], 0.9);
        let f = tree.force([0.0, 0.0], 100.0);
        assert_eq!(f, [0.0, 0.0]);
    }

    /// A single leaf particle produces a finite, non-zero force on a nearby
    /// point that is not at the exact same location.
    #[test]
    fn single_particle_produces_finite_force() {
        let tree = BarnesHutTree::new(&[([0.0, 0.0], 1.0)], 0.9);
        let f = tree.force([10.0, 0.0], 100.0);
        assert!(f[0].is_finite(), "fx must be finite");
        assert!(f[1].is_finite(), "fy must be finite");
        // Force should point away from the particle (positive x direction).
        assert!(f[0] > 0.0, "force should push away, got fx={}", f[0]);
    }

    /// Two identical particles placed symmetrically about the origin produce
    /// zero net force on a query point at the origin.
    #[test]
    fn symmetric_particles_cancel_at_midpoint() {
        let particles = [([-50.0_f32, 0.0], 1.0), ([50.0, 0.0], 1.0)];
        let tree = BarnesHutTree::new(&particles, 0.9);
        let f = tree.force([0.0, 0.0], 100.0);
        // The x-components should cancel; y-components are already zero.
        assert!(f[0].abs() < 1e-3, "fx should be near zero, got {}", f[0]);
        assert!(f[1].abs() < 1e-3, "fy should be near zero, got {}", f[1]);
    }

    /// Regression: many particles at the *exact same position* must not cause
    /// unbounded quadtree recursion / stack overflow. Before the `MIN_CELL_HALF`
    /// merge guard, this hung the build (the cell `half` halved forever while
    /// both coincident points kept landing in the same quadrant).
    #[test]
    fn coincident_particles_do_not_stack_overflow() {
        // 64 particles all at the origin — degenerate but legal (e.g. a graph
        // where every node was just added before the first physics tick).
        let particles: Vec<([f32; 2], f32)> = (0..64).map(|_| ([0.0_f32, 0.0], 1.0)).collect();
        let tree = BarnesHutTree::new(&particles, 0.9);
        // Query a nearby point: force must be finite (self-overlap is softened).
        let f = tree.force([5.0, 0.0], 100.0);
        assert!(f[0].is_finite() && f[1].is_finite());
        // And the query exactly on top of the cluster must also stay finite.
        let f0 = tree.force([0.0, 0.0], 100.0);
        assert!(f0[0].is_finite() && f0[1].is_finite());
    }

    /// The Barnes-Hut force law must match the naïve O(N²) path so a graph does
    /// not visibly change layout when it crosses the 50-node threshold. With a
    /// single far particle and a coarse theta, the BH approximation collapses
    /// to a single repulsion term, which must equal `spring::repulsion_force`.
    #[test]
    fn force_law_matches_naive_repulsion() {
        let b = [80.0_f32, 0.0];
        let query = [0.0_f32, 0.0];
        let k = 120.0_f32;
        let tree = BarnesHutTree::new(&[(b, 1.0)], 0.9);
        let bh = tree.force(query, k);
        let naive = super::super::spring::repulsion_force(query, b, k);
        assert!(
            (bh[0] - naive[0]).abs() < 1e-3 && (bh[1] - naive[1]).abs() < 1e-3,
            "BH {bh:?} should match naive {naive:?}"
        );
    }

    /// Building a tree with 100 particles does not panic, and every queried
    /// force is finite.
    #[test]
    fn large_tree_produces_finite_forces() {
        // Deterministic pseudo-random positions using a simple LCG.
        let mut state: u32 = 0xDEAD_BEEF;
        let mut next = || -> f32 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            // Map to [-500, 500]
            (state as f32 / u32::MAX as f32) * 1000.0 - 500.0
        };

        let particles: Vec<([f32; 2], f32)> = (0..100).map(|_| ([next(), next()], 1.0)).collect();

        let tree = BarnesHutTree::new(&particles, 0.9);

        for &(pos, _) in &particles {
            let f = tree.force(pos, 100.0);
            assert!(f[0].is_finite(), "fx is not finite at {:?}", pos);
            assert!(f[1].is_finite(), "fy is not finite at {:?}", pos);
        }
    }
}
