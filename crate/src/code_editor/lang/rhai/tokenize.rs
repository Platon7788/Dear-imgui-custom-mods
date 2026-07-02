//! Rhai line tokenizer — the state machine that walks one line of source and
//! emits [`Token`]s, carrying block-comment and multi-line backtick-template
//! state across lines.

use super::*;
use crate::code_editor::lang::{LineState, scan_block_comment};

const KEYWORDS: &[&str] = &[
    "let",
    "const",
    "if",
    "else",
    "while",
    "loop",
    "for",
    "in",
    "do",
    "until",
    "break",
    "continue",
    "return",
    "throw",
    "try",
    "catch",
    "fn",
    "private",
    "import",
    "export",
    "as",
    "switch",
    "is",
    "type_of",
    "print",
    "debug",
    "true",
    "false",
    "this",
    "call",
    "curry",
    "is_def_fn",
    "is_def_var",
    "is_shared",
    "eval",
    "global",
];

const BUILTIN_TYPES: &[&str] = &[
    "bool", "char", "i64", "f64", "String", "Array", "Map", "Blob", "Dynamic", "Instant", "FnPtr",
    "Decimal",
];

// ── Tokenizer ───────────────────────────────────────────────────────────────

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

    // ── Resume a multi-line backtick template from the previous line ──────
    // Rhai backtick strings (`` `…` ``) may span lines; the editor threads
    // `LineState::Str { quote: b'`', … }` back to us. Scan from column 0 as a
    // template body (String text + `${…}` interpolation) until it closes.
    if let LineState::Str { quote: b'`', .. } = state {
        let closed = scan_backtick_template(&mut i, line, bytes, &mut tokens, 0);
        if !closed {
            return (tokens, state);
        }
        // Closed mid-line — fall through to tokenize the remainder as code.
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

        // Consume one token; a multi-line construct that doesn't close on
        // this line hands back the carry state for the next line.
        if let Some(carry) = consume_token(&mut i, line, bytes, &mut tokens) {
            return (tokens, carry);
        }
    }

    let end = if depth > 0 {
        LineState::BlockComment(depth as u16)
    } else {
        LineState::Code
    };
    (tokens, end)
}

/// Consume exactly one Rhai token at `*i`, pushing it (or, for backtick
/// templates, several) to `tokens` and advancing `*i` by at least one byte.
///
/// Returns `Some(state)` when a multi-line construct (block comment or
/// backtick template) opened here and did **not** close before end-of-line —
/// the caller stops and threads that state to the next line. Returns `None`
/// for an ordinary single-line token.
fn consume_token(
    i: &mut usize,
    line: &str,
    bytes: &[u8],
    tokens: &mut Vec<Token>,
) -> Option<LineState> {
    let len = bytes.len();
    let b = bytes[*i];

    // ── Whitespace ───────────────────────────────────────────────────────
    if b == b' ' || b == b'\t' {
        let start = *i;
        while *i < len && (bytes[*i] == b' ' || bytes[*i] == b'\t') {
            *i += 1;
        }
        push(tokens, TokenKind::Whitespace, start, *i);
        return None;
    }

    // ── Line comment ─────────────────────────────────────────────────────
    if b == b'/' && *i + 1 < len && bytes[*i + 1] == b'/' {
        push(tokens, TokenKind::Comment, *i, len);
        *i = len;
        return None;
    }

    // ── Block comment (nesting-aware) ────────────────────────────────────
    if b == b'/' && *i + 1 < len && bytes[*i + 1] == b'*' {
        let start = *i;
        *i += 2;
        let depth = scan_block_comment(i, bytes, 1);
        push(tokens, TokenKind::Comment, start, *i);
        return (depth > 0).then_some(LineState::BlockComment(depth as u16));
    }

    // ── Backtick template string (with `${…}` interpolation) ─────────────
    if b == b'`' {
        let bt = *i;
        *i += 1;
        if scan_backtick_template(i, line, bytes, tokens, bt) {
            return None;
        }
        return Some(LineState::Str {
            quote: b'`',
            raw: false,
            hashes: 0,
            triple: false,
        });
    }

    // ── Double-quote string literal (single line) ────────────────────────
    if b == b'"' {
        let start = *i;
        *i += 1;
        while *i < len {
            if bytes[*i] == b'\\' && *i + 1 < len {
                *i += 2;
            } else if bytes[*i] == b'"' {
                *i += 1;
                break;
            } else {
                *i += 1;
            }
        }
        push(tokens, TokenKind::String, start, *i);
        return None;
    }

    // ── Char literal ─────────────────────────────────────────────────────
    if b == b'\'' {
        if let Some(end) = consume_char_literal(line, *i) {
            push(tokens, TokenKind::CharLit, *i, end);
            *i = end;
        } else {
            // Stray apostrophe — Rhai has no lifetime / raw-string usage.
            push(tokens, TokenKind::Punctuation, *i, *i + 1);
            *i += 1;
        }
        return None;
    }

    // ── Number ───────────────────────────────────────────────────────────
    if b.is_ascii_digit() || (b == b'.' && *i + 1 < len && bytes[*i + 1].is_ascii_digit()) {
        let start = *i;
        consume_number(i, bytes, NumberOpts::RUST_LIKE);
        push(tokens, TokenKind::Number, start, *i);
        return None;
    }

    // ── Identifier / Keyword / Type ──────────────────────────────────────
    if is_ident_start(b) {
        let start = *i;
        while *i < len && is_ident_continue(bytes[*i]) {
            *i += 1;
        }
        let word = &line[start..*i];
        let kind = if KEYWORDS.contains(&word) {
            TokenKind::Keyword
        } else if BUILTIN_TYPES.contains(&word)
            || word.chars().next().is_some_and(|c| c.is_uppercase())
        {
            TokenKind::TypeName
        } else {
            TokenKind::Identifier
        };
        push(tokens, kind, start, *i);
        return None;
    }

    // ── Operators ────────────────────────────────────────────────────────
    if matches!(
        b,
        b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'!' | b'<' | b'>' | b'&' | b'|' | b'^' | b'~'
    ) {
        let start = *i;
        *i += 1;
        if *i < len
            && matches!(
                (b, bytes[*i]),
                (b'=', b'=')
                    | (b'!', b'=')
                    | (b'<', b'=')
                    | (b'>', b'=')
                    | (b'-', b'>')
                    | (b'=', b'>')
                    | (b'&', b'&')
                    | (b'|', b'|')
                    | (b'+', b'=')
                    | (b'-', b'=')
                    | (b'*', b'=')
                    | (b'/', b'=')
                    | (b'*', b'*') // exponent
                    | (b'<', b'<')
                    | (b'>', b'>')
                    | (b'%', b'=')
                    | (b'&', b'=')
                    | (b'|', b'=')
                    | (b'^', b'=')
            )
        {
            *i += 1;
        }
        push(tokens, TokenKind::Operator, start, *i);
        return None;
    }

    // ── Range operators `..` / `..=` (e.g. `for x in 0..10`) ──────────────
    if b == b'.' && *i + 1 < len && bytes[*i + 1] == b'.' {
        let start = *i;
        *i += 2;
        if *i < len && bytes[*i] == b'=' {
            *i += 1;
        }
        push(tokens, TokenKind::Operator, start, *i);
        return None;
    }

    // ── `?.` null-safe access, `??` null-coalescing ──────────────────────
    if b == b'?' && *i + 1 < len && matches!(bytes[*i + 1], b'.' | b'?') {
        push(tokens, TokenKind::Operator, *i, *i + 2);
        *i += 2;
        return None;
    }

    // ── Punctuation ──────────────────────────────────────────────────────
    if matches!(
        b,
        b'(' | b')' | b'{' | b'}' | b'[' | b']' | b';' | b':' | b',' | b'.' | b'@' | b'?' | b'#'
    ) {
        push(tokens, TokenKind::Punctuation, *i, *i + 1);
        *i += 1;
        return None;
    }

    // ── Fallback: full Unicode scalar ────────────────────────────────────
    let ch_len = line[*i..].chars().next().map_or(1, |c| c.len_utf8());
    push(tokens, TokenKind::Identifier, *i, *i + ch_len);
    *i += ch_len;
    None
}

