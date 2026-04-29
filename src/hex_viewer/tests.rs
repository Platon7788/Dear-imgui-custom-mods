//! Unit + property tests for the `hex_viewer` module.
//!
//! Tests reach into private fields (`v.cursor`, `v.selection`, etc.)
//! through `pub(super)` visibility — no API surface is exposed just
//! for testing. Free helper functions (`parse_address`,
//! `format_bytes`, …) are imported from their respective sub-modules.

use super::config::AddressWidth;
use super::draw::col32;
use super::input::EditColumn;
use super::search::{
    PatternByte, base64_encode, find_pattern_masked, format_bytes, parse_address,
    parse_ascii_pattern, parse_hex_pattern_masked,
};
use super::*;

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
    use super::search::{parse_ascii_pattern, parse_utf16le_pattern, parse_utf8_pattern};
    assert_eq!(parse_ascii_pattern("AB").len(), 2, "ASCII = 1 byte/char");
    assert_eq!(parse_utf8_pattern("AB").len(), 2, "UTF-8 ASCII = 1 byte/char");
    assert_eq!(parse_utf8_pattern("Я").len(), 2, "UTF-8 Cyrillic = 2 bytes");
    assert_eq!(parse_utf16le_pattern("AB").len(), 4, "UTF-16LE BMP = 2 bytes/char");
    // Surrogate pair (𝄞 U+1D11E) → 2 UTF-16 code units = 4 wire bytes.
    assert_eq!(parse_utf16le_pattern("𝄞").len(), 4, "UTF-16LE surrogate = 4 bytes");
}

