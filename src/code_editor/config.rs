//! Configuration types for [`CodeEditor`].

use super::token::Token;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// ── Font re-exports for backward compatibility ───────────────────────────────
//
// The bundled TTF blobs and `BuiltinFont` enum live in the top-level `fonts`
// module now (centralised font registry). These re-exports keep existing
// callers like `code_editor::{BuiltinFont, HACK_FONT_DATA, ...}` working.

pub use crate::fonts::{
    BuiltinFont, HACK_FONT_DATA, JETBRAINS_MONO_FONT_DATA, JETBRAINS_MONO_LIGATURES_FONT_DATA,
    MDI_FONT_DATA, merge_mdi_icons,
};

// ── Global code-editor font ───────────────────────────────────────────────────

/// Raw `*mut ImFont` pointer for the **dedicated code-editor font**, stored as
/// a `usize` so it is `Send + Sync`.
///
/// When non-zero, every [`CodeEditor`] instance will push this specific font
/// (via `igPushFont`) instead of the default ImGui font.  This lets the IDE
/// use a proportional UI font (e.g. Segoe UI) while keeping the code editor
/// on a monospace font (e.g. Consolas) — which is required for the column↔pixel
/// mapping to be accurate.
///
/// You can set this manually, or use [`install_code_editor_font`] for zero-config
/// setup with an embedded JetBrains Mono font.
pub static CODE_EDITOR_FONT_PTR: AtomicUsize = AtomicUsize::new(0);

/// Read the current code-editor font pointer (0 = use default font).
#[inline]
pub fn code_editor_font_ptr() -> *mut dear_imgui_rs::sys::ImFont {
    let v = CODE_EDITOR_FONT_PTR.load(Ordering::Relaxed);
    v as *mut dear_imgui_rs::sys::ImFont
}

/// Install the built-in JetBrains Mono font **with MDI icons** into the ImGui
/// font atlas and register it as the code-editor font.
///
/// This is the recommended one-call setup.  It:
/// 1. Adds JetBrains Mono NL as a new atlas font (becomes the default if it's
///    the first font added).
/// 2. Merges Material Design Icons into the same font so that icon constants
///    from [`crate::icons`] render correctly (context menus, find bar, etc.).
/// 3. Stores the resulting `ImFont*` in [`CODE_EDITOR_FONT_PTR`] for use by
///    every [`CodeEditor`](super::CodeEditor) instance.
///
/// Call **once** at startup, **before** the renderer builds the font atlas.
///
/// # Example
///
/// ```rust,ignore
/// use dear_imgui_custom_mod::code_editor::install_code_editor_font;
///
/// let mut context = dear_imgui_rs::Context::create();
/// install_code_editor_font(&mut context, 15.0 * hidpi);
/// // … create renderer (builds font atlas) …
/// // CodeEditor now uses JetBrains Mono + MDI icons automatically.
/// ```
pub fn install_code_editor_font(ctx: &mut dear_imgui_rs::Context, size_pixels: f32) {
    install_code_editor_font_ex(ctx, size_pixels, BuiltinFont::default());
}

/// Like [`install_code_editor_font`] but lets you choose which built-in font
/// variant to use.
pub fn install_code_editor_font_ex(
    ctx: &mut dear_imgui_rs::Context,
    size_pixels: f32,
    font: BuiltinFont,
) {
    let ptr = crate::fonts::install_monospace(ctx, font, size_pixels, true);
    if !ptr.is_null() {
        CODE_EDITOR_FONT_PTR.store(ptr as usize, Ordering::SeqCst);
    }
}

/// Install a custom TTF font from raw bytes and register it as the code-editor
/// font, with MDI icons merged in.
pub fn install_custom_code_editor_font(
    ctx: &mut dear_imgui_rs::Context,
    data: &[u8],
    size_pixels: f32,
    name: &str,
) {
    let ptr = crate::fonts::install_ui_font(ctx, data, size_pixels, name, true);
    if !ptr.is_null() {
        CODE_EDITOR_FONT_PTR.store(ptr as usize, Ordering::SeqCst);
    }
}

// ── SyntaxDefinition ─────────────────────────────────────────────────────────

