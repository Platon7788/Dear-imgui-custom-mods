//! Markdown block-level constructs.
//!
//! Line-level dispatch after optional indentation: fenced-code open/close
//! (with the [`LineState::Fenced`] carry), ATX headings, thematic breaks,
//! blockquotes, and bullet / ordered list markers. Body text and list /
//! quote content is handed off to the inline tokenizer in [`super::inline`].

use super::inline::{inline, inline_range};
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

    // ── Indented code block (>= 4 columns of leading indent) ─────────────────
    // A line indented by 4+ spaces (a leading tab counts as >= 4) is a code
    // block: the whole remainder is plain code text, never run through the
    // inline emphasis/link tokenizer (so `    *not italic*` stays literal).
    // Purely line-local — no list-continuation context is tracked.
    if is_indented_code(bytes, cs) {
        tokens.push(tok(TokenKind::String, cs, len));
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

    // ── GFM table row (line-local) or ordinary paragraph text ────────────────
    // A normal line containing a pipe is tokenized as a table row: each `|`
    // is Punctuation and the cell text between pipes goes through the inline
    // tokenizer. A delimiter row (`|---|:--:|`) colours its dash/colon runs as
    // Operator. This also gracefully handles a paragraph with a stray `|`
    // (lone pipe → Punctuation, the rest inline). Purely line-local: no header
    // row above is required.
    if bytes[cs..len].contains(&b'|') {
        if is_delimiter_row(bytes, cs, len) {
            tokenize_delimiter_row(bytes, cs, len, &mut tokens);
        } else {
            tokenize_table_row(line, cs, len, &mut tokens);
        }
    } else {
        inline(line, cs, &mut tokens);
    }
    (tokens, LineState::Code)
}

/// True when the leading whitespace `bytes[..cs]` (all spaces / tabs) forms an
/// indented-code-block indent: 4+ columns, counting a tab as 4 columns. A
/// single leading tab therefore qualifies on its own.
fn is_indented_code(bytes: &[u8], cs: usize) -> bool {
    let mut columns = 0usize;
    for &b in &bytes[..cs] {
        columns += if b == b'\t' { 4 } else { 1 };
        if columns >= 4 {
            return true;
        }
    }
    false
}

/// A GFM delimiter row: `bytes[cs..len]` consists only of `|`, `:`, `-`, and
/// whitespace, and contains at least one `-` (e.g. `|---|:--:|`, `--- | ---`).
fn is_delimiter_row(bytes: &[u8], cs: usize, len: usize) -> bool {
    let mut has_dash = false;
    for &b in &bytes[cs..len] {
        match b {
            b'-' => has_dash = true,
            b'|' | b':' | b' ' | b'\t' => {}
            _ => return false,
        }
    }
    has_dash
}

/// Tokenize a delimiter row: pipes as Punctuation, `-`/`:` runs as Operator,
/// whitespace runs as Whitespace. Tiles `bytes[cs..len]` exactly.
fn tokenize_delimiter_row(bytes: &[u8], cs: usize, len: usize, tokens: &mut Vec<Token>) {
    let mut i = cs;
    while i < len {
        match bytes[i] {
            b'|' => {
                tokens.push(tok(TokenKind::Punctuation, i, i + 1));
                i += 1;
            }
            b' ' | b'\t' => {
                let s = i;
                while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                    i += 1;
                }
                tokens.push(tok(TokenKind::Whitespace, s, i));
            }
            _ => {
                // Run of `-` / `:` (all that remains in a delimiter row).
                let s = i;
                while i < len && (bytes[i] == b'-' || bytes[i] == b':') {
                    i += 1;
                }
                tokens.push(tok(TokenKind::Operator, s, i));
            }
        }
    }
}

/// Tokenize a GFM table row: each `|` is Punctuation, cell text between pipes
/// goes through the inline tokenizer (bounded to the cell). Tiles
/// `line[cs..len]` exactly; `cs..len` is known to contain at least one `|`.
fn tokenize_table_row(line: &str, cs: usize, len: usize, tokens: &mut Vec<Token>) {
    let bytes = line.as_bytes();
    let mut cell_start = cs;
    let mut i = cs;
    while i < len {
        if bytes[i] == b'|' {
            if i > cell_start {
                inline_range(line, cell_start, i, tokens);
            }
            tokens.push(tok(TokenKind::Punctuation, i, i + 1));
            i += 1;
            cell_start = i;
        } else {
            i += 1;
        }
    }
    if cell_start < len {
        inline_range(line, cell_start, len, tokens);
    }
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
