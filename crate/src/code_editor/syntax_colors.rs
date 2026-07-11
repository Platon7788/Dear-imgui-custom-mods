//! Syntax color palettes for the code editor.

// ── EditorTheme ───────────────────────────────────────────────────────────────

/// Built-in color theme preset for the editor.
///
/// Pass to [`EditorConfig::set_theme`] to switch the entire color palette at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum EditorTheme {
    /// Dark theme matching the RustForge IDE palette (default).
    #[default]
    DarkDefault,
    /// Monokai — classic dark theme with warm accent tones.
    Monokai,
    /// One Dark — Atom editor's dark theme, widely ported.
    OneDark,
    /// Solarized Dark — Ethan Schoonover's precise dark variant.
    SolarizedDark,
    /// Solarized Light — Ethan Schoonover's light variant.
    SolarizedLight,
    /// GitHub Light — matches github.com code view.
    GithubLight,
    /// Catppuccin Mocha — soft pastel palette over a desaturated charcoal base.
    Catppuccin,
    /// Nord — frost-blue accents over a polar-night base.
    Nord,
}

impl EditorTheme {
    /// All theme variants in menu order.
    pub const ALL: &'static [EditorTheme] = &[
        EditorTheme::DarkDefault,
        EditorTheme::Monokai,
        EditorTheme::OneDark,
        EditorTheme::SolarizedDark,
        EditorTheme::SolarizedLight,
        EditorTheme::GithubLight,
        EditorTheme::Catppuccin,
        EditorTheme::Nord,
    ];

    /// Display name shown in the Theme submenu.
    pub fn display_name(self) -> &'static str {
        match self {
            EditorTheme::DarkDefault => "Dark Default",
            EditorTheme::Monokai => "Monokai",
            EditorTheme::OneDark => "One Dark",
            EditorTheme::SolarizedDark => "Solarized Dark",
            EditorTheme::SolarizedLight => "Solarized Light",
            EditorTheme::GithubLight => "GitHub Light",
            EditorTheme::Catppuccin => "Catppuccin",
            EditorTheme::Nord => "Nord",
        }
    }

    /// Return the [`SyntaxColors`] palette for this theme.
    pub fn colors(self) -> SyntaxColors {
        match self {
            EditorTheme::DarkDefault => SyntaxColors::dark_default(),
            EditorTheme::Monokai => SyntaxColors::monokai(),
            EditorTheme::OneDark => SyntaxColors::one_dark(),
            EditorTheme::SolarizedDark => SyntaxColors::solarized_dark(),
            EditorTheme::SolarizedLight => SyntaxColors::solarized_light(),
            EditorTheme::GithubLight => SyntaxColors::github_light(),
            EditorTheme::Catppuccin => SyntaxColors::catppuccin(),
            EditorTheme::Nord => SyntaxColors::nord(),
        }
    }

    /// Map a crate-wide [`crate::theme::Theme`] to the closest [`EditorTheme`]
    /// preset. Used by [`EditorConfig::with_crate_theme`] /
    /// [`EditorConfig::set_crate_theme`] so the editor picks the syntax
    /// palette that visually matches whichever chrome theme the host app
    /// is running under.
    ///
    /// The crate ships two chrome themes (`Dark`, `Light`); each maps to
    /// the matching editor preset (`DarkDefault` / `GithubLight`). The
    /// remaining [`EditorTheme`] presets (OneDark, Monokai, Solarized,
    /// Catppuccin, Nord, …) stay available for direct selection via
    /// [`EditorConfig::set_theme`].
    pub fn from_crate_theme(theme: crate::theme::Theme) -> Self {
        use crate::theme::Theme;
        match theme {
            Theme::Dark => Self::DarkDefault,
            Theme::Light => Self::GithubLight,
        }
    }
}

// ── SyntaxColors ─────────────────────────────────────────────────────────────

