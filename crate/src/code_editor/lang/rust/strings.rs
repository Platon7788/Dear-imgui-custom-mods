//! Multi-line string helpers for the Rust tokenizer.
//!
//! Rust string / raw / byte / c-string literals may span lines. These free
//! functions scan a (possibly continued) string body and build the carry
//! [`LineState`] the tokenizer threads back on the next line. Extracted from
//! [`super::tokenize`] to keep that file under the 500-line limit.

use crate::code_editor::config::LineState;

/// Carry state for an unclosed `"`-delimited string (regular / byte / c / raw).
#[inline]
pub(super) fn str_carry(raw: bool, hashes: u8) -> LineState {
    LineState::Str {
        quote: b'"',
        raw,
        hashes,
        triple: false,
    }
}

/// Scan a non-raw string body from `*i` (first byte after the opening quote).
/// Returns `true` if the closing `"` was found. Honours `\"` escapes; a
/// trailing `\` is a line continuation that leaves the string open.
///
/// Thin `&mut usize`-shaped wrapper over the shared
/// [`crate::code_editor::lang::scan_dq_string_body`] (the RON tokenizer uses
/// the positional-return shape directly).
pub(super) fn scan_str_body(i: &mut usize, bytes: &[u8]) -> bool {
    let (end, closed) = crate::code_editor::lang::scan_dq_string_body(bytes, *i);
    *i = end;
    closed
}

/// Scan a raw string body from `*i` for a closing `"` followed by exactly
/// `hashes` `#`. Advances `*i` past the close (or to EOL); returns closed?.
///
/// Thin `&mut usize`-shaped wrapper over the shared
/// [`crate::code_editor::lang::scan_raw_string_body`].
pub(super) fn scan_raw_str_body(i: &mut usize, bytes: &[u8], hashes: usize) -> bool {
    let (end, closed) = crate::code_editor::lang::scan_raw_string_body(bytes, *i, hashes);
    *i = end;
    closed
}
