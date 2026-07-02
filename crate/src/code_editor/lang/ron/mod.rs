//! RON (Rusty Object Notation) tokenizer.
//!
//! RON is a data-only configuration format with Rust-flavoured syntax. This
//! tokenizer is similar to [`super::rust`] but tuned for RON semantics:
//!
//! - `//` line comments and `/* */` block comments (multi-line, nesting-aware).
//! - String, raw-string (`r"..."`, `r#"..."#`) and char literals.
//! - Hex / octal / binary / decimal numbers with `_` separators; optional
//!   leading sign.
//! - `true` / `false` keywords (RON has no other reserved words).
//! - Identifiers starting with an uppercase letter render as
//!   [`TokenKind::TypeName`] — matches the struct / enum-variant convention
//!   (`Some`, `None`, `Foo`).
//! - Identifiers (and quoted strings) immediately followed by `:` render as
//!   [`TokenKind::Attribute`] — the field-key / map-key role.
//! - `#![enable(...)]` extension attributes render as a single
//!   [`TokenKind::Attribute`] block.

use super::{
    NumberOpts, consume_char_literal, consume_number, is_ident_continue, is_ident_start,
    scan_block_comment,
};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

mod tokenize;

pub(super) use tokenize::tokenize;

const KEYWORDS: &[&str] = &["true", "false"];

// ── Shared scan helpers (used by `tokenize`) ─────────────────────────────────

/// Push a token spanning `start..start+len` of the given `kind`.
fn push(tokens: &mut Vec<Token>, kind: TokenKind, start: usize, len: usize) {
    tokens.push(Token { kind, start, len });
}

/// Scan a double-quoted string body from `start` (just past the opening `"`),
/// honouring `\`-escapes. Returns `(end, closed)`: `end` is the byte index
/// just past the closing `"` (or `len` if unterminated) and `closed` says
/// whether the close was found on this line. A lone trailing `\` (Rust-style
/// line continuation) leaves the string open, so a multi-line string carries.
fn scan_dq_string_close(bytes: &[u8], start: usize) -> (usize, bool) {
    let len = bytes.len();
    let mut i = start;
    while i < len {
        if bytes[i] == b'\\' && i + 1 < len {
            i += 2;
        } else if bytes[i] == b'"' {
            return (i + 1, true);
        } else {
            i += 1;
        }
    }
    (len, false)
}

/// Scan a raw-string body from `start` (just past the opening `r#…#"`) for the
/// matching `"` followed by exactly `hashes` `#`. Returns `(end, closed)` —
/// see [`scan_dq_string_close`].
fn scan_raw_string_close(bytes: &[u8], start: usize, hashes: usize) -> (usize, bool) {
    let len = bytes.len();
    let mut i = start;
    while i < len {
        if bytes[i] == b'"' {
            let mut end_hashes = 0;
            let mut j = i + 1;
            while j < len && bytes[j] == b'#' && end_hashes < hashes {
                end_hashes += 1;
                j += 1;
            }
            if end_hashes == hashes {
                return (j, true);
            }
        }
        i += 1;
    }
    (len, false)
}

/// Byte length of a signed non-finite float (`+inf` / `-inf` / `+NaN` /
/// `-NaN`, RON's serialization of non-finite `f32`/`f64`) at `i`, or `None`.
/// Bare `inf` / `NaN` are classified in the identifier branch so a field-key
/// role (`inf:`) can win first.
fn signed_special_float_len(bytes: &[u8], i: usize) -> Option<usize> {
    if !matches!(bytes.get(i), Some(&b'+') | Some(&b'-')) || i + 4 > bytes.len() {
        return None;
    }
    let seg = &bytes[i + 1..i + 4];
    ((seg == b"inf" || seg == b"NaN") && (i + 4 == bytes.len() || !is_ident_continue(bytes[i + 4])))
        .then_some(4)
}

// ── Language definition ─────────────────────────────────────────────────────

pub struct RonLang;