/// Token color palette for syntax highlighting.
///
/// All fields are `[r, g, b, a]` with values in `0.0..=1.0`.
/// Use [`EditorConfig::set_theme`] to apply a full preset, or modify
/// individual fields for custom overrides.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    // ── Hex-mode value-based colors (NxT palette) ─────────────────
    /// Null byte `00` — red.
    pub hex_null: [f32; 4],
    /// `FF` byte — amber.
    pub hex_ff: [f32; 4],
    /// Control chars `01–1F`, `7F` and high bytes `80–FE` — silver/default.
    pub hex_default: [f32; 4],
    /// Printable ASCII `20–7E` — green.
    pub hex_printable: [f32; 4],
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
    /// Editor text-area background (used for the child-window fill).
    pub editor_bg: [f32; 4],
    /// Breakpoint marker fill colour drawn in the gutter.
    pub breakpoint: [f32; 4],
    /// Vertical separator line between the gutter and the text area.
    pub gutter_separator: [f32; 4],
    /// Primary cursor / caret colour.
    pub cursor: [f32; 4],
    /// Whitespace marker glyph (mid-dot for spaces, arrow for tabs).
    pub whitespace_marker: [f32; 4],
    /// Eight semantic slots for line decorations (Wash / Rule / Ghost /
    /// EndPill). Hosts map their own domain roles onto slots, e.g.
    /// `S1 = opcode`, `S2 = subop`, `S3 = int`, etc. Each theme paints
    /// the eight harmoniously; hosts pick by role, not by hex.
    ///
    /// `#[serde(default)]` so older serialized `SyntaxColors` still
    /// deserialize (they get the theme's `default_decoration_slots`).
    #[serde(default = "SyntaxColors::default_decoration_slots")]
    pub decoration_slots: [[f32; 4]; 8],
}

impl SyntaxColors {
    /// Fallback for older RON configs missing `decoration_slots`. Neutral
    /// palette — hosts that care about theme fit should pick a preset with
    /// [`crate::code_editor::EditorConfig::set_theme`] which overwrites
    /// this field with theme-specific values.
    pub fn default_decoration_slots() -> [[f32; 4]; 8] {
        [[0.5, 0.5, 0.5, 1.0]; 8]
    }
}

impl Default for SyntaxColors {
    fn default() -> Self {
        Self::dark_default()
    }
}

impl SyntaxColors {
    // ── Dark Default (RustForge palette) ─────────────────────────────────

    /// Dark theme matching the RustForge IDE palette.
    pub fn dark_default() -> Self {
        Self {
            // Token literals — inlined to keep the palette self-contained.
            // The values mirror the legacy `theme::{ACCENT,TEXT_PRIMARY,
            // TEXT_MUTED,DANGER,WARNING,SEPARATOR}` constants which were
            // hard-pinned to the Dark theme; a per-theme palette must
            // not reach into another theme's tokens.
            keyword: [0.36, 0.61, 0.84, 1.0], // ACCENT
            type_name: [0.56, 0.84, 0.62, 1.0],
            lifetime: [0.85, 0.60, 0.85, 1.0],
            string: [0.80, 0.88, 0.52, 1.0],
            char_lit: [0.80, 0.88, 0.52, 1.0],
            number: [0.78, 0.58, 0.95, 1.0],
            comment: [0.47, 0.53, 0.60, 1.0],
            attribute: [0.82, 0.72, 0.36, 1.0],
            macro_call: [0.90, 0.75, 0.35, 1.0],
            operator: [0.72, 0.88, 0.98, 1.0],
            punctuation: [0.60, 0.62, 0.68, 1.0],
            identifier: [0.88, 0.90, 0.92, 1.0], // TEXT_PRIMARY
            user_code_marker: [0.85, 0.65, 0.25, 1.0], // WARNING
            hex_null: [0.95, 0.42, 0.47, 1.0],   // red  (NxT CLR_ZERO)
            hex_ff: [1.00, 0.78, 0.30, 1.0],     // amber (NxT CLR_FF)
            hex_default: [0.82, 0.86, 0.93, 1.0], // silver (NxT CLR_DEFAULT)
            hex_printable: [0.65, 0.92, 0.73, 1.0], // green (NxT CLR_ASCII)
            current_line_bg: [0.18, 0.20, 0.26, 1.0],
            selection_bg: [0.26, 0.52, 0.86, 0.55],
            search_match_bg: [0.62, 0.52, 0.10, 0.30],
            search_current_bg: [0.62, 0.52, 0.10, 0.62],
            line_number: [0.40, 0.42, 0.48, 1.0], // TEXT_MUTED
            line_number_active: [0.88, 0.90, 0.92, 1.0], // TEXT_PRIMARY
            bracket_match_bg: [0.38, 0.44, 0.58, 0.45],
            error_underline: [0.82, 0.27, 0.27, 1.0], // DANGER
            warning_underline: [0.85, 0.65, 0.25, 1.0], // WARNING
            gutter_bg: [0.09, 0.10, 0.13, 1.0],
            editor_bg: [0.11, 0.12, 0.16, 1.0],
            breakpoint: [0.82, 0.27, 0.27, 1.0],        // DANGER
            gutter_separator: [0.22, 0.25, 0.30, 1.0],  // SEPARATOR
            cursor: [0.88, 0.90, 0.92, 1.0],            // TEXT_PRIMARY
            whitespace_marker: [0.40, 0.42, 0.48, 1.0], // TEXT_MUTED
            // 8 harmonious rainbow slots — red / orange / yellow / green /
            // cyan / blue / purple / pink. Reuse existing NxT palette
            // values where they already sit at the right hue.
            decoration_slots: [
                [0.95, 0.42, 0.47, 1.0], // S1 red    (hex_null)
                [0.95, 0.65, 0.32, 1.0], // S2 orange
                [1.00, 0.78, 0.30, 1.0], // S3 yellow (hex_ff)
                [0.56, 0.84, 0.62, 1.0], // S4 green  (type_name)
                [0.55, 0.85, 0.90, 1.0], // S5 cyan
                [0.36, 0.61, 0.84, 1.0], // S6 blue   (keyword)
                [0.78, 0.58, 0.95, 1.0], // S7 purple (number)
                [0.90, 0.55, 0.82, 1.0], // S8 pink
            ],
        }
    }

