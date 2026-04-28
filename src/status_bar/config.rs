//! Configuration types for [`StatusBar`](super::StatusBar).
//!
//! The colour subset lives in [`crate::theme::StatusBarColors`] — this
//! struct only owns layout fields. For per-theme palettes use
//! [`crate::theme::Theme::statusbar`] (full config) or
//! [`crate::theme::Theme::statusbar_colors`] (palette only, no feature
//! gate).

use crate::theme::StatusBarColors;

/// Alignment of a status bar section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Configuration for the StatusBar widget.
///
/// Layout fields live here; colour tokens are bundled in
/// [`StatusBarColors`] (defined in the [`crate::theme`] module so that
/// custom themes can construct one without depending on this widget).
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
    /// Width of the inline progress-bar widget rendered by
    /// [`StatusItem::progress`](super::StatusItem::progress) (px).
    /// Default: `60.0`.
    pub progress_width: f32,
    /// Height of the inline progress bar (px). Default: `8.0`.
    pub progress_height: f32,
    /// Colour palette. Pluggable via [`Theme::statusbar_colors`](crate::theme::Theme::statusbar_colors)
    /// or built directly. Default: NxT-dark.
    pub colors: StatusBarColors,
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self {
            height: 22.0,
            item_padding: 8.0,
            separator_width: 1.0,
            show_separators: true,
            highlight_hover: false,
            progress_width: 60.0,
            progress_height: 8.0,
            colors: StatusBarColors::default(),
        }
    }
}
