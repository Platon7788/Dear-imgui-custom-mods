//! Unit tests for the YAML tokenizer. Split out of `tokenize.rs` to keep every
//! source file under the 500-line ceiling (CLAUDE.md).

use crate::code_editor::config::{Language, LineState};
use crate::code_editor::lang::tokenize_line;
use crate::code_editor::token::TokenKind;

fn tok(line: &str) -> Vec<(TokenKind, String)> {
    let (tokens, _) = tokenize_line(line, &Language::Yaml, LineState::Code);
    tokens
        .iter()
        .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
        .collect()
}

fn has(line: &str, kind: TokenKind, s: &str) -> bool {
    tok(line).iter().any(|(k, v)| *k == kind && v == s)
}

#[test]
fn hash_in_scalar_is_not_a_comment() {
    assert!(!has("url: http://x#frag", TokenKind::Comment, "#frag"));
    assert!(
        !tok("url: http://x#frag")
            .iter()
            .any(|(k, _)| *k == TokenKind::Comment)
    );
}

#[test]
fn indicators_and_markers() {
    assert!(has("body: |", TokenKind::Operator, "|"));
    assert!(has("---", TokenKind::Keyword, "---"));
    assert!(has("--- !tag", TokenKind::Keyword, "---"));
    assert!(tok("--- !tag").iter().any(|(_, s)| s.contains("tag")));
}

#[test]
fn key_value_and_nesting() {
    // `name` / `port` are mapping keys → Attribute; values keep their role.
    assert!(has("name: hello", TokenKind::Attribute, "name"));
    assert!(has("name: hello", TokenKind::Operator, ":"));
    assert!(has("name: hello", TokenKind::Identifier, "hello"));
    assert!(has("  port: 8080", TokenKind::Attribute, "port"));
    assert!(has("  port: 8080", TokenKind::Number, "8080"));
}

#[test]
fn bare_scalar_value_is_identifier() {
    assert!(has("- plain_value", TokenKind::Identifier, "plain_value"));
    assert!(
        !tok("- plain_value")
            .iter()
            .any(|t| t.0 == TokenKind::Attribute)
    );
    assert!(has("  - item", TokenKind::Punctuation, "-"));
}

#[test]
fn comment_line() {
    assert_eq!(tok("# this is a comment")[0].0, TokenKind::Comment);
}

#[test]
fn keywords_anchor_tag() {
    assert!(has("enabled: true", TokenKind::Keyword, "true"));
    assert!(has("base: &default", TokenKind::MacroCall, "&default"));
    assert!(has(
        "timestamp: !!timestamp 2024-01-01",
        TokenKind::TypeName,
        "!!timestamp"
    ));
}

#[test]
fn unterminated_quoted_no_panic() {
    assert!(
        tok(r#"key: "unclosed"#)
            .iter()
            .any(|t| t.0 == TokenKind::String)
    );
}

/// YAML 1.1 numbers: decimal, hex/octal/binary radix, `_` separators and
/// floats. The literal ends at whitespace or structural punctuation.
#[test]
fn radix_and_underscore_separators() {
    for (line, want) in [
        ("a: 0xDEAD_BEEF", "0xDEAD_BEEF"),
        ("a: 0o755", "0o755"),
        ("a: 0b1010", "0b1010"),
        ("a: 1_000_000", "1_000_000"),
        ("a: 3.14", "3.14"),
    ] {
        assert!(has(line, TokenKind::Number, want), "input {line:?}");
    }
}

/// `2:30` is a bare string, not a number followed by a colon.
#[test]
fn bare_string_not_number() {
    assert!(!tok("time: 2:30").iter().any(|t| t.0 == TokenKind::Number));
}

/// A single-quoted scalar treats a doubled `''` as one escaped quote:
/// `'it''s'` is a *single* string token, not two.
#[test]
fn single_quote_doubled_escape() {
    let strs: Vec<_> = tok("v: 'it''s'")
        .into_iter()
        .filter(|t| t.0 == TokenKind::String)
        .collect();
    assert_eq!(strs.len(), 1, "got {strs:?}");
    assert_eq!(strs[0].1, "'it''s'");
}

/// A `key: |` opens a block scalar whose body is carried across lines via
/// `LineState::YamlBlock`: indented / blank lines stay `String`; a dedent
/// exits back to `Code`.
#[test]
fn block_scalar_carry_state() {
    let (_, st) = tokenize_line("body: |", &Language::Yaml, LineState::Code);
    assert_eq!(st, LineState::YamlBlock { indent: 1 });

    // Indented body line stays a String and stays in the block.
    let (toks, st2) = tokenize_line("  hello world", &Language::Yaml, st);
    assert!(toks.iter().any(|t| t.kind == TokenKind::String));
    assert_eq!(st2, LineState::YamlBlock { indent: 1 });

    // Blank lines belong to the block.
    let (_, st3) = tokenize_line("", &Language::Yaml, st2);
    assert_eq!(st3, LineState::YamlBlock { indent: 1 });

    // A dedent (column 0) exits and re-tokenizes as ordinary YAML.
    let (toks4, st4) = tokenize_line("next: 1", &Language::Yaml, st3);
    assert_eq!(st4, LineState::Code);
    assert!(toks4.iter().any(|t| t.kind == TokenKind::Attribute));
}

/// The block-scalar body indent tracks the indicator line's indent, so a
/// sibling key dedents out while a more-indented line stays in.
#[test]
fn block_scalar_indent_tracks_key() {
    let (_, st) = tokenize_line("  data: >-", &Language::Yaml, LineState::Code);
    assert_eq!(st, LineState::YamlBlock { indent: 3 });

    let (_, sibling) = tokenize_line("  other: x", &Language::Yaml, st);
    assert_eq!(sibling, LineState::Code);

    let (toks, deep) = tokenize_line("      line", &Language::Yaml, st);
    assert!(toks.iter().any(|t| t.kind == TokenKind::String));
    assert_eq!(deep, LineState::YamlBlock { indent: 3 });
}
