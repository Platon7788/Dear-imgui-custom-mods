//! INI / config-file line tokenizer.
//!
//! Split out of `ini/mod.rs` so the module stays under the 500-line
//! ceiling and mirrors the `rust/` / `ron/` / `yaml/` directory layout.
//! All helpers are `pub(super)` — only `ini::IniLang::tokenize_line`
//! reaches [`tokenize`].

use crate::code_editor::config::LineState;
use crate::code_editor::lang::{
    NumberOpts, consume_number, is_ident_continue, is_ident_start, scan_ws,
};
use crate::code_editor::token::{Token, TokenKind};

// ── Cross-line carry ─────────────────────────────────────────────────────────

/// The [`LineState`] INI uses to carry a backslash line-continuation from
/// one line to the next.
///
/// INI has no multi-line *string* construct — its only cross-line form is a
/// value whose physical line ends in a trailing `\`. Rather than widen the
/// public [`LineState`] enum (a semver-breaking change that would touch every
/// language), we repurpose the generic `Str` carry with a sentinel
/// `quote: b'\\'`. INI only ever *emits* this exact value; on *input* it
/// treats **any** `Str { .. }` carry as a continuation, so an out-of-family
/// carry handed in by the tiling harness still resolves to a well-formed
/// value line (the span-tiling invariant must hold for every state).
const CONT: LineState = LineState::Str {
    quote: b'\\',
    raw: true,
    hashes: 0,
    triple: false,
};

// ── Keyword table ─────────────────────────────────────────────────────────────

/// Boolean / null-ish literals recognised **case-insensitively** in value
/// position. Rendered as [`TokenKind::Keyword`]. Only classified in the value
/// tokenizer, so a key literally named `true` still reads as an attribute.
const KEYWORDS: &[&str] = &["true", "false", "yes", "no", "on", "off", "none", "null"];

fn is_keyword(word: &str) -> bool {
    KEYWORDS.iter().any(|k| word.eq_ignore_ascii_case(k))
}

// ── Small utilities ───────────────────────────────────────────────────────────

/// Push a token spanning `start..start+len`. A zero-length span is dropped —
/// callers can hand empty runs (trimmed key, missing whitespace) without a
/// guard and the tiling invariant is preserved (no empty spans to trip over).
fn push(tokens: &mut Vec<Token>, kind: TokenKind, start: usize, len: usize) {
    if len > 0 {
        tokens.push(Token { kind, start, len });
    }
}

/// In common INI dialects a `;`/`#` only opens an inline comment when it is
/// the first non-whitespace byte on the line or is preceded by whitespace.
/// A `;`/`#` glued to a preceding value byte (e.g. `pass#word`,
/// `http://a;b`) is an ordinary value character, not a comment marker.
fn is_comment_start(bytes: &[u8], i: usize) -> bool {
    i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t'
}

/// A physical line continues onto the next when it ends in an **odd** number
/// of trailing backslashes (a lone `\` continues; `\\` is an escaped literal
/// backslash and does not). The `\` must be the final byte — a value with a
/// trailing inline comment (`x = v \ ; c`) does not continue.
fn ends_with_continuation(bytes: &[u8]) -> bool {
    let mut n = 0usize;
    let mut j = bytes.len();
    while j > 0 && bytes[j - 1] == b'\\' {
        n += 1;
        j -= 1;
    }
    n % 2 == 1
}

/// `true` when a numeric literal starts at `bytes[i]` — a digit, a `.`
/// before a digit (`.5`), or a `+`/`-` sign before either.
fn is_number_start(bytes: &[u8], i: usize) -> bool {
    let digit_at = |j: usize| bytes.get(j).is_some_and(u8::is_ascii_digit);
    match bytes[i] {
        b'0'..=b'9' => true,
        b'.' => digit_at(i + 1),
        b'-' | b'+' => digit_at(i + 1) || (bytes.get(i + 1) == Some(&b'.') && digit_at(i + 2)),
        _ => false,
    }
}

/// If `bytes[i] == b'%'` opens a Windows-style `%NAME%` reference (ident
/// chars then a closing `%`), return the index just past the closing `%`.
/// Otherwise `None` — so a bare `%` (e.g. in `50%`) stays a value byte.
fn scan_percent_var(bytes: &[u8], i: usize) -> Option<usize> {
    let len = bytes.len();
    let mut j = i + 1;
    if j >= len || !is_ident_start(bytes[j]) {
        return None;
    }
    while j < len && is_ident_continue(bytes[j]) {
        j += 1;
    }
    (j < len && bytes[j] == b'%').then_some(j + 1)
}

// ── String tokenizer (with escape-sequence highlighting) ─────────────────────

