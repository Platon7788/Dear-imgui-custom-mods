//! Configuration types for [`PropertyInspector`](super::PropertyInspector).

/// Configuration for the PropertyInspector widget.
#[derive(Debug, Clone, Copy)]
pub struct InspectorConfig {
    /// Key column width ratio (0.0..1.0).
    pub key_width_ratio: f32,
    /// Show search/filter bar at the top.
    pub show_filter: bool,
    /// Show category headers.
    pub show_categories: bool,
    /// Highlight changed values (diff mode).
    pub highlight_changes: bool,
    /// Row height in pixels.
    pub row_height: f32,
    /// Indent per nesting level.
    pub indent: f32,

    // ── Colors ──────────────────────────────────────────────
    /// Background color.
    pub color_bg: [f32; 4],
    /// Alternate row background.
    pub color_bg_alt: [f32; 4],
    /// Key/label text color.
    pub color_key: [f32; 4],
    /// Value text color.
    pub color_value: [f32; 4],
    /// Read-only value color (dimmed).
    pub color_readonly: [f32; 4],
    /// Category header background.
    pub color_category_bg: [f32; 4],
    /// Category header text.
    pub color_category_text: [f32; 4],
    /// Changed value highlight.
    pub color_changed: [f32; 4],
    /// Separator line.
    pub color_separator: [f32; 4],
}

impl Default for InspectorConfig {
    fn default() -> Self {
        Self {
            key_width_ratio: 0.40,
            show_filter: true,
            show_categories: true,
            highlight_changes: false,
            row_height: 22.0,
            indent: 16.0,

            color_bg: [0.11, 0.11, 0.13, 1.0],
            color_bg_alt: [0.13, 0.13, 0.16, 1.0],
            color_key: [0.70, 0.75, 0.82, 1.0],
            color_value: [0.88, 0.90, 0.93, 1.0],
            color_readonly: [0.50, 0.52, 0.58, 0.8],
            color_category_bg: [0.16, 0.17, 0.20, 1.0],
            color_category_text: [0.55, 0.60, 0.70, 1.0],
            color_changed: [1.00, 0.65, 0.20, 0.6],
            color_separator: [0.22, 0.24, 0.28, 0.5],
        }
    }
}

impl InspectorConfig {
    /// Replace every `color_*` field from the supplied crate-wide
    /// [`crate::theme::Theme`]. Mapping:
    ///
    /// - `bg`             ← `theme.window_bg()`
    /// - `bg_alt`         ← `nav.bg` (alternating row stripe)
    /// - `key`            ← `nav.icon_default` (label / muted)
    /// - `value`          ← `nav.icon_active` (primary text)
    /// - `readonly`       ← `nav.icon_default` at low alpha
    /// - `category_bg`    ← `nav.btn_hover` (collapsible header strip)
    /// - `category_text`  ← `statusbar.text_dim`
    /// - `changed`        ← `theme.warning()` at low alpha (diff highlight)
    /// - `separator`      ← `nav.separator`
    pub fn apply_theme(&mut self, theme: crate::theme::Theme) {
        let nav = theme.nav();
        let sb = theme.statusbar_colors();
        let with_a = |c: [f32; 4], a: f32| [c[0], c[1], c[2], a];

        self.color_bg = theme.window_bg();
        self.color_bg_alt = nav.bg;
        self.color_key = nav.icon_default;
        self.color_value = nav.icon_active;
        self.color_readonly = with_a(nav.icon_default, 0.80);
        self.color_category_bg = nav.btn_hover;
        self.color_category_text = sb.text_dim;
        self.color_changed = with_a(theme.warning(), 0.60);
        self.color_separator = with_a(nav.separator, 0.50);
    }

    /// Builder shortcut — equivalent to a `default()` followed by
    /// [`Self::apply_theme`].
    #[must_use]
    pub fn with_theme(mut self, theme: crate::theme::Theme) -> Self {
        self.apply_theme(theme);
        self
    }
}
