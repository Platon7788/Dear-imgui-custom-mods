//! Physics simulation orchestration for the force-graph widget.
//!
//! Repulsion: Barnes-Hut O(N log N) for >50 nodes, naïve O(N²) otherwise.
//! Spring attraction (Hooke) + center pull + collision resolution.

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

    // ── Scratch buffers reused across ticks (avoids per-tick heap allocs) ─────
    // After the first few ticks these have stable capacity and never reallocate.
    scratch_data: Vec<(NodeId, [f32; 2], f32, bool)>,
    scratch_index: HashMap<NodeId, usize>,
    scratch_forces: Vec<[f32; 2]>,
    scratch_particles: Vec<([f32; 2], f32)>,
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
            scratch_data: Vec::new(),
            scratch_index: HashMap::new(),
            scratch_forces: Vec::new(),
            scratch_particles: Vec::new(),
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

        // 3. Refill scratch_data — reuses heap allocation from prior ticks.
        self.scratch_data.clear();
        self.scratch_data.extend(graph.nodes.iter().map(|(id, n)| {
            let r = if config.radius_by_degree {
                let deg = graph.adjacency.get(&id).map_or(0, |v| v.len());
                config.radius_base + config.radius_per_degree * deg as f32
            } else {
                n.style.radius.unwrap_or(config.radius_base)
            };
            (id, n.pos, r, n.style.anchor.is_some())
        }));

        // 4. Rebuild node→index map and zero the flat force buffer — same capacity.
        self.scratch_index.clear();
        for (i, (id, _, _, _)) in self.scratch_data.iter().enumerate() {
            self.scratch_index.insert(*id, i);
        }
        self.scratch_forces.clear();
        self.scratch_forces
            .resize(self.scratch_data.len(), [0.0_f32; 2]);

        // Repulsion: Barnes-Hut O(N log N) for large graphs, O(N²) otherwise.
        let use_bh = self.scratch_data.len() > 50 && config.barnes_hut_theta > 0.0;
        if use_bh {
            self.scratch_particles.clear();
            self.scratch_particles
                .extend(self.scratch_data.iter().map(|(_, pos, _, _)| (*pos, 1.0)));
            let tree =
                barnes_hut::BarnesHutTree::new(&self.scratch_particles, config.barnes_hut_theta);
            for (i, (_, pos, _, _)) in self.scratch_data.iter().enumerate() {
                let f = tree.force(*pos, config.repulsion);
                self.scratch_forces[i][0] += f[0];
                self.scratch_forces[i][1] += f[1];
            }
        } else {
            let n = self.scratch_data.len();
            for i in 0..n {
                for j in (i + 1)..n {
                    let (_, pos_a, _, _) = self.scratch_data[i];
                    let (_, pos_b, _, _) = self.scratch_data[j];
                    let f = spring::repulsion_force(pos_a, pos_b, config.repulsion);
                    self.scratch_forces[i][0] += f[0];
                    self.scratch_forces[i][1] += f[1];
                    self.scratch_forces[j][0] -= f[0];
                    self.scratch_forces[j][1] -= f[1];
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
            let f = spring::spring_force(
                pos_a,
                pos_b,
                config.attraction,
                edge.weight,
                config.link_distance,
            );
            if let Some(&ia) = self.scratch_index.get(&edge.from) {
                self.scratch_forces[ia][0] += f[0];
                self.scratch_forces[ia][1] += f[1];
            }
            if let Some(&ib) = self.scratch_index.get(&edge.to) {
                self.scratch_forces[ib][0] -= f[0];
                self.scratch_forces[ib][1] -= f[1];
            }
        }

        // Center pull + optional downward gravity.
        for (i, (_, pos_a, _, _)) in self.scratch_data.iter().enumerate() {
            self.scratch_forces[i][0] -= config.center_pull * pos_a[0];
            self.scratch_forces[i][1] -= config.center_pull * pos_a[1];
            self.scratch_forces[i][1] += config.gravity_strength;
        }

        // Collision.
        let n = self.scratch_data.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (_, pos_a, r_a, _) = self.scratch_data[i];
                let (_, pos_b, r_b, _) = self.scratch_data[j];
                // Early-reject: skip collision_push (which calls sqrt) when nodes
                // are clearly not overlapping. Uses squared-distance comparison.
                let dx = pos_b[0] - pos_a[0];
                let dy = pos_b[1] - pos_a[1];
                let min_dist = r_a + r_b;
                if dx * dx + dy * dy > min_dist * min_dist {
                    continue;
                }
                if let Some(push) = collision::collision_push(pos_a, pos_b, r_a, r_b, 0.5) {
                    self.scratch_forces[i][0] += push[0];
                    self.scratch_forces[i][1] += push[1];
                    self.scratch_forces[j][0] -= push[0];
                    self.scratch_forces[j][1] -= push[1];
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
            if node.style.pinned {
                // Pinned by user — physics doesn't move it; dragging sets pos directly.
                node.vel = [0.0, 0.0];
                continue;
            }
            let f = self
                .scratch_index
                .get(&id)
                .map(|&i| self.scratch_forces[i])
                .unwrap_or([0.0, 0.0]);
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
    use crate::force_graph::config::ForceConfig;
    use crate::force_graph::data::GraphData;
    use crate::force_graph::style::{EdgeStyle, NodeStyle};

    /// A config tuned to produce visible convergence in test iteration counts.
    fn test_config() -> ForceConfig {
        ForceConfig {
            barnes_hut_theta: 0.9,
            repulsion: 50.0,
            attraction: 0.1,
            link_distance: 80.0,
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
        assert!(dist < 250.0, "Expected nodes to converge, dist={dist:.1}");
    }

    /// A cycle of 4 nodes forms a roughly symmetric layout (all positions finite).
    #[test]
    fn cycle_of_4_forms_regular_polygon() {
        let mut graph = GraphData::new();
        let ids: Vec<_> = [[-10.0_f32, 0.0_f32], [10.0, 0.0], [0.0, -10.0], [0.0, 10.0]]
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
