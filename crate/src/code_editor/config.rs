//! Configuration types for [`CodeEditor`].

pub use super::font_setup::{
    BuiltinFont, CODE_EDITOR_FONT_PTR, HACK_FONT_DATA, JETBRAINS_MONO_FONT_DATA,
    JETBRAINS_MONO_LIGATURES_FONT_DATA, MDI_FONT_DATA, code_editor_font_ptr,
    install_code_editor_font, install_code_editor_font_ex, install_custom_code_editor_font,
    merge_mdi_icons,
};
pub use super::lang::{Language, SyntaxDefinition};
pub use super::syntax_colors::{EditorTheme, SyntaxColors};

// ── ContextMenuConfig ─────────────────────────────────────────────────────────

/// Fine-grained control over which context-menu sections are visible.
///
/// Set `enabled = false` to suppress the menu entirely (useful for embedded
/// read-only viewers where right-click is handled by the host).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextMenuConfig {
    /// Show the context menu on right-click at all.
    pub enabled: bool,
    /// Cut / Copy / Paste section.
    pub show_clipboard: bool,
    /// Select All item.
    pub show_select_all: bool,
    /// Undo / Redo section.
    pub show_undo_redo: bool,
    /// Toggle Comment / Duplicate Line / Delete Line.
    pub show_code_actions: bool,
    /// Transform submenu (UPPERCASE, lowercase, Title Case, Trim).
    pub show_transform: bool,
    /// Find… item.
    pub show_find: bool,
    /// View submenu (line numbers, whitespace, color swatches…).
    pub show_view_toggles: bool,
    /// Language submenu.
    pub show_language_selector: bool,
    /// Theme submenu.
    pub show_theme_selector: bool,
    /// Font size ± buttons.
    pub show_font_size: bool,
    /// Cursor position info at the bottom of the menu.
    pub show_cursor_info: bool,
}

impl Default for ContextMenuConfig {
    /// Loads `context_menu.ron` — every section visible. `code_editor/
    /// config.ron` inlines the same field set; drift is guarded by
    /// `context_menu_inline_in_config_ron_matches_canonical`.
    fn default() -> Self {
        ron::from_str(include_str!("context_menu.ron"))
            .expect("built-in code_editor/context_menu.ron is valid")
    }
}

// ── EditorConfig ─────────────────────────────────────────────────────────────

/// Editor behavior and appearance configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EditorConfig {
    /// Tab size in spaces.
    pub tab_size: u8,
    /// Insert spaces instead of tabs.
    pub insert_spaces: bool,
    /// Auto-indent on Enter.
    pub auto_indent: bool,
    /// Auto-close brackets `()`, `{}`, `[]`.
    pub auto_close_brackets: bool,
    /// Auto-close quotes `""`, `''`.
    pub auto_close_quotes: bool,
    /// Show line numbers.
    pub show_line_numbers: bool,
    /// Highlight the current line.
    pub highlight_current_line: bool,
    /// Show bracket matching.
    pub bracket_matching: bool,
    /// Show whitespace characters.
    pub show_whitespace: bool,
    /// Draw a small colored swatch next to CSS hex color literals (`#RGB`, `0xRRGGBB`, etc.).
    pub show_color_swatches: bool,
    /// Automatically insert a space after every two hex digits when in hex-editing mode.
    ///
    /// Intended for use with [`Language::Hex`] or a custom hex DSL: typing `FF`
    /// automatically becomes `FF ` so the next byte starts cleanly.
    pub hex_auto_space: bool,
    /// Automatically uppercase typed hex digits (`a` → `A`).
    ///
    /// Works with any language but is most useful paired with [`Language::Hex`].
    pub hex_auto_uppercase: bool,
    /// Automatically switch keyboard layout to English (US) when the editor
    /// gains focus, and restore the previous layout when it loses focus.
    ///
    /// Useful for code editing and hex mode where non-Latin input is rarely needed.
    pub force_english_on_focus: bool,
    /// Read-only mode.
    pub read_only: bool,
    /// Show fold/unfold indicators in the gutter.
    pub show_fold_indicators: bool,
    /// Word wrap.
    pub word_wrap: bool,
    /// Smooth scrolling (animated).
    pub smooth_scrolling: bool,
    /// Syntax language. Skipped during serialization because `Language::Custom`
    /// contains a non-serializable `Arc<dyn SyntaxDefinition>`.
    #[serde(skip, default)]
    pub language: Language,
    /// Active color theme (preset name — see [`EditorTheme`]).
    ///
    /// To track the crate-wide [`crate::theme::Theme`] instead, call
    /// [`Self::with_crate_theme`] / [`Self::set_crate_theme`] which
    /// dispatches to the matching [`EditorTheme`] preset.
    pub theme: EditorTheme,
    /// Syntax colors — updated automatically by [`set_theme`](Self::set_theme).
    pub colors: SyntaxColors,
    /// Cursor blink rate in seconds (0 = no blink).
    pub cursor_blink_rate: f32,
    /// Scroll speed multiplier.
    pub scroll_speed: f32,
    /// Font size scale factor (1.0 = base font size; Ctrl+Scroll modifies this).
    pub font_size_scale: f32,
    /// Context-menu visibility configuration.
    pub context_menu: ContextMenuConfig,
    /// Maximum number of lines allowed (0 = unlimited).
    /// When set, Enter/paste will not add lines beyond this limit.
    pub max_lines: usize,
    /// Maximum character length per line (0 = unlimited).
    /// When set, typing/paste will not extend a line beyond this limit.
    pub max_line_length: usize,

    /// User-visible language for the right-click context menu, the
    /// find/replace bar, and the cursor-info footer. Default
    /// [`crate::i18n::Locale::En`]. Switching to
    /// [`crate::i18n::Locale::Ru`] requires the host to bake
    /// `GlyphRanges::Cyrillic` into the active font atlas — without
    /// that, non-ASCII characters render as `?`.
    /// `#[serde(default)]` so older `config.ron` files still parse.
    #[serde(default)]
    pub locale: crate::i18n::Locale,
}

impl Default for EditorConfig {
    fn default() -> Self {
        let mut cfg: EditorConfig = ron::from_str(include_str!("config.ron"))
            .expect("built-in code_editor/config.ron is valid");
        cfg.language = Language::Rust;
        cfg
    }
}

impl EditorConfig {
    /// Apply a built-in theme preset — updates `theme` and `colors` atomically.
    pub fn set_theme(&mut self, theme: EditorTheme) {
        self.colors = theme.colors();
        self.theme = theme;
    }

    /// Apply the [`EditorTheme`] preset that matches the supplied crate-wide
    /// [`crate::theme::Theme`] via [`EditorTheme::from_crate_theme`]. Use this
    /// when the editor lives inside an app whose chrome already tracks
    /// `Theme::Dark` / `Theme::Light`, so the syntax palette flips
    /// in sync without the host having to know about [`EditorTheme`].
    pub fn set_crate_theme(&mut self, theme: crate::theme::Theme) {
        self.set_theme(EditorTheme::from_crate_theme(theme));
    }

    /// Builder shortcut — equivalent to a `default()` followed by
    /// [`Self::set_crate_theme`]. Useful at editor construction time:
    ///
    /// ```rust,ignore
    /// let cfg = EditorConfig::default().with_crate_theme(crate::theme::Theme::Dark);
    /// ```
    #[must_use]
    pub fn with_crate_theme(mut self, theme: crate::theme::Theme) -> Self {
        self.set_crate_theme(theme);
        self
    }
}
