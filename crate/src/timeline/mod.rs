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
//!
//! ## File layout
//!
//! The widget is split across sibling files to keep every file well
//! under the 500-line cap:
//!
//! - `mod.rs`    — the [`Timeline`] struct, data API, viewport, free
//!   helpers ([`adaptive_ticks`], [`format_duration`]).
//! - `coords.rs` — coordinate/axis mapping, colour resolution, layout
//!   metrics ([`Timeline::time_to_x`] … [`Timeline::fit_to_content`]).
//! - `render.rs` — the per-frame [`Timeline::render`] entry point.
//! - `config.rs` / `config.ron` — schema + default values.
//! - `span.rs` / `track.rs` — data types.

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
mod coords;
mod render;
pub mod span;
pub mod track;

pub use config::{ColorMode, TimeUnit, TimelineConfig, TimelineMode};
pub use span::{Marker, Span};
pub use track::Track;

use crate::utils::color::rgba_f32;

/// Convert `[r, g, b, a]` to packed u32 color.
#[inline]
pub(super) fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// Simple string hash for palette indexing.
pub(super) fn str_hash(s: &str) -> usize {
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
pub(super) struct Viewport {
    /// Left edge time (seconds).
    pub(super) time_start: f64,
    /// Pixels per second.
    pub(super) pixels_per_second: f64,
    /// Zoom target for smooth interpolation.
    pub(super) zoom_target: f64,
    /// Vertical scroll offset in pixels.
    pub(super) scroll_y: f32,
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
    pub(super) id: String,
    /// Tracks (rows of spans).
    pub(super) tracks: Vec<Track>,
    /// Vertical marker lines.
    pub(super) markers: Vec<Marker>,
    /// Configuration.
    pub config: TimelineConfig,
    /// View state.
    pub(super) vp: Viewport,
    /// Currently selected span id.
    pub(super) selected_span: Option<u64>,
    /// Currently hovered span id (transient per-frame).
    pub(super) hovered_span: Option<u64>,
    /// Whether the user is panning.
    pub(super) panning: bool,
    /// Last mouse X during pan (pixels).
    pub(super) pan_start_x: f32,
    /// Time at pan start.
    pub(super) pan_start_time: f64,
}

impl Timeline {
    #[must_use]
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

    /// Override the user-visible language. Default English; pass
    /// [`crate::i18n::Locale::Ru`] for Russian. The host must bake
    /// `GlyphRanges::Cyrillic` into the active font atlas — without
    /// that, Cyrillic characters render as `?`.
    #[must_use]
    pub fn with_locale(mut self, locale: crate::i18n::Locale) -> Self {
        self.config.locale = locale;
        self
    }

    /// Mid-flight language switch.
    pub fn set_locale(&mut self, locale: crate::i18n::Locale) {
        self.config.locale = locale;
    }

    /// Currently-active locale.
    #[must_use]
    pub fn locale(&self) -> crate::i18n::Locale {
        self.config.locale
    }

    // ── Data API ────────────────────────────────────────────────────────────

    pub fn add_track(&mut self, track: Track) {
        self.tracks.push(track);
    }

    pub fn track_mut(&mut self, index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(index)
    }

    #[must_use]
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

    #[must_use]
    pub fn selected_span(&self) -> Option<u64> {
        self.selected_span
    }
}

// ── Adaptive tick calculation ───────────────────────────────────────────────

/// "Nice" tick intervals (seconds) used by [`adaptive_ticks`], ascending.
const NICE_INTERVALS: &[f64] = &[
    1e-9, 2e-9, 5e-9, 1e-8, 2e-8, 5e-8, 1e-7, 2e-7, 5e-7, 1e-6, 2e-6, 5e-6, 1e-5, 2e-5, 5e-5, 1e-4,
    2e-4, 5e-4, 1e-3, 2e-3, 5e-3, 1e-2, 2e-2, 5e-2, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0,
    100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0,
];

/// Pick a "nice" tick interval for the ruler given the visible time
/// span and the pixel width available.
///
/// Returns `(interval_seconds, unit)`. The interval is always
/// **finite and `> 0`** for any input: non-finite or non-positive
/// `visible_seconds` / `width_px` fall back to the smallest nice
/// interval (`1e-9`) so the caller's tick loop never spins on a zero
/// or `NaN` step.
pub(super) fn adaptive_ticks(visible_seconds: f64, width_px: f32) -> (f64, TimeUnit) {
    // Guard: a non-finite or non-positive visible span (zoomed to a
    // degenerate viewport) would yield a 0 / NaN raw interval and an
    // unusable tick step. Fall back to the smallest nice interval.
    if !visible_seconds.is_finite() || visible_seconds <= 0.0 || !width_px.is_finite() {
        return (NICE_INTERVALS[0], TimeUnit::Nanoseconds);
    }

    let target_ticks = f64::from((width_px / 100.0).max(2.0));
    let raw_interval = visible_seconds / target_ticks;

    let interval = NICE_INTERVALS
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
#[must_use]
pub(super) fn format_duration(seconds: f64) -> (f64, &'static str) {
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

#[cfg(test)]
mod tests;
