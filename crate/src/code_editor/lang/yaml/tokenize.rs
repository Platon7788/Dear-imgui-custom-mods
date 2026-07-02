//! YAML line tokenizer — the scanner that walks one line of source and emits
//! [`Token`]s, and the block-scalar carry helpers. A trailing `|` / `>`
//! indicator opens a multi-line body threaded across lines via
//! [`LineState::YamlBlock`]; [`block_body_line`] consumes those body lines.

use super::*;

// ── Block scalar body ───────────────────────────────────────────────────────

/// A line consumed while inside a `|` / `>` block-scalar body.
///
/// Returns `Some(tokens)` when the line stays in the block — it is blank
/// (empty or all-whitespace) or its leading indent is `>= indent` — colouring
/// the scalar content [`TokenKind::String`]. Returns `None` on a dedent so the
/// caller re-tokenizes the line as ordinary YAML. Emitted spans tile the line.
pub(in crate::code_editor::lang) fn block_body_line(line: &str, indent: u16) -> Option<Vec<Token>> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut ws = 0;
    while ws < len && (bytes[ws] == b' ' || bytes[ws] == b'\t') {
        ws += 1;
    }
    let blank = ws == len;
    if !blank && (ws as u16) < indent {
        return None; // dedent — end of the block scalar
    }
    let mut tokens = Vec::new();
    if ws > 0 {
        tokens.push(Token {
            kind: TokenKind::Whitespace,
            start: 0,
            len: ws,
        });
    }
    if ws < len {
        tokens.push(Token {
            kind: TokenKind::String,
            start: ws,
            len: len - ws,
        });
    }
    Some(tokens)
}

