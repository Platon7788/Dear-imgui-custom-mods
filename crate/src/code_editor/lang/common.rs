//! Shared tokenizer primitives used across the per-language modules.
//!
//! These helpers are deliberately free of any language-specific policy —
//! each language tokenizer composes them with its own keyword tables and
//! branch ordering. Splitting them out of `lang/mod.rs` keeps that file
//! focused on the public trait / `Language` enum / dispatch surface.

/// ASCII letter or `_`.
#[inline]
pub(crate) fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// ASCII alphanumeric or `_`.
#[inline]
pub(crate) fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Number-literal parsing options. Each language picks the combo
/// matching its specification — see the `*_LIKE` constants.
#[derive(Clone, Copy)]
pub(in crate::code_editor::lang) struct NumberOpts {
    /// Allow `_` between digits as a visual separator.
    pub(in crate::code_editor::lang) underscore: bool,
    /// Allow `0x` / `0b` / `0o` radix prefixes.
    pub(in crate::code_editor::lang) radix: bool,
    /// Allow `.` decimal point and `e`/`E` exponent — applies only to
    /// non-radix decimals (radix literals are integer-only).
    pub(in crate::code_editor::lang) float: bool,
}

impl NumberOpts {
    /// JSON spec: decimal only, no underscores, float OK.
    pub(in crate::code_editor::lang) const JSON: Self = Self {
        underscore: false,
        radix: false,
        float: true,
    };
    /// Rust / RON / TOML / YAML 1.1 / Rhai: full radix + `_` + float.
    pub(in crate::code_editor::lang) const RUST_LIKE: Self = Self {
        underscore: true,
        radix: true,
        float: true,
    };
}

/// Consume a numeric literal at `bytes[*i..]`, advancing `*i` past it.
/// Returns `true` if any byte was consumed.
///
/// `*i` should already point at the first body digit (or at `0` for a
/// radix prefix, or at a leading `.` for a `.5`-style float). The
/// caller is responsible for any leading sign.
///
/// Type-suffix handling (e.g. Rust's `42_u8`) is **not** part of this
/// helper — the caller appends an `is_ident_start` / `is_ident_continue`
/// run after the call if its language supports suffixes.
pub(in crate::code_editor::lang) fn consume_number(
    i: &mut usize,
    bytes: &[u8],
    opts: NumberOpts,
) -> bool {
    let len = bytes.len();
    let start = *i;

    // ── Radix prefix (0x / 0b / 0o) ──────────────────────────────────────
    if opts.radix && *i + 1 < len && bytes[*i] == b'0' {
        let radix = match bytes[*i + 1] {
            b'x' | b'X' => Some(b'x'),
            b'b' | b'B' => Some(b'b'),
            b'o' | b'O' => Some(b'o'),
            _ => None,
        };
        if let Some(k) = radix {
            *i += 2;
            while *i < len {
                let c = bytes[*i];
                let valid = match k {
                    b'x' => c.is_ascii_hexdigit(),
                    b'b' => c == b'0' || c == b'1',
                    b'o' => (b'0'..=b'7').contains(&c),
                    _ => false,
                };
                if !(valid || (opts.underscore && c == b'_')) {
                    break;
                }
                *i += 1;
            }
            return *i > start;
        }
    }

    // ── Decimal body ─────────────────────────────────────────────────────
    while *i < len && (bytes[*i].is_ascii_digit() || (opts.underscore && bytes[*i] == b'_')) {
        *i += 1;
    }

    if opts.float {
        // Decimal point — only when followed by a digit, so `1..2` range
        // syntax doesn't get its dots eaten.
        if *i + 1 < len && bytes[*i] == b'.' && bytes[*i + 1].is_ascii_digit() {
            *i += 1;
            while *i < len && (bytes[*i].is_ascii_digit() || (opts.underscore && bytes[*i] == b'_'))
            {
                *i += 1;
            }
        }
        // Exponent
        if *i < len && (bytes[*i] == b'e' || bytes[*i] == b'E') {
            *i += 1;
            if *i < len && (bytes[*i] == b'+' || bytes[*i] == b'-') {
                *i += 1;
            }
            while *i < len && (bytes[*i].is_ascii_digit() || (opts.underscore && bytes[*i] == b'_'))
            {
                *i += 1;
            }
        }
    }

    *i > start
}