#[test]
fn test_utf16le_endianness_pinned() {
    // Pin LE byte order so a Big-Endian regression (or accidental
    // u16-as-be cast) stays fixable. ASCII char 'H' (U+0048) =
    // little-endian bytes `[0x48, 0x00]`, NOT `[0x00, 0x48]`.
    use super::search::parse_utf16le_pattern;
    let bytes: Vec<u8> = parse_utf16le_pattern("H")
        .into_iter()
        .map(|p| match p {
            super::search::PatternByte::Exact(b) => b,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(bytes, vec![0x48, 0x00]);
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
fn test_address_width_auto_picks_32bit_for_small_buffer() {
    // 4 KiB at base 0 — fits comfortably in u32, must stay compact.
    assert_eq!(AddressWidth::Auto.hex_digits(0, 4096), 8);
}

#[test]
fn test_address_width_auto_picks_64bit_when_overflows_u32() {
    // base above u32::MAX → 64-bit gutter, regardless of length.
    assert_eq!(AddressWidth::Auto.hex_digits(0x1_0000_0000, 16), 16);
    // length pushes past u32::MAX → still 64-bit.
    assert_eq!(AddressWidth::Auto.hex_digits(u32::MAX as u64 - 4, 16), 16);
}

#[test]
fn test_address_width_explicit_overrides_auto() {
    // Explicit Bits32 keeps 8 even on a buffer that would auto-promote.
    assert_eq!(AddressWidth::Bits32.hex_digits(0x1_0000_0000, 0), 8);
    // Explicit Bits64 keeps 16 even on a tiny buffer.
    assert_eq!(AddressWidth::Bits64.hex_digits(0, 16), 16);
}

#[test]
fn test_inspector_height_accessors_round_trip() {
    let mut v = HexViewer::new("test");
    assert_eq!(v.inspector_height_px(), None, "fresh viewer = auto");
    v.set_inspector_height_px(120.0);
    assert_eq!(v.inspector_height_px(), Some(120.0));
    v.reset_inspector_height();
    assert_eq!(v.inspector_height_px(), None);
}

#[test]
fn test_inspector_height_setter_clamps_negative() {
    // Sentinel for "auto" is `0.0`; negative values must not poison it.
    let mut v = HexViewer::new("test");
    v.set_inspector_height_px(-50.0);
    assert_eq!(v.inspector_h, 0.0);
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

// ── Layout helpers (offset_col_width / ascii_col_x / address_flash) ─────────
//
// These pin the on-screen geometry contracts the draw + hit-test paths
// depend on. A stray `+1` in `offset_col_width` would silently shift
// every column right by one glyph and pre-2026-04-29-style "address text
// drowning in whitespace" would creep back in.

#[test]
fn test_offset_col_width_compact_for_32bit_address() {
    // 8 hex digits + `: ` (colon + trailing space from the format
    // string) = 10 char advances. After the 2026-04-29 padding-halve,
    // the cell no longer reserves an extra glyph of dead space.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]); // forces auto → 8-digit gutter
    v.config.address_width = AddressWidth::Bits32;
    v.char_advance = 7.0;
    assert_eq!(v.offset_col_width(), 70.0, "8 digits + 2 chars padding");
}

#[test]
fn test_offset_col_width_wide_for_64bit_address() {
    // 16 hex digits + `: ` = 18 char advances.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    v.config.address_width = AddressWidth::Bits64;
    v.char_advance = 7.0;
    assert_eq!(v.offset_col_width(), 126.0, "16 digits + 2 chars padding");
}

#[test]
fn test_offset_col_width_zero_when_offsets_hidden() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    v.config.show_offsets = false;
    v.char_advance = 7.0;
    assert_eq!(v.offset_col_width(), 0.0);
}

#[test]
fn test_ascii_col_x_right_anchors_when_room_available() {
    // Wide window: ASCII column floats to the right edge with one
    // char of trailing breathing room (so the bytes never sit under
    // the scrollbar).
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    v.char_advance = 10.0;
    v.inner_content_w = 1000.0;
    let win_x = 100.0;
    let bpr = v.config.bytes_per_row.value() as f32; // 16
    let expected_anchored = win_x + 1000.0 - bpr * 10.0 - 10.0;
    assert_eq!(v.ascii_col_x(win_x), expected_anchored);
}

#[test]
fn test_ascii_col_x_falls_back_to_natural_when_window_is_narrow() {
    // Narrow window: anchored position would land left of the
    // natural one (right after hex). The .max() clamp must keep the
    // ASCII column from overlapping the hex column.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    v.config.address_width = AddressWidth::Bits64;
    v.char_advance = 10.0;
    v.inner_content_w = 50.0; // way too small
    let win_x = 0.0;
    let origin_x = win_x + 10.0;
    let natural = origin_x + v.offset_col_width() + v.hex_col_width() + 10.0;
    assert_eq!(
        v.ascii_col_x(win_x),
        natural,
        "narrow window must fall back"
    );
}

#[test]
fn test_format_address_literal_matches_displayed_format() {
    // 16-digit uppercase: `0x` + 16 chars.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    v.config.address_width = AddressWidth::Bits64;
    v.config.uppercase = true;
    assert_eq!(v.format_address_literal(0x1234_ABCD), "0x000000001234ABCD");
    v.config.uppercase = false;
    assert_eq!(v.format_address_literal(0x1234_ABCD), "0x000000001234abcd");

    // 8-digit width.
    v.config.address_width = AddressWidth::Bits32;
    v.config.uppercase = true;
    assert_eq!(v.format_address_literal(0xCAFE), "0x0000CAFE");
}

// ── Public popup-trigger API ────────────────────────────────────────────────
//
// `request_goto` / `request_search` are the host-side entry points
// for the goto and search popups — they let callers wire global
// hotkeys (or menu items / toolbar buttons) without depending on
// the viewer being the focused widget. Pin the flag flips so a
// future refactor can't silently disconnect the public API from
// the popup state machine.

#[test]
fn request_goto_sets_show_flag() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    v.goto_buf = "stale".to_string();
    assert!(!v.show_goto, "popup starts closed");
    v.request_goto();
    assert!(v.show_goto, "request_goto must raise the open trigger");
    assert!(
        v.goto_buf.is_empty(),
        "request_goto must clear the input buffer"
    );
}

#[test]
fn request_search_sets_show_flag() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    assert!(!v.show_search);
    v.request_search();
    assert!(v.show_search);
}

#[test]
fn address_flash_is_initially_none() {
    // The address-gutter "just copied" highlight only fires after
    // a click in the address column. A freshly-constructed viewer
    // must not have a stale flash queued — would paint a mystery
    // pill on the first frame for no reason.
    let v = HexViewer::new("test");
    assert!(v.address_flash.is_none());
}

#[test]
fn component_center_initialised_to_origin() {
    // Component centre is captured each frame in `render()`. Until
    // the first render, it sits at `(0, 0)` — popups triggered via
    // `request_goto` before the viewer has rendered would anchor
    // at screen-origin, which is suboptimal but not a crash.
    // Pin the initial state so a future refactor doesn't smuggle a
    // garbage default in.
    let v = HexViewer::new("test");
    assert_eq!(v.component_center, [0.0, 0.0]);
    assert_eq!(v.popup_open_pos, [0.0, 0.0]);
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