/// Tokenize a quoted string starting at the opening quote `*i`, advancing
/// `*i` past the closing quote (or to end-of-line if unterminated).
///
/// Double-quoted strings honour `\`-escapes: normal runs render as
/// [`TokenKind::String`] and each escape (`\n`, `\t`, `\"`, `\\`, …) as
/// [`TokenKind::CharLit`], so escapes pop visually. Single-quoted strings
/// are literal — the whole thing is one `String` token, `\` included.
///
/// Every advance steps a full UTF-8 code point, so the emitted spans always
/// land on char boundaries (the renderer slices by them — a mid-codepoint cut
/// would panic).
fn tokenize_string(line: &str, tokens: &mut Vec<Token>, i: &mut usize) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let quote = bytes[*i];
    let escapes = quote == b'"';
    let mut run_start = *i; // the opening quote starts the first String run
    *i += 1;

    while *i < len {
        let b = bytes[*i];
        if escapes && b == b'\\' {
            // Flush the String run before the escape, then the escape itself.
            push(tokens, TokenKind::String, run_start, *i - run_start);
            let esc_start = *i;
            *i += 1; // the backslash
            if *i < len {
                let adv = line[*i..].chars().next().map_or(1, |c| c.len_utf8());
                *i += adv; // one full escaped code point (\n, \\, \你 …)
            }
            push(tokens, TokenKind::CharLit, esc_start, *i - esc_start);
            run_start = *i;
            continue;
        }
        if b == quote {
            *i += 1; // include the closing quote in the run
            break;
        }
        let adv = line[*i..].chars().next().map_or(1, |c| c.len_utf8());
        *i += adv;
    }

    // Trailing run — the close quote (or the rest of an unterminated string).
    push(tokens, TokenKind::String, run_start, *i - run_start);
}

// ── Value tokenizer ──────────────────────────────────────────────────────────

/// Tokenize the value region (right-hand side of `key =`, the tail of a
/// section line, or a bare line with no separator) from `*i` to end.
///
/// Recognises, in order: whitespace, inline comments, quoted strings,
/// `${VAR}` / `$VAR` / `%VAR%` interpolation (only at a token boundary),
/// numbers, keyword/boolean literals, and bare value runs.
fn tokenize_value(line: &str, tokens: &mut Vec<Token>, i: &mut usize) {
    let bytes = line.as_bytes();
    let len = bytes.len();

    while *i < len {
        let b = bytes[*i];

        // Whitespace.
        if b == b' ' || b == b'\t' {
            scan_ws(tokens, bytes, i);
            continue;
        }

        // Inline comment to end of line — only when the `;`/`#` opens one.
        if (b == b';' || b == b'#') && is_comment_start(bytes, *i) {
            push(tokens, TokenKind::Comment, *i, len - *i);
            *i = len;
            return;
        }

        // Quoted string.
        if b == b'"' || b == b'\'' {
            tokenize_string(line, tokens, i);
            continue;
        }

        // `${VAR}` / `$VAR` interpolation (token-boundary only).
        if b == b'$' && *i + 1 < len {
            let next = bytes[*i + 1];
            if next == b'{' {
                let start = *i;
                *i += 2;
                while *i < len && bytes[*i] != b'}' {
                    *i += 1;
                }
                if *i < len {
                    *i += 1; // include the closing `}`
                }
                push(tokens, TokenKind::MacroCall, start, *i - start);
                continue;
            }
            if is_ident_start(next) {
                let start = *i;
                *i += 1;
                while *i < len && is_ident_continue(bytes[*i]) {
                    *i += 1;
                }
                push(tokens, TokenKind::MacroCall, start, *i - start);
                continue;
            }
        }

        // `%VAR%` (Windows-style) interpolation (token-boundary only).
        if b == b'%'
            && let Some(end) = scan_percent_var(bytes, *i)
        {
            push(tokens, TokenKind::MacroCall, *i, end - *i);
            *i = end;
            continue;
        }

        // Number — signed, float, radix (`0x`/`0b`/`0o`), `_` separators.
        if is_number_start(bytes, *i) {
            let start = *i;
            if bytes[*i] == b'-' || bytes[*i] == b'+' {
                *i += 1;
            }
            consume_number(i, bytes, NumberOpts::RUST_LIKE);
            push(tokens, TokenKind::Number, start, *i - start);
            continue;
        }

        // Bare value run — up to whitespace or a comment-opening `;`/`#`
        // (a `;`/`#` glued to the value is absorbed, not a break). A run
        // that resolves to a boolean/null keyword renders as `Keyword`.
        let start = *i;
        while *i < len {
            let c = bytes[*i];
            if c == b' ' || c == b'\t' {
                break;
            }
            if (c == b';' || c == b'#') && is_comment_start(bytes, *i) {
                break;
            }
            *i += 1;
        }
        if *i == start {
            // Defensive: guarantee forward progress on any stray byte.
            let adv = line[start..].chars().next().map_or(1, |c| c.len_utf8());
            *i += adv;
        }
        let kind = if is_keyword(&line[start..*i]) {
            TokenKind::Keyword
        } else {
            TokenKind::Identifier
        };
        push(tokens, kind, start, *i - start);
    }
}

