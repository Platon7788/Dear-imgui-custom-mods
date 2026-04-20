//! Camera state — tracks pan offset, zoom level, inertia, and smooth animations.

/// Screen-space pan and zoom state for the knowledge-graph canvas.
///
/// Supports inertia-based panning, animated pan/zoom transitions, and a
/// fit-to-bounds helper suitable for framing a set of graph nodes.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// World-space offset applied before zoom (pan).
    pub offset: [f32; 2],
    /// Zoom factor (1.0 = 100%).
    pub zoom: f32,
    /// Inertia velocity (pixels/second). Decays to zero each frame.
    pub velocity: [f32; 2],

    /// Animation target for `offset`, cleared once the target is reached.
    target_offset: Option<[f32; 2]>,
    /// Animation target for `zoom`, cleared once the target is reached.
    target_zoom: Option<f32>,
    /// Lerp coefficient applied per second — higher values feel snappier.
    anim_speed: f32,

    /// Minimum allowed zoom level.
    pub zoom_min: f32,
    /// Maximum allowed zoom level.
    pub zoom_max: f32,
}

impl Camera {
    /// Create a new camera at origin with zoom 1.0 and no inertia.
    pub fn new() -> Self {
        Self {
            offset: [0.0, 0.0],
            zoom: 1.0,
            velocity: [0.0, 0.0],
            target_offset: None,
            target_zoom: None,
            anim_speed: 8.0,
            zoom_min: 0.05,
            zoom_max: 20.0,
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
    /// The zoom level is clamped to `[zoom_min, zoom_max]`.
    pub fn zoom_at(&mut self, factor: f32, pivot_screen: [f32; 2], canvas_min: [f32; 2]) {
        let pivot_world = self.screen_to_world(pivot_screen, canvas_min);
        self.zoom = (self.zoom * factor).clamp(self.zoom_min, self.zoom_max);
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

    /// Begin a smooth animated pan+zoom toward `target_offset` (world-space)
    /// and `target_zoom`. The animation runs during [`Self::update_animation`].
    pub fn animate_to(&mut self, target_offset: [f32; 2], target_zoom: f32) {
        self.target_offset = Some(target_offset);
        self.target_zoom = Some(target_zoom.clamp(self.zoom_min, self.zoom_max));
    }

    /// Smooth-pan so that world position `world_pos` is centred in a canvas
    /// of size `canvas_size`, at zoom `target_zoom`.
    pub fn animate_to_node(
        &mut self,
        world_pos: [f32; 2],
        canvas_size: [f32; 2],
        target_zoom: f32,
    ) {
        let z = target_zoom.clamp(self.zoom_min, self.zoom_max);
        let target_offset = [
            canvas_size[0] * 0.5 - world_pos[0] * z,
            canvas_size[1] * 0.5 - world_pos[1] * z,
        ];
        self.animate_to(target_offset, z);
    }

    /// Compute the pan + zoom required to fit `bounds_min`…`bounds_max` inside
    /// `canvas_size` with `padding` pixels of margin, then start a smooth
    /// animation toward that view.
    ///
    /// Has no effect when the bounds have zero area.
    pub fn fit_to_bounds(
        &mut self,
        bounds_min: [f32; 2],
        bounds_max: [f32; 2],
        canvas_size: [f32; 2],
        padding: f32,
    ) {
        let w = bounds_max[0] - bounds_min[0];
        let h = bounds_max[1] - bounds_min[1];
        if w < 0.001 || h < 0.001 {
            return;
        }
        let avail_w = (canvas_size[0] - padding * 2.0).max(1.0);
        let avail_h = (canvas_size[1] - padding * 2.0).max(1.0);
        let z = (avail_w / w)
            .min(avail_h / h)
            .clamp(self.zoom_min, self.zoom_max);
        let cx = (bounds_min[0] + bounds_max[0]) * 0.5;
        let cy = (bounds_min[1] + bounds_max[1]) * 0.5;
        let target_offset = [canvas_size[0] * 0.5 - cx * z, canvas_size[1] * 0.5 - cy * z];
        self.animate_to(target_offset, z);
    }

    /// Stop any running animation immediately.
    pub fn cancel_animation(&mut self) {
        self.target_offset = None;
        self.target_zoom = None;
    }

    /// Returns `true` while a smooth pan/zoom animation is in progress.
    pub fn is_animating(&self) -> bool {
        self.target_offset.is_some() || self.target_zoom.is_some()
    }

    /// Advance the smooth animation by `dt` seconds.
    ///
    /// Should be called once per frame after [`Self::update_inertia`]. Cancels
    /// animation automatically when the target is reached within 0.5 px / 0.001
    /// zoom.
    pub fn update_animation(&mut self, dt: f32) {
        let t = (self.anim_speed * dt).min(1.0);

        if let Some(to) = self.target_offset {
            let nx = self.offset[0] + (to[0] - self.offset[0]) * t;
            let ny = self.offset[1] + (to[1] - self.offset[1]) * t;
            self.offset = [nx, ny];
            if (nx - to[0]).abs() < 0.5 && (ny - to[1]).abs() < 0.5 {
                self.offset = to;
                self.target_offset = None;
            }
        }

        if let Some(tz) = self.target_zoom {
            let nz = self.zoom + (tz - self.zoom) * t;
            self.zoom = nz;
            if (nz - tz).abs() < 0.001 {
                self.zoom = tz;
                self.target_zoom = None;
            }
        }
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
            target_offset: None,
            target_zoom: None,
            anim_speed: 8.0,
            zoom_min: 0.05,
            zoom_max: 20.0,
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
            target_offset: None,
            target_zoom: None,
            anim_speed: 8.0,
            zoom_min: 0.05,
            zoom_max: 20.0,
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

    #[test]
    fn fit_to_bounds_sets_animation_target() {
        let mut cam = Camera::new();
        cam.fit_to_bounds([-100.0, -100.0], [100.0, 100.0], [800.0, 600.0], 20.0);
        assert!(cam.is_animating());
    }

    #[test]
    fn animate_to_node_centres_correctly() {
        let mut cam = Camera::new();
        cam.animate_to_node([0.0, 0.0], [800.0, 600.0], 1.0);
        assert!(cam.is_animating());
    }

    #[test]
    fn zoom_at_respects_limits() {
        let mut cam = Camera::new();
        // zoom out past minimum
        for _ in 0..100 {
            cam.zoom_at(0.5, [0.0, 0.0], [0.0, 0.0]);
        }
        assert!(cam.zoom >= cam.zoom_min);
        // zoom in past maximum
        for _ in 0..100 {
            cam.zoom_at(2.0, [0.0, 0.0], [0.0, 0.0]);
        }
        assert!(cam.zoom <= cam.zoom_max);
    }

    #[test]
    fn cancel_animation_stops_immediately() {
        let mut cam = Camera::new();
        cam.animate_to([500.0, 500.0], 3.0);
        assert!(cam.is_animating());
        cam.cancel_animation();
        assert!(!cam.is_animating());
    }
}