/// Try to consume a char literal at `line[i..]`.
///
/// Returns `Some(end_byte_index)` on success — the byte position
/// **after** the closing `'`. Returns `None` if the construct doesn't
/// look like a complete char literal (the caller falls back to the
/// next applicable branch — e.g. Rust lifetime, Rhai punctuation).
///
/// The helper advances by full UTF-8 code points (via
/// [`char::len_utf8`]), so non-ASCII chars like `'é'`, `'你'`, `'😀'`
/// classify as a single token rather than fragmenting into the
/// fallback bucket.
///
/// Recognised escape sequences:
/// - Single-character: `\n`, `\r`, `\t`, `\0`, `\\`, `\'`, `\"` etc.
///   (any byte after `\`).
/// - Hex: `\xHH` (one or two hex digits).
/// - Unicode: `\u{HHHHHH}` (1–6 hex digits in braces).
pub(in crate::code_editor::lang) fn consume_char_literal(line: &str, i: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();

    // Must start with single quote and have at least one body byte.
    if i + 1 >= len || bytes[i] != b'\'' {
        return None;
    }

    let mut p = i + 1;

    if bytes[p] == b'\\' {
        // Escape sequence.
        p += 1;
        if p >= len {
            return None;
        }
        match bytes[p] {
            b'x' => {
                // \xHH — up to 2 hex digits.
                p += 1;
                let mut count = 0;
                while count < 2 && p < len && bytes[p].is_ascii_hexdigit() {
                    p += 1;
                    count += 1;
                }
            }
            b'u' => {
                // \u{H..HHHH} — 1 to 6 hex digits in braces (Rust spec).
                p += 1;
                if p < len && bytes[p] == b'{' {
                    p += 1;
                    let mut digits = 0;
                    while digits < 6 && p < len && bytes[p].is_ascii_hexdigit() {
                        p += 1;
                        digits += 1;
                    }
                    if p < len && bytes[p] == b'}' {
                        p += 1;
                    }
                }
            }
            _ => {
                // Single-byte escape (\n, \r, \t, \\, \', etc.).
                p += 1;
            }
        }
    } else {
        // Non-escape body — consume one full UTF-8 codepoint.
        let c = line[p..].chars().next()?;
        p += c.len_utf8();
    }

    // Closing quote.
    if p < len && bytes[p] == b'\'' {
        Some(p + 1)
    } else {
        None
    }
}

/// Scan a `/* … */` block-comment body, supporting Rust-style **nested**
/// comments (`/* outer /* inner */ still-outer */`).
///
/// `*i` must point at the first byte to scan *inside* the comment (i.e.
/// the caller has already consumed the opening `/*` when `depth == 1`, or
/// `depth` reflects how many comments were still open at the end of the
/// previous line). Returns the depth still open at end-of-line: `0` means
/// the comment closed on this line, `>0` means it continues.
///
/// `*i` is advanced to the byte after the last consumed byte (the byte
/// after the closing `*/` when the comment closes, or `len` otherwise).
///
/// The editor's per-line carry state is a
/// [`LineState::BlockComment(depth)`](super::LineState::BlockComment), so the
/// returned depth is threaded across lines: nesting is tracked exactly both
/// within a line (`/* /* */ */` closes correctly) and across lines.
pub(in crate::code_editor::lang) fn scan_block_comment(
    i: &mut usize,
    bytes: &[u8],
    mut depth: u32,
) -> u32 {
    let len = bytes.len();
    while *i < len {
        if *i + 1 < len && bytes[*i] == b'/' && bytes[*i + 1] == b'*' {
            depth += 1;
            *i += 2;
        } else if *i + 1 < len && bytes[*i] == b'*' && bytes[*i + 1] == b'/' {
            depth -= 1;
            *i += 2;
            if depth == 0 {
                break;
            }
        } else {
            *i += 1;
        }
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ident_predicates() {
        assert!(is_ident_start(b'a'));
        assert!(is_ident_start(b'_'));
        assert!(!is_ident_start(b'1'));
        assert!(is_ident_continue(b'9'));
        assert!(!is_ident_continue(b'-'));
    }

    #[test]
    fn number_radix_and_float() {
        let mut i = 0;
        let b = b"0xDEAD_BEEF";
        assert!(consume_number(&mut i, b, NumberOpts::RUST_LIKE));
        assert_eq!(i, b.len());

        let mut i = 0;
        let b = b"3.14e-5";
        assert!(consume_number(&mut i, b, NumberOpts::RUST_LIKE));
        assert_eq!(i, b.len());

        // JSON: underscore not consumed.
        let mut i = 0;
        let b = b"1_000";
        consume_number(&mut i, b, NumberOpts::JSON);
        assert_eq!(i, 1);
    }

    #[test]
    fn number_does_not_eat_range_dots() {
        // `1..2` — the `.` must not be eaten as a decimal point.
        let mut i = 0;
        let b = b"1..2";
        consume_number(&mut i, b, NumberOpts::RUST_LIKE);
        assert_eq!(i, 1, "range `1..2` should stop after `1`");
    }

    #[test]
    fn char_literal_variants() {
        assert_eq!(consume_char_literal("'a'", 0), Some(3));
        assert_eq!(consume_char_literal(r"'\n'", 0), Some(4));
        assert_eq!(consume_char_literal(r"'\x41'", 0), Some(6));
        // Unterminated — no closing quote.
        assert_eq!(consume_char_literal("'a", 0), None);
        // Lifetime-shaped — `'a` with following ident byte, no quote.
        assert_eq!(consume_char_literal("'abc", 0), None);
    }

    #[test]
    fn block_comment_single_line_nesting() {
        // `/* a /* b */ c */` closes fully → depth 0.
        let b = b"/* a /* b */ c */";
        let mut i = 2; // caller consumed opening `/*`
        let depth = scan_block_comment(&mut i, b, 1);
        assert_eq!(depth, 0, "nested comment should close on the same line");
        assert_eq!(i, b.len());
    }

    #[test]
    fn block_comment_carries_over() {
        // `/* a /* b` leaves two open → depth 2 (collapsed to `true`).
        let b = b"/* a /* b";
        let mut i = 2;
        let depth = scan_block_comment(&mut i, b, 1);
        assert_eq!(depth, 2);
        assert_eq!(i, b.len());
    }

    #[test]
    fn block_comment_resume_closes() {
        // Resume with depth 1, line closes it.
        let b = b" still */ after";
        let mut i = 0;
        let depth = scan_block_comment(&mut i, b, 1);
        assert_eq!(depth, 0);
        assert_eq!(&b[i..], b" after");
    }
}