// ── Section-header tokenizer ─────────────────────────────────────────────────

/// Tokenize a section header starting at the `[` (`*i`).
///
/// The plain form `[database]` stays a single [`TokenKind::Attribute`] token
/// (bracket-to-bracket). The git-config form `[core "sub"]` pulls the quoted
/// sub-section out as a [`TokenKind::String`], leaving the `[core ` / `]`
/// pieces as attributes. Any trailing content (whitespace / inline comment)
/// is handed to [`tokenize_value`].
fn tokenize_section(line: &str, tokens: &mut Vec<Token>, i: &mut usize) {
    let bytes = line.as_bytes();
    let len = bytes.len();

    loop {
        // Attribute run — the brackets and bare section name up to the next
        // quoted sub-section or the closing `]`.
        let start = *i;
        while *i < len && bytes[*i] != b'"' && bytes[*i] != b']' {
            *i += 1;
        }
        if *i < len && bytes[*i] == b']' {
            *i += 1; // include the closing bracket
            push(tokens, TokenKind::Attribute, start, *i - start);
            break; // section closed
        }
        push(tokens, TokenKind::Attribute, start, *i - start);
        if *i >= len {
            break; // unterminated section header
        }
        // bytes[*i] == b'"' — a quoted sub-section (git-config style).
        tokenize_string(line, tokens, i);
    }

    tokenize_value(line, tokens, i);
}

// ── Line drivers ─────────────────────────────────────────────────────────────

/// Tokenize a fresh (non-continuation) line: blank, full-line comment,
/// section header, `key = value` / `key : value`, or a bare value line.
/// Returns the carry state (a value line ending in `\` opens [`CONT`]).
fn tokenize_fresh(line: &str) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(8);
    let mut i = 0;

    // Leading whitespace.
    scan_ws(&mut tokens, bytes, &mut i);
    if i >= len {
        return (tokens, LineState::Code);
    }

    // Full-line comment.
    if bytes[i] == b';' || bytes[i] == b'#' {
        push(&mut tokens, TokenKind::Comment, i, len - i);
        return (tokens, LineState::Code);
    }

    // Section header `[section]` / `[core "sub"]`.
    if bytes[i] == b'[' {
        tokenize_section(line, &mut tokens, &mut i);
        return (tokens, LineState::Code);
    }

    // Key = value / key : value — scan up to the first separator or comment.
    let key_start = i;
    while i < len {
        let c = bytes[i];
        if c == b'=' || c == b':' || c == b';' || c == b'#' {
            break;
        }
        i += 1;
    }
    let stop = i;
    let has_separator = stop < len && (bytes[stop] == b'=' || bytes[stop] == b':');

    if has_separator {
        // Trim trailing whitespace off the key so it colours cleanly.
        let mut key_end = stop;
        while key_end > key_start && (bytes[key_end - 1] == b' ' || bytes[key_end - 1] == b'\t') {
            key_end -= 1;
        }
        push(
            &mut tokens,
            TokenKind::Attribute,
            key_start,
            key_end - key_start,
        );
        push(&mut tokens, TokenKind::Whitespace, key_end, stop - key_end);
        push(&mut tokens, TokenKind::Operator, stop, 1);
        i = stop + 1;
    } else {
        // No `=`/`:` — the whole remainder is a value (may hold a comment).
        i = key_start;
    }

    tokenize_value(line, &mut tokens, &mut i);

    let end = if ends_with_continuation(bytes) {
        CONT
    } else {
        LineState::Code
    };
    (tokens, end)
}

/// Tokenize a continuation line (the previous physical line ended in `\`):
/// the whole line is one value region. Continues again when it too ends in
/// a trailing `\`.
fn tokenize_continuation(line: &str) -> (Vec<Token>, LineState) {
    let mut tokens = Vec::with_capacity(8);
    let mut i = 0;
    tokenize_value(line, &mut tokens, &mut i);
    let end = if ends_with_continuation(line.as_bytes()) {
        CONT
    } else {
        LineState::Code
    };
    (tokens, end)
}

/// Entry point — dispatch on the incoming carry state.
pub(super) fn tokenize(line: &str, state: LineState) -> (Vec<Token>, LineState) {
    match state {
        // Any string-carry is a backslash continuation (see [`CONT`]).
        LineState::Str { .. } => tokenize_continuation(line),
        // INI never emits block-comment / fenced / html / yaml carries;
        // treat any such (unexpected) carry as a fresh line.
        _ => tokenize_fresh(line),
    }
}
