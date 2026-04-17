//! Configuration types for [`StatusBar`](super::StatusBar).
//!
//! For per-theme palettes use [`crate::theme::Theme::statusbar()`] — it
//! returns a fully configured `StatusBarConfig`.

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
    /// Paint a background behind items under the mouse cursor.
    ///
    /// When `false` (default), neither plain text items nor clickable items
    /// receive any hover paint — the bar stays fully static visually.
    /// Clickable items continue to emit [`StatusBarEvent`](super::StatusBarEvent)s
    /// and show their tooltip regardless of this flag; only the optional
    /// hover/active rectangle is gated by it.
    ///
    /// Set to `true` when you want the pre-0.8.1 behavior with Windows-style
    /// button feedback on hover and press.
    pub highlight_hover: bool,

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
            highlight_hover: false,

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