/// Trait for custom syntax definitions.
///
/// Implement this to provide token-level highlighting for any language or DSL.
/// The trait is object-safe and stored as `Arc<dyn SyntaxDefinition>` inside
/// [`Language::Custom`], allowing cheap cloning of the language config.
///
/// # Example — minimal hex-packet DSL
/// ```rust,no_run
/// use dear_imgui_custom_mod::code_editor::{SyntaxDefinition, token::{Token, TokenKind}};
/// use std::sync::Arc;
///
/// struct HexPacketSyntax;
/// impl SyntaxDefinition for HexPacketSyntax {
///     fn name(&self) -> &str { "HexPacket" }
///     fn tokenize_line(&self, line: &str, _in_bc: bool) -> (Vec<Token>, bool) {
///         if line.trim_start().starts_with("//") {
///             return (vec![Token { kind: TokenKind::Comment, start: 0, len: line.len() }], false);
///         }
///         // … tokenize hex bytes …
///         (vec![], false)
///     }
/// }
/// // editor.config.language = Language::Custom(Arc::new(HexPacketSyntax));
/// ```
pub trait SyntaxDefinition: Send + Sync {
    /// Short display name shown in the Language menu.
    fn name(&self) -> &str;

    /// Tokenize a single line.
    ///
    /// `in_block_comment` carries state from the previous line.
    /// Return `(tokens, still_in_block_comment)`.
    fn tokenize_line(&self, line: &str, in_block_comment: bool) -> (Vec<Token>, bool);

    /// Prefix used by Toggle Comment (`Ctrl+/`). `None` disables the command.
    fn line_comment_prefix(&self) -> Option<&str> {
        Some("//")
    }

    /// Start/end delimiters for block comments (e.g. `("/*", "*/")` for C-style).
    /// Returns `None` if the language has no block comment syntax.
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        Some(("/*", "*/"))
    }

    /// Matching bracket pairs used for bracket highlighting.
    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('(', ')'), ('{', '}'), ('[', ']')]
    }

    /// Characters at the end of a line that trigger increased indentation on Enter.
    fn auto_indent_after(&self) -> &[char] {
        &['{']
    }

    /// Characters at the start of a new line that trigger decreased indentation.
    fn auto_dedent_on(&self) -> &[char] {
        &['}']
    }

    /// Pairs for auto-close: typing the open string automatically inserts the
    /// close string after the cursor.
    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("(", ")"), ("{", "}"), ("[", "]"), ("\"", "\"")]
    }

    /// Whether a character should be considered part of a "word" for
    /// double-click selection and Ctrl+arrow word navigation.
    fn is_word_char(&self, c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }
}

// ── Language ──────────────────────────────────────────────────────────────────

/// Syntax language for highlighting.
///
/// The `Custom` variant accepts any [`SyntaxDefinition`] implementation,
/// enabling fully custom tokenizers for domain-specific languages.
#[derive(Clone, Default)]
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
    /// Hex byte stream — each line is a sequence of `XX` byte pairs separated
    /// by spaces. `//` comments are supported. Bytes are colored by value:
    /// `00` = null (dim), `01–1F`/`7F` = control (red), `20–7E` = printable
    /// (cyan), `80–FF` = high (purple). Invalid non-hex characters use
    /// [`TokenKind::Operator`].
    ///
    /// Pair this with [`EditorConfig::hex_auto_space`] and
    /// [`EditorConfig::hex_auto_uppercase`] for a full hex-editing experience.
    Hex,
    /// Rhai scripting language (embedded scripting for Rust).
    Rhai,
    /// JSON (JavaScript Object Notation).
    Json,
    /// YAML (YAML Ain't Markup Language).
    Yaml,
    /// XML / HTML markup.
    Xml,
    /// x86/x86-64 assembly (AT&T + Intel/NASM/MASM unified).
    Asm,
    /// Fully custom syntax via a [`SyntaxDefinition`] trait object.
    Custom(Arc<dyn SyntaxDefinition>),
}

impl std::fmt::Debug for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::None => write!(f, "Language::None"),
            Language::Rust => write!(f, "Language::Rust"),
            Language::Toml => write!(f, "Language::Toml"),
            Language::Ron => write!(f, "Language::Ron"),
            Language::Hex => write!(f, "Language::Hex"),
            Language::Rhai => write!(f, "Language::Rhai"),
            Language::Json => write!(f, "Language::Json"),
            Language::Yaml => write!(f, "Language::Yaml"),
            Language::Xml => write!(f, "Language::Xml"),
            Language::Asm => write!(f, "Language::Asm"),
            Language::Custom(def) => write!(f, "Language::Custom(\"{}\")", def.name()),
        }
    }
}

impl PartialEq for Language {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Language::None, Language::None)
                | (Language::Rust, Language::Rust)
                | (Language::Toml, Language::Toml)
                | (Language::Ron, Language::Ron)
                | (Language::Hex, Language::Hex)
                | (Language::Rhai, Language::Rhai)
                | (Language::Json, Language::Json)
                | (Language::Yaml, Language::Yaml)
                | (Language::Xml, Language::Xml)
                | (Language::Asm, Language::Asm) // Two Custom variants are distinct (no identity comparison).
        )
    }
}

