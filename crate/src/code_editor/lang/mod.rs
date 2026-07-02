//! Language definitions, syntax traits, and font management for code_editor.
//!
//! Each built-in language is a unit struct implementing [`SyntaxDefinition`].
//! The [`tokenize_line`] function dispatches to the correct tokenizer,
//! and [`definition`] returns the full language metadata (bracket pairs,
//! auto-indent rules, comment delimiters, etc.).

pub mod asm;
pub mod diff;
pub mod dockerfile;
pub mod hex;
pub mod ini;
pub mod json;
pub mod markdown;
pub mod rhai;
pub mod ron;
pub mod rust;
pub mod sql;
pub mod toml;
pub mod xml;
pub mod yaml;

use std::sync::Arc;

// ── LineState ────────────────────────────────────────────────────────────────

/// Per-line tokenizer carry-state threaded from one line to the next.
///
/// Replaces the old single `bool` "still in a block comment" flag: it carries
/// the same information (`Code` / `BlockComment`) plus room for the multi-line
/// modes a richer highlighter needs (strings, Markdown fences, HTML raw-text,
/// YAML block scalars). Every variant is now produced by at least one built-in
/// tokenizer (e.g. `Str` by Rust/RON/TOML/Rhai, `Fenced` by Markdown, `HtmlRaw`
/// by XML/HTML, `YamlBlock` by YAML); the span-tiling invariant is enforced for
/// all of them by `all_langs_no_panic_full_coverage_all_states`.
///
/// `Copy` + `Eq` so the editor can compare and store one per line cheaply and
/// use equality for its incremental block-comment convergence early-exit.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LineState {
    /// Ordinary code — no multi-line construct is open.
    #[default]
    Code,
    /// Inside a nesting block comment; carries nesting depth.
    BlockComment(u16),
    /// Inside a multi-line string. `quote` = b'"' or b'\''; `raw` disables
    /// escapes; `hashes` = number of `#` for raw strings (Rust r#"…"#);
    /// `triple` = a triple-quoted string (TOML/Python """…""").
    Str {
        /// Opening quote byte (`b'"'` or `b'\''`).
        quote: u8,
        /// Raw string — backslash escapes are disabled.
        raw: bool,
        /// Number of `#` hashes for a raw string (Rust `r#"…"#`).
        hashes: u8,
        /// Triple-quoted string (`"""…"""`).
        triple: bool,
    },
    /// Inside a Markdown fenced code block. `fence` = b'`' or b'~';
    /// `count` = fence length (to match the close).
    Fenced {
        /// Fence byte (`` b'`' `` or `b'~'`).
        fence: u8,
        /// Fence length — the close fence must be at least this long.
        count: u8,
    },
    /// Inside an HTML raw-text element body. `is_style` = <style> vs <script>.
    HtmlRaw {
        /// `<style>` (`true`) vs `<script>` (`false`).
        is_style: bool,
    },
    /// Inside a YAML block scalar body; `indent` = min indent that stays in.
    YamlBlock {
        /// Minimum indentation that remains part of the block scalar.
        indent: u16,
    },
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
/// use dear_imgui_custom_mod::code_editor::{LineState, SyntaxDefinition, token::{Token, TokenKind}};
/// use std::sync::Arc;
///
/// struct HexPacketSyntax;
/// impl SyntaxDefinition for HexPacketSyntax {
///     fn name(&self) -> &str { "HexPacket" }
///     fn tokenize_line(&self, line: &str, _state: LineState) -> (Vec<Token>, LineState) {
///         if line.trim_start().starts_with("//") {
///             return (vec![Token { kind: TokenKind::Comment, start: 0, len: line.len() }], LineState::Code);
///         }
///         // … tokenize hex bytes …
///         (vec![], LineState::Code)
///     }
/// }
/// // editor.config.language = Language::Custom(Arc::new(HexPacketSyntax));
/// ```
pub trait SyntaxDefinition: Send + Sync {
    /// Short display name shown in the Language menu.
    fn name(&self) -> &str;

