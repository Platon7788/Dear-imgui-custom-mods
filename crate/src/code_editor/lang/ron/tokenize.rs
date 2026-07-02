//! RON line tokenizer.

use super::*;

pub(in crate::code_editor::lang) fn tokenize(
    line: &str,
    state: LineState,
) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;
    let mut depth: u32 = if let LineState::BlockComment(d) = state {
        u32::from(d)
    } else {
        0
    };

    // ── Continuation of a multi-line string opened on a previous line ────
    // `raw` = raw string (`r#"…`), scanning for the matching `"#…#`; plain
    // strings scan for the next un-escaped `"`. Colour the whole run String;
    // either it closes here (→ back to code) or it stays open.
    if let LineState::Str { raw, hashes, .. } = state {
        let (end, closed) = if raw {
            scan_raw_string_close(bytes, 0, hashes as usize)
        } else {
            scan_dq_string_close(bytes, 0)
        };
        if end > 0 {
            push(&mut tokens, TokenKind::String, 0, end);
        }
        i = end;
        if !closed {
            return (tokens, state);
        }
    }

    while i < len {
        // ── Inside a (possibly nested) block comment ─────────────────────
        if depth > 0 {
            let start = i;
            depth = scan_block_comment(&mut i, bytes, depth);
            push(&mut tokens, TokenKind::Comment, start, i - start);
            continue;
        }

        let b = bytes[i];

        // ── Whitespace ───────────────────────────────────────────────────
        if b == b' ' || b == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            push(&mut tokens, TokenKind::Whitespace, start, i - start);
            continue;
        }

        // ── Line comment ─────────────────────────────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            push(&mut tokens, TokenKind::Comment, i, len - i);
            return (tokens, LineState::Code);
        }

        // ── Block comment start (nesting-aware) ──────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            depth = scan_block_comment(&mut i, bytes, 1);
            push(&mut tokens, TokenKind::Comment, start, i - start);
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
            push(&mut tokens, TokenKind::Attribute, start, i - start);
            continue;
        }

        // ── String literal (also: map key when followed by `:`). When it
        //    doesn't close on this line, carry a `Str` state so the string
        //    can span physical lines. ──────────────────────────────────────
        if b == b'"' {
            let start = i;
            let (end, closed) = scan_dq_string_close(bytes, i + 1);
            i = end;
            if !closed {
                push(&mut tokens, TokenKind::String, start, i - start);
                return (
                    tokens,
                    LineState::Str {
                        quote: b'"',
                        raw: false,
                        hashes: 0,
                        triple: false,
                    },
                );
            }
            // Lookahead past whitespace for `:` → map key.
            let mut j = i;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let kind = if j < len && bytes[j] == b':' {
                TokenKind::Attribute
            } else {
                TokenKind::String
            };
            push(&mut tokens, kind, start, i - start);
            continue;
        }

        // ── Raw string (r"..." or r#"..."#), may span physical lines ─────
        if b == b'r' && i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') {
            let start = i;
            let mut p = i + 1;
            let mut hashes = 0usize;
            while p < len && bytes[p] == b'#' {
                hashes += 1;
                p += 1;
            }
            if p < len && bytes[p] == b'"' {
                let (end, closed) = scan_raw_string_close(bytes, p + 1, hashes);
                push(&mut tokens, TokenKind::String, start, end - start);
                i = end;
                if !closed {
                    return (
                        tokens,
                        LineState::Str {
                            quote: b'"',
                            raw: true,
                            // Clamp: raw strings allow at most 255 `#`, and an
                            // unclamped `as u8` would wrap 256→0 and mis-close.
                            hashes: hashes.min(255) as u8,
                            triple: false,
                        },
                    );
                }
                continue;
            }
            // Not a raw string — `i` untouched, fall through to identifier.
        }

        // ── Char literal ─────────────────────────────────────────────────
        // Helper short-circuits on `bytes[i] != b'\''`, so the call is
        // a single byte compare in the common case.
        if let Some(end) = consume_char_literal(line, i) {
            push(&mut tokens, TokenKind::CharLit, i, end - i);
            i = end;
            continue;
        }

        // ── Signed non-finite floats: +inf / -inf / +NaN / -NaN ──────────
        // Before the number branch so the sign isn't split off.
        if let Some(n) = signed_special_float_len(bytes, i) {
            push(&mut tokens, TokenKind::Number, i, n);
            i += n;
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
            push(&mut tokens, TokenKind::Number, start, i - start);
            continue;
        }

        // ── Identifier / Keyword / Type / Field-key ──────────────────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            // Lookahead past whitespace for `:` → field / map key.
            let mut j = i;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let followed_by_colon = j < len && bytes[j] == b':';

            // The colon-follows (field / map key) check runs BEFORE the
            // uppercase→TypeName rule, so a capitalized key `Key:` reads as
            // an Attribute — the same as a quoted `"key":`. Bare `inf`/`NaN`
            // in value position are non-finite float literals.
            let kind = if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if followed_by_colon {
                TokenKind::Attribute
            } else if word == "inf" || word == "NaN" {
                TokenKind::Number
            } else if word.chars().next().is_some_and(|c| c.is_uppercase()) {
                TokenKind::TypeName
            } else {
                TokenKind::Identifier
            };
            push(&mut tokens, kind, start, i - start);
            continue;
        }

        // ── Range operators (.., ..=) ────────────────────────────────────
        if b == b'.' && i + 1 < len && bytes[i + 1] == b'.' {
            let start = i;
            i += 2;
            if i < len && bytes[i] == b'=' {
                i += 1;
            }
            push(&mut tokens, TokenKind::Operator, start, i - start);
            continue;
        }

        // ── Operators (`:` separates key from value, `=` legacy) ─────────
        if matches!(b, b':' | b'=' | b'-' | b'+') {
            push(&mut tokens, TokenKind::Operator, i, 1);
            i += 1;
            continue;
        }

        // ── Punctuation ──────────────────────────────────────────────────
        if matches!(
            b,
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';'
        ) {
            push(&mut tokens, TokenKind::Punctuation, i, 1);
            i += 1;
            continue;
        }

        // ── Fallback: full Unicode scalar ────────────────────────────────
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        push(&mut tokens, TokenKind::Identifier, i, ch_len);
        i += ch_len;
    }

    let end = if depth > 0 {
        // Saturate: a 65536-deep nested comment must stay "open", not wrap to 0.
        LineState::BlockComment(depth.min(u16::MAX as u32) as u16)
    } else {
        LineState::Code
    };
    (tokens, end)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::{Language, LineState};
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn tok(line: &str) -> Vec<(TokenKind, String)> {
        let (tokens, _) = tokenize_line(line, &Language::Ron, LineState::Code);
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
        let (toks, still_in) = tokenize_line("/* start", &Language::Ron, LineState::Code);
        assert_eq!(still_in, LineState::BlockComment(1));
        assert_eq!(toks[0].kind, TokenKind::Comment);

        let (toks2, still_in2) =
            tokenize_line("middle */ rest", &Language::Ron, LineState::BlockComment(1));
        assert_eq!(still_in2, LineState::Code);
        assert_eq!(toks2[0].kind, TokenKind::Comment);
    }

    /// RON inherits Rust's nested block comments.
    #[test]
    fn nested_block_comment() {
        let (_, still_in) = tokenize_line("/* a /* b */ c */", &Language::Ron, LineState::Code);
        assert_eq!(still_in, LineState::Code, "balanced nest closes");
        let (_, still_in2) = tokenize_line("/* a /* b */", &Language::Ron, LineState::Code);
        assert_eq!(
            still_in2,
            LineState::BlockComment(1),
            "one level still open"
        );
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
        let (toks, _) = tokenize_line(line, &Language::Ron, LineState::Code);
        let total: usize = toks.iter().map(|t| t.len).sum();
        assert_eq!(total, line.len());
    }
}