    // ── Monokai ───────────────────────────────────────────────────────────

    /// Monokai — classic dark theme, warm accent tones.
    pub fn monokai() -> Self {
        Self {
            keyword: [0.976, 0.149, 0.447, 1.0],   // #F92672 pink-red
            type_name: [0.400, 0.851, 0.910, 1.0], // #66D9E8 cyan
            lifetime: [0.651, 0.886, 0.180, 1.0],  // #A6E22E green
            string: [0.902, 0.863, 0.455, 1.0],    // #E6DB74 yellow
            char_lit: [0.902, 0.863, 0.455, 1.0],
            number: [0.682, 0.506, 1.000, 1.0],  // #AE81FF purple
            comment: [0.459, 0.443, 0.369, 1.0], // #75715E warm grey
            attribute: [0.651, 0.886, 0.180, 1.0], // #A6E22E green
            macro_call: [0.651, 0.886, 0.180, 1.0],
            operator: [0.976, 0.149, 0.447, 1.0], // same as keyword
            punctuation: [0.973, 0.973, 0.949, 1.0], // #F8F8F2 near-white
            identifier: [0.973, 0.973, 0.949, 1.0],
            user_code_marker: [0.976, 0.149, 0.447, 1.0],
            hex_null: [0.95, 0.42, 0.47, 1.0],           // red
            hex_ff: [1.00, 0.78, 0.30, 1.0],             // amber
            hex_default: [0.82, 0.86, 0.93, 1.0],        // silver
            hex_printable: [0.65, 0.92, 0.73, 1.0],      // green
            current_line_bg: [0.243, 0.239, 0.196, 1.0], // #3E3D32
            selection_bg: [0.350, 0.340, 0.280, 0.75],
            search_match_bg: [0.651, 0.886, 0.180, 0.25],
            search_current_bg: [0.651, 0.886, 0.180, 0.55],
            line_number: [0.459, 0.443, 0.369, 1.0],
            line_number_active: [0.973, 0.973, 0.949, 1.0],
            bracket_match_bg: [0.400, 0.851, 0.910, 0.30],
            error_underline: [0.976, 0.149, 0.447, 1.0],
            warning_underline: [0.902, 0.863, 0.455, 1.0],
            gutter_bg: [0.118, 0.122, 0.110, 1.0],  // #1E1F1C
            editor_bg: [0.153, 0.157, 0.133, 1.0],  // #272822
            breakpoint: [0.976, 0.149, 0.447, 1.0], // pink-red
            gutter_separator: [0.243, 0.239, 0.196, 1.0], // #3E3D32
            cursor: [0.973, 0.973, 0.949, 1.0],     // near-white
            whitespace_marker: [0.459, 0.443, 0.369, 1.0], // warm grey
            // Monokai canonical palette — F92672 / FD971F / E6DB74 /
            // A6E22E / 66D9EF / softer-blue / AE81FF / plus a pink accent.
            decoration_slots: [
                [0.976, 0.149, 0.447, 1.0], // S1 #F92672 magenta-red
                [0.992, 0.592, 0.122, 1.0], // S2 #FD971F orange
                [0.902, 0.859, 0.455, 1.0], // S3 #E6DB74 yellow
                [0.651, 0.886, 0.180, 1.0], // S4 #A6E22E green
                [0.400, 0.851, 0.937, 1.0], // S5 #66D9EF cyan
                [0.44, 0.62, 0.82, 1.0],    // S6 softer blue
                [0.682, 0.506, 1.000, 1.0], // S7 #AE81FF purple
                [1.000, 0.620, 0.831, 1.0], // S8 pink
            ],
        }
    }

