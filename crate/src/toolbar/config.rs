//! Configuration types for [`Toolbar`](super::Toolbar).

/// Configuration for the Toolbar widget.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
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
        ron::from_str(include_str!("config.ron")).expect("built-in toolbar/config.ron is valid")
    }
}

impl ToolbarConfig {
    /// Replace every `color_*` field from the supplied crate-wide
    /// [`crate::theme::Theme`]. Mapping mirrors `nav_panel`'s palette so
    /// a horizontal toolbar reads as the same chrome surface as a
    /// vertical nav panel:
    ///
    /// - `bg`              ← `nav.bg`
    /// - `text`            ← `nav.icon_active`
    /// - `disabled`        ← `nav.icon_default` at half alpha
    /// - `hover`           ← `nav.btn_hover`
    /// - `active`          ← `nav.btn_active`
    /// - `toggled`         ← `theme.accent()` at low alpha
    /// - `separator`       ← `nav.separator`
    /// - `border`          ← `nav.separator` at lower alpha
    /// - `hover_underline` ← `theme.accent()` at higher alpha
    ///
    /// Layout fields (`height`, `padding`, `*_thickness`) untouched.
    pub fn apply_theme(&mut self, theme: crate::theme::Theme) {
        let nav = theme.nav();
        let with_a = |c: [f32; 4], a: f32| [c[0], c[1], c[2], a];

        self.color_bg = nav.bg;
        self.color_text = nav.icon_active;
        self.color_disabled = with_a(nav.icon_default, 0.50);
        self.color_hover = nav.btn_hover;
        self.color_active = nav.btn_active;
        self.color_toggled = with_a(theme.accent(), 0.70);
        self.color_separator = with_a(nav.separator, 0.60);
        self.color_border = with_a(nav.separator, 0.50);
        self.color_hover_underline = with_a(theme.accent(), 0.80);
    }

    /// Builder shortcut — equivalent to a `default()` followed by
    /// [`Self::apply_theme`].
    #[must_use]
    pub fn with_theme(mut self, theme: crate::theme::Theme) -> Self {
        self.apply_theme(theme);
        self
    }
}
