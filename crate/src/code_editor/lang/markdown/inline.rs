//! Markdown inline-span tokenizer.
//!
//! Walks the inline portion of a line — inline code spans, bold / italic /
//! strikethrough emphasis, links / images, autolinks, and backslash escapes —
//! appending tokens that exactly tile the span. Every branch advances by at
//! least one byte; runs stop only on ASCII "special" bytes, so multi-byte
//! UTF-8 sequences are always consumed whole and every span lands on a char
//! boundary.

use super::*;

/// Tokenize the inline span `line[start..]`, appending to `tokens`.
///
/// Every branch advances by at least one byte; runs stop only on ASCII
/// "special" bytes, so multi-byte UTF-8 sequences are always consumed whole
/// and every span lands on a char boundary.
pub(super) fn inline(line: &str, start: usize, tokens: &mut Vec<Token>) {
    inline_range(line, start, line.len(), tokens);
}

/// Tokenize the inline span `line[start..end]`, appending to `tokens`.
///
/// The `end` bound lets callers (e.g. GFM table cells) restrict inline
/// tokenization to a slice of the line — an unclosed marker inside the range
/// falls back to punctuation instead of spilling past `end`. `end` must be a
/// char boundary. Every branch advances by at least one byte; runs stop only
/// on ASCII "special" bytes, so multi-byte UTF-8 sequences are always consumed
/// whole and every span lands on a char boundary.
pub(super) fn inline_range(line: &str, start: usize, end: usize, tokens: &mut Vec<Token>) {
    let bytes = line.as_bytes();
    let mut i = start;

    while i < end {
        match bytes[i] {
            // ── Inline code span (run-length matched) ────────────────────────
            b'`' => {
                let open = i;
                let mut n = 0usize;
                while i < end && bytes[i] == b'`' {
                    i += 1;
                    n += 1;
                }
                // Search for a closing run of exactly `n` backticks.
                let mut j = i;
                let mut close = None;
                while j < end {
                    if bytes[j] == b'`' {
                        let mut m = 0usize;
                        while j < end && bytes[j] == b'`' {
                            j += 1;
                            m += 1;
                        }
                        if m == n {
                            close = Some(j);
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                match close {
                    Some(end) => {
                        tokens.push(tok(TokenKind::String, open, end));
                        i = end;
                    }
                    None => {
                        // Unclosed — the opening run is plain punctuation.
                        tokens.push(tok(TokenKind::Punctuation, open, i));
                    }
                }
            }

            // ── Emphasis: bold (`**`/`__`) or italic (`*`/`_`) ───────────────
            d @ (b'*' | b'_') => {
                if i + 1 < end && bytes[i + 1] == d {
                    // Bold — find the closing double delimiter.
                    let open = i;
                    let mut j = i + 2;
                    let mut found = None;
                    while j + 1 < end {
                        if bytes[j] == d && bytes[j + 1] == d {
                            found = Some(j);
                            break;
                        }
                        j += 1;
                    }
                    match found {
                        Some(cl) => {
                            tokens.push(tok(TokenKind::String, open, cl + 2));
                            i = cl + 2;
                        }
                        None => {
                            tokens.push(tok(TokenKind::Punctuation, open, open + 2));
                            i = open + 2;
                        }
                    }
                } else {
                    // Italic — find the closing single delimiter.
                    let open = i;
                    let mut j = i + 1;
                    let mut found = None;
                    while j < end {
                        if bytes[j] == d {
                            found = Some(j);
                            break;
                        }
                        j += 1;
                    }
                    match found {
                        Some(cl) => {
                            tokens.push(tok(TokenKind::String, open, cl + 1));
                            i = cl + 1;
                        }
                        None => {
                            tokens.push(tok(TokenKind::Punctuation, open, open + 1));
                            i = open + 1;
                        }
                    }
                }
            }

            // ── Strikethrough (`~~…~~`) ──────────────────────────────────────
            b'~' => {
                if i + 1 < end && bytes[i + 1] == b'~' {
                    let open = i;
                    let mut j = i + 2;
                    let mut found = None;
                    while j + 1 < end {
                        if bytes[j] == b'~' && bytes[j + 1] == b'~' {
                            found = Some(j);
                            break;
                        }
                        j += 1;
                    }
                    match found {
                        Some(cl) => {
                            tokens.push(tok(TokenKind::String, open, cl + 2));
                            i = cl + 2;
                        }
                        None => {
                            tokens.push(tok(TokenKind::Punctuation, open, open + 2));
                            i = open + 2;
                        }
                    }
                } else {
                    tokens.push(tok(TokenKind::Identifier, i, i + 1));
                    i += 1;
                }
            }

            // ── Link `[text](url)` ───────────────────────────────────────────
            b'[' => {
                if let Some((cb, cp)) = link_bounds(bytes, end, i) {
                    push_link(tokens, i, cb, cp);
                    i = cp + 1;
                } else {
                    tokens.push(tok(TokenKind::Punctuation, i, i + 1));
                    i += 1;
                }
            }

            // ── Image `![alt](url)` ──────────────────────────────────────────
            b'!' => {
                if i + 1 < end
                    && bytes[i + 1] == b'['
                    && let Some((cb, cp)) = link_bounds(bytes, end, i + 1)
                {
                    tokens.push(tok(TokenKind::Punctuation, i, i + 1)); // `!`
                    push_link(tokens, i + 1, cb, cp);
                    i = cp + 1;
                } else {
                    // Lone `!`.
                    tokens.push(tok(TokenKind::Identifier, i, i + 1));
                    i += 1;
                }
            }

            // ── Autolink `<http://…>` / `<user@host>` ────────────────────────
            b'<' => {
                let open = i;
                let mut j = i + 1;
                while j < end && bytes[j] != b'>' {
                    j += 1;
                }
                if j < end {
                    let inner = &line[open + 1..j];
                    if inner.contains("://") || (inner.contains('@') && !inner.contains(' ')) {
                        tokens.push(tok(TokenKind::String, open, j + 1));
                        i = j + 1;
                        continue;
                    }
                }
                // Not an autolink (e.g. an HTML tag `<b>`): lone `<`.
                tokens.push(tok(TokenKind::Punctuation, open, open + 1));
                i = open + 1;
            }

            // ── Backslash escape ─────────────────────────────────────────────
            b'\\' => {
                if i + 1 < end {
                    let nb = bytes[i + 1];
                    let adv = if nb < 0x80 {
                        2
                    } else {
                        1 + line[i + 1..end].chars().next().map_or(1, |c| c.len_utf8())
                    };
                    tokens.push(tok(TokenKind::Identifier, i, i + adv));
                    i += adv;
                } else {
                    tokens.push(tok(TokenKind::Identifier, i, i + 1));
                    i += 1;
                }
            }

            // ── Plain run (stops at the next inline-special ASCII byte) ───────
            _ => {
                let run_start = i;
                while i < end
                    && !matches!(
                        bytes[i],
                        b'`' | b'*' | b'_' | b'~' | b'[' | b'!' | b'<' | b'\\'
                    )
                {
                    i += 1;
                }
                tokens.push(tok(TokenKind::Identifier, run_start, i));
            }
        }
    }
}

/// Return `(close_bracket, close_paren)` byte offsets if a full inline link
/// `[…](…)` starts at `open_bracket` (which must be a `[`) and closes before
/// `end`, else `None`.
fn link_bounds(bytes: &[u8], end: usize, open_bracket: usize) -> Option<(usize, usize)> {
    let mut j = open_bracket + 1;
    while j < end && bytes[j] != b']' {
        j += 1;
    }
    if j >= end {
        return None; // no closing `]`
    }
    if j + 1 >= end || bytes[j + 1] != b'(' {
        return None; // `]` not immediately followed by `(`
    }
    let mut k = j + 2;
    while k < end && bytes[k] != b')' {
        k += 1;
    }
    if k >= end {
        return None; // no closing `)`
    }
    Some((j, k))
}

/// Push the six-part token sequence for a link/image body whose `[` is at
/// `open_bracket`, `]` at `cb`, and `)` at `cp` (with `(` at `cb + 1`).
fn push_link(tokens: &mut Vec<Token>, open_bracket: usize, cb: usize, cp: usize) {
    tokens.push(tok(TokenKind::Punctuation, open_bracket, open_bracket + 1)); // `[`
    if cb > open_bracket + 1 {
        tokens.push(tok(TokenKind::Identifier, open_bracket + 1, cb)); // text / alt
    }
    tokens.push(tok(TokenKind::Punctuation, cb, cb + 1)); // `]`
    tokens.push(tok(TokenKind::Punctuation, cb + 1, cb + 2)); // `(`
    if cp > cb + 2 {
        tokens.push(tok(TokenKind::String, cb + 2, cp)); // url
    }
    tokens.push(tok(TokenKind::Punctuation, cp, cp + 1)); // `)`
}