    // ── One Dark ──────────────────────────────────────────────────────────

    /// One Dark — Atom editor's dark theme.
    pub fn one_dark() -> Self {
        Self {
            keyword: [0.776, 0.471, 0.867, 1.0],   // #C678DD purple
            type_name: [0.898, 0.753, 0.482, 1.0], // #E5C07B tan
            lifetime: [0.820, 0.604, 0.400, 1.0],  // #D19A66 orange
            string: [0.596, 0.765, 0.475, 1.0],    // #98C379 green
            char_lit: [0.596, 0.765, 0.475, 1.0],
            number: [0.820, 0.604, 0.400, 1.0],  // #D19A66 orange
            comment: [0.361, 0.388, 0.439, 1.0], // #5C6370 grey
            attribute: [0.878, 0.424, 0.459, 1.0], // #E06C75 red/pink
            macro_call: [0.380, 0.686, 0.937, 1.0], // #61AFEF blue
            operator: [0.337, 0.714, 0.761, 1.0], // #56B6C2 cyan
            punctuation: [0.671, 0.698, 0.749, 1.0], // #ABB2BF grey
            identifier: [0.671, 0.698, 0.749, 1.0],
            user_code_marker: [0.898, 0.753, 0.482, 1.0],
            hex_null: [0.95, 0.42, 0.47, 1.0],           // red
            hex_ff: [1.00, 0.78, 0.30, 1.0],             // amber
            hex_default: [0.82, 0.86, 0.93, 1.0],        // silver
            hex_printable: [0.65, 0.92, 0.73, 1.0],      // green
            current_line_bg: [0.173, 0.192, 0.235, 1.0], // #2C313C
            selection_bg: [0.28, 0.38, 0.60, 0.55],
            search_match_bg: [0.380, 0.686, 0.937, 0.25],
            search_current_bg: [0.380, 0.686, 0.937, 0.55],
            line_number: [0.271, 0.294, 0.341, 1.0],
            line_number_active: [0.671, 0.698, 0.749, 1.0],
            bracket_match_bg: [0.337, 0.714, 0.761, 0.30],
            error_underline: [0.878, 0.424, 0.459, 1.0],
            warning_underline: [0.820, 0.604, 0.400, 1.0],
            gutter_bg: [0.129, 0.145, 0.169, 1.0],  // #21252B
            editor_bg: [0.157, 0.173, 0.204, 1.0],  // #282C34
            breakpoint: [0.878, 0.424, 0.459, 1.0], // red/pink
            gutter_separator: [0.220, 0.243, 0.286, 1.0],
            cursor: [0.671, 0.698, 0.749, 1.0], // #ABB2BF
            whitespace_marker: [0.361, 0.388, 0.439, 1.0], // muted grey
            // Atom One Dark palette — e06c75 / d19a66 / e5c07b / 98c379 /
            // 56b6c2 / 61afef / c678dd / be5046.
            decoration_slots: [
                [0.878, 0.424, 0.459, 1.0], // S1 #e06c75 red
                [0.820, 0.604, 0.400, 1.0], // S2 #d19a66 orange
                [0.898, 0.753, 0.482, 1.0], // S3 #e5c07b yellow
                [0.596, 0.765, 0.475, 1.0], // S4 #98c379 green
                [0.337, 0.714, 0.761, 1.0], // S5 #56b6c2 cyan
                [0.380, 0.686, 0.937, 1.0], // S6 #61afef blue
                [0.776, 0.471, 0.867, 1.0], // S7 #c678dd purple
                [0.745, 0.314, 0.271, 1.0], // S8 #be5046 dark red
            ],
        }
    }

    // ── Solarized Dark ────────────────────────────────────────────────────