    /// Tokenize a single line.
    ///
    /// `state` carries the multi-line tokenizer state from the previous line
    /// (see [`LineState`]). Return `(tokens, state_at_end_of_line)` — the
    /// returned state is fed as the `state` argument for the next line.
    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState);

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
    /// `00` → [`TokenKind::HexNull`], `FF` → [`TokenKind::HexFF`],
    /// printable ASCII `20–7E` → [`TokenKind::HexPrintable`], everything else
    /// (control bytes and high bytes) → [`TokenKind::HexDefault`]. A lone hex
    /// nibble renders as [`TokenKind::Attribute`] (amber warning), and invalid
    /// non-hex characters use [`TokenKind::Operator`].
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
    /// SQL — case-insensitive keywords, `--` / `/* */` comments, `'…'`
    /// strings and `"…"` / `` `…` `` quoted identifiers.
    Sql,
    /// Unified diff / patch — whole-line colouring by leading prefix.
    Diff,
    /// INI / config files — `[section]`, `key = value`, `;`/`#` comments.
    Ini,
    /// Dockerfile — instruction keywords, `$VAR` variables, `--flag` options.
    Dockerfile,
    /// Markdown — headings, lists, blockquotes, inline emphasis/code/links,
    /// and multi-line fenced code blocks (tracked via [`LineState::Fenced`]).
    Markdown,
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
            Language::Sql => write!(f, "Language::Sql"),
            Language::Diff => write!(f, "Language::Diff"),
            Language::Ini => write!(f, "Language::Ini"),
            Language::Dockerfile => write!(f, "Language::Dockerfile"),
            Language::Markdown => write!(f, "Language::Markdown"),
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
                | (Language::Asm, Language::Asm)
                | (Language::Sql, Language::Sql)
                | (Language::Diff, Language::Diff)
                | (Language::Ini, Language::Ini)
                | (Language::Dockerfile, Language::Dockerfile)
                | (Language::Markdown, Language::Markdown) // Two Custom variants are distinct (no identity comparison).
        )
    }
}

// ── Ergonomics API ──────────────────────────────────────────────────────────

impl Language {
    /// Human-readable display name for the language.
    ///
    /// For [`Language::Custom`] this forwards to the definition's own
    /// [`SyntaxDefinition::name`], so the returned reference borrows `self`.
    pub fn name(&self) -> &str {
        match self {
            Language::None => "Plain Text",
            Language::Rust => "Rust",
            Language::Toml => "TOML",
            Language::Ron => "RON",
            Language::Rhai => "Rhai",
            Language::Json => "JSON",
            Language::Yaml => "YAML",
            Language::Xml => "XML",
            Language::Asm => "Assembly",
            Language::Hex => "Hex",
            Language::Sql => "SQL",
            Language::Diff => "Diff",
            Language::Ini => "INI",
            Language::Dockerfile => "Dockerfile",
            Language::Markdown => "Markdown",
            Language::Custom(def) => def.name(),
        }
    }

    /// All built-in languages in a sensible menu order.
    ///
    /// [`Language::Custom`] is excluded — it holds an `Arc` and has no
    /// canonical instance. A `Language::ALL` associated constant is not
    /// possible for the same reason, so this returns a borrowed slice from
    /// a private `const` table instead.
    pub fn builtins() -> &'static [Language] {
        const ALL: &[Language] = &[
            Language::Rust,
            Language::Toml,
            Language::Ron,
            Language::Rhai,
            Language::Json,
            Language::Yaml,
            Language::Xml,
            Language::Asm,
            Language::Hex,
            Language::Sql,
            Language::Diff,
            Language::Ini,
            Language::Dockerfile,
            Language::Markdown,
            Language::None,
        ];
        ALL
    }

    /// Map a file extension to a built-in language.
    ///
    /// Case-insensitive; accepts the extension with or without a leading
    /// dot (`"rs"`, `".RS"`). Returns `None` for plain-text (`txt`) and
    /// unrecognised extensions.
    pub fn from_extension(ext: &str) -> Option<Language> {
        let ext = ext.strip_prefix('.').unwrap_or(ext).to_ascii_lowercase();
        Some(match ext.as_str() {
            "rs" => Language::Rust,
            "toml" => Language::Toml,
            "ron" => Language::Ron,
            "rhai" => Language::Rhai,
            "json" | "jsonc" | "json5" => Language::Json,
            "yaml" | "yml" => Language::Yaml,
            "xml" | "html" | "htm" | "svg" | "xhtml" => Language::Xml,
            "s" | "asm" | "nasm" => Language::Asm,
            "hex" => Language::Hex,
            "sql" => Language::Sql,
            "diff" | "patch" => Language::Diff,
            "ini" | "cfg" | "conf" => Language::Ini,
            "dockerfile" => Language::Dockerfile,
            "md" | "markdown" | "mdown" | "mkd" => Language::Markdown,
            _ => return None,
        })
    }

    /// Detect the language from a file path.
    ///
    /// A filename whose stem is `Dockerfile` (case-insensitive, any
    /// directory) maps to [`Language::Dockerfile`] — this covers both the
    /// bare `Dockerfile` and the common `Dockerfile.dev` / `Dockerfile.prod`
    /// variants. Otherwise the file extension is looked up via
    /// [`Language::from_extension`] (so the `api.dockerfile` form also works).
    pub fn from_path(path: &str) -> Option<Language> {
        let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let stem = file.split('.').next().unwrap_or(file);
        if stem.eq_ignore_ascii_case("Dockerfile") {
            return Some(Language::Dockerfile);
        }
        let ext = file.rsplit_once('.').map(|(_, e)| e)?;
        Language::from_extension(ext)
    }
}

