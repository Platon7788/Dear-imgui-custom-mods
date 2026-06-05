//! Address-width policy, column geometry (offset / hex / ASCII),
//! address-literal formatting, inspector-height accessors, and per-byte
//! colour overrides + the search-match probe.

use crate::hex_viewer::config::AddressWidth;
use crate::hex_viewer::search::PatternByte;
use crate::hex_viewer::*;
use crate::utils::color::col32;

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
fn test_address_width_auto_saturates_at_u64_max() {
    // Hardening: base + len near u64::MAX must saturate (not wrap) and
    // still resolve to the wide 16-digit gutter without panicking.
    assert_eq!(AddressWidth::Auto.hex_digits(u64::MAX, usize::MAX), 16);
}

// ── Layout helpers (offset_col_width / ascii_col_x / address_flash) ─────────
//
// These pin the on-screen geometry contracts the draw + hit-test paths
// depend on. A stray `+1` in `offset_col_width` would silently shift
// every column right by one glyph.

#[test]
fn test_offset_col_width_compact_for_32bit_address() {
    // 8 hex digits + 1 trailing space (no colon — dropped 2026-04-30)
    // = 9 char advances.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]); // forces auto → 8-digit gutter
    v.config.address_width = AddressWidth::Bits32;
    v.char_advance = 7.0;
    assert_eq!(v.offset_col_width(), 63.0, "8 digits + 1 trailing space");
}

#[test]
fn test_offset_col_width_wide_for_64bit_address() {
    // 16 hex digits + 1 trailing space = 17 char advances.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    v.config.address_width = AddressWidth::Bits64;
    v.char_advance = 7.0;
    assert_eq!(v.offset_col_width(), 119.0, "16 digits + 1 trailing space");
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
    // char of trailing breathing room.
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
fn test_hex_col_width_accounts_for_group_spacing() {
    // 16 bpr, DWord grouping (every 4) → 4 groups → 3 extra spaces.
    // width = (16*3 + 3) * ca = 51 * ca.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    v.config.bytes_per_row = BytesPerRow::SIXTEEN;
    v.config.grouping = ByteGrouping::DWord;
    v.char_advance = 10.0;
    assert_eq!(v.hex_col_width(), 510.0, "(16*3 + 3) * 10");
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
    assert_eq!(v.inspector_height_px(), None);
}

#[test]
fn test_byte_colors_region() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0; 16]);
    v.config.category_colors = false;
    v.regions
        .push(ColorRegion::new(4, 4, [1.0, 0.0, 0.0, 1.0], "magic"));
    // `byte_fg_with_overrides` reads `self.byte_palette` for the
    // default path; the palette is only rebuilt inside `render()`, so
    // prime it here for the region-override branch under test.
    let fg = v.byte_fg_with_overrides(5, 0);
    assert_eq!(fg, col32([1.0, 0.0, 0.0, 1.0]));
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