/// End-of-line carry: enter a block-scalar body when a trailing `|` / `>`
/// indicator opened one, otherwise plain `Code`.
fn end_state(pending_block: Option<u16>) -> LineState {
    match pending_block {
        Some(indent) => LineState::YamlBlock { indent },
        None => LineState::Code,
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

pub(in crate::code_editor::lang) fn tokenize(line: &str) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;
    let mut lead_indent: u16 = 0;
    // Set when a trailing block-scalar indicator opens a multi-line body.
    let mut pending_block: Option<u16> = None;

    // Leading whitespace (significant in YAML) — also the line's indent.
    if i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        let start = i;
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        lead_indent = (i - start).min(u16::MAX as usize) as u16;
        tokens.push(Token {
            kind: TokenKind::Whitespace,
            start,
            len: i - start,
        });
    }

    // Full-line comment
    if i < len && bytes[i] == b'#' {
        tokens.push(Token {
            kind: TokenKind::Comment,
            start: i,
            len: len - i,
        });
        return (tokens, LineState::Code);
    }

    // Directive (%YAML, %TAG)
    if i < len && bytes[i] == b'%' {
        tokens.push(Token {
            kind: TokenKind::Attribute,
            start: i,
            len: len - i,
        });
        return (tokens, LineState::Code);
    }

    // Document markers (`---` / `...`) may carry trailing content, so emit the
    // 3-char marker and keep tokenizing the rest of the line.
    if (bytes[i..].starts_with(b"---") || bytes[i..].starts_with(b"..."))
        && (i + 3 == len || bytes[i + 3] == b' ' || bytes[i + 3] == b'\t')
    {
        tokens.push(Token {
            kind: TokenKind::Keyword,
            start: i,
            len: 3,
        });
        i += 3;
    }

    while i < len {
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

        // ── Comment (only when `#` follows whitespace / line start) ───────
        if b == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return (tokens, end_state(pending_block));
        }

        // ── Null tilde `~` ───────────────────────────────────────────────
        if b == b'~' {
            tokens.push(Token {
                kind: TokenKind::Keyword,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Block scalar indicator `|` / `>` (+ chomping -/+ and indent) ──
        if b == b'|' || b == b'>' {
            let start = i;
            i += 1;
            while i < len && matches!(bytes[i], b'-' | b'+' | b'0'..=b'9') {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Operator,
                start,
                len: i - start,
            });
            // A block scalar opens only when the indicator is the last
            // non-comment content on the line; the body's indent is
            // approximated as 1 + this line's indent.
            let mut k = i;
            while k < len && (bytes[k] == b' ' || bytes[k] == b'\t') {
                k += 1;
            }
            if k >= len || bytes[k] == b'#' {
                pending_block = Some(lead_indent.saturating_add(1));
            }
            continue;
        }

        // ── Anchor (&name) / Alias (*name) ───────────────────────────────
        if (b == b'&' || b == b'*') && i + 1 < len && is_ident_start(bytes[i + 1]) {
            let start = i;
            i += 1;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::MacroCall,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Tag (!!type or !custom) ──────────────────────────────────────
        if b == b'!' {
            let start = i;
            i += 1;
            while i < len && !matches!(bytes[i], b' ' | b'\t' | b'\n' | b',') {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::TypeName,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Quoted string ────────────────────────────────────────────────
        // `"…"` honours `\` escapes; `'…'` treats a doubled `''` as an
        // escaped quote (only a lone `'` closes the scalar).
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len && quote == b'"' {
                    i += 2;
                } else if bytes[i] == quote {
                    if quote == b'\'' && i + 1 < len && bytes[i + 1] == b'\'' {
                        i += 2; // doubled '' — an escaped quote, stay in string
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            // A quoted scalar immediately followed (after optional spaces) by
            // a `:` + space/EOL is a mapping key → Attribute; otherwise it is
            // a String value. Mirrors the bare-key `:`-lookahead (and RON's
            // quoted-key rule).
            let mut j = i;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let kind = if j < len
                && bytes[j] == b':'
                && (j + 1 >= len || bytes[j + 1] == b' ' || bytes[j + 1] == b'\t')
            {
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

        // ── Key-value colon ──────────────────────────────────────────────
        if b == b':' && (i + 1 >= len || bytes[i + 1] == b' ' || bytes[i + 1] == b'\t') {
            tokens.push(Token {
                kind: TokenKind::Operator,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── List dash ────────────────────────────────────────────────────
        if b == b'-' && (i + 1 >= len || bytes[i + 1] == b' ' || bytes[i + 1] == b'\t') {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Flow punctuation ─────────────────────────────────────────────
        if matches!(b, b'{' | b'}' | b'[' | b']' | b',') {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── YAML special floats (`.inf` / `+.inf` / `-.inf` / `.nan`) ────
        // Leading-dot infinities/NaN (YAML 1.1/1.2). Matched as whole tokens
        // ending at a word boundary so `.information` stays a bare scalar.
        if b == b'.' || ((b == b'-' || b == b'+') && i + 1 < len && bytes[i + 1] == b'.') {
            let start = i;
            let after = if b == b'.' { i + 1 } else { i + 2 };
            let word = &bytes[after..];
            let matched = word.starts_with(b"inf")
                || word.starts_with(b"Inf")
                || word.starts_with(b"INF")
                || word.starts_with(b"nan")
                || word.starts_with(b"NaN")
                || word.starts_with(b"NAN");
            let end = after + 3;
            if matched
                && (end >= len || matches!(bytes[end], b' ' | b'\t' | b'#' | b',' | b']' | b'}'))
            {
                tokens.push(Token {
                    kind: TokenKind::Number,
                    start,
                    len: end - start,
                });
                i = end;
                continue;
            }
            // Not a special float — fall through to the bare-scalar handler.
        }

        // ── Number ───────────────────────────────────────────────────────
        // YAML 1.1 radix/underscore/float, but the tail must end at
        // whitespace or structural punctuation; otherwise it is a bare
        // scalar (e.g. `2:30`, `1.2.3`).
        if b.is_ascii_digit()
            || ((b == b'-' || b == b'+') && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            let save = i;
            if b == b'-' || b == b'+' {
                i += 1;
            }
            consume_number(&mut i, bytes, NumberOpts::RUST_LIKE);
            if i >= len || matches!(bytes[i], b' ' | b'\t' | b'#' | b',' | b']' | b'}') {
                tokens.push(Token {
                    kind: TokenKind::Number,
                    start,
                    len: i - start,
                });
                continue;
            }
            i = save; // not a number — fall through to unquoted string
        }

        // ── Unquoted string / bare value ─────────────────────────────────
        {
            let start = i;
            while i < len {
                let c = bytes[i];
                if c == b'#' && i > 0 && matches!(bytes[i - 1], b' ' | b'\t') {
                    break;
                }
                if c == b':' && (i + 1 >= len || bytes[i + 1] == b' ' || bytes[i + 1] == b'\t') {
                    break;
                }
                if matches!(c, b'{' | b'}' | b'[' | b']' | b',') {
                    break;
                }
                i += 1;
            }
            // Guaranteed-advance guard: a leading delimiter that slipped past
            // consumes one char as punctuation so the scan can't spin.
            if i == start && i < len {
                let adv = line[i..].chars().next().map_or(1, |c| c.len_utf8());
                tokens.push(Token {
                    kind: TokenKind::Punctuation,
                    start,
                    len: adv,
                });
                i += adv;
                continue;
            }
            // Trim trailing whitespace from the token.
            let mut end = i;
            while end > start && matches!(bytes[end - 1], b' ' | b'\t') {
                end -= 1;
            }
            // A scan that stopped at a `: ` separator marks a mapping key.
            let stopped_at_key_colon = i < len && bytes[i] == b':';
            if end > start {
                let word = &line[start..end];
                let kind = if KEYWORDS.contains(&word) {
                    TokenKind::Keyword
                } else if stopped_at_key_colon {
                    TokenKind::Attribute
                } else {
                    TokenKind::Identifier
                };
                tokens.push(Token {
                    kind,
                    start,
                    len: end - start,
                });
            }
            if end < i {
                tokens.push(Token {
                    kind: TokenKind::Whitespace,
                    start: end,
                    len: i - end,
                });
            }
        }
    }

    (tokens, end_state(pending_block))
}
