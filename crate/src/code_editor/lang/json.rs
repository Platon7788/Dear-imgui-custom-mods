//! JSON / JSONC tokenizer.
//!
//! Keys are highlighted as [`TokenKind::Attribute`] (distinguished from string
//! values by lookahead for `:`).  JSONC-style `//` line comments are supported.

use super::{NumberOpts, consume_number, is_ident_continue, is_ident_start};
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
            loop {
                if i >= len {
                    break;
                }
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    in_block_comment = false;
                    break;
                }
                i += 1;
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
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Whitespace,
                start,
                len: i - start,
            });
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
            in_block_comment = true;
            loop {
                if i >= len {
                    break;
                }
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    in_block_comment = false;
                    break;
                }
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Comment,
                start,
                len: i - start,
            });
            continue;
        }

        // ── String (key or value) ────────────────────────────────────────
        if b == b'"' {
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                } else if bytes[i] == b'"' {
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

        // ── Number ───────────────────────────────────────────────────────
        // JSON-spec strict: decimal only, no underscore separators,
        // no radix prefixes. `1_000` would not be a single Number;
        // the `_000` portion would fall through to identifier handling.
        if b.is_ascii_digit() || (b == b'-' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            if b == b'-' {
                i += 1;
            }
            consume_number(&mut i, bytes, NumberOpts::JSON);
            tokens.push(Token {
                kind: TokenKind::Number,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Identifier / keyword ─────────────────────────────────────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            let kind = if KEYWORDS.contains(&word) {
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

    /// JSON spec rejects radix prefixes — `0x10` is `0` then identifier.
    #[test]
    fn no_radix_prefixes() {
        let toks = tok("0x10");
        let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
        // Only `0` parses as a number.
        assert_eq!(nums.len(), 1);
        assert_eq!(nums[0].1, "0");
    }

    /// JSON does support exponent and decimal-point floats.
    #[test]
    fn decimal_with_exponent() {
        let toks = tok("-3.14e+2");
        let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
        assert_eq!(nums.len(), 1);
        assert_eq!(nums[0].1, "-3.14e+2");
    }
}
