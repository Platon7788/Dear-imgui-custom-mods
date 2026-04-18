//! Physics simulation orchestration for the knowledge-graph widget.
//!
//! Phase A: naïve O(N²) repulsion + Hooke spring attraction + center pull.
//! Phase B will replace repulsion with Barnes-Hut for O(N log N).

mod barnes_hut;
mod collision;
mod spring;

use std::collections::HashMap;

use super::config::ForceConfig;
use super::data::{GraphData, NodeId};

/// Physics simulation state.
pub(crate) struct Simulation {
    /// Total physics ticks run so far.
    iter_count: u32,
    /// Consecutive frames where kinetic energy was below `SLEEP_THRESHOLD`.
    sleep_counter: u32,
    /// When true, [`Simulation::tick`] returns early — no position updates.
    pub(crate) asleep: bool,
}

/// Sum of squared velocities below which we start counting frames toward sleep.
const SLEEP_THRESHOLD: f32 = 0.01;
/// Consecutive frames below threshold before the simulation is put to sleep (~2 s at 60 FPS).
const SLEEP_FRAMES: u32 = 120;

impl Simulation {
    /// Create a new, awake simulation with zero tick count.
    pub(crate) fn new() -> Self {
        Self {
            iter_count: 0,
            sleep_counter: 0,
            asleep: false,
        }
    }

    /// Run one physics tick. Skips if asleep and the graph is not dirty.
    ///
    /// `dt` is the frame delta time in seconds. Values below `1/240` are
    /// clamped to prevent NaN on the first frame when `dt` may be 0.
    pub(crate) fn tick(&mut self, graph: &mut GraphData, config: &ForceConfig, dt: f32) {
        // 1. Check if graph mutated since last frame — if so, wake up.
        if graph.dirty {
            self.asleep = false;
            self.sleep_counter = 0;
            graph.dirty = false;
        }

        // 2. Return early if asleep.
        if self.asleep {
            return;
        }

        let dt = dt.max(1.0 / 240.0); // prevent NaN on first frame with dt=0

        // 3. Collect all (id, pos, radius, anchored) to avoid borrow conflicts.
        let node_data: Vec<(NodeId, [f32; 2], f32, bool)> = graph
            .nodes
            .iter()
            .map(|(id, n)| {
                let r = if config.radius_by_degree {
                    let deg = graph.adjacency.get(&id).map_or(0, |v| v.len());
                    config.radius_base + config.radius_per_degree * deg as f32
                } else {
                    n.style.radius.unwrap_or(config.radius_base)
                };
                (id, n.pos, r, n.style.anchor.is_some())
            })
            .collect();

        // 4. Compute forces (naïve O(N²) repulsion).
        let mut forces: HashMap<NodeId, [f32; 2]> =
            node_data.iter().map(|(id, _, _, _)| (*id, [0.0_f32; 2])).collect();

        // Repulsion: every pair.
        for i in 0..node_data.len() {
            for j in (i + 1)..node_data.len() {
                let (id_a, pos_a, _, _) = node_data[i];
                let (id_b, pos_b, _, _) = node_data[j];
                let f = spring::repulsion_force(pos_a, pos_b, config.repulsion);
                if let Some(fa) = forces.get_mut(&id_a) {
                    fa[0] += f[0];
                    fa[1] += f[1];
                }
                if let Some(fb) = forces.get_mut(&id_b) {
                    fb[0] -= f[0];
                    fb[1] -= f[1];
                }
            }
        }

        // Spring attraction: per edge.
        for (_, edge) in graph.edges.iter() {
            let pos_a = match graph.nodes.get(edge.from) {
                Some(n) => n.pos,
                None => continue,
            };
            let pos_b = match graph.nodes.get(edge.to) {
                Some(n) => n.pos,
                None => continue,
            };
            let f = spring::spring_force(pos_a, pos_b, config.attraction, edge.weight);
            if let Some(fa) = forces.get_mut(&edge.from) {
                fa[0] += f[0];
                fa[1] += f[1];
            }
            if let Some(fb) = forces.get_mut(&edge.to) {
                fb[0] -= f[0];
                fb[1] -= f[1];
            }
        }

        // Center pull.
        for (id_a, pos_a, _, _) in &node_data {
            if let Some(f) = forces.get_mut(id_a) {
                f[0] -= config.center_pull * pos_a[0];
                f[1] -= config.center_pull * pos_a[1];
            }
        }

        // Collision.
        for i in 0..node_data.len() {
            for j in (i + 1)..node_data.len() {
                let (id_a, pos_a, r_a, _) = node_data[i];
                let (id_b, pos_b, r_b, _) = node_data[j];
                if let Some(push) = collision::collision_push(pos_a, pos_b, r_a, r_b, 0.5) {
                    if let Some(fa) = forces.get_mut(&id_a) {
                        fa[0] += push[0];
                        fa[1] += push[1];
                    }
                    if let Some(fb) = forces.get_mut(&id_b) {
                        fb[0] -= push[0];
                        fb[1] -= push[1];
                    }
                }
            }
        }

        // 5. Apply forces to velocities and positions.
        let mut total_vel_sq = 0.0_f32;
        for (id, node) in graph.nodes.iter_mut() {
            if node.style.anchor.is_some() {
                // Anchored node — keep at anchor position, zero velocity.
                if let Some(anchor) = node.style.anchor {
                    node.pos = anchor;
                }
                node.vel = [0.0, 0.0];
                continue;
            }
            let f = forces.get(&id).copied().unwrap_or([0.0, 0.0]);
            let decay_factor = (1.0 - config.velocity_decay * dt).max(0.0);
            node.vel[0] = (node.vel[0] + f[0] * dt) * decay_factor;
            node.vel[1] = (node.vel[1] + f[1] * dt) * decay_factor;
            node.pos[0] += node.vel[0] * dt;
            node.pos[1] += node.vel[1] * dt;
            total_vel_sq += node.vel[0] * node.vel[0] + node.vel[1] * node.vel[1];
        }

        self.iter_count += 1;

        // 6. Sleep-on-idle.
        if total_vel_sq < SLEEP_THRESHOLD {
            self.sleep_counter += 1;
            if self.sleep_counter >= SLEEP_FRAMES {
                self.asleep = true;
            }
        } else {
            self.sleep_counter = 0;
        }
    }

