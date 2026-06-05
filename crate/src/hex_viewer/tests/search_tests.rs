//! Address / pattern parsing, search execution, string encodings,
//! clipboard formatting, and parser property tests.

use crate::hex_viewer::config::{CopyFormat, HexSearchMode, StringEncoding};
use crate::hex_viewer::search::{
    PatternByte, base64_encode, find_pattern_masked, format_bytes, parse_address,
    parse_ascii_pattern, parse_hex_pattern_masked,
};
use crate::hex_viewer::*;

#[test]
fn test_parse_address_hex() {
    assert_eq!(parse_address("0x100"), Some(0x100));
    assert_eq!(parse_address("0xFF"), Some(0xFF));
    assert_eq!(parse_address("0X1A2B"), Some(0x1A2B));
}

#[test]
fn test_parse_address_decimal() {
    assert_eq!(parse_address("256"), Some(256));
    assert_eq!(parse_address("0"), Some(0));
}

#[test]
fn test_parse_hex_pattern_masked() {
    let p = parse_hex_pattern_masked("4D 5A ?? 00");
    assert_eq!(p.len(), 4);
    assert_eq!(p[0], PatternByte::Exact(0x4D));
    assert_eq!(p[2], PatternByte::Any);
}

#[test]
fn test_find_pattern_masked() {
    let data = [0x00, 0x4D, 0x5A, 0xFF, 0x00, 0x4D, 0x5A, 0x90];
    let pattern = parse_hex_pattern_masked("4D 5A ??");
    let results = find_pattern_masked(&data, &pattern);
    assert_eq!(results, vec![1, 5]);
}

#[test]
fn test_find_pattern_exact() {
    let data = [0x00, 0x4D, 0x5A, 0x00, 0x4D, 0x5A, 0x90];
    let pattern = parse_hex_pattern_masked("4D 5A");
    let results = find_pattern_masked(&data, &pattern);
    assert_eq!(results, vec![1, 4]);
}

#[test]
fn test_search_ascii() {
    let mut v = HexViewer::new("test");
    v.set_data(b"Hello World Hello");
    v.search_buf = "Hello".to_string();
    v.config.search_mode = HexSearchMode::String(StringEncoding::Ascii);
    v.do_search();
    assert_eq!(v.search_results, vec![0, 12]);
}

#[test]
fn test_search_utf8_cyrillic() {
    // Cyrillic "Тест" → UTF-8 wire bytes
    //   Т = D0 A2, е = D0 B5, с = D1 81, т = D1 82
    // Pattern matches at offsets where the full 8-byte sequence appears.
    let mut v = HexViewer::new("test");
    let needle: &[u8] = "Тест".as_bytes(); // 8 bytes
    let mut data = vec![0u8; 32];
    data.extend_from_slice(needle);
    data.extend_from_slice(b"  pad ");
    data.extend_from_slice(needle);
    v.set_data(&data);
    v.search_buf = "Тест".to_string();
    v.config.search_mode = HexSearchMode::String(StringEncoding::Utf8);
    v.do_search();
    assert_eq!(v.search_results, vec![32, 32 + 8 + 6]);
}

#[test]
fn test_search_utf16le_windows_string() {
    // Match "Hi" as UTF-16LE — the canonical Windows wchar_t encoding.
    //   H = 0x48 0x00, i = 0x69 0x00
    let mut v = HexViewer::new("test");
    let mut data = vec![0u8; 8];
    data.extend_from_slice(&[0x48, 0x00, 0x69, 0x00]); // "Hi"
    data.extend_from_slice(&[0xFF; 4]);
    data.extend_from_slice(&[0x48, 0x00, 0x69, 0x00]); // "Hi"
    v.set_data(&data);
    v.search_buf = "Hi".to_string();
    v.config.search_mode = HexSearchMode::String(StringEncoding::Utf16Le);
    v.do_search();
    assert_eq!(v.search_results, vec![8, 16]);
}

#[test]
fn test_string_encoding_byte_widths() {
    // Pin the wire-byte widths so a future regression can't silently
    // shift the byte count and break consumer pattern math.
    use crate::hex_viewer::search::{
        parse_ascii_pattern, parse_utf8_pattern, parse_utf16le_pattern,
    };
    assert_eq!(parse_ascii_pattern("AB").len(), 2, "ASCII = 1 byte/char");
    assert_eq!(
        parse_utf8_pattern("AB").len(),
        2,
        "UTF-8 ASCII = 1 byte/char"
    );
    assert_eq!(parse_utf8_pattern("Я").len(), 2, "UTF-8 Cyrillic = 2 bytes");
    assert_eq!(
        parse_utf16le_pattern("AB").len(),
        4,
        "UTF-16LE BMP = 2 bytes/char"
    );
    // Surrogate pair (𝄞 U+1D11E) → 2 UTF-16 code units = 4 wire bytes.
    assert_eq!(
        parse_utf16le_pattern("𝄞").len(),
        4,
        "UTF-16LE surrogate = 4 bytes"
    );
}

#[test]
fn test_utf16le_endianness_pinned() {
    // Pin LE byte order so a Big-Endian regression (or accidental
    // u16-as-be cast) stays fixable. ASCII char 'H' (U+0048) =
    // little-endian bytes `[0x48, 0x00]`, NOT `[0x00, 0x48]`.
    use crate::hex_viewer::search::parse_utf16le_pattern;
    let bytes: Vec<u8> = parse_utf16le_pattern("H")
        .into_iter()
        .map(|p| match p {
            PatternByte::Exact(b) => b,
            PatternByte::Any => unreachable!(),
        })
        .collect();
    assert_eq!(bytes, vec![0x48, 0x00]);
}

