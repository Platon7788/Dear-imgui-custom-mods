//! Configuration types for [`Timeline`](super::Timeline).

/// Display mode for the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimelineMode {
    /// Top-down: parent spans at top, children below (icicle chart).
    #[default]
    TopDown,
    /// Bottom-up: aggregated stacks, hottest at top (flame graph).
    BottomUp,
}

impl TimelineMode {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::TopDown => "Top-Down",
            Self::BottomUp => "Flame",
        }
    }
}

/// How span colors are assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Color by span category / name hash.
    #[default]
    ByName,
    /// Color by duration heat (short=blue → long=red).
    ByDuration,
    /// Color by depth level.
    ByDepth,
    /// Use each span's explicit color field.
    Explicit,
}

/// Time unit for the ruler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
}

impl TimeUnit {
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Nanoseconds => "ns",
            Self::Microseconds => "\u{00B5}s",
            Self::Milliseconds => "ms",
            Self::Seconds => "s",
        }
    }

    pub fn factor(self) -> f64 {
        match self {
            Self::Nanoseconds => 1e9,
            Self::Microseconds => 1e6,
            Self::Milliseconds => 1e3,
            Self::Seconds => 1.0,
        }
    }
}

/// Configuration for the Timeline widget.
#[derive(Debug, Clone)]
pub struct TimelineConfig {
    // ── Layout ──────────────────────────────────────────────
    /// Height of each span bar in pixels.
    pub row_height: f32,
    /// Vertical gap between span bars.
    pub row_gap: f32,
    /// Height of the time ruler at top.
    pub ruler_height: f32,
    /// Width of the track label sidebar.
    pub track_label_width: f32,
    /// Minimum span bar width in pixels (avoid invisible spans).
    pub min_span_width: f32,
    /// Height of track header separator.
    pub track_header_height: f32,

    // ── Behavior ────────────────────────────────────────────
    /// Display mode (top-down or flame).
    pub mode: TimelineMode,
    /// How to assign span colors.
    pub color_mode: ColorMode,
    /// Show the ruler at the top.
    pub show_ruler: bool,
    /// Show track labels on the left.
    pub show_track_labels: bool,
    /// Show tooltip on hover.
    pub show_tooltip: bool,
    /// Smooth zoom interpolation.
    pub smooth_zoom: bool,
    /// Smooth zoom speed factor.
    pub smooth_zoom_speed: f32,
    /// Minimum zoom level (seconds per pixel).
    pub min_zoom: f64,
    /// Maximum zoom level.
    pub max_zoom: f64,
    /// Show vertical marker lines.
    pub show_markers: bool,

    // ── Colors ──────────────────────────────────────────────
    /// Background color.
    pub color_bg: [f32; 4],
    /// Alternate track background (striping).
    pub color_bg_alt: [f32; 4],
    /// Ruler background.
    pub color_ruler_bg: [f32; 4],
    /// Ruler text/tick color.
    pub color_ruler_text: [f32; 4],
    /// Track label text color.
    pub color_track_label: [f32; 4],
    /// Track header separator line.
    pub color_track_separator: [f32; 4],
    /// Span text color (on bar).
    pub color_span_text: [f32; 4],
    /// Selected span outline.
    pub color_selection: [f32; 4],
    /// Hovered span outline.
    pub color_hover: [f32; 4],
    /// Marker line color.
    pub color_marker: [f32; 4],
    /// Tooltip background.
    pub color_tooltip_bg: [f32; 4],
    /// Tooltip text.
    pub color_tooltip_text: [f32; 4],

    /// Palette for span bars (cycled by name hash / depth).
    pub span_palette: Vec<[f32; 4]>,
}

impl Default for TimelineConfig {
    fn default() -> Self {
        Self {
            row_height: 20.0,
            row_gap: 1.0,
            ruler_height: 24.0,
            track_label_width: 120.0,
            min_span_width: 2.0,
            track_header_height: 22.0,

            mode: TimelineMode::TopDown,
            color_mode: ColorMode::ByName,
            show_ruler: true,
            show_track_labels: true,
            show_tooltip: true,
            smooth_zoom: true,
            smooth_zoom_speed: 12.0,
            min_zoom: 1e-9,
            max_zoom: 1e6,
            show_markers: true,

            color_bg: [0.12, 0.12, 0.14, 1.0],
            color_bg_alt: [0.14, 0.14, 0.16, 1.0],
            color_ruler_bg: [0.16, 0.18, 0.20, 1.0],
            color_ruler_text: [0.60, 0.65, 0.70, 1.0],
            color_track_label: [0.70, 0.75, 0.80, 1.0],
            color_track_separator: [0.25, 0.28, 0.32, 0.8],
            color_span_text: [0.95, 0.95, 0.95, 1.0],
            color_selection: [1.00, 0.85, 0.20, 1.0],
            color_hover: [0.80, 0.85, 1.00, 0.8],
            color_marker: [0.90, 0.30, 0.30, 0.7],
            color_tooltip_bg: [0.10, 0.10, 0.12, 0.95],
            color_tooltip_text: [0.90, 0.90, 0.92, 1.0],

            span_palette: vec![
                [0.35, 0.55, 0.85, 0.9], // blue
                [0.55, 0.75, 0.40, 0.9], // green
                [0.85, 0.55, 0.35, 0.9], // orange
                [0.70, 0.45, 0.75, 0.9], // purple
                [0.40, 0.70, 0.70, 0.9], // teal
                [0.85, 0.65, 0.35, 0.9], // gold
                [0.60, 0.40, 0.35, 0.9], // brown
                [0.75, 0.35, 0.45, 0.9], // rose
                [0.45, 0.60, 0.45, 0.9], // sage
                [0.55, 0.50, 0.80, 0.9], // lavender
            ],
        }
    }
}
