//! Language definitions for syntax highlighting.
//!
//! Each built-in language is a unit struct implementing [`SyntaxDefinition`].
//! The [`tokenize_line`] function dispatches to the correct tokenizer,
//! and [`definition`] returns the full language metadata (bracket pairs,
//! auto-indent rules, comment delimiters, etc.).

pub mod asm;
pub mod hex;
pub mod json;
pub mod rhai;
pub mod rust;
pub mod toml;
pub mod xml;
pub mod yaml;

use super::config::{Language, SyntaxDefinition};

// Re-export for convenience (backward compat with old `tokenizer` module).
pub use super::token::{Token, TokenKind};

// ── Shared helpers ──────────────────────────────────────────────────────────

/// ASCII letter or `_`.
#[inline]
pub(crate) fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// ASCII alphanumeric or `_`.
#[inline]
pub(crate) fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Consume a decimal number (integer, float with `.`, exponent with `e/E`).
/// Advances `*i` past the number.  Underscores are allowed as separators.
pub(crate) fn consume_decimal(i: &mut usize, bytes: &[u8]) {
    let len = bytes.len();
    while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
        *i += 1;
    }
    // Decimal point
    if *i < len && bytes[*i] == b'.' && *i + 1 < len && bytes[*i + 1].is_ascii_digit() {
        *i += 1;
        while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
            *i += 1;
        }
    }
    // Exponent
    if *i < len && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        *i += 1;
        if *i < len && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
            *i += 1;
        }
    }
}

// ── Plain text "language" ───────────────────────────────────────────────────

/// No-op highlighter for plain text.
pub struct PlainTextLang;

impl SyntaxDefinition for PlainTextLang {
    fn name(&self) -> &str { "Plain Text" }

    fn tokenize_line(&self, line: &str, _in_block_comment: bool) -> (Vec<Token>, bool) {
        if line.is_empty() {
            (vec![], false)
        } else {
            (vec![Token { kind: TokenKind::Identifier, start: 0, len: line.len() }], false)
        }
    }

    fn line_comment_prefix(&self) -> Option<&str> { None }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> { None }
    fn bracket_pairs(&self) -> &[(char, char)] { &[] }
    fn auto_indent_after(&self) -> &[char] { &[] }
    fn auto_dedent_on(&self) -> &[char] { &[] }
    fn auto_close_pairs(&self) -> &[(&str, &str)] { &[] }
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
        Language::None                 => PlainTextLang.tokenize_line(line, in_block_comment),
        Language::Rust | Language::Ron => rust::RustLang.tokenize_line(line, in_block_comment),
        Language::Rhai                 => rhai::RhaiLang.tokenize_line(line, in_block_comment),
        Language::Toml                 => toml::TomlLang.tokenize_line(line, in_block_comment),
        Language::Json                 => json::JsonLang.tokenize_line(line, in_block_comment),
        Language::Yaml                 => yaml::YamlLang.tokenize_line(line, in_block_comment),
        Language::Xml                  => xml::XmlLang.tokenize_line(line, in_block_comment),
        Language::Hex                  => hex::HexLang.tokenize_line(line, in_block_comment),
        Language::Asm                  => asm::AsmLang.tokenize_line(line, in_block_comment),
        Language::Custom(def)          => def.tokenize_line(line, in_block_comment),
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
        Language::None                 => &PlainTextLang,
        Language::Rust | Language::Ron => &rust::RustLang,
        Language::Rhai                 => &rhai::RhaiLang,
        Language::Toml                 => &toml::TomlLang,
        Language::Json                 => &json::JsonLang,
        Language::Yaml                 => &yaml::YamlLang,
        Language::Xml                  => &xml::XmlLang,
        Language::Hex                  => &hex::HexLang,
        Language::Asm                  => &asm::AsmLang,
        Language::Custom(def)          => def.as_ref(),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_editor::config::Language;

    #[test]
    fn test_empty_line_all_langs() {
        for lang in [
            Language::None, Language::Rust, Language::Ron, Language::Rhai,
            Language::Toml, Language::Json, Language::Yaml, Language::Xml,
            Language::Hex, Language::Asm,
        ] {
            let (toks, _) = tokenize_line("", &lang, false);
            assert!(toks.is_empty(), "non-empty tokens for {:?} on empty line", lang);
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
        assert_eq!(definition(&Language::Ron).name(), "Rust");
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
        assert_eq!(definition(&Language::Rust).line_comment_prefix(), Some("//"));
        assert_eq!(definition(&Language::Toml).line_comment_prefix(), Some("#"));
        assert_eq!(definition(&Language::Yaml).line_comment_prefix(), Some("#"));
        assert_eq!(definition(&Language::Xml).line_comment_prefix(), None);
        assert_eq!(definition(&Language::None).line_comment_prefix(), None);

        assert_eq!(definition(&Language::Rust).block_comment_delimiters(), Some(("/*", "*/")));
        assert_eq!(definition(&Language::Xml).block_comment_delimiters(), Some(("<!--", "-->")));
        assert!(definition(&Language::Yaml).block_comment_delimiters().is_none());
    }

    #[test]
    fn test_covers_full_line_rust() {
        let line = "pub fn foo(x: i32) -> bool { true }";
        let (toks, _) = tokenize_line(line, &Language::Rust, false);
        let total: usize = toks.iter().map(|t| t.len).sum();
        assert_eq!(total, line.len());
    }
}
