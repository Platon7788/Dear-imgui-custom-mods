//! Unit + property tests for the `hex_viewer` module.
//!
//! Tests reach into private fields (`v.cursor`, `v.selection`, etc.)
//! through `pub(super)` visibility — no API surface is exposed just
//! for testing. Free helper functions (`parse_address`,
//! `format_bytes`, …) are imported from their respective sub-modules.

use super::*;
use super::draw::col32;
use super::input::EditColumn;
use super::search::{
    PatternByte, base64_encode, find_pattern_masked, format_bytes, parse_address,
    parse_ascii_pattern, parse_hex_pattern_masked,
};

#[test]
fn test_new_viewer() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0x41, 0x42, 0x43, 0x44]);
    assert_eq!(v.data().len(), 4);
    assert_eq!(v.cursor(), 0);
}

#[test]
fn test_set_cursor() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0; 256]);
    v.set_cursor(100);
    assert_eq!(v.cursor(), 100);
    v.set_cursor(9999);
    assert_eq!(v.cursor(), 255);
}

#[test]
fn test_selection() {
    let sel = Selection { start: 5, end: 10 };
    assert!(!sel.is_empty());
    assert_eq!(sel.len(), 5);
    assert!(sel.contains(5));
    assert!(sel.contains(9));
    assert!(!sel.contains(10));
}

#[test]
fn test_selection_reverse() {
    let sel = Selection { start: 10, end: 5 };
    assert_eq!(sel.ordered(), (5, 10));
    assert_eq!(sel.len(), 5);
    assert!(sel.contains(7));
}

#[test]
fn test_selected_bytes() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0x10, 0x20, 0x30, 0x40, 0x50]);
    v.selection = Selection { start: 1, end: 4 };
    assert_eq!(v.selected_bytes(), &[0x20, 0x30, 0x40]);
}

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
    v.config.search_mode = HexSearchMode::Ascii;
    v.do_search();
    assert_eq!(v.search_results, vec![0, 12]);
}

#[test]
fn test_byte_category() {
    assert_eq!(ByteCategory::of(0x00), ByteCategory::Zero);
    assert_eq!(ByteCategory::of(0x01), ByteCategory::Control);
    assert_eq!(ByteCategory::of(0x41), ByteCategory::Printable);
    assert_eq!(ByteCategory::of(0x80), ByteCategory::High);
    assert_eq!(ByteCategory::of(0xFF), ByteCategory::Full);
}

#[test]
fn test_undo_stack() {
    let mut stack = UndoStack::new(10);
    assert!(!stack.can_undo());
    stack.push(UndoEntry {
        offset: 0,
        old_bytes: vec![0xAA],
        new_bytes: vec![0xBB],
    });
    assert!(stack.can_undo());
    let entry = stack.undo().unwrap();
    assert_eq!(entry.old_bytes, vec![0xAA]);
    assert!(stack.can_redo());
}

#[test]
fn test_nav_history() {
    let mut nav = NavHistory::new(10);
    nav.push(0x1000);
    let back = nav.go_back(0x2000);
    assert_eq!(back, Some(0x1000));
    let fwd = nav.go_forward(0x1000);
    assert_eq!(fwd, Some(0x2000));
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
fn test_bytes_per_row_new_values() {
    assert_eq!(BytesPerRow::EIGHT.value(), 8);
    assert_eq!(BytesPerRow::TWELVE.value(), 12);
    assert_eq!(BytesPerRow::SIXTEEN.value(), 16);
    assert_eq!(BytesPerRow::TWENTY.value(), 20);
    assert_eq!(BytesPerRow::TWENTY_FOUR.value(), 24);
    assert_eq!(BytesPerRow::TWENTY_EIGHT.value(), 28);
    assert_eq!(BytesPerRow::THIRTY_TWO.value(), 32);
    assert_eq!(BytesPerRow::ALL.len(), 7);
}

#[test]
fn test_vec_data_provider() {
    let mut p = VecDataProvider::new(vec![0x10, 0x20, 0x30, 0x40]);
    assert_eq!(p.len(), 4);
    let mut buf = [0u8; 2];
    assert_eq!(p.read(1, &mut buf), 2);
    assert_eq!(buf, [0x20, 0x30]);
    assert!(p.write(2, &[0xFF]));
    assert_eq!(p.data()[2], 0xFF);
}

#[test]
fn test_config_defaults() {
    let cfg = HexViewerConfig::default();
    assert_eq!(cfg.bytes_per_row, BytesPerRow::SIXTEEN);
    assert!(cfg.show_ascii);
    assert!(cfg.category_colors);
}

#[test]
fn test_goto() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0; 1024]);
    v.goto(512);
    assert_eq!(v.cursor(), 512);
}

#[test]
fn test_byte_colors_region() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0; 16]);
    v.config.category_colors = false;
    v.regions
        .push(ColorRegion::new(4, 4, [1.0, 0.0, 0.0, 1.0], "magic"));
    let fg = v.byte_fg_with_overrides(5, 0);
    assert_eq!(fg, col32([1.0, 0.0, 0.0, 1.0]));
}

