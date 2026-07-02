//! Markdown block-level constructs.
//!
//! Line-level dispatch after optional indentation: fenced-code open/close
//! (with the [`LineState::Fenced`] carry), ATX headings, thematic breaks,
//! blockquotes, and bullet / ordered list markers. Body text and list /
//! quote content is handed off to the inline tokenizer in [`super::inline`].

use super::inline::inline;
use super::*;

// ── Fenced-code body / close ────────────────────────────────────────────────

/// Tokenize a line while inside a fenced code block opened with `count`
/// repetitions of the `fence` byte (`` b'`' `` or `b'~'`).
///
/// A closing fence — optional indentation, a run of `>= count` of the same
/// fence char, then only whitespace — ends the block and returns
/// [`LineState::Code`]. Any other line (including one that merely *looks* like
/// a heading or comment) is coloured as plain code text and stays fenced.
pub(super) fn tokenize_fenced(line: &str, fence: u8, count: u8) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();

    if len == 0 {
        // Blank line inside the block — no tokens, stay fenced.
        return (vec![], LineState::Fenced { fence, count });
    }

    // Optional leading whitespace.
    let mut i = 0;
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let ws_end = i;

    // Run of fence chars.
    let fence_start = i;
    let mut n = 0usize;
    while i < len && bytes[i] == fence {
        i += 1;
        n += 1;
    }
    let fence_end = i;

    // The remainder of a close fence must be whitespace only.
    let only_ws = bytes[fence_end..].iter().all(|&c| c == b' ' || c == b'\t');

    if n >= count as usize && only_ws {
        // Closing fence — colour it and leave the block.
        let mut tokens = Vec::with_capacity(3);
        if ws_end > 0 {
            tokens.push(tok(TokenKind::Whitespace, 0, ws_end));
        }
        tokens.push(tok(TokenKind::Operator, fence_start, fence_end));
        if fence_end < len {
            tokens.push(tok(TokenKind::Whitespace, fence_end, len));
        }
        return (tokens, LineState::Code);
    }

    // Ordinary code line — whole line as plain code text, still fenced.
    (
        vec![tok(TokenKind::String, 0, len)],
        LineState::Fenced { fence, count },
    )
}

// ── Normal (non-fenced) line ────────────────────────────────────────────────

pub(super) fn tokenize_normal(line: &str) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();

    if len == 0 {
        return (vec![], LineState::Code);
    }

    let mut tokens = Vec::with_capacity(8);

    // Leading whitespace (block-level indentation).
    let mut i = 0;
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i > 0 {
        tokens.push(tok(TokenKind::Whitespace, 0, i));
    }
    let cs = i;

    if cs >= len {
        // Whitespace-only line.
        return (tokens, LineState::Code);
    }

    // ── Fenced code open (``` or ~~~, 3+) ────────────────────────────────────
    let first = bytes[cs];
    if first == b'`' || first == b'~' {
        let mut k = cs;
        let mut n = 0usize;
        while k < len && bytes[k] == first {
            k += 1;
            n += 1;
        }
        if n >= 3 {
            tokens.push(tok(TokenKind::Operator, cs, k));
            if k < len {
                // Info string (language tag / rest of line).
                tokens.push(tok(TokenKind::Keyword, k, len));
            }
            let count = n.min(u8::MAX as usize) as u8;
            return (
                tokens,
                LineState::Fenced {
                    fence: first,
                    count,
                },
            );
        }
        // Fewer than 3 → not a fence; fall through to inline handling
        // (`` `code` ``, `~~strike~~`, etc.).
    }

    // ── Thematic break / horizontal rule ─────────────────────────────────────
    if is_thematic_break(bytes, cs, len) {
        tokens.push(tok(TokenKind::Operator, cs, len));
        return (tokens, LineState::Code);
    }

    // ── ATX heading ──────────────────────────────────────────────────────────
    if first == b'#' {
        let mut k = cs;
        let mut h = 0usize;
        while k < len && bytes[k] == b'#' {
            k += 1;
            h += 1;
        }
        if h <= 6 && (k >= len || bytes[k] == b' ' || bytes[k] == b'\t') {
            tokens.push(tok(TokenKind::Operator, cs, k)); // the `#` run
            if k < len {
                tokens.push(tok(TokenKind::Keyword, k, len)); // heading text
            }
            return (tokens, LineState::Code);
        }
        // e.g. `#tag` (no space) or 7+ `#` → not a heading; fall through.
    }

    // ── Blockquote ───────────────────────────────────────────────────────────
    if first == b'>' {
        let mut k = cs + 1;
        while k < len && (bytes[k] == b' ' || bytes[k] == b'\t') {
            k += 1;
        }
        tokens.push(tok(TokenKind::Comment, cs, k)); // `>` + following spaces
        inline(line, k, &mut tokens);
        return (tokens, LineState::Code);
    }

    // ── Bullet list marker (`- ` / `* ` / `+ `) ──────────────────────────────
    if matches!(first, b'-' | b'*' | b'+')
        && cs + 1 < len
        && (bytes[cs + 1] == b' ' || bytes[cs + 1] == b'\t')
    {
        tokens.push(tok(TokenKind::Operator, cs, cs + 1)); // the bullet
        inline(line, cs + 1, &mut tokens);
        return (tokens, LineState::Code);
    }

    // ── Ordered list marker (`1. ` / `1) `) ──────────────────────────────────
    if first.is_ascii_digit() {
        let mut k = cs;
        while k < len && bytes[k].is_ascii_digit() {
            k += 1;
        }
        if k < len
            && (bytes[k] == b'.' || bytes[k] == b')')
            && (k + 1 >= len || bytes[k + 1] == b' ' || bytes[k + 1] == b'\t')
        {
            tokens.push(tok(TokenKind::Number, cs, k)); // the digits
            tokens.push(tok(TokenKind::Punctuation, k, k + 1)); // `.` or `)`
            inline(line, k + 1, &mut tokens);
            return (tokens, LineState::Code);
        }
        // e.g. `1.5` or `2024 ...` → not a list marker; fall through.
    }

    // ── Ordinary paragraph text ──────────────────────────────────────────────
    inline(line, cs, &mut tokens);
    (tokens, LineState::Code)
}

/// A thematic break is a line whose content is only `-`, `*`, or `_`
/// (a single kind, 3+ of them) optionally separated by spaces/tabs.
fn is_thematic_break(bytes: &[u8], cs: usize, len: usize) -> bool {
    let mut marker = 0u8;
    let mut count = 0usize;
    let mut k = cs;
    while k < len {
        match bytes[k] {
            b' ' | b'\t' => {}
            c @ (b'-' | b'*' | b'_') => {
                if marker == 0 {
                    marker = c;
                } else if c != marker {
                    return false;
                }
                count += 1;
            }
            _ => return false,
        }
        k += 1;
    }
    count >= 3
}
