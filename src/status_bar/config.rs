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
    /// Show the 1 px line drawn along the top edge of the bar.
    /// Set to `false` if the host wants a flush, border-less look —
    /// e.g. when the bar surface already merges with the page below.
    /// Default: `true`.
    pub show_top_border: bool,
    /// Pixel offset from the *left* edge where the top-border line
    /// starts. Use this to skip the area covered by a left-docked
    /// nav panel so the border doesn't draw a phantom seam through
    /// the panel's surface.
    ///
    /// Typical wiring: after `nav_panel.render(...)`, take
    /// `result.occupied_size[0]` and feed it here. Default: `0.0`.
    pub top_border_offset_left: f32,
    /// Pixel offset from the *right* edge where the top-border line
    /// stops. Mirror of [`Self::top_border_offset_left`] for a right-
    /// docked nav panel. Default: `0.0`.
    pub top_border_offset_right: f32,
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
            show_top_border: true,
            top_border_offset_left: 0.0,
            top_border_offset_right: 0.0,
            progress_width: 60.0,
            progress_height: 8.0,
            colors: StatusBarColors::default(),
        }
    }
}