// Re-export for convenience (backward compat with old `tokenizer` module).
pub use super::token::{Token, TokenKind};

// ── Shared helpers ──────────────────────────────────────────────────────────

mod common;

pub(in crate::code_editor::lang) use common::{
    NumberOpts, consume_char_literal, consume_number, scan_block_comment,
};
pub(crate) use common::{is_ident_continue, is_ident_start};

// ── Plain text "language" ───────────────────────────────────────────────────

/// No-op highlighter for plain text.
pub struct PlainTextLang;

impl SyntaxDefinition for PlainTextLang {
    fn name(&self) -> &str {
        "Plain Text"
    }

    fn tokenize_line(&self, line: &str, _state: LineState) -> (Vec<Token>, LineState) {
        if line.is_empty() {
            (vec![], LineState::Code)
        } else {
            (
                vec![Token {
                    kind: TokenKind::Identifier,
                    start: 0,
                    len: line.len(),
                }],
                LineState::Code,
            )
        }
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        None
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        None
    }
    fn bracket_pairs(&self) -> &[(char, char)] {
        &[]
    }
    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }
    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[]
    }
}

// ── Dispatch functions ──────────────────────────────────────────────────────

/// Tokenize a single line of source code.
///
/// Dispatches to the appropriate built-in tokenizer or custom definition.
/// This is the hot-path function called per visible line each frame —
/// dispatch is via direct match (no vtable) for built-in languages.
pub fn tokenize_line(line: &str, language: &Language, state: LineState) -> (Vec<Token>, LineState) {
    match language {
        Language::None => PlainTextLang.tokenize_line(line, state),
        Language::Rust => rust::RustLang.tokenize_line(line, state),
        Language::Ron => ron::RonLang.tokenize_line(line, state),
        Language::Rhai => rhai::RhaiLang.tokenize_line(line, state),
        Language::Toml => toml::TomlLang.tokenize_line(line, state),
        Language::Json => json::JsonLang.tokenize_line(line, state),
        Language::Yaml => yaml::YamlLang.tokenize_line(line, state),
        Language::Xml => xml::XmlLang.tokenize_line(line, state),
        Language::Hex => hex::HexLang.tokenize_line(line, state),
        Language::Asm => asm::AsmLang.tokenize_line(line, state),
        Language::Sql => sql::SqlLang.tokenize_line(line, state),
        Language::Diff => diff::DiffLang.tokenize_line(line, state),
        Language::Ini => ini::IniLang.tokenize_line(line, state),
        Language::Dockerfile => dockerfile::DockerfileLang.tokenize_line(line, state),
        Language::Markdown => markdown::MarkdownLang.tokenize_line(line, state),
        Language::Custom(def) => def.tokenize_line(line, state),
    }
}

/// Get the [`SyntaxDefinition`] for a language.
///
/// Returns a reference to a zero-size static instance for built-in languages,
/// or to the inner `Arc` for [`Language::Custom`].  Use this for metadata
/// queries (bracket pairs, comment delimiters, auto-indent rules) — the
/// vtable overhead is irrelevant for these cold-path calls.
pub fn definition(language: &Language) -> &dyn SyntaxDefinition {
    match language {
        Language::None => &PlainTextLang,
        Language::Rust => &rust::RustLang,
        Language::Ron => &ron::RonLang,
        Language::Rhai => &rhai::RhaiLang,
        Language::Toml => &toml::TomlLang,
        Language::Json => &json::JsonLang,
        Language::Yaml => &yaml::YamlLang,
        Language::Xml => &xml::XmlLang,
        Language::Hex => &hex::HexLang,
        Language::Asm => &asm::AsmLang,
        Language::Sql => &sql::SqlLang,
        Language::Diff => &diff::DiffLang,
        Language::Ini => &ini::IniLang,
        Language::Dockerfile => &dockerfile::DockerfileLang,
        Language::Markdown => &markdown::MarkdownLang,
        Language::Custom(def) => def.as_ref(),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
