//! Camera state — tracks pan offset, zoom level, and inertia.

/// Screen-space pan and zoom state for the knowledge-graph canvas.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// World-space offset applied before zoom (pan).
    pub offset: [f32; 2],
    /// Zoom factor (1.0 = 100%).
    pub zoom: f32,
    /// Inertia velocity (pixels/second). Decays to zero each frame.
    pub(crate) velocity: [f32; 2],
}

impl Camera {
    /// Create a new camera at origin with zoom 1.0 and no inertia.
    pub fn new() -> Self {
        Self {
            offset: [0.0, 0.0],
            zoom: 1.0,
            velocity: [0.0, 0.0],
        }
    }

    /// Convert a screen-space point to world-space.
    pub fn screen_to_world(&self, p: [f32; 2], canvas_min: [f32; 2]) -> [f32; 2] {
        [
            (p[0] - canvas_min[0] - self.offset[0]) / self.zoom,
            (p[1] - canvas_min[1] - self.offset[1]) / self.zoom,
        ]
    }

    /// Convert a world-space point to screen-space.
    pub fn world_to_screen(&self, p: [f32; 2], canvas_min: [f32; 2]) -> [f32; 2] {
        [
            p[0] * self.zoom + self.offset[0] + canvas_min[0],
            p[1] * self.zoom + self.offset[1] + canvas_min[1],
        ]
    }

    /// Apply a pan delta (in screen pixels).
    ///
    /// Records the delta as the current inertia velocity so that releasing
    /// the mouse continues the pan smoothly.
    pub fn pan(&mut self, delta: [f32; 2]) {
        self.offset[0] += delta[0];
        self.offset[1] += delta[1];
        self.velocity[0] = delta[0];
        self.velocity[1] = delta[1];
    }

    /// Zoom by `factor` around a pivot point in screen space.
    ///
    /// The pivot point stays at the same world position after the zoom is
    /// applied — this matches the behaviour of most map/graph viewers.
    ///
    /// The zoom level is clamped to `[0.05, 50.0]`.
    pub fn zoom_at(&mut self, factor: f32, pivot_screen: [f32; 2], canvas_min: [f32; 2]) {
        let pivot_world = self.screen_to_world(pivot_screen, canvas_min);
        self.zoom = (self.zoom * factor).clamp(0.05, 50.0);
        // Re-compute offset so pivot_world maps back to pivot_screen.
        self.offset[0] = pivot_screen[0] - canvas_min[0] - pivot_world[0] * self.zoom;
        self.offset[1] = pivot_screen[1] - canvas_min[1] - pivot_world[1] * self.zoom;
    }

    /// Decay inertia velocity toward zero. Call once per frame.
    ///
    /// `decay` is the damping coefficient (higher = faster decay).
    /// The offset is advanced by the residual velocity each frame.
    pub fn update_inertia(&mut self, dt: f32, decay: f32) {
        let factor = (1.0 - decay * dt).max(0.0);
        self.offset[0] += self.velocity[0] * factor;
        self.offset[1] += self.velocity[1] * factor;
        self.velocity[0] *= factor;
        self.velocity[1] *= factor;
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_screen_roundtrip_identity() {
        let cam = Camera {
            offset: [100.0, -50.0],
            zoom: 1.5,
            velocity: [0.0, 0.0],
        };
        let canvas_min = [10.0, 10.0];
        let world = [42.0, -17.0];
        let screen = cam.world_to_screen(world, canvas_min);
        let back = cam.screen_to_world(screen, canvas_min);
        assert!((back[0] - world[0]).abs() < 1e-4);
        assert!((back[1] - world[1]).abs() < 1e-4);
    }

    #[test]
    fn zoom_at_pivot_keeps_pivot_pos_stable() {
        let mut cam = Camera::new();
        let canvas_min = [0.0, 0.0];
        let pivot = [200.0, 150.0];
        let world_before = cam.screen_to_world(pivot, canvas_min);
        cam.zoom_at(2.0, pivot, canvas_min);
        let world_after = cam.screen_to_world(pivot, canvas_min);
        assert!(
            (world_after[0] - world_before[0]).abs() < 1e-3,
            "Pivot world x changed: {} vs {}",
            world_before[0],
            world_after[0]
        );
        assert!(
            (world_after[1] - world_before[1]).abs() < 1e-3,
            "Pivot world y changed: {} vs {}",
            world_before[1],
            world_after[1]
        );
    }

    #[test]
    fn inertia_decays_to_zero() {
        let mut cam = Camera {
            offset: [0.0, 0.0],
            zoom: 1.0,
            velocity: [100.0, 100.0],
        };
        for _ in 0..200 {
            cam.update_inertia(1.0 / 60.0, 10.0);
        }
        assert!(
            cam.velocity[0].abs() < 0.001,
            "velocity.x did not decay: {}",
            cam.velocity[0]
        );
        assert!(
            cam.velocity[1].abs() < 0.001,
            "velocity.y did not decay: {}",
            cam.velocity[1]
        );
    }
}