    /// Solarized Dark — Ethan Schoonover's precise dark variant.
    pub fn solarized_dark() -> Self {
        Self {
            keyword: [0.522, 0.600, 0.000, 1.0],   // #859900 olive
            type_name: [0.149, 0.545, 0.824, 1.0], // #268BD2 blue
            lifetime: [0.827, 0.212, 0.510, 1.0],  // #D33682 magenta
            string: [0.165, 0.631, 0.596, 1.0],    // #2AA198 cyan
            char_lit: [0.165, 0.631, 0.596, 1.0],
            number: [0.827, 0.212, 0.510, 1.0],  // #D33682 magenta
            comment: [0.345, 0.431, 0.459, 1.0], // #586E75 base01
            attribute: [0.796, 0.294, 0.086, 1.0], // #CB4B16 orange
            macro_call: [0.710, 0.537, 0.000, 1.0], // #B58900 yellow
            operator: [0.514, 0.580, 0.588, 1.0], // #839496 base0
            punctuation: [0.396, 0.482, 0.514, 1.0], // #657B83 base00
            identifier: [0.514, 0.580, 0.588, 1.0],
            user_code_marker: [0.710, 0.537, 0.000, 1.0],
            hex_null: [0.86, 0.20, 0.18, 1.0],      // solarized red
            hex_ff: [0.71, 0.54, 0.00, 1.0],        // solarized yellow
            hex_default: [0.51, 0.58, 0.59, 1.0],   // solarized base0
            hex_printable: [0.52, 0.60, 0.00, 1.0], // solarized green
            current_line_bg: [0.027, 0.212, 0.259, 1.0], // #073642 base02
            selection_bg: [0.149, 0.545, 0.824, 0.50],
            search_match_bg: [0.710, 0.537, 0.000, 0.25],
            search_current_bg: [0.710, 0.537, 0.000, 0.55],
            line_number: [0.345, 0.431, 0.459, 1.0],
            line_number_active: [0.514, 0.580, 0.588, 1.0],
            bracket_match_bg: [0.149, 0.545, 0.824, 0.25],
            error_underline: [0.863, 0.196, 0.184, 1.0], // #DC322F red
            warning_underline: [0.796, 0.294, 0.086, 1.0],
            gutter_bg: [0.027, 0.212, 0.259, 1.0],  // #073642
            editor_bg: [0.000, 0.169, 0.212, 1.0],  // #002B36 base03
            breakpoint: [0.863, 0.196, 0.184, 1.0], // red
            gutter_separator: [0.027, 0.212, 0.259, 1.0], // base02
            cursor: [0.514, 0.580, 0.588, 1.0],     // base0
            whitespace_marker: [0.345, 0.431, 0.459, 1.0], // base01
            // Solarized aurora palette — dc322f / cb4b16 / b58900 / 859900 /
            // 2aa198 / 268bd2 / 6c71c4 / d33682.
            decoration_slots: [
                [0.863, 0.196, 0.184, 1.0], // S1 #dc322f red
                [0.796, 0.294, 0.086, 1.0], // S2 #cb4b16 orange
                [0.710, 0.537, 0.000, 1.0], // S3 #b58900 yellow
                [0.522, 0.600, 0.000, 1.0], // S4 #859900 green
                [0.165, 0.631, 0.596, 1.0], // S5 #2aa198 cyan
                [0.149, 0.545, 0.824, 1.0], // S6 #268bd2 blue
                [0.424, 0.443, 0.769, 1.0], // S7 #6c71c4 violet
                [0.827, 0.212, 0.510, 1.0], // S8 #d33682 magenta
            ],
        }
    }

    // ── Solarized Light ───────────────────────────────────────────────────