impl SyntaxDefinition for RonLang {
    fn name(&self) -> &str {
        "RON"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        tokenize(line, state)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("//")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        Some(("/*", "*/"))
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('(', ')'), ('{', '}'), ('[', ']')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &['(', '{', '[']
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[')', '}', ']']
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("(", ")"), ("{", "}"), ("[", "]"), ("\"", "\""), ("'", "'")]
    }
}

// ── Tests for the LineState carry-state features ─────────────────────────────

#[cfg(test)]
mod carry_tests {
    use crate::code_editor::config::{Language, LineState};
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    /// A regular `"…"` string that doesn't close on its line carries a
    /// `Str` state; the next line continues (and eventually closes) it.
    #[test]
    fn multiline_string_opens_carries_and_closes() {
        let (t1, s1) = tokenize_line("name: \"hello", &Language::Ron, LineState::Code);
        assert!(matches!(
            s1,
            LineState::Str {
                quote: b'"',
                raw: false,
                triple: false,
                ..
            }
        ));
        assert!(t1.iter().any(|t| t.kind == TokenKind::String));
        // Line 2 in that state stays open (still no closing quote).
        let (t2, s2) = tokenize_line("still going", &Language::Ron, s1);
        assert_eq!(s2, s1);
        assert_eq!(t2[0].kind, TokenKind::String);
        // Line 3 closes the string.
        let (t3, s3) = tokenize_line("world\"", &Language::Ron, s2);
        assert_eq!(s3, LineState::Code);
        assert_eq!(t3[0].kind, TokenKind::String);
    }

    /// A raw string `r#"…` carries its hash count across lines and closes
    /// only on the matching `"#`.
    #[test]
    fn multiline_raw_string_carries_hashes() {
        let (_t1, s1) = tokenize_line("p: r#\"C:\\start", &Language::Ron, LineState::Code);
        assert!(matches!(
            s1,
            LineState::Str {
                raw: true,
                hashes: 1,
                triple: false,
                ..
            }
        ));
        // A bare `"` (without the trailing `#`) does NOT close it.
        let (_t2, s2) = tokenize_line("has a \" quote", &Language::Ron, s1);
        assert_eq!(s2, s1);
        // The matching `"#` closes it.
        let (t3, s3) = tokenize_line("end\"#", &Language::Ron, s2);
        assert_eq!(s3, LineState::Code);
        assert_eq!(t3[0].kind, TokenKind::String);
    }

    /// An empty line while a string is open keeps the state and emits no
    /// tokens (span-tiling stays trivially correct).
    #[test]
    fn empty_line_inside_string_keeps_state() {
        let (_t1, s1) = tokenize_line("x: \"open", &Language::Ron, LineState::Code);
        let (t2, s2) = tokenize_line("", &Language::Ron, s1);
        assert_eq!(s2, s1);
        assert!(t2.is_empty());
    }

    #[test]
    fn special_floats_are_numbers() {
        for (line, want) in [
            ("x: inf", "inf"),
            ("x: -inf", "-inf"),
            ("x: +inf", "+inf"),
            ("x: NaN", "NaN"),
        ] {
            let (toks, _) = tokenize_line(line, &Language::Ron, LineState::Code);
            let nums: Vec<_> = toks
                .iter()
                .filter(|t| t.kind == TokenKind::Number)
                .map(|t| &line[t.start..t.start + t.len])
                .collect();
            assert_eq!(nums, vec![want], "input {line:?}");
        }
    }

    /// A capitalized map/field key `Key:` is an Attribute (the colon-follows
    /// check runs before the uppercase→TypeName rule), matching string keys.
    #[test]
    fn capitalized_key_is_attribute_not_typename() {
        let (toks, _) = tokenize_line("Key: 5", &Language::Ron, LineState::Code);
        assert_eq!(toks[0].kind, TokenKind::Attribute);
        assert_eq!(toks[0].len, "Key".len());
        // A capitalized bare value (not followed by `:`) stays a TypeName.
        let (toks2, _) = tokenize_line("v: Some", &Language::Ron, LineState::Code);
        assert!(toks2.iter().any(|t| t.kind == TokenKind::TypeName));
    }
}
