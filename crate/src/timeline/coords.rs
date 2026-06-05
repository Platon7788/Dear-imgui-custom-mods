//! Coordinate / axis mapping, layout metrics and colour resolution for
//! [`Timeline`]. Kept apart from `render.rs` so the per-frame draw code
//! stays focused on ImGui calls while the pure math lives here and is
//! unit-testable without an ImGui context.

use super::{ColorMode, Span, Timeline};

impl Timeline {
    // ── Coordinate helpers ──────────────────────────────────────────────────

    /// Map a time (seconds) to a horizontal screen pixel.
    ///
    /// The multiply happens in `f64` (sub-nanosecond precision over long
    /// captures) and only the final pixel is narrowed to `f32`.
    pub(super) fn time_to_x(&self, t: f64, content_x: f32) -> f32 {
        content_x + ((t - self.vp.time_start) * self.vp.pixels_per_second) as f32
    }

    /// Inverse of [`Self::time_to_x`]. Guards against a zero / negative
    /// `pixels_per_second` (which would divide by zero) by flooring it
    /// at a tiny positive epsilon.
    pub(super) fn x_to_time(&self, x: f32, content_x: f32) -> f64 {
        let pps = self.vp.pixels_per_second.max(1e-9);
        self.vp.time_start + f64::from(x - content_x) / pps
    }

    // ── Data range / fit ────────────────────────────────────────────────────

    /// Time range of all data across all tracks.
    ///
    /// Returns `(0.0, 1.0)` when there is no data so callers can divide
    /// by a non-zero span unconditionally.
    #[must_use]
    pub fn data_time_range(&self) -> (f64, f64) {
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for t in &self.tracks {
            if let Some((s, e)) = t.time_range() {
                lo = lo.min(s);
                hi = hi.max(e);
            }
        }
        if lo > hi { (0.0, 1.0) } else { (lo, hi) }
    }

    /// Zoom to fit all data into `content_width` pixels.
    ///
    /// The computed pixels-per-second is clamped to the configured
    /// `[min_zoom, max_zoom]` range — without that, a near-instant or
    /// extremely long capture could drive the viewport past the limits
    /// the interactive zoom path enforces, leaving pan/zoom stuck.
    pub fn fit_to_content(&mut self, content_width: f32) {
        let (lo, hi) = self.data_time_range();
        let duration = (hi - lo).max(1e-9);
        let usable = content_width - self.config.track_label_width;
        if usable > 10.0 {
            let pps =
                (f64::from(usable) / duration).clamp(self.config.min_zoom, self.config.max_zoom);
            self.vp.time_start = lo;
            self.vp.pixels_per_second = pps;
            self.vp.zoom_target = pps;
        }
    }

    // ── Color resolution ────────────────────────────────────────────────────

    /// Resolve the fill colour for `span` given a pre-computed
    /// `data_range` (lo, hi) shared across the whole render frame.
    /// The render path computes `data_range` ONCE up front and
    /// threads it here — without that, `ByDuration` mode used to
    /// re-walk every track on every span (O(spans × spans) per
    /// frame), measured as the worst hot-path during the
    /// 2026-04-30 audit on multi-thousand-span traces.
    pub(super) fn span_color(&self, span: &Span, data_range: (f64, f64)) -> [f32; 4] {
        let cfg = &self.config;
        let palette = &cfg.span_palette;

        let Some(&first) = palette.first() else {
            return [0.5, 0.5, 0.5, 0.9];
        };

        match cfg.color_mode {
            ColorMode::Explicit => span.color.unwrap_or(first),
            ColorMode::ByName => {
                let idx = super::str_hash(&span.category) % palette.len();
                palette[idx]
            }
            ColorMode::ByDepth => {
                let idx = span.depth as usize % palette.len();
                palette[idx]
            }
            ColorMode::ByDuration => {
                let (lo, hi) = data_range;
                let range = (hi - lo).max(1e-12);
                let t = ((span.duration() / range) as f32).clamp(0.0, 1.0);
                let r = (t * 2.0).min(1.0);
                let g = if t < 0.5 { t * 2.0 } else { 2.0 - t * 2.0 };
                let b = (1.0 - t * 2.0).max(0.0);
                [r, g, b, 0.9]
            }
        }
    }

    /// Total content height of all tracks (in pixels), respecting
    /// collapsed tracks.
    pub(super) fn total_content_height(&self) -> f32 {
        let cfg = &self.config;
        let mut h = 0.0_f32;
        for track in &self.tracks {
            if track.collapsed {
                h += cfg.track_header_height;
            } else {
                h += cfg.track_header_height
                    + track.depth_rows() as f32 * (cfg.row_height + cfg.row_gap);
            }
        }
        h
    }
}
