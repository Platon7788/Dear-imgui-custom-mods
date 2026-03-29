//! Configuration types for [`CodeEditor`].

/// Syntax language for highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// No syntax highlighting (plain text).
    None,
    /// Rust language highlighting.
    #[default]
    Rust,
    /// TOML configuration files.
    Toml,
    /// RON (Rusty Object Notation).
    Ron,
}

/// Token color palette for syntax highlighting.
#[derive(Debug, Clone)]
pub struct SyntaxColors {
    pub keyword: [f32; 4],
    pub type_name: [f32; 4],
    pub lifetime: [f32; 4],
    pub string: [f32; 4],
    pub char_lit: [f32; 4],
    pub number: [f32; 4],
    pub comment: [f32; 4],
    pub attribute: [f32; 4],
    pub macro_call: [f32; 4],
    pub operator: [f32; 4],
    pub punctuation: [f32; 4],
    pub identifier: [f32; 4],
    pub user_code_marker: [f32; 4],
    pub current_line_bg: [f32; 4],
    pub selection_bg: [f32; 4],
    pub search_match_bg: [f32; 4],
    pub search_current_bg: [f32; 4],
    pub line_number: [f32; 4],
    pub line_number_active: [f32; 4],
    pub bracket_match_bg: [f32; 4],
    pub error_underline: [f32; 4],
    pub warning_underline: [f32; 4],
    pub gutter_bg: [f32; 4],
}

impl Default for SyntaxColors {
    fn default() -> Self {
        use crate::theme;
        Self {
            keyword:          theme::ACCENT,               // blue — fn, let, struct…
            type_name:        [0.56, 0.84, 0.62, 1.0],    // teal-green — Vec, Option…
            lifetime:         [0.85, 0.60, 0.85, 1.0],    // lavender — 'a, 'static
            string:           [0.80, 0.88, 0.52, 1.0],    // warm yellow-green
            char_lit:         [0.80, 0.88, 0.52, 1.0],
            number:           [0.78, 0.58, 0.95, 1.0],    // soft purple
            comment:          [0.47, 0.53, 0.60, 1.0],    // slate grey
            attribute:        [0.82, 0.72, 0.36, 1.0],    // golden — #[derive…]
            macro_call:       [0.90, 0.75, 0.35, 1.0],    // amber — println!, vec!
            operator:         [0.72, 0.88, 0.98, 1.0],    // light cyan — = + - * & |
            punctuation:      [0.60, 0.62, 0.68, 1.0],    // muted blue-grey
            identifier:       theme::TEXT_PRIMARY,
            user_code_marker: theme::WARNING,
            current_line_bg:  [0.18, 0.20, 0.26, 1.0],
            selection_bg:     [0.30, 0.52, 0.82, 0.38],
            search_match_bg:  [0.62, 0.52, 0.10, 0.30],
            search_current_bg:[0.62, 0.52, 0.10, 0.62],
            line_number:      theme::TEXT_MUTED,
            line_number_active: theme::TEXT_PRIMARY,
            bracket_match_bg: [0.38, 0.44, 0.58, 0.45],
            error_underline:  theme::DANGER,
            warning_underline: theme::WARNING,
            gutter_bg:        [0.09, 0.10, 0.13, 1.0],
        }
    }
}

/// Editor behavior configuration.
#[derive(Debug, Clone)]
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
    /// Show minimap.
    pub show_minimap: bool,
    /// Read-only mode.
    pub read_only: bool,
    /// Word wrap.
    pub word_wrap: bool,
    /// Smooth scrolling (animated).
    pub smooth_scrolling: bool,
    /// Syntax language.
    pub language: Language,
    /// Syntax colors.
    pub colors: SyntaxColors,
    /// Cursor blink rate in seconds (0 = no blink).
    pub cursor_blink_rate: f32,
    /// Scroll speed multiplier.
    pub scroll_speed: f32,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
            auto_indent: true,
            auto_close_brackets: true,
            auto_close_quotes: true,
            show_line_numbers: true,
            highlight_current_line: true,
            bracket_matching: true,
            show_whitespace: false,
            show_minimap: false,
            read_only: false,
            word_wrap: false,
            smooth_scrolling: true,
            language: Language::Rust,
            colors: SyntaxColors::default(),
            cursor_blink_rate: 0.53,
            scroll_speed: 3.0,
        }
    }
}
