//! Configuration types for [`Timeline`](super::Timeline).

/// Display mode for the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        ron::from_str(include_str!("config.ron"))
            .expect("built-in timeline/config.ron is valid")
    }
}

impl TimelineConfig {
    /// Replace every `color_*` field from the supplied crate-wide
    /// [`crate::theme::Theme`]. Mapping:
    ///
    /// - `bg`              ← `theme.window_bg()`
    /// - `bg_alt`          ← `nav.bg` (track stripe surface)
    /// - `ruler_bg`        ← `nav.btn_hover` (chrome strip above tracks)
    /// - `ruler_text`      ← `statusbar.text_dim`
    /// - `track_label`     ← `nav.icon_active`
    /// - `track_separator` ← `nav.separator`
    /// - `span_text`       ← `nav.icon_active`
    /// - `selection`       ← `theme.warning()` (amber outline)
    /// - `hover`           ← `theme.accent()` lighter tint
    /// - `marker`          ← `theme.danger()` muted
    /// - `tooltip_bg`      ← darker `nav.bg`
    /// - `tooltip_text`    ← `nav.icon_active`
    ///
    /// `span_palette` is left untouched — it's the per-span hue rotation
    /// used by [`crate::timeline::ColorMode::ByName`] / `ByDepth`, which
    /// stays consistent across themes. Override directly if needed.
    pub fn apply_theme(&mut self, theme: crate::theme::Theme) {
        let nav = theme.nav();
        let sb = theme.statusbar_colors();
        let with_a = |c: [f32; 4], a: f32| [c[0], c[1], c[2], a];
        let lift = |c: [f32; 4], d: f32| {
            [
                (c[0] + d).clamp(0.0, 1.0),
                (c[1] + d).clamp(0.0, 1.0),
                (c[2] + d).clamp(0.0, 1.0),
                c[3],
            ]
        };

        self.color_bg = theme.window_bg();
        self.color_bg_alt = nav.bg;
        self.color_ruler_bg = nav.btn_hover;
        self.color_ruler_text = sb.text_dim;
        self.color_track_label = nav.icon_active;
        self.color_track_separator = with_a(nav.separator, 0.80);
        self.color_span_text = nav.icon_active;
        self.color_selection = theme.warning();
        self.color_hover = with_a(lift(theme.accent(), 0.10), 0.80);
        self.color_marker = with_a(theme.danger(), 0.70);
        self.color_tooltip_bg = with_a(lift(nav.bg, -0.04), 0.95);
        self.color_tooltip_text = nav.icon_active;
    }

    /// Builder shortcut — equivalent to a `default()` followed by
    /// [`Self::apply_theme`].
    #[must_use]
    pub fn with_theme(mut self, theme: crate::theme::Theme) -> Self {
        self.apply_theme(theme);
        self
    }
}