// ── Backtick template helpers ────────────────────────────────────────────────

/// Push a token spanning the byte range `[start, end)` (a no-op when empty).
#[inline]
fn push(tokens: &mut Vec<Token>, kind: TokenKind, start: usize, end: usize) {
    if end > start {
        tokens.push(Token {
            kind,
            start,
            len: end - start,
        });
    }
}

/// Scan a backtick-template body from `*i`, emitting `String` literal
/// segments, `${` / `}` punctuation, and normal Rhai tokens for the
/// interpolated expressions. `seg_start` marks where the pending String
/// segment begins — the opening backtick index for a fresh template, or `0`
/// when resuming a multi-line template. Returns `true` if the closing
/// backtick was found on this line.
fn scan_backtick_template(
    i: &mut usize,
    line: &str,
    bytes: &[u8],
    tokens: &mut Vec<Token>,
    mut seg_start: usize,
) -> bool {
    let len = bytes.len();
    while *i < len {
        match bytes[*i] {
            b'`' => {
                *i += 1;
                push(tokens, TokenKind::String, seg_start, *i);
                return true;
            }
            b'$' if *i + 1 < len && bytes[*i + 1] == b'{' => {
                push(tokens, TokenKind::String, seg_start, *i);
                push(tokens, TokenKind::Punctuation, *i, *i + 2);
                *i += 2;
                scan_interp_expr(i, line, bytes, tokens);
                seg_start = *i;
            }
            _ => *i += 1,
        }
    }
    push(tokens, TokenKind::String, seg_start, *i);
    false
}

/// Tokenize an interpolated expression inside `${ … }`, starting just past
/// the `${`. Braces are depth-tracked so the matching `}` (emitted as
/// Punctuation) ends the expression; braces inside nested strings/chars are
/// skipped because [`consume_token`] swallows those as single tokens. Stops
/// at end-of-line if the expression spills across lines.
fn scan_interp_expr(i: &mut usize, line: &str, bytes: &[u8], tokens: &mut Vec<Token>) {
    let len = bytes.len();
    let mut depth = 1u32;
    while *i < len {
        match bytes[*i] {
            b'{' => {
                depth += 1;
                push(tokens, TokenKind::Punctuation, *i, *i + 1);
                *i += 1;
            }
            b'}' => {
                depth -= 1;
                push(tokens, TokenKind::Punctuation, *i, *i + 1);
                *i += 1;
                if depth == 0 {
                    return;
                }
            }
            // Any multi-line construct opened here (`consume_token` returns
            // `Some`) drives `*i` to EOL, so the loop exits cleanly.
            _ => {
                let _ = consume_token(i, line, bytes, tokens);
            }
        }
    }
}