    /// Solarized Light — Ethan Schoonover's light variant.
    pub fn solarized_light() -> Self {
        Self {
            keyword: [0.522, 0.600, 0.000, 1.0],   // #859900 olive
            type_name: [0.149, 0.545, 0.824, 1.0], // #268BD2 blue
            lifetime: [0.827, 0.212, 0.510, 1.0],  // #D33682 magenta
            string: [0.165, 0.631, 0.596, 1.0],    // #2AA198 cyan
            char_lit: [0.165, 0.631, 0.596, 1.0],
            number: [0.827, 0.212, 0.510, 1.0],
            comment: [0.576, 0.631, 0.631, 1.0], // #93A1A1 base1
            attribute: [0.796, 0.294, 0.086, 1.0], // #CB4B16 orange
            macro_call: [0.710, 0.537, 0.000, 1.0], // #B58900 yellow
            operator: [0.396, 0.482, 0.514, 1.0], // #657B83 base00
            punctuation: [0.514, 0.580, 0.588, 1.0], // #839496 base0
            identifier: [0.396, 0.482, 0.514, 1.0],
            user_code_marker: [0.710, 0.537, 0.000, 1.0],
            hex_null: [0.86, 0.20, 0.18, 1.0],      // solarized red
            hex_ff: [0.71, 0.54, 0.00, 1.0],        // solarized yellow
            hex_default: [0.40, 0.48, 0.51, 1.0],   // solarized base00
            hex_printable: [0.52, 0.60, 0.00, 1.0], // solarized green
            current_line_bg: [0.933, 0.910, 0.835, 1.0], // #EEE8D5 base2
            selection_bg: [0.149, 0.545, 0.824, 0.40],
            search_match_bg: [0.710, 0.537, 0.000, 0.20],
            search_current_bg: [0.710, 0.537, 0.000, 0.45],
            line_number: [0.576, 0.631, 0.631, 1.0],
            line_number_active: [0.396, 0.482, 0.514, 1.0],
            bracket_match_bg: [0.149, 0.545, 0.824, 0.20],
            error_underline: [0.863, 0.196, 0.184, 1.0],
            warning_underline: [0.796, 0.294, 0.086, 1.0],
            gutter_bg: [0.933, 0.910, 0.835, 1.0], // #EEE8D5 base2
            editor_bg: [0.992, 0.965, 0.890, 1.0], // #FDF6E3 base3
            breakpoint: [0.863, 0.196, 0.184, 1.0], // red
            gutter_separator: [0.808, 0.808, 0.694, 1.0],
            cursor: [0.396, 0.482, 0.514, 1.0], // base00
            whitespace_marker: [0.576, 0.631, 0.631, 1.0], // base1
            // Solarized aurora, slightly darker than the Dark variant so
            // the colours pop against the light base3 backdrop.
            decoration_slots: [
                [0.720, 0.150, 0.130, 1.0], // S1 red   darker
                [0.660, 0.240, 0.050, 1.0], // S2 orange darker
                [0.580, 0.440, 0.000, 1.0], // S3 yellow darker
                [0.420, 0.490, 0.000, 1.0], // S4 green darker
                [0.110, 0.510, 0.480, 1.0], // S5 cyan  darker
                [0.110, 0.440, 0.680, 1.0], // S6 blue  darker
                [0.340, 0.350, 0.620, 1.0], // S7 violet darker
                [0.680, 0.170, 0.420, 1.0], // S8 magenta darker
            ],
        }
    }

    // ── GitHub Light ──────────────────────────────────────────────────────

    /// GitHub Light — matches github.com code view.
    pub fn github_light() -> Self {
        Self {
            keyword: [0.843, 0.227, 0.286, 1.0],   // #D73A49 red
            type_name: [0.435, 0.259, 0.757, 1.0], // #6F42C1 purple
            lifetime: [0.435, 0.259, 0.757, 1.0],
            string: [0.012, 0.184, 0.384, 1.0], // #032F62 dark blue
            char_lit: [0.012, 0.184, 0.384, 1.0],
            number: [0.000, 0.361, 0.773, 1.0],  // #005CC5 blue
            comment: [0.416, 0.451, 0.490, 1.0], // #6A737D grey
            attribute: [0.435, 0.259, 0.757, 1.0],
            macro_call: [0.843, 0.227, 0.286, 1.0],
            operator: [0.843, 0.227, 0.286, 1.0],
            punctuation: [0.141, 0.161, 0.180, 1.0], // #24292E near-black
            identifier: [0.141, 0.161, 0.180, 1.0],
            user_code_marker: [0.639, 0.353, 0.000, 1.0],
            hex_null: [0.82, 0.18, 0.15, 1.0],      // github red
            hex_ff: [0.73, 0.55, 0.00, 1.0],        // github amber
            hex_default: [0.35, 0.40, 0.46, 1.0],   // github gray
            hex_printable: [0.12, 0.50, 0.28, 1.0], // github green
            current_line_bg: [0.945, 0.973, 1.000, 1.0], // #F1F8FF
            selection_bg: [0.012, 0.400, 0.839, 0.38],
            search_match_bg: [1.000, 0.847, 0.000, 0.30],
            search_current_bg: [1.000, 0.847, 0.000, 0.60],
            line_number: [0.729, 0.733, 0.741, 1.0], // #BABBBD
            line_number_active: [0.141, 0.161, 0.180, 1.0],
            bracket_match_bg: [0.012, 0.400, 0.839, 0.15],
            error_underline: [0.843, 0.227, 0.286, 1.0],
            warning_underline: [0.639, 0.353, 0.000, 1.0],
            gutter_bg: [0.965, 0.973, 0.980, 1.0],  // #F6F8FA
            editor_bg: [1.000, 1.000, 1.000, 1.0],  // #FFFFFF
            breakpoint: [0.843, 0.227, 0.286, 1.0], // github red
            gutter_separator: [0.882, 0.890, 0.902, 1.0],
            cursor: [0.141, 0.161, 0.180, 1.0], // near-black
            whitespace_marker: [0.729, 0.733, 0.741, 1.0], // #BABBBD
            // GitHub Light palette — cf222e / c14b13 / a37100 / 1a7f37 /
            // 0e737b / 0853ad / 7e3cad / be2f8a. Saturated for light bg.
            decoration_slots: [
                [0.812, 0.133, 0.180, 1.0], // S1 #cf222e red
                [0.757, 0.294, 0.075, 1.0], // S2 #c14b13 orange
                [0.639, 0.443, 0.000, 1.0], // S3 #a37100 yellow
                [0.102, 0.498, 0.216, 1.0], // S4 #1a7f37 green
                [0.055, 0.451, 0.482, 1.0], // S5 #0e737b cyan
                [0.031, 0.325, 0.678, 1.0], // S6 #0853ad blue
                [0.494, 0.235, 0.678, 1.0], // S7 #7e3cad purple
                [0.745, 0.184, 0.541, 1.0], // S8 #be2f8a magenta
            ],
        }
    }