    /// Force the simulation to wake up (e.g. after user interaction).
    pub(crate) fn wake(&mut self) {
        self.asleep = false;
        self.sleep_counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::config::ForceConfig;
    use crate::knowledge_graph::data::GraphData;
    use crate::knowledge_graph::style::{EdgeStyle, NodeStyle};

    /// A config tuned to produce visible convergence in test iteration counts.
    fn test_config() -> ForceConfig {
        ForceConfig {
            barnes_hut_theta: 0.9,
            repulsion: 50.0,
            attraction: 0.1,
            center_pull: 0.001,
            collision_radius: 20.0,
            velocity_decay: 0.6,
            gravity_strength: 0.0,
            radius_by_degree: false,
            radius_base: 20.0,
            radius_per_degree: 2.0,
        }
    }

    /// Two connected nodes start far apart and converge toward the spring rest length.
    #[test]
    fn two_connected_nodes_converge_to_rest_length() {
        let mut graph = GraphData::new();
        let a = graph.add_node(NodeStyle::new("A"));
        let b = graph.add_node(NodeStyle::new("B"));

        // Manually place nodes far apart so we can observe convergence.
        graph.nodes.get_mut(a).unwrap().pos = [0.0, 0.0];
        graph.nodes.get_mut(b).unwrap().pos = [400.0, 0.0];

        graph.add_edge(a, b, EdgeStyle::new(), 1.0, false);
        graph.dirty = true;

        let mut sim = Simulation::new();
        let cfg = test_config();
        for _ in 0..500 {
            sim.tick(&mut graph, &cfg, 1.0 / 60.0);
        }

        let pos_a = graph.nodes.get(a).unwrap().pos;
        let pos_b = graph.nodes.get(b).unwrap().pos;
        let dx = pos_b[0] - pos_a[0];
        let dy = pos_b[1] - pos_a[1];
        let dist = (dx * dx + dy * dy).sqrt();
        // Nodes should be closer to the 80-unit rest length than to 400.
        assert!(
            dist < 250.0,
            "Expected nodes to converge, dist={dist:.1}"
        );
    }

    /// A cycle of 4 nodes forms a roughly symmetric layout (all positions finite).
    #[test]
    fn cycle_of_4_forms_regular_polygon() {
        let mut graph = GraphData::new();
        let ids: Vec<_> = [
            [-10.0_f32, 0.0_f32],
            [10.0, 0.0],
            [0.0, -10.0],
            [0.0, 10.0],
        ]
        .iter()
        .enumerate()
        .map(|(i, pos)| {
            let id = graph.add_node(NodeStyle::new(format!("n{i}")));
            graph.nodes.get_mut(id).unwrap().pos = *pos;
            id
        })
        .collect();

        let n = ids.len();
        for i in 0..n {
            graph.add_edge(ids[i], ids[(i + 1) % n], EdgeStyle::new(), 1.0, false);
        }
        graph.dirty = true;

        let mut sim = Simulation::new();
        let cfg = test_config();
        for _ in 0..800 {
            sim.tick(&mut graph, &cfg, 1.0 / 60.0);
        }

        // All positions should remain finite.
        for id in &ids {
            let pos = graph.nodes.get(*id).unwrap().pos;
            assert!(pos[0].is_finite(), "pos.x is not finite for node {:?}", id);
            assert!(pos[1].is_finite(), "pos.y is not finite for node {:?}", id);
        }
    }

    /// An anchored node should not move even under forces.
    #[test]
    fn anchored_node_stays_put() {
        let anchor_pos = [50.0_f32, 75.0_f32];
        let mut graph = GraphData::new();
        let anchored = graph.add_node(NodeStyle::new("anchor").with_anchor(anchor_pos));
        let mover = graph.add_node(NodeStyle::new("mover"));
        graph.nodes.get_mut(mover).unwrap().pos = [55.0, 80.0];
        graph.add_edge(anchored, mover, EdgeStyle::new(), 1.0, false);
        graph.dirty = true;

        let mut sim = Simulation::new();
        let cfg = test_config();
        for _ in 0..200 {
            sim.tick(&mut graph, &cfg, 1.0 / 60.0);
        }

        let pos = graph.nodes.get(anchored).unwrap().pos;
        assert!(
            (pos[0] - anchor_pos[0]).abs() < 1e-4,
            "Anchored node x moved: {:.4}",
            pos[0]
        );
        assert!(
            (pos[1] - anchor_pos[1]).abs() < 1e-4,
            "Anchored node y moved: {:.4}",
            pos[1]
        );
    }

    /// All positions remain finite after 500 ticks regardless of initial layout.
    #[test]
    fn positions_stay_finite_after_500_ticks() {
        let mut graph = GraphData::new();
        // Place two nodes at the exact same spot (degenerate case).
        let a = graph.add_node(NodeStyle::new("A"));
        let b = graph.add_node(NodeStyle::new("B"));
        let c = graph.add_node(NodeStyle::new("C"));
        graph.nodes.get_mut(a).unwrap().pos = [0.0, 0.0];
        graph.nodes.get_mut(b).unwrap().pos = [0.0, 0.0];
        graph.nodes.get_mut(c).unwrap().pos = [1000.0, -1000.0];
        graph.dirty = true;

        let mut sim = Simulation::new();
        let cfg = test_config();
        for _ in 0..500 {
            sim.tick(&mut graph, &cfg, 1.0 / 60.0);
        }

        for (_, node) in graph.nodes.iter() {
            assert!(node.pos[0].is_finite(), "pos.x became non-finite");
            assert!(node.pos[1].is_finite(), "pos.y became non-finite");
            assert!(node.vel[0].is_finite(), "vel.x became non-finite");
            assert!(node.vel[1].is_finite(), "vel.y became non-finite");
        }
    }

    /// After ~2 s of no movement the simulation should fall asleep.
    #[test]
    fn sleep_on_idle_after_2s_static() {
        let mut graph = GraphData::new();
        // Single isolated node — no edges; center pull only force, quickly damps out.
        graph.add_node(NodeStyle::new("solo"));
        graph.dirty = true;

        let mut sim = Simulation::new();
        let cfg = test_config();
        // Run well beyond the 120-frame sleep threshold.
        for _ in 0..300 {
            sim.tick(&mut graph, &cfg, 1.0 / 60.0);
        }

        assert!(sim.asleep, "Simulation should have fallen asleep");
    }

    /// Setting `graph.dirty = true` wakes a sleeping simulation.
    #[test]
    fn mutation_wakes_simulation() {
        let mut graph = GraphData::new();
        graph.add_node(NodeStyle::new("solo"));
        graph.dirty = true;

        let mut sim = Simulation::new();
        let cfg = test_config();

        // Run until asleep.
        for _ in 0..300 {
            sim.tick(&mut graph, &cfg, 1.0 / 60.0);
        }
        assert!(sim.asleep, "Should be asleep before wake test");

        // Simulate a mutation.
        graph.dirty = true;
        sim.tick(&mut graph, &cfg, 1.0 / 60.0);

        assert!(!sim.asleep, "Mutation should have woken the simulation");
        assert!(
            sim.sleep_counter < SLEEP_FRAMES,
            "Sleep counter should not be at sleep threshold after one tick post-wake"
        );
    }
}
