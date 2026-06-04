//! RON line tokenizer.

use super::*;

pub(in crate::code_editor::lang) fn tokenize(
    line: &str,
    in_block_comment: bool,
) -> (Vec<Token>, bool) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;
    let mut depth: u32 = u32::from(in_block_comment);

    while i < len {
        // ── Inside a (possibly nested) block comment ─────────────────────
        if depth > 0 {
            let start = i;
            depth = scan_block_comment(&mut i, bytes, depth);
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

        // ── Line comment ─────────────────────────────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return (tokens, false);
        }

        // ── Block comment start (nesting-aware) ──────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            depth = scan_block_comment(&mut i, bytes, 1);
            tokens.push(Token {
                kind: TokenKind::Comment,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Extension attribute: #![enable(...)] ─────────────────────────
        if b == b'#' && i + 1 < len && bytes[i + 1] == b'!' {
            let start = i;
            let mut bracket_depth = 0u32;
            while i < len {
                match bytes[i] {
                    b'[' => bracket_depth += 1,
                    b']' => {
                        bracket_depth = bracket_depth.saturating_sub(1);
                        if bracket_depth == 0 {
                            i += 1;
                            break;
                        }
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

        // ── String literal (also: map key when followed by `:`) ──────────
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
            // Lookahead past whitespace for `:` → map key
            let mut j = i;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let kind = if j < len && bytes[j] == b':' {
                TokenKind::Attribute
            } else {
                TokenKind::String
            };
            tokens.push(Token {
                kind,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Raw string (r"..." or r#"..."#) ──────────────────────────────
        if b == b'r' && i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') {
            let start = i;
            i += 1;
            let mut hashes = 0usize;
            while i < len && bytes[i] == b'#' {
                hashes += 1;
                i += 1;
            }
            if i < len && bytes[i] == b'"' {
                i += 1;
                'raw: loop {
                    if i >= len {
                        break;
                    }
                    if bytes[i] == b'"' {
                        let mut end_hashes = 0;
                        let mut j = i + 1;
                        while j < len && bytes[j] == b'#' && end_hashes < hashes {
                            end_hashes += 1;
                            j += 1;
                        }
                        if end_hashes == hashes {
                            i = j;
                            break 'raw;
                        }
                    }
                    i += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::String,
                    start,
                    len: i - start,
                });
                continue;
            }
            i = start; // not a raw string — fall through
        }

        // ── Char literal ─────────────────────────────────────────────────
        // Helper short-circuits on `bytes[i] != b'\''`, so the call is
        // a single byte compare in the common case.
        if let Some(end) = consume_char_literal(line, i) {
            tokens.push(Token {
                kind: TokenKind::CharLit,
                start: i,
                len: end - i,
            });
            i = end;
            continue;
        }

        // ── Number ───────────────────────────────────────────────────────
        if b.is_ascii_digit()
            || ((b == b'-' || b == b'+')
                && i + 1 < len
                && (bytes[i + 1].is_ascii_digit()
                    || (bytes[i + 1] == b'.' && i + 2 < len && bytes[i + 2].is_ascii_digit())))
            || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit())
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

        // ── Identifier / Keyword / Type / Field-key ──────────────────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            // Lookahead past whitespace for `:` → field / map key
            let mut j = i;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let followed_by_colon = j < len && bytes[j] == b':';

            let kind = if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if word.chars().next().is_some_and(|c| c.is_uppercase()) {
                TokenKind::TypeName
            } else if followed_by_colon {
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

        // ── Range operators (.., ..=) ────────────────────────────────────
        if b == b'.' && i + 1 < len && bytes[i + 1] == b'.' {
            let start = i;
            i += 2;
            if i < len && bytes[i] == b'=' {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Operator,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Operators (`:` separates key from value, `=` legacy) ─────────
        if matches!(b, b':' | b'=' | b'-' | b'+') {
            tokens.push(Token {
                kind: TokenKind::Operator,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Punctuation ──────────────────────────────────────────────────
        if matches!(
            b,
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';'
        ) {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Fallback: full Unicode scalar ────────────────────────────────
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        tokens.push(Token {
            kind: TokenKind::Identifier,
            start: i,
            len: ch_len,
        });
        i += ch_len;
    }

    (tokens, depth > 0)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::Language;
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn tok(line: &str) -> Vec<(TokenKind, String)> {
        let (tokens, _) = tokenize_line(line, &Language::Ron, false);
        tokens
            .iter()
            .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
            .collect()
    }

    #[test]
    fn keywords() {
        let toks = tok("enabled: true");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Keyword && t.1 == "true")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "enabled")
        );
    }

    #[test]
    fn struct_with_field_keys() {
        let toks = tok("GameConfig(width: 1920, title: \"Hi\")");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::TypeName && t.1 == "GameConfig")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "width")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "title")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Number && t.1 == "1920")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::String && t.1 == "\"Hi\"")
        );
    }

    #[test]
    fn enum_variants_are_typenames() {
        let toks = tok("value: Some(42)");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::TypeName && t.1 == "Some")
        );
        let toks = tok("value: None");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::TypeName && t.1 == "None")
        );
    }

    #[test]
    fn map_with_string_keys() {
        let toks = tok("\"key\": 42");
        assert_eq!(toks[0].0, TokenKind::Attribute);
        assert_eq!(toks[0].1, "\"key\"");
    }

    #[test]
    fn quoted_string_value() {
        let toks = tok("title: \"Hello\"");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::String && t.1 == "\"Hello\"")
        );
    }

    #[test]
    fn line_comment() {
        let toks = tok("x: 5 // comment");
        let last = toks.last().unwrap();
        assert_eq!(last.0, TokenKind::Comment);
        assert!(last.1.contains("comment"));
    }

    #[test]
    fn block_comment_multi_line() {
        let (toks, still_in) = tokenize_line("/* start", &Language::Ron, false);
        assert!(still_in);
        assert_eq!(toks[0].kind, TokenKind::Comment);

        let (toks2, still_in2) = tokenize_line("middle */ rest", &Language::Ron, true);
        assert!(!still_in2);
        assert_eq!(toks2[0].kind, TokenKind::Comment);
    }

    /// RON inherits Rust's nested block comments.
    #[test]
    fn nested_block_comment() {
        let (_, still_in) = tokenize_line("/* a /* b */ c */", &Language::Ron, false);
        assert!(!still_in, "balanced nest closes");
        let (_, still_in2) = tokenize_line("/* a /* b */", &Language::Ron, false);
        assert!(still_in2, "one level still open");
    }

    #[test]
    fn leading_plus_on_float_is_single_number() {
        let toks = tok("a: +1.5");
        let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
        assert_eq!(nums.len(), 1);
        assert_eq!(nums[0].1, "+1.5");
        let plus_ops: Vec<_> = toks
            .iter()
            .filter(|t| t.0 == TokenKind::Operator && t.1 == "+")
            .collect();
        assert!(plus_ops.is_empty());
    }

    #[test]
    fn signed_radix_combinations() {
        for (input, want_lit) in [
            ("a: -0xFF", "-0xFF"),
            ("a: +0xDEAD_BEEF", "+0xDEAD_BEEF"),
            ("a: -0b1010", "-0b1010"),
            ("a: -0o755", "-0o755"),
            ("a: +42", "+42"),
        ] {
            let toks = tok(input);
            let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
            assert_eq!(nums.len(), 1, "input {input:?} produced {toks:?}");
            assert_eq!(nums[0].1, want_lit);
        }
    }

    #[test]
    fn numbers_with_signs_and_radix() {
        let toks = tok("a: -3.14");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Number && t.1 == "-3.14")
        );
        let toks = tok("b: 0xFF_u8");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Number && t.1.starts_with("0xFF"))
        );
        let toks = tok("c: 0b1010");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Number && t.1 == "0b1010")
        );
        let toks = tok("d: 0o755");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Number && t.1 == "0o755")
        );
    }

    #[test]
    fn raw_string() {
        let toks = tok(r###"path: r#"C:\Users"#"###);
        let strings: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::String).collect();
        assert_eq!(strings.len(), 1);
    }

    #[test]
    fn char_literal() {
        let toks = tok("c: 'x'");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::CharLit && t.1 == "'x'")
        );
    }

    #[test]
    fn char_literal_unicode() {
        for (input, want_lit) in [
            ("c: 'é'", "'é'"),
            ("c: '你'", "'你'"),
            ("c: '😀'", "'😀'"),
            (r"c: '\n'", r"'\n'"),
            (r"c: '\u{1F600}'", r"'\u{1F600}'"),
        ] {
            let toks = tok(input);
            let chars: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::CharLit).collect();
            assert_eq!(chars.len(), 1, "input {input:?} produced {toks:?}");
            assert_eq!(chars[0].1, want_lit);
        }
    }

    #[test]
    fn extension_attribute() {
        let line = "#![enable(implicit_some)]";
        let toks = tok(line);
        assert_eq!(toks[0].0, TokenKind::Attribute);
        assert_eq!(toks[0].1, line);
    }

    #[test]
    fn definition_name_is_ron() {
        use crate::code_editor::lang::definition;
        assert_eq!(definition(&Language::Ron).name(), "RON");
    }

    #[test]
    fn covers_full_line() {
        let line = "Foo(name: \"v\", count: 42, // tail";
        let (toks, _) = tokenize_line(line, &Language::Ron, false);
        let total: usize = toks.iter().map(|t| t.len).sum();
        assert_eq!(total, line.len());
    }
}
