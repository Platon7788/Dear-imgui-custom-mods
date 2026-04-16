//! Configuration types for [`StatusBar`](super::StatusBar).

use crate::borderless_window::TitlebarTheme;

/// Alignment of a status bar section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Configuration for the StatusBar widget.
#[derive(Debug, Clone, Copy)]
pub struct StatusBarConfig {
    /// Total height of the bar in pixels.
    pub height: f32,
    /// Horizontal padding inside each item.
    pub item_padding: f32,
    /// Separator width between items.
    pub separator_width: f32,
    /// Show separator lines between items.
    pub show_separators: bool,

    // ── Colors ──────────────────────────────────────────────
    /// Bar background color.
    pub color_bg: [f32; 4],
    /// Default text color.
    pub color_text: [f32; 4],
    /// Dimmed/secondary text color.
    pub color_text_dim: [f32; 4],
    /// Separator line color.
    pub color_separator: [f32; 4],
    /// Hovered item background.
    pub color_hover: [f32; 4],
    /// Clicked item background.
    pub color_active: [f32; 4],

    /// Success indicator color (green dot).
    pub color_success: [f32; 4],
    /// Warning indicator color (yellow dot).
    pub color_warning: [f32; 4],
    /// Error indicator color (red dot).
    pub color_error: [f32; 4],
    /// Info indicator color (blue dot).
    pub color_info: [f32; 4],
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self {
            height: 22.0,
            item_padding: 8.0,
            separator_width: 1.0,
            show_separators: true,

            color_bg:        [0.12, 0.12, 0.15, 1.0],
            color_text:      [0.85, 0.87, 0.90, 1.0],
            color_text_dim:  [0.50, 0.52, 0.58, 1.0],
            color_separator: [0.25, 0.27, 0.32, 0.6],
            color_hover:     [0.20, 0.22, 0.28, 1.0],
            color_active:    [0.25, 0.28, 0.35, 1.0],

            color_success:   [0.30, 0.80, 0.40, 1.0],
            color_warning:   [0.90, 0.75, 0.20, 1.0],
            color_error:     [0.90, 0.30, 0.30, 1.0],
            color_info:      [0.40, 0.65, 0.90, 1.0],
        }
    }
}

// ─── Conversion from a borderless-window titlebar theme ──────────────────────

/// Derive a matching [`StatusBarConfig`] from a [`TitlebarTheme`].
///
/// Keeps the bar visually in sync with the titlebar: background is slightly
/// darkened, text / separator / hover colors come straight from the titlebar
/// palette. Semantic indicator colors (success / warning / error / info) stay
/// constant across themes so the bar meaning does not shift when switching
/// palettes. Non-color fields use the `StatusBarConfig::default()` values.
impl From<&TitlebarTheme> for StatusBarConfig {
    fn from(theme: &TitlebarTheme) -> Self {
        let tb = theme.colors();
        let bg = [
            (tb.bg[0] * 0.90).clamp(0.0, 1.0),
            (tb.bg[1] * 0.90).clamp(0.0, 1.0),
            (tb.bg[2] * 0.90).clamp(0.0, 1.0),
            1.0,
        ];
        let defaults = Self::default();
        Self {
            color_bg: bg,
            color_text: tb.title,
            color_text_dim: tb.title_inactive,
            color_separator: tb.separator,
            color_hover: tb.btn_hover_bg,
            color_active: tb.btn_hover_bg,
            // Semantic colors kept constant for cross-theme consistency.
            color_success: defaults.color_success,
            color_warning: defaults.color_warning,
            color_error: defaults.color_error,
            // `info` tracks the titlebar accent (maximize-button color).
            color_info: tb.btn_maximize,
            ..defaults
        }
    }
}