#[test]
fn test_format_bytes_hex_spaced() {
    assert_eq!(
        format_bytes(&[0x4D, 0x5A, 0x90], CopyFormat::HexSpaced, true),
        "4D 5A 90"
    );
}

#[test]
fn test_format_bytes_c_array() {
    assert_eq!(
        format_bytes(&[0x4D, 0x5A], CopyFormat::CArray, true),
        "{ 0x4D, 0x5A }"
    );
}

#[test]
fn test_format_bytes_base64() {
    assert_eq!(
        format_bytes(&[0x4D, 0x5A, 0x90], CopyFormat::Base64, true),
        "TVqQ"
    );
}

#[test]
fn test_base64_encode() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
}

#[test]
fn hex_search_mode_round_trip_through_do_search() {
    // Pre-fix coverage was only `HexSearchMode::String(...)`; this
    // pins that the Hex pattern path also resolves matches and
    // populates `search_results`. Wildcards (`??`) must match any
    // byte at that position.
    let mut v = HexViewer::new("test");
    v.set_data(&[0x4D, 0x5A, 0x90, 0x00, 0x4D, 0x5A, 0xAB, 0x00]);
    v.config.search_mode = HexSearchMode::Hex;
    v.search_buf = "4D 5A ?? 00".to_string();
    v.do_search();
    assert_eq!(
        v.search_results,
        vec![0, 4],
        "wildcard hex search must find both `MZ?? 00` matches"
    );
}

#[test]
fn do_search_at_buffer_tail_keeps_selection_in_bounds() {
    // Regression guard: a pattern that matches at the very end of the
    // buffer must produce a selection whose `end` is exactly `len`
    // (half-open `[start, len)`), never past it.
    let mut v = HexViewer::new("test");
    v.set_data(&[0x00, 0x00, 0x4D, 0x5A]); // "MZ" at the tail
    v.config.search_mode = HexSearchMode::Hex;
    v.search_buf = "4D 5A".to_string();
    v.do_search();
    assert_eq!(v.search_results, vec![2]);
    assert_eq!(v.selection.start, 2);
    assert_eq!(v.selection.end, 4, "selection end == buffer len, not past");
    assert_eq!(v.selection.end, v.data().len());
}

#[test]
fn unused_warnings_dont_fire_on_legacy_copy_format_path() {
    // Belt-and-braces: every `CopyFormat` variant compiles + formats.
    // A future `format_bytes` refactor can't silently drop a variant
    // without failing the build.
    let bytes = &[0x4D, 0x5Au8];
    for fmt in [
        CopyFormat::HexSpaced,
        CopyFormat::HexCompact,
        CopyFormat::RustArray,
        CopyFormat::CArray,
        CopyFormat::Ascii,
        CopyFormat::Base64,
    ] {
        let _ = format_bytes(bytes, fmt, false);
    }
}

// ── Property-based tests ─────────────────────────────────────────────
//
// Parsers walk user-controlled strings — a crash here is a DoS for
// hosts that hand arbitrary clipboard content to the viewer. These
// properties assert "never panic, always return sane output" on
// 256+ random inputs per property.

use proptest::prelude::*;

proptest! {
    /// `parse_address` must never panic regardless of what the user types.
    #[test]
    fn prop_parse_address_never_panics(s in ".{0,32}") {
        let _ = parse_address(&s);
    }

    /// Hex-prefix addresses `0x...` within 16 hex chars must round-trip
    /// through `parse_address` back to the same value.
    #[test]
    fn prop_parse_address_hex_roundtrips(value in any::<u64>()) {
        let s = format!("0x{value:X}");
        prop_assert_eq!(parse_address(&s), Some(value));
    }

    /// Short decimal addresses (≤ 4 chars, no hex ambiguity) must
    /// round-trip. `parse_address` auto-detects hex for longer
    /// all-hex-digit strings so > 4 chars is not a clean roundtrip
    /// domain for decimal — that's by design for UX.
    #[test]
    fn prop_parse_address_decimal_roundtrips(value in 0u64..10_000) {
        let s = value.to_string();
        if s.len() <= 4 {
            prop_assert_eq!(parse_address(&s), Some(value));
        }
    }

    /// `parse_hex_pattern_masked` must never panic on arbitrary string
    /// input and must always return a Vec (possibly empty).
    #[test]
    fn prop_parse_hex_pattern_never_panics(s in ".{0,64}") {
        let result = parse_hex_pattern_masked(&s);
        prop_assert!(result.len() <= s.len() + 1);
    }

    /// `parse_ascii_pattern` never panics. Result length equals the
    /// UTF-8 byte length (not char count) — the function iterates
    /// over `s.bytes()` for raw-byte search.
    #[test]
    fn prop_parse_ascii_pattern_never_panics(s in ".{0,64}") {
        let result = parse_ascii_pattern(&s);
        prop_assert_eq!(result.len(), s.len());
    }

    /// `find_pattern_masked` invariant: every returned index must fall
    /// within `data.len() - pattern.len()`. No off-by-one OOB reads.
    #[test]
    fn prop_find_pattern_in_bounds(
        data in prop::collection::vec(any::<u8>(), 0..256),
        pattern_str in "[0-9A-Fa-f ?\\*]{0,16}",
    ) {
        let pattern = parse_hex_pattern_masked(&pattern_str);
        if pattern.is_empty() {
            return Ok(());
        }
        let matches = find_pattern_masked(&data, &pattern);
        for &idx in &matches {
            prop_assert!(
                idx + pattern.len() <= data.len(),
                "match at {idx} OOB for data len {}", data.len()
            );
        }
    }
}