    // ── Catppuccin Mocha ──────────────────────────────────────────────────

    /// Catppuccin Mocha — soft pastel palette over a desaturated charcoal
    /// base. Built from the canonical Catppuccin Mocha swatches. Select
    /// directly via [`EditorTheme::Catppuccin`].
    pub fn catppuccin() -> Self {
        Self {
            keyword: [0.804, 0.518, 0.812, 1.0],   // mauve
            type_name: [0.953, 0.835, 0.553, 1.0], // yellow
            lifetime: [0.961, 0.659, 0.376, 1.0],  // peach
            string: [0.651, 0.890, 0.631, 1.0],    // green
            char_lit: [0.651, 0.890, 0.631, 1.0],
            number: [0.961, 0.659, 0.376, 1.0],      // peach
            comment: [0.435, 0.475, 0.580, 1.0],     // overlay2
            attribute: [0.835, 0.604, 0.961, 1.0],   // pink
            macro_call: [0.537, 0.706, 0.980, 1.0],  // sapphire
            operator: [0.537, 0.819, 0.953, 1.0],    // sky
            punctuation: [0.706, 0.737, 0.835, 1.0], // subtext0
            identifier: [0.804, 0.839, 0.957, 1.0],  // text
            user_code_marker: [0.953, 0.835, 0.553, 1.0],
            hex_null: [0.953, 0.545, 0.659, 1.0], // red/maroon
            hex_ff: [0.980, 0.886, 0.643, 1.0],   // yellow
            hex_default: [0.706, 0.737, 0.835, 1.0],
            hex_printable: [0.651, 0.890, 0.631, 1.0],
            current_line_bg: [0.196, 0.196, 0.275, 1.0], // surface0
            selection_bg: [0.537, 0.706, 0.980, 0.40],
            search_match_bg: [0.980, 0.886, 0.643, 0.30],
            search_current_bg: [0.980, 0.886, 0.643, 0.55],
            line_number: [0.486, 0.510, 0.612, 1.0], // overlay1
            line_number_active: [0.804, 0.839, 0.957, 1.0],
            bracket_match_bg: [0.537, 0.819, 0.953, 0.30],
            error_underline: [0.953, 0.545, 0.659, 1.0],
            warning_underline: [0.980, 0.886, 0.643, 1.0],
            gutter_bg: [0.157, 0.157, 0.227, 1.0], // mantle
            editor_bg: [0.118, 0.118, 0.180, 1.0], // base
            breakpoint: [0.953, 0.545, 0.659, 1.0],
            gutter_separator: [0.196, 0.196, 0.275, 1.0],
            cursor: [0.804, 0.839, 0.957, 1.0],
            whitespace_marker: [0.486, 0.510, 0.612, 1.0],
            // Catppuccin Mocha palette — pink / peach / yellow / green /
            // teal / blue / mauve / red. Pastel harmony over base #1E1E2E.
            decoration_slots: [
                [0.953, 0.545, 0.659, 1.0], // S1 #f38ba8 pink
                [0.980, 0.702, 0.529, 1.0], // S2 #fab387 peach
                [0.976, 0.886, 0.686, 1.0], // S3 #f9e2af yellow
                [0.651, 0.890, 0.631, 1.0], // S4 #a6e3a1 green
                [0.580, 0.886, 0.835, 1.0], // S5 #94e2d5 teal
                [0.537, 0.706, 0.980, 1.0], // S6 #89b4fa blue
                [0.796, 0.651, 0.969, 1.0], // S7 #cba6f7 mauve
                [0.929, 0.510, 0.647, 1.0], // S8 #eba0ac maroon-pink
            ],
        }
    }

