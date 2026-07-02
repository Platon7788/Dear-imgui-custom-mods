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
pub(super) fn scan_str_body(i: &mut usize, bytes: &[u8]) -> bool {
    let len = bytes.len();
    while *i < len {
        if bytes[*i] == b'\\' {
            *i += if *i + 1 < len { 2 } else { 1 };
        } else if bytes[*i] == b'"' {
            *i += 1;
            return true;
        } else {
            *i += 1;
        }
    }
    false
}

/// Scan a raw string body from `*i` for a closing `"` followed by exactly
/// `hashes` `#`. Advances `*i` past the close (or to EOL); returns closed?.
pub(super) fn scan_raw_str_body(i: &mut usize, bytes: &[u8], hashes: usize) -> bool {
    let len = bytes.len();
    while *i < len {
        if bytes[*i] == b'"' {
            let mut end_hashes = 0;
            let mut j = *i + 1;
            while j < len && bytes[j] == b'#' && end_hashes < hashes {
                end_hashes += 1;
                j += 1;
            }
            if end_hashes == hashes {
                *i = j;
                return true;
            }
        }
        *i += 1;
    }
    false
}
