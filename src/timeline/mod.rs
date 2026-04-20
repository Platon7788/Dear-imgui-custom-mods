//! # Timeline
//!
//! Zoomable horizontal timeline for profiler data.
//! Shows nested call spans as colored bars across multiple tracks
//! (one per thread / category). Supports pan/zoom, markers, tooltips,
//! selection, and both top-down (icicle) and bottom-up (flame) modes.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::timeline::{Timeline, Track, Span};
//!
//! let mut tl = Timeline::new("##profiler");
//! let mut track = Track::new("Main Thread");
//! track.add_span(Span::new(0, 0.0, 0.050, 0, "frame"));
//! track.add_span(Span::new(1, 0.0, 0.020, 1, "update"));
//! track.add_span(Span::new(2, 0.020, 0.050, 1, "render"));
//! tl.add_track(track);
//! // In render loop: tl.render(ui);
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
pub mod span;
pub mod track;

pub use config::{ColorMode, TimeUnit, TimelineConfig, TimelineMode};
pub use span::{Marker, Span};
pub use track::Track;

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

/// Convert `[r, g, b, a]` to packed u32 color.
fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// Simple string hash for palette indexing.
fn str_hash(s: &str) -> usize {
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h as usize
}

// ── Timeline events ─────────────────────────────────────────────────────────

/// Event emitted by the timeline on user interaction.
#[derive(Debug, Clone)]
pub enum TimelineEvent {
    /// A span was clicked.
    SpanClicked { span_id: u64 },
    /// A span was double-clicked.
    SpanDoubleClicked { span_id: u64 },
    /// A marker was clicked.
    MarkerClicked { index: usize },
    /// View was panned / zoomed (new visible range in seconds).
    ViewChanged { start: f64, end: f64 },
}

// ── Viewport state ──────────────────────────────────────────────────────────

/// Internal view state for pan/zoom.
#[derive(Debug, Clone)]
struct Viewport {
    /// Left edge time (seconds).
    time_start: f64,
    /// Pixels per second.
    pixels_per_second: f64,
    /// Zoom target for smooth interpolation.
    zoom_target: f64,
    /// Vertical scroll offset in pixels.
    scroll_y: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            time_start: 0.0,
            pixels_per_second: 10_000.0,
            zoom_target: 10_000.0,
            scroll_y: 0.0,
        }
    }
}

// ── Timeline widget ─────────────────────────────────────────────────────────

/// Profiler timeline / flame graph widget.
pub struct Timeline {
    /// ImGui ID string.
    id: String,
    /// Tracks (rows of spans).
    tracks: Vec<Track>,
    /// Vertical marker lines.
    markers: Vec<Marker>,
    /// Configuration.
    pub config: TimelineConfig,
    /// View state.
    vp: Viewport,
    /// Currently selected span id.
    selected_span: Option<u64>,
    /// Currently hovered span id (transient per-frame).
    hovered_span: Option<u64>,
    /// Whether the user is panning.
    panning: bool,
    /// Last mouse X during pan (pixels).
    pan_start_x: f32,
    /// Time at pan start.
    pan_start_time: f64,
}