#[test]
fn test_is_search_match_binary() {
    // Regression: pre-fix `is_search_match` linear-scanned all matches.
    // Post-fix uses partition_point — guard that boundary cases still
    // hold (offsets just before / just after / spanning a match).
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 32]);
    v.search_results = vec![5, 12, 25];
    v.search_pattern = vec![PatternByte::Any; 3]; // covers [s, s+3)
    assert!(!v.is_search_match(4));
    assert!(v.is_search_match(5));
    assert!(v.is_search_match(7)); // last byte of first match
    assert!(!v.is_search_match(8));
    assert!(v.is_search_match(13));
    assert!(!v.is_search_match(15));
    assert!(v.is_search_match(27));
    assert!(!v.is_search_match(28));
}

#[test]
fn test_shift_arrow_anchors_selection() {
    // Regression: pre-fix Shift+Arrow only updated `selection.end`,
    // leaving `selection.start = 0` so growing selections always
    // started at offset 0. Post-fix anchors `start` at the previous
    // cursor position the moment the user begins selecting.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 32]);
    v.cursor = 10;
    v.move_cursor_with_selection(11, true);
    assert_eq!(v.selection.start, 10);
    assert_eq!(v.selection.end, 11);
    // Continued shift-extends keep the anchor.
    v.move_cursor_with_selection(13, true);
    assert_eq!(v.selection.start, 10);
    assert_eq!(v.selection.end, 13);
    // Releasing shift collapses selection.
    v.move_cursor_with_selection(14, false);
    assert!(v.selection.is_empty());
}

#[test]
fn test_commit_pending_edit_replaces_upper_nibble() {
    // Half-typed nibble must commit as upper-nibble replacement
    // (lower nibble of the original byte preserved). Mirrors HxD
    // behavior — gives the user a way to write a single nibble.
    let mut v = HexViewer::new("test");
    v.set_data(&[0xAB, 0xCD]);
    v.cursor = 0;
    v.edit_column = Some(EditColumn::Hex);
    v.edit_nibble = Some(0xF);
    v.commit_pending_edit();
    assert_eq!(v.data[0], 0xFB, "upper nibble replaced, lower kept");
    assert_eq!(v.edit_nibble, None, "nibble consumed");
    assert!(v.undo_stack().can_undo(), "undo entry pushed");
}

#[test]
fn test_commit_pending_edit_no_op_when_unchanged() {
    // Typing the same nibble that's already there must not
    // pollute undo history with a no-op entry.
    let mut v = HexViewer::new("test");
    v.set_data(&[0xAB]);
    v.cursor = 0;
    v.edit_nibble = Some(0xA); // upper already 0xA
    v.commit_pending_edit();
    assert_eq!(v.data[0], 0xAB);
    assert!(!v.undo_stack().can_undo(), "no undo for no-op");
}

#[test]
fn test_move_commits_pending_nibble() {
    // Arrow keys / page nav route through move_cursor_with_selection,
    // which must flush any half-typed nibble before moving.
    let mut v = HexViewer::new("test");
    v.set_data(&[0xAB, 0xCD]);
    v.cursor = 0;
    v.edit_column = Some(EditColumn::Hex);
    v.edit_nibble = Some(0x9);
    v.move_cursor_with_selection(1, false);
    assert_eq!(v.cursor, 1);
    assert_eq!(v.data[0], 0x9B, "nibble flushed before move");
    assert_eq!(v.edit_nibble, None);
}

#[test]
fn test_set_cursor_clears_selection_and_pushes_nav() {
    // Regression: pre-fix `set_cursor` left stale selection in place
    // and only pushed nav-history for jumps > bytes_per_row.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 64]);
    v.selection = Selection { start: 4, end: 10 };
    v.cursor = 4;
    v.set_cursor(8); // small jump (within one row)
    assert!(v.selection.is_empty(), "selection must clear on goto");
    assert!(v.nav_history().can_go_back(), "nav must record the jump");
}

#[test]
fn test_byte_colors_changed() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0xAA, 0xBB, 0xCC]);
    v.set_reference(&[0xAA, 0xCC, 0xCC]);
    v.config.highlight_changes = true;
    let fg1 = v.byte_fg_with_overrides(1, 0xBB);
    assert_eq!(fg1, col32(v.config.color_changed));
}

// ── Property-based tests ─────────────────────────────────────────────
//
// Parsers walk user-controlled strings — a crash here is a DoS for
// hosts that hand arbitrary clipboard content to the viewer. These
// properties assert "never panic, always return sane output" on
// 256+ random inputs per property.

use proptest::prelude::*;

proptest! {
    // Under default config (256 cases per prop).

    /// `parse_address` must never panic regardless of what the user types.
    /// Output is either `None` (input was garbage) or `Some(value)` where
    /// `value` fits in u64 (tautological, but proves the function returns).
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
    /// input and must always return a Vec (possibly empty) — no
    /// intermediate allocations leak.
    #[test]
    fn prop_parse_hex_pattern_never_panics(s in ".{0,64}") {
        let result = parse_hex_pattern_masked(&s);
        // Every byte is either Fixed(u8) or Wildcard — no invariants
        // to check beyond "didn't panic".
        prop_assert!(result.len() <= s.len() + 1);
    }

    /// `parse_ascii_pattern` never panics. Result length equals the
    /// UTF-8 byte length (not char count) — the function iterates
    /// over `s.bytes()` for raw-byte search, a deliberate choice so
    /// the search matches on wire bytes rather than Unicode scalars.
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
