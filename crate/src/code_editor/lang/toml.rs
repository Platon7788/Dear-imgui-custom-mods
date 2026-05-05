//! TOML configuration file tokenizer.

use super::{NumberOpts, consume_number, is_ident_continue, is_ident_start};
use crate::code_editor::config::SyntaxDefinition;
use crate::code_editor::token::{Token, TokenKind};

const KEYWORDS: &[&str] = &["true", "false"];

// ── Language definition ─────────────────────────────────────────────────────

pub struct TomlLang;

impl SyntaxDefinition for TomlLang {
    fn name(&self) -> &str {
        "TOML"
    }

    fn tokenize_line(&self, line: &str, _in_block_comment: bool) -> (Vec<Token>, bool) {
        (tokenize(line), false)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("#")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        None
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('[', ']'), ('{', '}')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")]
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(8);
    let mut i = 0;

    while i < len {
        let b = bytes[i];

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

        // Comment
        if b == b'#' {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return tokens;
        }

        // Section headers [section] or [[array.of.tables]]
        if b == b'[' {
            let start = i;
            let mut depth = 0u32;
            while i < len {
                match bytes[i] {
                    b'[' => depth += 1,
                    b']' => {
                        depth = depth.saturating_sub(1);
                        i += 1;
                        if depth == 0 {
                            break;
                        }
                        continue;
                    }
                    _ => {}
                }
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Attribute,
                start,
                len: i - start,
            });
            continue;
        }

        // String (double or single quote, including triple-quoted)
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::String,
                start,
                len: i - start,
            });
            continue;
        }

        // Number — TOML supports decimal, hex/oct/bin radix, underscore
        // separators and exponent (`1_000`, `0xDEAD_BEEF`, `0o755`,
        // `0b1010`, `1.5e10`).
        if b.is_ascii_digit()
            || ((b == b'-' || b == b'+') && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            if b == b'-' || b == b'+' {
                i += 1;
            }
            consume_number(&mut i, bytes, NumberOpts::RUST_LIKE);
            tokens.push(Token {
                kind: TokenKind::Number,
                start,
                len: i - start,
            });
            continue;
        }

        // Identifier / keyword (bare keys can contain `-`)
        if is_ident_start(b) {
            let start = i;
            while i < len && (is_ident_continue(bytes[i]) || bytes[i] == b'-') {
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

        // Operator (=)
        if b == b'=' {
            tokens.push(Token {
                kind: TokenKind::Operator,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // Punctuation
        if matches!(b, b'{' | b'}' | b',' | b'.' | b']') {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // Fallback
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        tokens.push(Token {
            kind: TokenKind::Identifier,
            start: i,
            len: ch_len,
        });
        i += ch_len;
    }

    tokens
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::Language;
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    #[test]
    fn section_header() {
        let (toks, _) = tokenize_line("[package]", &Language::Toml, false);
        assert_eq!(toks[0].kind, TokenKind::Attribute);
    }

    #[test]
    fn array_of_tables() {
        let (toks, _) = tokenize_line("[[dependencies.serde]]", &Language::Toml, false);
        assert_eq!(toks[0].kind, TokenKind::Attribute);
        // Should be a single token covering the full header
        assert_eq!(toks[0].len, "[[dependencies.serde]]".len());
    }

    #[test]
    fn key_value() {
        let (toks, _) = tokenize_line("name = \"hello\"", &Language::Toml, false);
        assert!(toks.iter().any(|t| t.kind == TokenKind::Identifier));
        assert!(toks.iter().any(|t| t.kind == TokenKind::String));
        assert!(toks.iter().any(|t| t.kind == TokenKind::Operator));
    }

    #[test]
    fn comment() {
        let (toks, _) = tokenize_line("# comment", &Language::Toml, false);
        assert_eq!(toks[0].kind, TokenKind::Comment);
    }

    #[test]
    fn bare_key_with_dash() {
        let (toks, _) = tokenize_line("my-key = 42", &Language::Toml, false);
        assert!(toks.iter().any(|t| t.kind == TokenKind::Identifier));
    }

    /// TOML supports hex/oct/bin radix, underscore separators and
    /// exponent. Regression for ADR-027 phase 2 — drift between
    /// `lang/*.rs` number tokenizers.
    #[test]
    fn radix_and_underscore_separators() {
        for (line, want_lit) in [
            ("a = 0xDEAD_BEEF", "0xDEAD_BEEF"),
            ("a = 0o755", "0o755"),
            ("a = 0b1010", "0b1010"),
            ("a = 1_000_000", "1_000_000"),
            ("a = 1.5e10", "1.5e10"),
            ("a = 3.14", "3.14"),
            ("a = -42", "-42"),
        ] {
            let (toks, _) = tokenize_line(line, &Language::Toml, false);
            let nums: Vec<_> = toks
                .iter()
                .filter(|t| t.kind == TokenKind::Number)
                .map(|t| &line[t.start..t.start + t.len])
                .collect();
            assert_eq!(nums, vec![want_lit], "input: {line:?}");
        }
    }
}