impl Timeline {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tracks: Vec::new(),
            markers: Vec::new(),
            config: TimelineConfig::default(),
            vp: Viewport::default(),
            selected_span: None,
            hovered_span: None,
            panning: false,
            pan_start_x: 0.0,
            pan_start_time: 0.0,
        }
    }

    // ── Data API ────────────────────────────────────────────────────────────

    pub fn add_track(&mut self, track: Track) {
        self.tracks.push(track);
    }

    pub fn track_mut(&mut self, index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(index)
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn clear_tracks(&mut self) {
        self.tracks.clear();
    }

    pub fn add_marker(&mut self, marker: Marker) {
        self.markers.push(marker);
    }

    pub fn clear_markers(&mut self) {
        self.markers.clear();
    }

    pub fn selected_span(&self) -> Option<u64> {
        self.selected_span
    }

    /// Time range of all data across all tracks.
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

    /// Zoom to fit all data.
    pub fn fit_to_content(&mut self, content_width: f32) {
        let (lo, hi) = self.data_time_range();
        let duration = (hi - lo).max(1e-9);
        let usable = content_width - self.config.track_label_width;
        if usable > 10.0 {
            let pps = usable as f64 / duration;
            self.vp.time_start = lo;
            self.vp.pixels_per_second = pps;
            self.vp.zoom_target = pps;
        }
    }

    // ── Coordinate helpers ──────────────────────────────────────────────────

    fn time_to_x(&self, t: f64, content_x: f32) -> f32 {
        content_x + ((t - self.vp.time_start) * self.vp.pixels_per_second) as f32
    }

    fn x_to_time(&self, x: f32, content_x: f32) -> f64 {
        let pps = self.vp.pixels_per_second.max(1e-9);
        self.vp.time_start + (x - content_x) as f64 / pps
    }

    // ── Color resolution ────────────────────────────────────────────────────

    fn span_color(&self, span: &Span) -> [f32; 4] {
        let cfg = &self.config;
        let palette = &cfg.span_palette;

        if palette.is_empty() {
            return [0.5, 0.5, 0.5, 0.9];
        }

        match cfg.color_mode {
            ColorMode::Explicit => span.color.unwrap_or(palette[0]),
            ColorMode::ByName => {
                let idx = str_hash(&span.category) % palette.len();
                palette[idx]
            }
            ColorMode::ByDepth => {
                let idx = span.depth as usize % palette.len();
                palette[idx]
            }
            ColorMode::ByDuration => {
                let (lo, hi) = self.data_time_range();
                let range = (hi - lo).max(1e-12);
                let t = ((span.duration() / range) as f32).clamp(0.0, 1.0);
                let r = (t * 2.0).min(1.0);
                let g = if t < 0.5 { t * 2.0 } else { 2.0 - t * 2.0 };
                let b = (1.0 - t * 2.0).max(0.0);
                [r, g, b, 0.9]
            }
        }
    }

    /// Total content height of all tracks.
    fn total_content_height(&self) -> f32 {
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

    // ── Render ──────────────────────────────────────────────────────────────

    /// Render the timeline. Returns events from this frame.
    pub fn render(&mut self, ui: &Ui) -> Vec<TimelineEvent> {
        let mut events = Vec::new();
        self.hovered_span = None;

        let avail = ui.content_region_avail();
        let _id_tok = ui.push_id(&self.id);

        // Snapshot mutable state needed inside the closure
        let cfg = self.config.clone();

        ui.child_window("##timeline_canvas")
            .size(avail)
            .build(ui, || {
                let win_pos = ui.cursor_screen_pos();
                let win_size = ui.content_region_avail();
                let draw = ui.get_window_draw_list();

                // ── Background ──────────────────────────────────────────
                draw.add_rect(
                    win_pos,
                    [win_pos[0] + win_size[0], win_pos[1] + win_size[1]],
                    col32(cfg.color_bg),
                )
                .filled(true)
                .build();

                // ── Layout ──────────────────────────────────────────────
                let label_w = if cfg.show_track_labels {
                    cfg.track_label_width
                } else {
                    0.0
                };
                let ruler_h = if cfg.show_ruler {
                    cfg.ruler_height
                } else {
                    0.0
                };
                let content_x = win_pos[0] + label_w;
                let content_w = (win_size[0] - label_w).max(1.0);
                let content_y = win_pos[1] + ruler_h;

                // ── Smooth zoom ─────────────────────────────────────────
                if cfg.smooth_zoom {
                    let dt = ui.io().delta_time() as f64;
                    let diff = self.vp.zoom_target - self.vp.pixels_per_second;
                    if diff.abs() > 0.01 {
                        self.vp.pixels_per_second +=
                            diff * (1.0 - (-cfg.smooth_zoom_speed as f64 * dt).exp());
                    } else {
                        self.vp.pixels_per_second = self.vp.zoom_target;
                    }
                }

                // ── Input: pan & zoom ───────────────────────────────────
                let mouse_pos = ui.io().mouse_pos();
                let in_content = mouse_pos[0] >= content_x
                    && mouse_pos[0] < win_pos[0] + win_size[0]
                    && mouse_pos[1] >= win_pos[1]
                    && mouse_pos[1] < win_pos[1] + win_size[1];

                if in_content && ui.is_window_hovered() {
                    let wheel = ui.io().mouse_wheel();
                    let shift_held = ui.io().key_shift();
                    if wheel.abs() > 0.01 && !shift_held {
                        let zoom_factor = 1.15_f64.powf(wheel as f64);
                        let mouse_time = self.x_to_time(mouse_pos[0], content_x);

                        let new_pps =
                            (self.vp.zoom_target * zoom_factor).clamp(cfg.min_zoom, cfg.max_zoom);
                        self.vp.zoom_target = new_pps;

                        if !cfg.smooth_zoom {
                            self.vp.pixels_per_second = new_pps;
                        }

                        self.vp.time_start =
                            mouse_time - (mouse_pos[0] - content_x) as f64 / new_pps;

                        events.push(TimelineEvent::ViewChanged {
                            start: self.vp.time_start,
                            end: self.vp.time_start + content_w as f64 / new_pps,
                        });
                    }

                    // Pan with middle or right mouse button
                    if ui.is_mouse_clicked(MouseButton::Middle)
                        || ui.is_mouse_clicked(MouseButton::Right)
                    {
                        self.panning = true;
                        self.pan_start_x = mouse_pos[0];
                        self.pan_start_time = self.vp.time_start;
                    }
                }

                if self.panning {
                    if ui.is_mouse_down(MouseButton::Middle) || ui.is_mouse_down(MouseButton::Right)
                    {
                        let dx = mouse_pos[0] - self.pan_start_x;
                        self.vp.time_start =
                            self.pan_start_time - dx as f64 / self.vp.pixels_per_second;
                    } else {
                        self.panning = false;
                    }
                }

                // ── Ruler ───────────────────────────────────────────────
                if cfg.show_ruler {
                    draw.add_rect(
                        [content_x, win_pos[1]],
                        [content_x + content_w, win_pos[1] + ruler_h],
                        col32(cfg.color_ruler_bg),
                    )
                    .filled(true)
                    .build();

                    let visible_duration = content_w as f64 / self.vp.pixels_per_second;
                    let (tick_interval, unit) = adaptive_ticks(visible_duration, content_w);

                    if tick_interval > 0.0 {
                        let first_tick =
                            (self.vp.time_start / tick_interval).floor() * tick_interval;
                        let end_time = self.vp.time_start + visible_duration;

                        let mut t = first_tick;
                        let mut safety = 0;
                        while t <= end_time && safety < 2000 {
                            safety += 1;
                            let x = self.time_to_x(t, content_x);

                            if x >= content_x && x <= content_x + content_w {
                                draw.add_line(
                                    [x, win_pos[1] + ruler_h - 6.0],
                                    [x, win_pos[1] + ruler_h],
                                    col32(cfg.color_ruler_text),
                                )
                                .build();

                                let val = t * unit.factor();
                                let label = if val.abs() < 0.001 {
                                    format!("0{}", unit.suffix())
                                } else if val.fract().abs() < 0.001 {
                                    format!("{:.0}{}", val, unit.suffix())
                                } else {
                                    format!("{:.1}{}", val, unit.suffix())
                                };

                                let text_size = calc_text_size(&label);
                                let tx = (x - text_size[0] * 0.5).max(content_x);
                                let ty = win_pos[1] + (ruler_h - 6.0 - text_size[1]) * 0.5;
                                draw.add_text([tx, ty], col32(cfg.color_ruler_text), &label);
                            }
                            t += tick_interval;
                        }
                    }
                }

                // ── Track label background ──────────────────────────────
                if cfg.show_track_labels && label_w > 0.0 {
                    draw.add_rect(
                        [win_pos[0], content_y],
                        [win_pos[0] + label_w, win_pos[1] + win_size[1]],
                        col32([
                            cfg.color_bg[0] + 0.02,
                            cfg.color_bg[1] + 0.02,
                            cfg.color_bg[2] + 0.03,
                            1.0,
                        ]),
                    )
                    .filled(true)
                    .build();
                }

                // ── Tracks & spans ──────────────────────────────────────
                let mut y = content_y - self.vp.scroll_y;

                for (ti, track) in self.tracks.iter().enumerate() {
                    let rows = if track.collapsed {
                        0
                    } else {
                        track.depth_rows()
                    };
                    let track_h = if track.collapsed {
                        cfg.track_header_height
                    } else {
                        cfg.track_header_height + rows as f32 * (cfg.row_height + cfg.row_gap)
                    };

                    // Cull off-screen tracks
                    if y + track_h < content_y || y > win_pos[1] + win_size[1] {
                        y += track_h;
                        continue;
                    }

                    // Track stripe
                    if ti % 2 == 1 {
                        draw.add_rect(
                            [content_x, y],
                            [win_pos[0] + win_size[0], y + track_h],
                            col32(cfg.color_bg_alt),
                        )
                        .filled(true)
                        .build();
                    }

                    // Track label
                    if cfg.show_track_labels {
                        let arrow = if track.collapsed {
                            "\u{25B8}"
                        } else {
                            "\u{25BE}"
                        };
                        let label_text = format!("{} {}", arrow, track.name);
                        let text_y = y + (cfg.track_header_height - 14.0) * 0.5;
                        draw.add_text(
                            [win_pos[0] + 4.0, text_y],
                            col32(cfg.color_track_label),
                            &label_text,
                        );
                    }

                    // Track header separator
                    draw.add_line(
                        [content_x, y + cfg.track_header_height - 1.0],
                        [win_pos[0] + win_size[0], y + cfg.track_header_height - 1.0],
                        col32(cfg.color_track_separator),
                    )
                    .build();

                    // Spans
                    if !track.collapsed {
                        let span_base_y = y + cfg.track_header_height;

                        for span in &track.spans {
                            let sx = self.time_to_x(span.start, content_x);
                            let ex = self.time_to_x(span.end, content_x);

                            if ex < content_x || sx > win_pos[0] + win_size[0] {
                                continue;
                            }

                            let span_w = (ex - sx).max(cfg.min_span_width);
                            let sy =
                                span_base_y + span.depth as f32 * (cfg.row_height + cfg.row_gap);
                            let ey = sy + cfg.row_height;

                            if ey < content_y || sy > win_pos[1] + win_size[1] {
                                continue;
                            }

                            let span_color = self.span_color(span);
                            draw.add_rect([sx, sy], [sx + span_w, ey], col32(span_color))
                                .filled(true)
                                .build();

                            // Span text (only if wide enough)
                            if span_w > 20.0 {
                                let text_size = calc_text_size(&span.label);
                                if text_size[0] < span_w - 4.0 {
                                    let tx = sx + (span_w - text_size[0]) * 0.5;
                                    let ty = sy + (cfg.row_height - text_size[1]) * 0.5;
                                    draw.add_text(
                                        [tx, ty],
                                        col32(cfg.color_span_text),
                                        &span.label,
                                    );
                                } else if span_w > 6.0 {
                                    let ty = sy + (cfg.row_height - calc_text_size("A")[1]) * 0.5;
                                    draw.add_text(
                                        [sx + 2.0, ty],
                                        col32(cfg.color_span_text),
                                        &span.label,
                                    );
                                }
                            }

                            // Hover / click detection
                            if in_content
                                && mouse_pos[0] >= sx
                                && mouse_pos[0] < sx + span_w
                                && mouse_pos[1] >= sy
                                && mouse_pos[1] < ey
                            {
                                self.hovered_span = Some(span.id);

                                draw.add_rect([sx, sy], [sx + span_w, ey], col32(cfg.color_hover))
                                    .build();

                                if ui.is_mouse_clicked(MouseButton::Left) {
                                    self.selected_span = Some(span.id);
                                    events.push(TimelineEvent::SpanClicked { span_id: span.id });
                                }
                                if ui.is_mouse_double_clicked(MouseButton::Left) {
                                    events.push(TimelineEvent::SpanDoubleClicked {
                                        span_id: span.id,
                                    });
                                }

                                // Tooltip
                                if cfg.show_tooltip {
                                    ui.tooltip(|| {
                                        let dur = span.duration();
                                        let (val, suffix) = format_duration(dur);
                                        ui.text(format!(
                                            "{} \u{2014} {:.2}{}",
                                            span.label, val, suffix
                                        ));
                                        if !span.category.is_empty() && span.category != span.label
                                        {
                                            ui.text(format!("Category: {}", span.category));
                                        }
                                        if let Some(ref src) = span.source {
                                            ui.text(format!("Source: {}", src));
                                        }
                                        ui.text(format!(
                                            "Start: {:.4}ms  End: {:.4}ms",
                                            span.start * 1000.0,
                                            span.end * 1000.0,
                                        ));
                                        ui.text(format!("Depth: {}", span.depth));
                                    });
                                }
                            }

                            // Selection outline
                            if self.selected_span == Some(span.id) {
                                draw.add_rect(
                                    [sx - 1.0, sy - 1.0],
                                    [sx + span_w + 1.0, ey + 1.0],
                                    col32(cfg.color_selection),
                                )
                                .build();
                            }
                        }
                    }

                    y += track_h;
                }

                // ── Markers ─────────────────────────────────────────────
                if cfg.show_markers {
                    for (mi, marker) in self.markers.iter().enumerate() {
                        let mx = self.time_to_x(marker.time, content_x);
                        if mx < content_x || mx > win_pos[0] + win_size[0] {
                            continue;
                        }
                        let mc = marker.color.unwrap_or(cfg.color_marker);
                        draw.add_line([mx, win_pos[1]], [mx, win_pos[1] + win_size[1]], col32(mc))
                            .build();

                        draw.add_text([mx + 2.0, win_pos[1] + 2.0], col32(mc), &marker.label);

                        if in_content
                            && (mouse_pos[0] - mx).abs() < 4.0
                            && ui.is_mouse_clicked(MouseButton::Left)
                        {
                            events.push(TimelineEvent::MarkerClicked { index: mi });
                        }
                    }
                }

                // ── Vertical scroll ─────────────────────────────────────
                let total_h = self.total_content_height();
                let visible_h = win_size[1] - ruler_h;
                if total_h > visible_h {
                    let wheel_v = ui.io().mouse_wheel();
                    if ui.is_key_down(dear_imgui_rs::Key::LeftShift) && wheel_v.abs() > 0.01 {
                        self.vp.scroll_y =
                            (self.vp.scroll_y - wheel_v * 40.0).clamp(0.0, total_h - visible_h);
                    }
                }

                // Dummy for scroll extent
                ui.set_cursor_pos([0.0, total_h + ruler_h]);
                ui.dummy([1.0, 1.0]);
            });

        events
    }
}

// ── Adaptive tick calculation ───────────────────────────────────────────────

fn adaptive_ticks(visible_seconds: f64, width_px: f32) -> (f64, TimeUnit) {
    let target_ticks = (width_px / 100.0).max(2.0) as f64;
    let raw_interval = visible_seconds / target_ticks;

    let nice: &[f64] = &[
        1e-9, 2e-9, 5e-9, 1e-8, 2e-8, 5e-8, 1e-7, 2e-7, 5e-7, 1e-6, 2e-6, 5e-6, 1e-5, 2e-5, 5e-5,
        1e-4, 2e-4, 5e-4, 1e-3, 2e-3, 5e-3, 1e-2, 2e-2, 5e-2, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0,
        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0,
    ];

    let interval = nice
        .iter()
        .copied()
        .find(|&n| n >= raw_interval)
        .unwrap_or(raw_interval);

    let unit = if interval < 1e-6 {
        TimeUnit::Nanoseconds
    } else if interval < 1e-3 {
        TimeUnit::Microseconds
    } else if interval < 1.0 {
        TimeUnit::Milliseconds
    } else {
        TimeUnit::Seconds
    };

    (interval, unit)
}

/// Format a duration in seconds to a human-readable (value, suffix) pair.
fn format_duration(seconds: f64) -> (f64, &'static str) {
    if seconds < 1e-6 {
        (seconds * 1e9, "ns")
    } else if seconds < 1e-3 {
        (seconds * 1e6, "\u{00B5}s")
    } else if seconds < 1.0 {
        (seconds * 1e3, "ms")
    } else {
        (seconds, "s")
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_basic() {
        let s = Span::new(1, 0.0, 0.050, 0, "frame");
        assert_eq!(s.id, 1);
        assert!((s.duration() - 0.050).abs() < 1e-12);
        assert_eq!(s.label, "frame");
    }

    #[test]
    fn span_builders() {
        let s = Span::new(2, 0.0, 1.0, 0, "test")
            .with_category("cat")
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_source("main.rs:42");
        assert_eq!(s.category, "cat");
        assert!(s.color.is_some());
        assert_eq!(s.source.as_deref(), Some("main.rs:42"));
    }

    #[test]
    fn track_add_span_sorted() {
        let mut t = Track::new("test");
        t.add_span(Span::new(1, 0.5, 1.0, 0, "b"));
        t.add_span(Span::new(2, 0.0, 0.3, 0, "a"));
        t.add_span(Span::new(3, 0.2, 0.8, 1, "c"));
        assert_eq!(t.spans[0].label, "a");
        assert_eq!(t.spans[1].label, "c");
        assert_eq!(t.spans[2].label, "b");
    }

    #[test]
    fn track_max_depth() {
        let mut t = Track::new("test");
        t.add_span(Span::new(1, 0.0, 1.0, 0, "a"));
        t.add_span(Span::new(2, 0.0, 0.5, 1, "b"));
        t.add_span(Span::new(3, 0.0, 0.2, 3, "c"));
        assert_eq!(t.max_depth(), 3);
        assert_eq!(t.depth_rows(), 4);
    }

    #[test]
    fn track_time_range() {
        let mut t = Track::new("t");
        assert!(t.time_range().is_none());
        t.add_span(Span::new(1, 0.1, 0.5, 0, "a"));
        t.add_span(Span::new(2, 0.3, 0.9, 0, "b"));
        let (lo, hi) = t.time_range().unwrap();
        assert!((lo - 0.1).abs() < 1e-12);
        assert!((hi - 0.9).abs() < 1e-12);
    }

    #[test]
    fn marker_basic() {
        let m = Marker::new(0.016, "frame").with_color([1.0, 1.0, 0.0, 1.0]);
        assert!((m.time - 0.016).abs() < 1e-12);
        assert!(m.color.is_some());
    }

    #[test]
    fn timeline_data_range() {
        let mut tl = Timeline::new("##test");
        assert_eq!(tl.data_time_range(), (0.0, 1.0));

        let mut t = Track::new("main");
        t.add_span(Span::new(1, 0.01, 0.05, 0, "a"));
        tl.add_track(t);

        let (lo, hi) = tl.data_time_range();
        assert!((lo - 0.01).abs() < 1e-12);
        assert!((hi - 0.05).abs() < 1e-12);
    }

    #[test]
    fn timeline_fit_to_content() {
        let mut tl = Timeline::new("##test");
        let mut t = Track::new("main");
        t.add_span(Span::new(1, 0.0, 0.1, 0, "a"));
        tl.add_track(t);
        tl.fit_to_content(1000.0);
        assert!((tl.vp.pixels_per_second - 8800.0).abs() < 1.0);
    }

    #[test]
    fn adaptive_ticks_basic() {
        let (interval, _unit) = adaptive_ticks(1.0, 1000.0);
        assert!(interval > 0.0);
    }

    #[test]
    fn format_duration_ranges() {
        let (v, s) = format_duration(0.5e-9);
        assert!(s == "ns");
        assert!(v > 0.0);

        let (v, s) = format_duration(500e-6);
        assert!(s == "\u{00B5}s");
        assert!((v - 500.0).abs() < 0.1);

        let (v, s) = format_duration(0.042);
        assert!(s == "ms");
        assert!((v - 42.0).abs() < 0.1);

        let (v, s) = format_duration(2.5);
        assert!(s == "s");
        assert!((v - 2.5).abs() < 0.01);
    }

    #[test]
    fn str_hash_deterministic() {
        assert_eq!(str_hash("update"), str_hash("update"));
        assert_ne!(str_hash("update"), str_hash("render"));
    }

    #[test]
    fn config_defaults() {
        let cfg = TimelineConfig::default();
        assert_eq!(cfg.row_height, 20.0);
        assert!(cfg.show_ruler);
        assert!(cfg.show_tooltip);
        assert_eq!(cfg.span_palette.len(), 10);
    }

    #[test]
    fn color_by_name() {
        let mut tl = Timeline::new("##test");
        let mut t = Track::new("t");
        t.add_span(Span::new(1, 0.0, 1.0, 0, "a"));
        t.add_span(Span::new(2, 0.0, 1.0, 0, "b"));
        tl.add_track(t);
        tl.config.color_mode = ColorMode::ByName;

        let c1 = tl.span_color(&tl.tracks[0].spans[0]);
        let c2 = tl.span_color(&tl.tracks[0].spans[1]);
        assert!(c1[3] > 0.0);
        assert!(c2[3] > 0.0);
    }

    #[test]
    fn color_by_duration() {
        let mut tl = Timeline::new("##test");
        let mut t = Track::new("t");
        t.add_span(Span::new(1, 0.0, 0.001, 0, "short"));
        t.add_span(Span::new(2, 0.0, 1.0, 0, "long"));
        tl.add_track(t);
        tl.config.color_mode = ColorMode::ByDuration;

        let cs = tl.span_color(&tl.tracks[0].spans[0]);
        let cl = tl.span_color(&tl.tracks[0].spans[1]);
        assert!(cs[2] > cl[2]);
        assert!(cl[0] > cs[0]);
    }

    #[test]
    fn track_collapsed() {
        let mut t = Track::new("t");
        t.add_span(Span::new(1, 0.0, 1.0, 0, "a"));
        t.collapsed = true;
        assert_eq!(t.depth_rows(), 1);
    }

    #[test]
    fn timeline_clear() {
        let mut tl = Timeline::new("##test");
        tl.add_track(Track::new("a"));
        tl.add_marker(Marker::new(0.0, "m"));
        tl.clear_tracks();
        tl.clear_markers();
        assert!(tl.tracks.is_empty());
        assert!(tl.markers.is_empty());
    }
}
