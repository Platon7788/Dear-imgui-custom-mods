//! JSON / JSONC / JSON5 tokenizer.
//!
//! Keys are highlighted as [`TokenKind::Attribute`] (distinguished from string
//! values by lookahead for `:`), including JSON5 single-quoted and unquoted
//! identifier keys. JSONC-style `//` and `/* */` comments are supported (the
//! block-comment carry threads through [`LineState::BlockComment`]). JSON5
//! extras: single-quoted strings, `Infinity` / `NaN`, hex `0x` literals,
//! leading `+`, and bare-leading (`.5`) / trailing (`5.`) decimal points.

use super::{NumberOpts, consume_number, is_ident_continue, is_ident_start, scan_until, scan_ws};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

const KEYWORDS: &[&str] = &["true", "false", "null"];

// ── Language definition ─────────────────────────────────────────────────────

pub struct JsonLang;

impl SyntaxDefinition for JsonLang {
    fn name(&self) -> &str {
        "JSON"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        // JSONC `/* */` comments do not nest, so the carry is a plain
        // "inside a block comment" flag mapped to/from `LineState`.
        let in_bc = matches!(state, LineState::BlockComment(_));
        let (tokens, still_in_block) = tokenize(line, in_bc);
        let end = if still_in_block {
            LineState::BlockComment(1)
        } else {
            LineState::Code
        };
        (tokens, end)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("//")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        Some(("/*", "*/"))
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('{', '}'), ('[', ']')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &['{', '[']
    }
    fn auto_dedent_on(&self) -> &[char] {
        &['}', ']']
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("{", "}"), ("[", "]"), ("\"", "\"")]
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str, mut in_block_comment: bool) -> (Vec<Token>, bool) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;

    while i < len {
        // ── Inside a JSONC block comment (non-nesting) ───────────────────
        if in_block_comment {
            let start = i;
            if scan_until(bytes, &mut i, b"*/") {
                in_block_comment = false;
            }
            tokens.push(Token {
                kind: TokenKind::Comment,
                start,
                len: i - start,
            });
            continue;
        }

        let b = bytes[i];

        // ── Whitespace ───────────────────────────────────────────────────
        if b == b' ' || b == b'\t' {
            scan_ws(&mut tokens, bytes, &mut i);
            continue;
        }

        // ── Line comment (JSONC) ─────────────────────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return (tokens, false);
        }

        // ── Block comment start (JSONC, non-nesting) ─────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            in_block_comment = !scan_until(bytes, &mut i, b"*/");
            tokens.push(Token {
                kind: TokenKind::Comment,
                start,
                len: i - start,
            });
            continue;
        }

        // ── String (key or value) — JSON5 allows single quotes too ───────
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                } else if bytes[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            // Key vs value: look ahead past whitespace for `:`
            let mut j = i;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let kind = if j < len && bytes[j] == b':' {
                TokenKind::Attribute // JSON key
            } else {
                TokenKind::String // JSON string value
            };
            tokens.push(Token {
                kind,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Number (JSON5) ───────────────────────────────────────────────
        // Digits, hex `0x`, leading `+`/`-`, bare-leading `.5`, trailing
        // `5.`, plus the `Infinity` / `NaN` keywords (checked before the
        // identifier branch so they colour as numbers). A `+`/`-`/`.` that
        // does not begin a literal falls through to the branches below.
        if (b.is_ascii_digit()
            || b == b'+'
            || b == b'-'
            || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit())
            || bytes[i..].starts_with(b"Infinity")
            || bytes[i..].starts_with(b"NaN"))
            && let Some(end) = consume_json5_number(line, i)
        {
            tokens.push(Token {
                kind: TokenKind::Number,
                start: i,
                len: end - i,
            });
            i = end;
            continue;
        }

        // ── Identifier / keyword / unquoted key (JSON5) ──────────────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            // An unquoted identifier followed by `:` is a JSON5 object key.
            let mut j = i;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let kind = if j < len && bytes[j] == b':' {
                TokenKind::Attribute
            } else if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else {
                TokenKind::Identifier
            };
            tokens.push(Token {
                kind,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Colon ────────────────────────────────────────────────────────
        if b == b':' {
            tokens.push(Token {
                kind: TokenKind::Operator,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Punctuation ──────────────────────────────────────────────────
        if matches!(b, b'{' | b'}' | b'[' | b']' | b',') {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Fallback ─────────────────────────────────────────────────────
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        tokens.push(Token {
            kind: TokenKind::Identifier,
            start: i,
            len: ch_len,
        });
        i += ch_len;
    }

    (tokens, in_block_comment)
}

/// Try to consume a JSON5 number starting at `start`.
///
/// Handles an optional leading `+`/`-`, the `Infinity` / `NaN` keywords (as
/// whole words), hex `0x` literals, decimals with a bare-leading (`.5`) or
/// trailing (`5.`) point, and exponents. Returns the end byte index past the
/// literal, or `None` when the bytes do not form a number (the caller then
/// falls through to other branches).
fn consume_json5_number(line: &str, start: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = start;

    // Optional sign.
    if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }

    // `Infinity` / `NaN` — only when a whole word (not an identifier prefix).
    for word in [b"Infinity".as_slice(), b"NaN".as_slice()] {
        if bytes[i..].starts_with(word) {
            let end = i + word.len();
            if end >= len || !is_ident_continue(bytes[end]) {
                return Some(end);
            }
        }
    }

    // Numeric body.
    let opts = NumberOpts {
        underscore: false,
        radix: true,
        float: true,
    };
    let body = i;
    if i < len && bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
        // Bare-leading `.5`.
        consume_number(&mut i, bytes, opts);
    } else if i < len && bytes[i].is_ascii_digit() {
        let is_hex = bytes[i] == b'0' && i + 1 < len && matches!(bytes[i + 1], b'x' | b'X');
        consume_number(&mut i, bytes, opts);
        // Trailing bare `.` (`5.`) — consume_number leaves the dot when no
        // fractional digit follows. Include it and any exponent.
        if !is_hex && i < len && bytes[i] == b'.' {
            i += 1;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
                i += 1;
                if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                    i += 1;
                }
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
        }
    }

    if i > body { Some(i) } else { None }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::{Language, LineState};
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn tok(line: &str) -> Vec<(TokenKind, String)> {
        let (tokens, _) = tokenize_line(line, &Language::Json, LineState::Code);
        tokens
            .iter()
            .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
            .collect()
    }

    #[test]
    fn key_value() {
        let toks = tok(r#"  "name": "hello""#);
        let attrs: Vec<_> = toks
            .iter()
            .filter(|t| t.0 == TokenKind::Attribute)
            .collect();
        let strings: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::String).collect();
        assert_eq!(attrs.len(), 1);
        assert_eq!(strings.len(), 1);
        assert_eq!(attrs[0].1, r#""name""#);
        assert_eq!(strings[0].1, r#""hello""#);
    }

    #[test]
    fn keywords() {
        let toks = tok("true, false, null");
        let kws: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Keyword).collect();
        assert_eq!(kws.len(), 3);
    }

    #[test]
    fn numbers() {
        let toks = tok("42, -3.14, 1e10");
        let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
        assert_eq!(nums.len(), 3);
    }

    #[test]
    fn jsonc_comment() {
        let toks = tok("// this is a comment");
        assert_eq!(toks[0].0, TokenKind::Comment);
    }

    /// JSONC `/* */` block comments are now highlighted, including the
    /// multi-line carry-over via `in_block_comment`.
    #[test]
    fn jsonc_block_comment() {
        let toks = tok("/* inline */ 42");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Comment && t.1 == "/* inline */")
        );
        assert!(toks.iter().any(|t| t.0 == TokenKind::Number && t.1 == "42"));

        // Multi-line: open on one line, close on the next.
        let (_, still_in) = tokenize_line("/* start", &Language::Json, LineState::Code);
        assert_eq!(still_in, LineState::BlockComment(1));
        let (toks2, done) = tokenize_line("end */ 1", &Language::Json, LineState::BlockComment(1));
        assert_eq!(done, LineState::Code);
        assert_eq!(toks2[0].kind, TokenKind::Comment);
        assert_eq!(&"end */ 1"[..toks2[0].len], "end */");
    }

    #[test]
    fn nested_structure() {
        let toks = tok(r#"{"a": [1, 2]}"#);
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == r#""a""#)
        );
        assert!(toks.iter().any(|t| t.0 == TokenKind::Number && t.1 == "1"));
    }

    /// JSON spec is strict: `_` is **not** a valid digit separator.
    /// `1_000` should NOT tokenize as a single number — the `_000`
    /// portion falls through to identifier handling.
    /// Regression for ADR-027 phase 2.
    #[test]
    fn no_underscore_separators() {
        let toks = tok("1_000");
        // First token: Number "1"
        let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
        assert_eq!(nums.len(), 1);
        assert_eq!(nums[0].1, "1");
        // The `_000` lands in an identifier-ish bucket (TokenKind::Identifier).
        assert!(toks.iter().any(|t| t.1.starts_with('_')));
    }

    /// JSON does support exponent and decimal-point floats.
    #[test]
    fn decimal_with_exponent() {
        let toks = tok("-3.14e+2");
        let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
        assert_eq!(nums.len(), 1);
        assert_eq!(nums[0].1, "-3.14e+2");
    }

    fn only_number(line: &str) -> Vec<String> {
        tok(line)
            .into_iter()
            .filter(|t| t.0 == TokenKind::Number)
            .map(|t| t.1)
            .collect()
    }

    /// JSON5 single-quoted strings: `'a'` is a key (followed by `:`), `'b'`
    /// is a value.
    #[test]
    fn json5_single_quoted_string() {
        let toks = tok(r#"{ 'a': 'b' }"#);
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "'a'")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::String && t.1 == "'b'")
        );
    }

    /// JSON5 unquoted identifier keys colour as [`TokenKind::Attribute`].
    #[test]
    fn json5_unquoted_key() {
        let toks = tok("{ key: 1 }");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "key")
        );
        assert!(toks.iter().any(|t| t.0 == TokenKind::Number && t.1 == "1"));
    }

    /// JSON5 `Infinity` / `NaN` (with optional sign) tokenize as numbers, but
    /// a longer identifier such as `NaNoTech` does not.
    #[test]
    fn json5_infinity_and_nan() {
        for lit in ["Infinity", "-Infinity", "+Infinity", "NaN", "-NaN"] {
            assert_eq!(only_number(lit), vec![lit.to_string()], "input {lit:?}");
        }
        let toks = tok("NaNoTech");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Identifier));
        assert!(!toks.iter().any(|t| t.0 == TokenKind::Number));
    }

    /// JSON5 numeric extras: hex, leading `+`, bare-leading `.5`, trailing `5.`.
    #[test]
    fn json5_number_forms() {
        for (line, lit) in [("0x1F", "0x1F"), ("+5", "+5"), (".5", ".5"), ("5.", "5.")] {
            assert_eq!(only_number(line), vec![lit.to_string()], "input {line:?}");
        }
    }
}
