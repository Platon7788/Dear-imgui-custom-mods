//! Configuration types for [`Toolbar`](super::Toolbar).

/// Configuration for the Toolbar widget.
#[derive(Debug, Clone, Copy)]
pub struct ToolbarConfig {
    /// Toolbar height in pixels.
    pub height: f32,
    /// Horizontal padding between items.
    pub item_spacing: f32,
    /// Button padding inside each button.
    pub button_padding: f32,
    /// Separator width.
    pub separator_width: f32,
    /// Separator margin on each side.
    pub separator_margin: f32,
    /// Rounding of toolbar buttons.
    pub button_rounding: f32,

    // ── Colors ──────────────────────────────────────────────
    /// Toolbar background.
    pub color_bg: [f32; 4],
    /// Button text/icon color.
    pub color_text: [f32; 4],
    /// Disabled item color.
    pub color_disabled: [f32; 4],
    /// Hovered button background.
    pub color_hover: [f32; 4],
    /// Active/pressed button background.
    pub color_active: [f32; 4],
    /// Toggled-on button background.
    pub color_toggled: [f32; 4],
    /// Separator line color.
    pub color_separator: [f32; 4],
    /// Bottom border color.
    pub color_border: [f32; 4],
    /// Underline color drawn beneath hovered items.
    pub color_hover_underline: [f32; 4],
    /// Thickness of the hover underline in pixels.
    pub hover_underline_thickness: f32,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            height: 30.0,
            item_spacing: 2.0,
            button_padding: 6.0,
            separator_width: 1.0,
            separator_margin: 4.0,
            button_rounding: 3.0,

            color_bg: [0.13, 0.14, 0.17, 1.0],
            color_text: [0.85, 0.87, 0.90, 1.0],
            color_disabled: [0.40, 0.42, 0.48, 0.5],
            color_hover: [0.22, 0.24, 0.30, 1.0],
            color_active: [0.28, 0.32, 0.40, 1.0],
            color_toggled: [0.25, 0.38, 0.55, 0.7],
            color_separator: [0.25, 0.27, 0.32, 0.6],
            color_border: [0.20, 0.22, 0.27, 0.5],
            color_hover_underline: [0.40, 0.63, 0.88, 0.8],
            hover_underline_thickness: 2.0,
        }
    }
}
