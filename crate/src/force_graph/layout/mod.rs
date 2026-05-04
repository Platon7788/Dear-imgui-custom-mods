//! Initial node placement strategies.

mod community;

use super::data::GraphData;

/// Place new nodes in a Fibonacci-lattice spiral for visually pleasing
/// initial positions before physics settles.
///
/// Called automatically by `GraphData::add_node`; exposed here for
/// layouts that want to batch-reposition after a `clear()`.
pub(crate) fn initial_position(index: usize) -> [f32; 2] {
    let r = (index as f32).sqrt() * 15.0;
    let angle = index as f32 * 2.399_963; // golden angle ≈ 137.5°
    [r * angle.cos(), r * angle.sin()]
}

/// Randomize all node positions — called by `GraphViewer::reset_layout`.
///
/// Anchored nodes are left in place. All others are repositioned using
/// [`initial_position`] and their velocities are zeroed. Sets `graph.dirty`
/// so the simulation wakes on the next frame.
pub(crate) fn scatter_positions(graph: &mut GraphData) {
    for (i, (_, node)) in graph.nodes.iter_mut().enumerate() {
        if node.style.anchor.is_none() {
            node.pos = initial_position(i);
            node.vel = [0.0, 0.0];
        }
    }
    graph.dirty = true;
}
