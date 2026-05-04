//! Configuration types for [`DiffViewer`](super::DiffViewer).

/// Display mode for the diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum DiffMode {
    /// Two panels side by side.
    #[default]
    SideBySide,
    /// Single panel with +/- prefixes.
    Unified,
}

/// Configuration for the DiffViewer widget.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiffViewerConfig {
    /// Display mode.
    pub mode: DiffMode,
    /// Show line numbers.
    pub show_line_numbers: bool,
    /// Fold unchanged regions (show "... N unchanged lines").
    pub fold_unchanged: bool,
    /// Number of context lines around each change when folding.
    pub context_lines: usize,
    /// Show mini-map on the scrollbar.
    pub show_minimap: bool,
    /// Synchronized scroll between panels.
    pub sync_scroll: bool,

    // ── Colors ──────────────────────────────────────────────
    /// Background color.
    pub color_bg: [f32; 4],
    /// Gutter (line number) background.
    pub color_gutter_bg: [f32; 4],
    /// Line number text.
    pub color_line_number: [f32; 4],
    /// Normal text color.
    pub color_text: [f32; 4],
    /// Added line background.
    pub color_added_bg: [f32; 4],
    /// Added line text.
    pub color_added_text: [f32; 4],
    /// Removed line background.
    pub color_removed_bg: [f32; 4],
    /// Removed line text.
    pub color_removed_text: [f32; 4],
    /// Modified line background.
    pub color_modified_bg: [f32; 4],
    /// Inline change highlight (character-level).
    pub color_inline_change: [f32; 4],
    /// Fold separator line/text.
    pub color_fold: [f32; 4],
    /// Header/filename.
    pub color_header: [f32; 4],
    /// Separator between panels.
    pub color_separator: [f32; 4],
    /// Current (selected) hunk highlight.
    pub color_current_hunk: [f32; 4],

    /// User-visible language for the toolbar Prev/Next buttons.
    /// Default [`crate::i18n::Locale::En`]. Switching to
    /// [`crate::i18n::Locale::Ru`] requires the host to bake
    /// `GlyphRanges::Cyrillic` into the active font atlas.
    /// `#[serde(default)]` so older `config.ron` files still parse.
    #[serde(default)]
    pub locale: crate::i18n::Locale,
}

impl Default for DiffViewerConfig {
    fn default() -> Self {
        ron::from_str(include_str!("config.ron"))
            .expect("built-in diff_viewer/config.ron is valid")
    }
}

impl DiffViewerConfig {
    /// Overwrite every `color_*` field from the supplied crate-wide
    /// [`crate::theme::Theme`]. The semantic mapping is:
    ///
    /// - `bg`            ← `theme.window_bg()` (matches host backdrop)
    /// - `gutter_bg`     ← `nav.bg` (line-number strip = nav surface)
    /// - `line_number`   ← `nav.icon_default`, `text` ← `nav.icon_active`
    /// - `added_*`       ← `success` family
    /// - `removed_*`     ← `danger` family
    /// - `modified_*`    ← `warning` family
    /// - `inline_change` ← `warning` at low alpha
    /// - `fold`          ← muted `nav.separator`
    /// - `header`        ← `statusbar.text_dim`
    /// - `separator`     ← `nav.separator`
    /// - `current_hunk`  ← `accent` at low alpha
    ///
    /// All non-colour fields are left untouched so callers can mix
    /// `with_theme(...)` with custom layout overrides.
    pub fn apply_theme(&mut self, theme: crate::theme::Theme) {
        let nav = theme.nav();
        let sb = theme.statusbar_colors();
        let with_a = |c: [f32; 4], a: f32| [c[0], c[1], c[2], a];

        self.color_bg = theme.window_bg();
        self.color_gutter_bg = nav.bg;
        self.color_line_number = nav.icon_default;
        self.color_text = nav.icon_active;
        self.color_added_bg = with_a(theme.success(), 0.30);
        self.color_added_text = theme.success();
        self.color_removed_bg = with_a(theme.danger(), 0.30);
        self.color_removed_text = theme.danger();
        self.color_modified_bg = with_a(theme.warning(), 0.28);
        self.color_inline_change = with_a(theme.warning(), 0.35);
        self.color_fold = with_a(nav.separator, 0.70);
        self.color_header = sb.text_dim;
        self.color_separator = with_a(nav.separator, 0.80);
        self.color_current_hunk = with_a(theme.accent(), 0.30);
    }

    /// Builder shortcut — equivalent to a `default()` followed by
    /// [`Self::apply_theme`].
    #[must_use]
    pub fn with_theme(mut self, theme: crate::theme::Theme) -> Self {
        self.apply_theme(theme);
        self
    }
}
