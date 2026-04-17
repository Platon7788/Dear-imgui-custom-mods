//! Titlebar color palette — the colours part of the theme system.
//!
//! The selector enum lives at top level as [`crate::theme::Theme`] now —
//! every built-in theme exposes its titlebar palette through
//! [`Theme::titlebar()`](crate::theme::Theme::titlebar). Custom palettes
//! are built by constructing a [`TitlebarColors`] directly and handing it
//! to [`BorderlessConfig::with_colors`](super::config::BorderlessConfig::with_colors).

/// A complete set of colors for the borderless titlebar.
#[derive(Debug, Clone)]
pub struct TitlebarColors {
    /// Titlebar background.
    pub bg: [f32; 4],
    /// 1-px separator line below the titlebar.
    pub separator: [f32; 4],
    /// Title text color.
    pub title: [f32; 4],
    /// Minimize button icon color.
    pub btn_minimize: [f32; 4],
    /// Maximize / restore button icon color.
    pub btn_maximize: [f32; 4],
    /// Close button icon color.
    pub btn_close: [f32; 4],
    /// Hover background for minimize and maximize buttons.
    pub btn_hover_bg: [f32; 4],
    /// Hover background for the close button.
    pub btn_close_hover_bg: [f32; 4],
    /// Window icon color (if [`BorderlessConfig::icon`](super::config::BorderlessConfig::icon) is set).
    pub icon: [f32; 4],
    /// Titlebar background color used to "erase" overlapping icon layers (restore icon).
    pub bg_erase: [f32; 4],
    /// Subtle hover tint over the drag-move zone.
    pub drag_hint: [f32; 4],
    /// Titlebar background when the window loses OS focus.
    pub bg_inactive: [f32; 4],
    /// Title text color when the window loses OS focus.
    pub title_inactive: [f32; 4],
}
