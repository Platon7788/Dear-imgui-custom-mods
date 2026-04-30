//! Configuration types for [`DiffViewer`](super::DiffViewer).

/// Display mode for the diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffMode {
    /// Two panels side by side.
    #[default]
    SideBySide,
    /// Single panel with +/- prefixes.
    Unified,
}

/// Configuration for the DiffViewer widget.
#[derive(Debug, Clone)]
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
}

impl Default for DiffViewerConfig {
    fn default() -> Self {
        Self {
            mode: DiffMode::SideBySide,
            show_line_numbers: true,
            fold_unchanged: true,
            context_lines: 3,
            show_minimap: false,
            sync_scroll: true,

            color_bg: [0.11, 0.11, 0.13, 1.0],
            color_gutter_bg: [0.13, 0.14, 0.16, 1.0],
            color_line_number: [0.40, 0.42, 0.48, 1.0],
            color_text: [0.85, 0.87, 0.90, 1.0],
            color_added_bg: [0.15, 0.30, 0.18, 0.5],
            color_added_text: [0.55, 0.90, 0.55, 1.0],
            color_removed_bg: [0.35, 0.15, 0.15, 0.5],
            color_removed_text: [0.90, 0.55, 0.55, 1.0],
            color_modified_bg: [0.30, 0.28, 0.15, 0.4],
            color_inline_change: [0.90, 0.75, 0.20, 0.35],
            color_fold: [0.35, 0.38, 0.45, 0.7],
            color_header: [0.50, 0.55, 0.65, 1.0],
            color_separator: [0.25, 0.27, 0.32, 0.8],
            color_current_hunk: [0.30, 0.45, 0.65, 0.3],
        }
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
