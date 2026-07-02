//! Rust line tokenizer — the state machine that walks one line of source
//! and emits [`Token`]s, carrying block-comment state across lines.

use super::keywords::{builtin_types_set, keywords_set};
use super::strings::{scan_raw_str_body, scan_str_body, str_carry};
use super::*;
use crate::code_editor::lang::{LineState, scan_block_comment};

pub(in crate::code_editor::lang) fn tokenize(
    line: &str,
    state: LineState,
) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;
    // Depth of currently-open (possibly nested) block comments, threaded
    // across lines via `LineState::BlockComment(depth)`.
    let mut depth: u32 = if let LineState::BlockComment(d) = state {
        u32::from(d)
    } else {
        0
    };

    // ── Resume a multi-line string opened on a previous line ─────────────
    // Rust string / raw / byte / c-string literals may span lines; the editor
    // threads `LineState::Str { … }` back so we scan from column 0 for the
    // close, colour the run as String, then stay open or fall through to code.
    if let LineState::Str { raw, hashes, .. } = state {
        let closed = if raw {
            scan_raw_str_body(&mut i, bytes, usize::from(hashes))
        } else {
            scan_str_body(&mut i, bytes)
        };
        if i > 0 {
            tokens.push(Token {
                kind: TokenKind::String,
                start: 0,
                len: i,
            });
        }
        if !closed {
            return (tokens, state);
        }
        // Closed mid-line — fall through to the main loop, resuming at `i`.
    }

    // USER CODE markers — only when the line begins in ordinary code.
    if depth == 0 && matches!(state, LineState::Code) {
        let trimmed = line.trim();
        if trimmed.starts_with("// USER CODE BEGIN") || trimmed.starts_with("// USER CODE END") {
            tokens.push(Token {
                kind: TokenKind::UserCodeMarker,
                start: 0,
                len: line.len(),
            });
            return (tokens, LineState::Code);
        }
    }

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
            // `///` outer-doc or `//!` inner-doc — but NOT `////`+ (plain).
            let is_doc =
                (i + 2 < len && bytes[i + 2] == b'/' && !(i + 3 < len && bytes[i + 3] == b'/'))
                    || (i + 2 < len && bytes[i + 2] == b'!');
            tokens.push(Token {
                kind: if is_doc {
                    TokenKind::DocComment
                } else {
                    TokenKind::Comment
                },
                start: i,
                len: len - i,
            });
            return (tokens, LineState::Code);
        }

        // ── Block comment start (nesting-aware) ──────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            // `/**` outer-doc or `/*!` inner-doc — but NOT `/**/` (empty).
            let is_doc = i + 2 < len
                && ((bytes[i + 2] == b'*' && !(i + 3 < len && bytes[i + 3] == b'/'))
                    || bytes[i + 2] == b'!');
            i += 2;
            depth = scan_block_comment(&mut i, bytes, 1);
            tokens.push(Token {
                kind: if is_doc {
                    TokenKind::DocComment
                } else {
                    TokenKind::Comment
                },
                start,
                len: i - start,
            });
            continue;
        }

        // ── Attribute ────────────────────────────────────────────────────
        if b == b'#' && i + 1 < len && (bytes[i + 1] == b'[' || bytes[i + 1] == b'!') {
            let start = i;
            // `#!` that is NOT followed by `[` (e.g. a shebang line, or a
            // stray `#!`) is not a real attribute — treat `#` as punctuation
            // and let the rest re-tokenize.
            if bytes[i + 1] == b'!' && !(i + 2 < len && bytes[i + 2] == b'[') {
                tokens.push(Token {
                    kind: TokenKind::Punctuation,
                    start,
                    len: 1,
                });
                i += 1;
                continue;
            }
            let mut bracket_depth = 0u32;
            let mut saw_bracket = false;
            while i < len {
                match bytes[i] {
                    b'[' => {
                        bracket_depth += 1;
                        saw_bracket = true;
                    }
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
            // If no `[` was ever seen the attribute was malformed; we still
            // emit one Attribute token spanning what we scanned (covers the
            // whole line for unterminated `#![` etc.) so the line stays
            // fully colored without panicking.
            let _ = saw_bracket;
            tokens.push(Token {
                kind: TokenKind::Attribute,
                start,
                len: i - start,
            });
            continue;
        }

        // ── String literal ───────────────────────────────────────────────
        if b == b'"' {
            let start = i;
            i += 1;
            let closed = scan_str_body(&mut i, bytes);
            tokens.push(Token {
                kind: TokenKind::String,
                start,
                len: i - start,
            });
            if !closed {
                return (tokens, str_carry(false, 0));
            }
            continue;
        }

        // ── Byte / byte-string literal (b'x', b"...") ────────────────────
        if b == b'b' && i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'\'') {
            // `b"..."` byte string or `b'x'` byte char.
            if bytes[i + 1] == b'"' {
                let start = i;
                i += 2;
                let closed = scan_str_body(&mut i, bytes);
                tokens.push(Token {
                    kind: TokenKind::String,
                    start,
                    len: i - start,
                });
                if !closed {
                    return (tokens, str_carry(false, 0));
                }
                continue;
            } else if let Some(end) = consume_char_literal(line, i + 1) {
                // b'x' — `b` prefix + char literal.
                tokens.push(Token {
                    kind: TokenKind::CharLit,
                    start: i,
                    len: end - i,
                });
                i = end;
                continue;
            }
            // Otherwise fall through — `b` is an ordinary identifier start.
        }

        // ── C-string literal (c"...", stabilized in Rust 1.77) ───────────
        if b == b'c' && i + 1 < len && bytes[i + 1] == b'"' {
            let start = i;
            i += 2;
            let closed = scan_str_body(&mut i, bytes);
            tokens.push(Token {
                kind: TokenKind::String,
                start,
                len: i - start,
            });
            if !closed {
                return (tokens, str_carry(false, 0));
            }
            continue;
        }

        // ── Raw string (r"..."/r#"..."#, br"...", cr"...") ───────────────
        if (b == b'r' && i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#'))
            || ((b == b'b' || b == b'c')
                && i + 2 < len
                && bytes[i + 1] == b'r'
                && (bytes[i + 2] == b'"' || bytes[i + 2] == b'#'))
        {
            let start = i;
            // Skip optional `b` (byte) / `c` (C-string) prefix.
            if b == b'b' || b == b'c' {
                i += 1;
            }
            i += 1; // skip `r`
            let mut hashes = 0usize;
            while i < len && bytes[i] == b'#' {
                hashes += 1;
                i += 1;
            }
            if i < len && bytes[i] == b'"' {
                i += 1;
                let closed = scan_raw_str_body(&mut i, bytes, hashes);
                tokens.push(Token {
                    kind: TokenKind::String,
                    start,
                    len: i - start,
                });
                if !closed {
                    return (tokens, str_carry(true, hashes.min(255) as u8));
                }
                continue;
            }
            i = start; // not a raw string — fall through
        }

        // ── Char literal ─────────────────────────────────────────────────
        // Helper short-circuits on `bytes[i] != b'\''`, so calling it
        // unconditionally costs only a single byte compare for the
        // non-`'` case. On a stray `'` (e.g. `'a` lifetime) it returns
        // `None` and we fall through to the lifetime branch below.
        if let Some(end) = consume_char_literal(line, i) {
            tokens.push(Token {
                kind: TokenKind::CharLit,
                start: i,
                len: end - i,
            });
            i = end;
            continue;
        }

        // ── Lifetime ─────────────────────────────────────────────────────
        if b == b'\'' && i + 1 < len && is_ident_start(bytes[i + 1]) {
            let start = i;
            i += 1;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Lifetime,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Number ───────────────────────────────────────────────────────
        if b.is_ascii_digit() || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            consume_number(&mut i, bytes, NumberOpts::RUST_LIKE);
            // Type suffix (e.g. `42_u8`, `0xFF_i32`)
            if i < len && is_ident_start(bytes[i]) {
                while i < len && is_ident_continue(bytes[i]) {
                    i += 1;
                }
            }
            tokens.push(Token {
                kind: TokenKind::Number,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Identifier / Keyword / Type / Macro ──────────────────────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            // Macro invocation: `name!` but not `!=` (the operator), and not
            // a keyword (`return!` is never a macro). `macro_rules!` is the
            // canonical case kept as a MacroCall by excluding it from the
            // keyword guard.
            if i < len
                && bytes[i] == b'!'
                && !(i + 1 < len && bytes[i + 1] == b'=')
                && (!keywords_set().contains(word) || word == "macro_rules")
            {
                i += 1;
                tokens.push(Token {
                    kind: TokenKind::MacroCall,
                    start,
                    len: i - start,
                });
                continue;
            }
            let kind = if keywords_set().contains(word) {
                TokenKind::Keyword
            } else if builtin_types_set().contains(word)
                || word.chars().next().is_some_and(|c| c.is_uppercase())
            {
                TokenKind::TypeName
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

        // ── Operators ────────────────────────────────────────────────────
        if matches!(
            b,
            b'+' | b'-'
                | b'*'
                | b'/'
                | b'%'
                | b'='
                | b'!'
                | b'<'
                | b'>'
                | b'&'
                | b'|'
                | b'^'
                | b'~'
        ) {
            let start = i;
            i += 1;
            if i < len
                && matches!(
                    (b, bytes[i]),
                    (b'=', b'=')
                        | (b'!', b'=')
                        | (b'<', b'=')
                        | (b'>', b'=')
                        | (b'-', b'>')
                        | (b'=', b'>')
                        | (b'&', b'&')
                        | (b'|', b'|')
                        | (b'<', b'<')
                        | (b'>', b'>')
                        | (b'+', b'=')
                        | (b'-', b'=')
                        | (b'*', b'=')
                        | (b'/', b'=')
                        | (b'%', b'=')
                        | (b'&', b'=')
                        | (b'|', b'=')
                        | (b'^', b'=')
                )
            {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Operator,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Punctuation ──────────────────────────────────────────────────
        if matches!(
            b,
            b'(' | b')'
                | b'{'
                | b'}'
                | b'['
                | b']'
                | b';'
                | b':'
                | b','
                | b'.'
                | b'@'
                | b'?'
                | b'#'
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

    let end = if depth > 0 {
        // Saturate: a 65536-deep nested comment must stay "open", not wrap to 0.
        LineState::BlockComment(depth.min(u16::MAX as u32) as u16)
    } else {
        LineState::Code
    };
    (tokens, end)
}
