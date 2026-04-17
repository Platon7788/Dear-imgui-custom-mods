//! Nav-panel color palette.
//!
//! The theme selector is the unified [`crate::theme::Theme`] at top level now
//! — retrieve the matching palette with [`Theme::nav()`](crate::theme::Theme::nav).
//! For custom palettes, build a [`NavColors`] directly and pass it via
//! [`NavPanelConfig::with_colors`](super::config::NavPanelConfig::with_colors).

/// Complete color set for the navigation panel.
#[derive(Debug, Clone)]
pub struct NavColors {
    /// Panel background.
    pub bg: [f32; 4],
    /// Button hover background.
    pub btn_hover: [f32; 4],
    /// Active button background.
    pub btn_active: [f32; 4],
    /// Active indicator bar color (accent).
    pub indicator: [f32; 4],
    /// Default icon tint (monochrome fallback).
    pub icon_default: [f32; 4],
    /// Icon color when active.
    pub icon_active: [f32; 4],
    /// Separator line color.
    pub separator: [f32; 4],
    /// Badge circle background.
    pub badge_bg: [f32; 4],
    /// Badge text color.
    pub badge_text: [f32; 4],
    /// Submenu flyout background.
    pub submenu_bg: [f32; 4],
    /// Submenu item hover.
    pub submenu_hover: [f32; 4],
    /// Submenu item text.
    pub submenu_text: [f32; 4],
    /// Submenu border.
    pub submenu_border: [f32; 4],
    /// Submenu separator.
    pub submenu_separator: [f32; 4],
    /// Toggle button icon color.
    pub toggle_icon: [f32; 4],
}