// ── EditorTheme ───────────────────────────────────────────────────────────────

/// Built-in color theme preset for the editor.
///
/// Pass to [`EditorConfig::set_theme`] to switch the entire color palette at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
        }
    }
}

// ── ContextMenuConfig ─────────────────────────────────────────────────────────

/// Fine-grained control over which context-menu sections are visible.
///
/// Set `enabled = false` to suppress the menu entirely (useful for embedded
/// read-only viewers where right-click is handled by the host).
#[derive(Debug, Clone)]
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
    fn default() -> Self {
        Self {
            enabled: true,
            show_clipboard: true,
            show_select_all: true,
            show_undo_redo: true,
            show_code_actions: true,
            show_transform: true,
            show_find: true,
            show_view_toggles: true,
            show_language_selector: true,
            show_theme_selector: true,
            show_font_size: true,
            show_cursor_info: true,
        }
    }
}

// ── SyntaxColors ─────────────────────────────────────────────────────────────

/// Token color palette for syntax highlighting.
///
/// All fields are `[r, g, b, a]` with values in `0.0..=1.0`.
/// Use [`EditorConfig::set_theme`] to apply a full preset, or modify
/// individual fields for custom overrides.
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
        use crate::theme;
        Self {
            keyword: theme::ACCENT,
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
            identifier: theme::TEXT_PRIMARY,
            user_code_marker: theme::WARNING,
            hex_null: [0.95, 0.42, 0.47, 1.0], // red  (NxT CLR_ZERO)
            hex_ff: [1.00, 0.78, 0.30, 1.0],   // amber (NxT CLR_FF)
            hex_default: [0.82, 0.86, 0.93, 1.0], // silver (NxT CLR_DEFAULT)
            hex_printable: [0.65, 0.92, 0.73, 1.0], // green (NxT CLR_ASCII)
            current_line_bg: [0.18, 0.20, 0.26, 1.0],
            selection_bg: [0.26, 0.52, 0.86, 0.55],
            search_match_bg: [0.62, 0.52, 0.10, 0.30],
            search_current_bg: [0.62, 0.52, 0.10, 0.62],
            line_number: theme::TEXT_MUTED,
            line_number_active: theme::TEXT_PRIMARY,
            bracket_match_bg: [0.38, 0.44, 0.58, 0.45],
            error_underline: theme::DANGER,
            warning_underline: theme::WARNING,
            gutter_bg: [0.09, 0.10, 0.13, 1.0],
            editor_bg: [0.11, 0.12, 0.16, 1.0],
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
            gutter_bg: [0.118, 0.122, 0.110, 1.0], // #1E1F1C
            editor_bg: [0.153, 0.157, 0.133, 1.0], // #272822
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
            gutter_bg: [0.129, 0.145, 0.169, 1.0], // #21252B
            editor_bg: [0.157, 0.173, 0.204, 1.0], // #282C34
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
            gutter_bg: [0.027, 0.212, 0.259, 1.0], // #073642
            editor_bg: [0.000, 0.169, 0.212, 1.0], // #002B36 base03
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
            gutter_bg: [0.965, 0.973, 0.980, 1.0], // #F6F8FA
            editor_bg: [1.000, 1.000, 1.000, 1.0], // #FFFFFF
        }
    }
}

// ── EditorConfig ─────────────────────────────────────────────────────────────

/// Editor behavior and appearance configuration.
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
    /// Syntax language.
    pub language: Language,
    /// Active color theme (preset name — see [`EditorTheme`]).
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
            show_color_swatches: true,
            hex_auto_space: false,
            hex_auto_uppercase: false,
            force_english_on_focus: false,
            read_only: false,
            show_fold_indicators: true,
            word_wrap: false,
            smooth_scrolling: true,
            language: Language::Rust,
            theme: EditorTheme::DarkDefault,
            colors: SyntaxColors::dark_default(),
            cursor_blink_rate: 0.53,
            scroll_speed: 3.0,
            font_size_scale: 1.0,
            context_menu: ContextMenuConfig::default(),
            max_lines: 0,
            max_line_length: 0,
        }
    }
}

impl EditorConfig {
    /// Apply a built-in theme preset — updates `theme` and `colors` atomically.
    pub fn set_theme(&mut self, theme: EditorTheme) {
        self.colors = theme.colors();
        self.theme = theme;
    }
}