    // ── Nord ──────────────────────────────────────────────────────────────

    /// Nord — frost-blue accents over a polar-night base. Select directly
    /// via [`EditorTheme::Nord`] when you want the editor in Nord colours.
    pub fn nord() -> Self {
        Self {
            keyword: [0.506, 0.631, 0.757, 1.0],   // nord9 frost-blue
            type_name: [0.553, 0.749, 0.812, 1.0], // nord8 light-frost
            lifetime: [0.706, 0.557, 0.678, 1.0],  // nord15 purple
            string: [0.639, 0.745, 0.549, 1.0],    // nord14 aurora-green
            char_lit: [0.639, 0.745, 0.549, 1.0],
            number: [0.706, 0.557, 0.678, 1.0],    // nord15 purple
            comment: [0.298, 0.337, 0.416, 1.0],   // nord3 polar-night
            attribute: [0.922, 0.796, 0.545, 1.0], // nord13 yellow
            macro_call: [0.553, 0.749, 0.812, 1.0],
            operator: [0.533, 0.753, 0.816, 1.0], // nord8 cyan
            punctuation: [0.847, 0.871, 0.914, 1.0], // nord5 snow-storm
            identifier: [0.925, 0.937, 0.957, 1.0], // nord4 snow-storm
            user_code_marker: [0.922, 0.796, 0.545, 1.0],
            hex_null: [0.749, 0.380, 0.416, 1.0], // nord11 red
            hex_ff: [0.922, 0.796, 0.545, 1.0],   // nord13 yellow
            hex_default: [0.847, 0.871, 0.914, 1.0],
            hex_printable: [0.639, 0.745, 0.549, 1.0],
            current_line_bg: [0.231, 0.259, 0.322, 1.0], // nord1
            selection_bg: [0.506, 0.631, 0.757, 0.40],
            search_match_bg: [0.922, 0.796, 0.545, 0.25],
            search_current_bg: [0.922, 0.796, 0.545, 0.55],
            line_number: [0.392, 0.439, 0.522, 1.0], // nord3-mid
            line_number_active: [0.925, 0.937, 0.957, 1.0],
            bracket_match_bg: [0.553, 0.749, 0.812, 0.30],
            error_underline: [0.749, 0.380, 0.416, 1.0],
            warning_underline: [0.820, 0.529, 0.439, 1.0], // nord12 orange
            gutter_bg: [0.180, 0.204, 0.251, 1.0],         // nord0
            editor_bg: [0.231, 0.259, 0.322, 1.0],         // nord1 (mute)
            breakpoint: [0.749, 0.380, 0.416, 1.0],
            gutter_separator: [0.298, 0.337, 0.416, 1.0],
            cursor: [0.925, 0.937, 0.957, 1.0],
            whitespace_marker: [0.392, 0.439, 0.522, 1.0],
            // Nord aurora + frost palette — bf616a / d08770 / ebcb8b /
            // a3be8c / 88c0d0 / 5e81ac / b48ead / 81a1c1.
            decoration_slots: [
                [0.749, 0.380, 0.416, 1.0], // S1 #bf616a red    (aurora)
                [0.816, 0.529, 0.439, 1.0], // S2 #d08770 orange (aurora)
                [0.922, 0.796, 0.545, 1.0], // S3 #ebcb8b yellow (aurora)
                [0.639, 0.745, 0.549, 1.0], // S4 #a3be8c green  (aurora)
                [0.533, 0.753, 0.816, 1.0], // S5 #88c0d0 cyan   (frost)
                [0.369, 0.506, 0.675, 1.0], // S6 #5e81ac blue   (frost)
                [0.706, 0.557, 0.678, 1.0], // S7 #b48ead purple (aurora)
                [0.506, 0.631, 0.757, 1.0], // S8 #81a1c1 slate  (frost)
            ],
        }
    }
}
