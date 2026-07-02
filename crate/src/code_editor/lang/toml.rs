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

        // Section headers [section] or [[array.of.tables]] — ONLY when the
        // bracket is the first non-whitespace token on the line. A `[` after
        // `=` (or anywhere in a value) is an inline array, handled as
        // punctuation below — not swallowed as a header Attribute.
        if b == b'[' && tokens.iter().all(|t| t.kind == TokenKind::Whitespace) {
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

        // String (double or single quote). Single-line only — TOML's
        // triple-quoted multi-line strings are not tracked across lines (this
        // tokenizer is stateless), so `"""…"""` on one line reads as an empty
        // string followed by its content. Backslash escapes apply only to
        // basic (double-quoted) strings; literal (single-quoted) treat `\`
        // verbatim.
        if b == b'"' || b == b'\'' {
            let quote = b;
            let escapes = quote == b'"';
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote {
                if escapes && bytes[i] == b'\\' && i + 1 < len {
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

        // Identifier / keyword / bare key (bare keys can contain `-`).
        // A bare identifier followed (past whitespace) by `=` is a key —
        // classify it as Attribute to match the section-header / key role.
        if is_ident_start(b) {
            let start = i;
            while i < len && (is_ident_continue(bytes[i]) || bytes[i] == b'-') {
                i += 1;
            }
            let word = &line[start..i];
            let mut j = i;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let followed_by_eq = j < len && bytes[j] == b'=';
            let kind = if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if followed_by_eq {
                TokenKind::Attribute
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

        // Punctuation (incl. inline-array brackets in value position)
        if matches!(b, b'{' | b'}' | b',' | b'.' | b'[' | b']') {
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
    fn inline_array_value_is_not_a_section_header() {
        // Regression: `[` after `=` used to be scanned as a section header,
        // swallowing the whole array into one Attribute token.
        let line = r#"members = ["crate", "app"]"#;
        let (toks, _) = tokenize_line(line, &Language::Toml, false);
        let puncts: Vec<_> = toks
            .iter()
            .filter(|t| t.kind == TokenKind::Punctuation)
            .map(|t| &line[t.start..t.start + t.len])
            .collect();
        assert!(puncts.contains(&"["), "'[' in value position should be punctuation");
        assert!(puncts.contains(&"]"), "']' should be punctuation");
        assert!(toks.iter().any(|t| t.kind == TokenKind::String));
        // `members` (the key) is the only Attribute; no Attribute spans a bracket.
        assert!(
            !toks.iter().any(|t| t.kind == TokenKind::Attribute
                && line[t.start..t.start + t.len].contains('['))
        );
    }

    #[test]
    fn key_value() {
        let (toks, _) = tokenize_line("name = \"hello\"", &Language::Toml, false);
        // `name` is a key → Attribute.
        assert!(toks.iter().any(|t| t.kind == TokenKind::Attribute
            && &"name = \"hello\""[t.start..t.start + t.len] == "name"));
        assert!(toks.iter().any(|t| t.kind == TokenKind::String));
        assert!(toks.iter().any(|t| t.kind == TokenKind::Operator));
    }

    /// A bare key before `=` is an Attribute; a bare value after `=`
    /// stays an Identifier.
    #[test]
    fn bare_value_is_identifier() {
        let line = "color = red";
        let (toks, _) = tokenize_line(line, &Language::Toml, false);
        assert!(
            toks.iter()
                .any(|t| t.kind == TokenKind::Attribute
                    && &line[t.start..t.start + t.len] == "color")
        );
        assert!(
            toks.iter().any(
                |t| t.kind == TokenKind::Identifier && &line[t.start..t.start + t.len] == "red"
            )
        );
    }

    /// TOML literal (single-quoted) strings do not process `\` escapes:
    /// the backslash is part of the value and the string still closes at
    /// the single quote.
    #[test]
    fn literal_string_no_escape() {
        let line = r"path = 'C:\temp\new'";
        let (toks, _) = tokenize_line(line, &Language::Toml, false);
        assert!(
            toks.iter().any(|t| t.kind == TokenKind::String
                && &line[t.start..t.start + t.len] == r"'C:\temp\new'")
        );
    }

    #[test]
    fn comment() {
        let (toks, _) = tokenize_line("# comment", &Language::Toml, false);
        assert_eq!(toks[0].kind, TokenKind::Comment);
    }

    #[test]
    fn bare_key_with_dash() {
        let line = "my-key = 42";
        let (toks, _) = tokenize_line(line, &Language::Toml, false);
        // The dashed key before `=` is an Attribute spanning `my-key`.
        assert!(
            toks.iter()
                .any(|t| t.kind == TokenKind::Attribute
                    && &line[t.start..t.start + t.len] == "my-key")
        );
    }

    /// Sign + radix combinations. TOML accepts `+` and `-` as
    /// optional prefix on integers (incl. `+0xFF` per spec).
    /// Caller eats the sign, helper handles the radix body — single
    /// Number token covers both.
    #[test]
    fn signed_radix_combinations() {
        for (input, want_lit) in [
            ("x = -0xFF", "-0xFF"),
            ("x = +0xDEAD_BEEF", "+0xDEAD_BEEF"),
            ("x = +0b1010", "+0b1010"),
            ("x = -0o755", "-0o755"),
        ] {
            let (toks, _) = tokenize_line(input, &Language::Toml, false);
            let nums: Vec<_> = toks
                .iter()
                .filter(|t| t.kind == TokenKind::Number)
                .map(|t| &input[t.start..t.start + t.len])
                .collect();
            assert_eq!(nums, vec![want_lit], "input: {input:?}");
        }
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
