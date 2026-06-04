//! Language definitions, syntax traits, and font management for code_editor.
//!
//! Each built-in language is a unit struct implementing [`SyntaxDefinition`].
//! The [`tokenize_line`] function dispatches to the correct tokenizer,
//! and [`definition`] returns the full language metadata (bracket pairs,
//! auto-indent rules, comment delimiters, etc.).

pub mod asm;
pub mod hex;
pub mod json;
pub mod rhai;
pub mod ron;
pub mod rust;
pub mod toml;
pub mod xml;
pub mod yaml;

use std::sync::Arc;

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

    fn tokenize_line(&self, line: &str, _in_block_comment: bool) -> (Vec<Token>, bool) {
        if line.is_empty() {
            (vec![], false)
        } else {
            (
                vec![Token {
                    kind: TokenKind::Identifier,
                    start: 0,
                    len: line.len(),
                }],
                false,
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
pub fn tokenize_line(
    line: &str,
    language: &Language,
    in_block_comment: bool,
) -> (Vec<Token>, bool) {
    match language {
        Language::None => PlainTextLang.tokenize_line(line, in_block_comment),
        Language::Rust => rust::RustLang.tokenize_line(line, in_block_comment),
        Language::Ron => ron::RonLang.tokenize_line(line, in_block_comment),
        Language::Rhai => rhai::RhaiLang.tokenize_line(line, in_block_comment),
        Language::Toml => toml::TomlLang.tokenize_line(line, in_block_comment),
        Language::Json => json::JsonLang.tokenize_line(line, in_block_comment),
        Language::Yaml => yaml::YamlLang.tokenize_line(line, in_block_comment),
        Language::Xml => xml::XmlLang.tokenize_line(line, in_block_comment),
        Language::Hex => hex::HexLang.tokenize_line(line, in_block_comment),
        Language::Asm => asm::AsmLang.tokenize_line(line, in_block_comment),
        Language::Custom(def) => def.tokenize_line(line, in_block_comment),
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
        Language::Custom(def) => def.as_ref(),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_line_all_langs() {
        for lang in [
            Language::None,
            Language::Rust,
            Language::Ron,
            Language::Rhai,
            Language::Toml,
            Language::Json,
            Language::Yaml,
            Language::Xml,
            Language::Hex,
            Language::Asm,
        ] {
            let (toks, _) = tokenize_line("", &lang, false);
            assert!(
                toks.is_empty(),
                "non-empty tokens for {:?} on empty line",
                lang
            );
        }
    }

    #[test]
    fn test_plain_text() {
        let (toks, bc) = tokenize_line("hello world", &Language::None, false);
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].kind, TokenKind::Identifier);
        assert!(!bc);
    }

    #[test]
    fn test_definition_names() {
        assert_eq!(definition(&Language::None).name(), "Plain Text");
        assert_eq!(definition(&Language::Rust).name(), "Rust");
        assert_eq!(definition(&Language::Ron).name(), "RON");
        assert_eq!(definition(&Language::Rhai).name(), "Rhai");
        assert_eq!(definition(&Language::Toml).name(), "TOML");
        assert_eq!(definition(&Language::Json).name(), "JSON");
        assert_eq!(definition(&Language::Yaml).name(), "YAML");
        assert_eq!(definition(&Language::Xml).name(), "XML");
        assert_eq!(definition(&Language::Hex).name(), "Hex");
        assert_eq!(definition(&Language::Asm).name(), "Assembly");
    }

    #[test]
    fn test_definition_bracket_pairs() {
        let pairs = definition(&Language::Rust).bracket_pairs();
        assert!(pairs.contains(&('(', ')')));
        assert!(pairs.contains(&('{', '}')));

        let xml_pairs = definition(&Language::Xml).bracket_pairs();
        assert!(xml_pairs.contains(&('<', '>')));

        let plain_pairs = definition(&Language::None).bracket_pairs();
        assert!(plain_pairs.is_empty());
    }

    #[test]
    fn test_definition_comment_delimiters() {
        assert_eq!(
            definition(&Language::Rust).line_comment_prefix(),
            Some("//")
        );
        assert_eq!(definition(&Language::Toml).line_comment_prefix(), Some("#"));
        assert_eq!(definition(&Language::Yaml).line_comment_prefix(), Some("#"));
        assert_eq!(definition(&Language::Xml).line_comment_prefix(), None);
        assert_eq!(definition(&Language::None).line_comment_prefix(), None);

        assert_eq!(
            definition(&Language::Rust).block_comment_delimiters(),
            Some(("/*", "*/"))
        );
        assert_eq!(
            definition(&Language::Xml).block_comment_delimiters(),
            Some(("<!--", "-->"))
        );
        assert!(
            definition(&Language::Yaml)
                .block_comment_delimiters()
                .is_none()
        );
    }

    #[test]
    fn test_covers_full_line_rust() {
        let line = "pub fn foo(x: i32) -> bool { true }";
        let (toks, _) = tokenize_line(line, &Language::Rust, false);
        let total: usize = toks.iter().map(|t| t.len).sum();
        assert_eq!(total, line.len());
    }

    const ALL_LANGS: &[Language] = &[
        Language::None,
        Language::Rust,
        Language::Ron,
        Language::Rhai,
        Language::Toml,
        Language::Json,
        Language::Yaml,
        Language::Xml,
        Language::Hex,
        Language::Asm,
    ];

    /// Adversarial inputs must never panic and must always produce tokens
    /// whose byte spans exactly tile the line (contiguous, no gaps, no
    /// overlaps, total == line length). This is the single most important
    /// invariant: the renderer slices the line by these spans, so any gap
    /// or overlap is a UTF-8 panic waiting to happen.
    #[test]
    fn all_langs_no_panic_full_coverage_both_states() {
        let samples = [
            "",
            " ",
            "\t\t",
            "/* unterminated",
            "*/ stray close",
            "\"unterminated string",
            "'unterminated char",
            "r#\"unterminated raw",
            "你好世界 — 多字节",
            "😀😀😀",
            "0x 0b 0o 1_2_3 1.2e+3 .5 1..2 1..=3",
            "#![attr(unclosed",
            "<!-- xml unclosed",
            "<![CDATA[ unclosed",
            "key: value # comment",
            "DE AD BE EF GG",
            "&amp; <b>x</b>",
            "let x = b'\\x41'; // mix",
            "\\\\\\ stray backslashes",
            ":::,,,...===!!!",
        ];
        for lang in ALL_LANGS {
            for &in_bc in &[false, true] {
                for s in &samples {
                    let (toks, _) = tokenize_line(s, lang, in_bc);
                    let mut pos = 0usize;
                    for t in &toks {
                        assert_eq!(
                            t.start, pos,
                            "non-contiguous span in {lang:?} (bc={in_bc}) on {s:?}: {toks:?}"
                        );
                        // Spans must land on char boundaries so the renderer
                        // can slice without panicking.
                        assert!(
                            s.is_char_boundary(t.start) && s.is_char_boundary(t.start + t.len),
                            "span not on char boundary in {lang:?} on {s:?}"
                        );
                        pos += t.len;
                    }
                    assert_eq!(
                        pos,
                        s.len(),
                        "span total != line len in {lang:?} (bc={in_bc}) on {s:?}: {toks:?}"
                    );
                }
            }
        }
    }

    /// Block-comment carry state must be idempotent in the sense that a
    /// language advertising no block comments never reports "still in a
    /// block comment".
    #[test]
    fn non_block_comment_langs_never_carry_state() {
        for lang in [Language::Toml, Language::Yaml, Language::Asm, Language::Hex] {
            for line in ["/* not a comment here */", "x */", "/* x"] {
                let (_, carry) = tokenize_line(line, &lang, false);
                assert!(!carry, "{lang:?} unexpectedly carried block-comment state");
            }
        }
    }
}
