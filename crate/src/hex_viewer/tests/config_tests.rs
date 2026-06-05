//! Config defaults / ron round-trip / locale + i18n guard tests,
//! constructor sanity, data providers, byte category, and the public
//! popup-trigger API.

use crate::hex_viewer::*;

// ── Constructor + basic state ───────────────────────────────────────────────

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
fn test_goto() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0; 1024]);
    v.goto(512);
    assert_eq!(v.cursor(), 512);
}

#[test]
fn test_set_cursor_on_empty_buffer_is_safe() {
    // No data: cursor stays 0 and nothing panics (saturating clamp).
    let mut v = HexViewer::new("test");
    v.set_cursor(42);
    assert_eq!(v.cursor(), 0);
    assert_eq!(v.selected_bytes(), &[] as &[u8]);
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
fn test_bytes_per_row_new_clamps_and_rounds() {
    // Below 4 clamps up to 4; above 64 clamps to 64; non-multiples of
    // 4 round down.
    assert_eq!(BytesPerRow::new(0).value(), 4);
    assert_eq!(BytesPerRow::new(2).value(), 4);
    assert_eq!(BytesPerRow::new(17).value(), 16);
    assert_eq!(BytesPerRow::new(1000).value(), 64);
}

// ── Byte category ───────────────────────────────────────────────────────────

#[test]
fn test_byte_category() {
    assert_eq!(ByteCategory::of(0x00), ByteCategory::Zero);
    assert_eq!(ByteCategory::of(0x01), ByteCategory::Control);
    assert_eq!(ByteCategory::of(0x41), ByteCategory::Printable);
    assert_eq!(ByteCategory::of(0x80), ByteCategory::High);
    assert_eq!(ByteCategory::of(0xFF), ByteCategory::Full);
    // Boundaries: 0x1F is the last control, 0x20 the first printable,
    // 0x7E the last printable, 0x7F a control (DEL), 0xFE high.
    assert_eq!(ByteCategory::of(0x1F), ByteCategory::Control);
    assert_eq!(ByteCategory::of(0x20), ByteCategory::Printable);
    assert_eq!(ByteCategory::of(0x7E), ByteCategory::Printable);
    assert_eq!(ByteCategory::of(0x7F), ByteCategory::Control);
    assert_eq!(ByteCategory::of(0xFE), ByteCategory::High);
}

// ── Data providers ──────────────────────────────────────────────────────────

#[test]
fn test_vec_data_provider() {
    use crate::hex_viewer::provider::HexDataProvider;
    let mut p = VecDataProvider::new(vec![0x10, 0x20, 0x30, 0x40]);
    assert_eq!(p.len(), 4);
    let mut buf = [0u8; 2];
    assert_eq!(p.read(1, &mut buf), 2);
    assert_eq!(buf, [0x20, 0x30]);
    assert!(p.write(2, &[0xFF]));
    assert_eq!(p.data()[2], 0xFF);
}

#[test]
fn test_arc_vec_data_provider_read() {
    use crate::hex_viewer::provider::HexDataProvider;
    use std::sync::Arc;
    let arc = Arc::new(vec![0x10u8, 0x20, 0x30, 0x40]);
    let p = ArcVecDataProvider::from_arc(arc);
    assert_eq!(p.len(), 4);
    assert!(!p.is_empty());
    let mut buf = [0u8; 4];
    assert_eq!(p.read(0, &mut buf), 4);
    assert_eq!(buf, [0x10, 0x20, 0x30, 0x40]);
    // Partial read at the tail.
    let mut buf2 = [0u8; 3];
    assert_eq!(p.read(2, &mut buf2), 2);
    assert_eq!(&buf2[..2], &[0x30, 0x40]);
    // Read past end returns 0.
    let mut buf3 = [0u8; 4];
    assert_eq!(p.read(4, &mut buf3), 0);
    assert_eq!(p.read(100, &mut buf3), 0);
}

#[test]
fn test_arc_vec_data_provider_write_cow() {
    use crate::hex_viewer::provider::HexDataProvider;
    use std::sync::Arc;
    let arc = Arc::new(vec![0x10u8, 0x20, 0x30, 0x40]);
    let original = Arc::clone(&arc);
    let mut p = ArcVecDataProvider::from_arc(arc);
    // Sharing two Arcs at this point — write triggers `Arc::make_mut`
    // COW which clones the inner `Vec`.
    assert!(p.write(1, &[0xAA, 0xBB]));
    let mut buf = [0u8; 4];
    p.read(0, &mut buf);
    assert_eq!(buf, [0x10, 0xAA, 0xBB, 0x40]);
    // Original Arc still has the pre-write bytes — COW left it untouched.
    assert_eq!(*original, vec![0x10, 0x20, 0x30, 0x40]);
    // OOB write refuses.
    assert!(!p.write(10, &[0xFF]));
    assert!(!p.write(3, &[0xFF, 0xFF])); // straddles end
}

// ── Config defaults + DDD ron pattern ───────────────────────────────────────

#[test]
fn test_config_defaults() {
    let cfg = HexViewerConfig::default();
    assert_eq!(cfg.bytes_per_row, BytesPerRow::SIXTEEN);
    assert!(cfg.show_ascii);
    assert!(cfg.category_colors);
}

#[test]
fn config_color_offset_is_a_flat_field() {
    // Regression guard: if `HexViewerConfig` ever gains a nested
    // `colors: HexColors` struct, the popup code (`self.config.color_offset`)
    // must be updated in lockstep — this test would fail to compile.
    let v = HexViewer::new("test");
    let _check: [f32; 4] = v.config().color_offset;
    let _check_o: [f32; 4] = v.config().color_hex;
}

#[test]
fn icons_available_default_is_true() {
    let v = HexViewer::new("test");
    assert!(v.config().icons_available);
}

#[test]
fn config_ron_parses() {
    let cfg: HexViewerConfig = ron::from_str(include_str!("../config.ron"))
        .expect("hex_viewer/config.ron must parse against the current schema");
    assert!(cfg.category_colors);
}

#[test]
fn config_default_round_trips_through_ron() {
    let original = HexViewerConfig::default();
    let ron_text =
        ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default()).unwrap();
    let restored: HexViewerConfig = ron::from_str(&ron_text).unwrap();

    assert_eq!(original.bytes_per_row, restored.bytes_per_row);
    assert_eq!(original.grouping, restored.grouping);
    assert_eq!(original.show_ascii, restored.show_ascii);
    assert_eq!(original.show_inspector, restored.show_inspector);
    assert_eq!(original.address_width, restored.address_width);
    assert_eq!(original.uppercase, restored.uppercase);
    assert_eq!(original.endianness, restored.endianness);
    assert_eq!(original.editable, restored.editable);
    assert_eq!(original.base_address, restored.base_address);
    assert_eq!(original.search_mode, restored.search_mode);
    assert_eq!(original.copy_format, restored.copy_format);
    assert_eq!(original.max_undo, restored.max_undo);
    assert_eq!(original.locale, restored.locale);
}

// ── i18n guard tests (the four required by CLAUDE.md) ───────────────────────

#[test]
fn hex_viewer_strings_resolve() {
    // Both catalogues must resolve and carry non-empty key strings.
    let en = crate::i18n::hex_viewer::strings(crate::i18n::Locale::En);
    let ru = crate::i18n::hex_viewer::strings(crate::i18n::Locale::Ru);
    assert!(!en.goto_title.is_empty());
    assert!(!ru.goto_title.is_empty());
    assert!(!en.menu_settings.is_empty());
    assert!(!ru.menu_settings.is_empty());
    // EN and RU should differ for at least one user-visible label.
    assert_ne!(en.menu_settings, ru.menu_settings);
}

#[test]
fn default_locale_is_english() {
    let cfg = HexViewerConfig::default();
    assert_eq!(cfg.locale, crate::i18n::Locale::En);
    // And the widget mirrors it through the accessor.
    let v = HexViewer::new("test");
    assert_eq!(v.locale(), crate::i18n::Locale::En);
}

#[test]
fn locale_round_trips_through_ron() {
    // Round-trip a non-default locale to lock in that the field really
    // is `Serialize + Deserialize` (not silently skipped).
    let cfg = HexViewerConfig {
        locale: crate::i18n::Locale::Ru,
        ..HexViewerConfig::default()
    };
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: HexViewerConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.locale, crate::i18n::Locale::Ru);
}

#[test]
fn locale_field_optional_in_ron() {
    // `#[serde(default)]` lets older `config.ron` files (saved before
    // the locale field existed) deserialise without errors.
    let cfg: HexViewerConfig = ron::from_str(
        r#"(
            bytes_per_row: (16),
            grouping: DWord,
            show_ascii: true,
            show_inspector: true,
            show_offsets: true,
            show_column_headers: true,
            show_column_dividers: true,
            show_splitter: false,
            address_width: Auto,
            uppercase: true,
            endianness: Little,
            editable: false,
            base_address: 0,
            highlight_changes: false,
            category_colors: true,
            dim_zeros: true,
            auto_refresh_frames: 0,
            search_mode: Hex,
            copy_format: HexSpaced,
            max_undo: 256,
            icons_available: true,
        )"#,
    )
    .expect("hex_viewer config without `locale` field must still parse");
    assert_eq!(cfg.locale, crate::i18n::Locale::En);
}

#[test]
fn with_locale_and_set_locale_round_trip() {
    // Builder + setter both flip the stored locale; the getter mirrors.
    let v = HexViewer::new("test").with_locale(crate::i18n::Locale::Ru);
    assert_eq!(v.locale(), crate::i18n::Locale::Ru);
    let mut v2 = HexViewer::new("test");
    v2.set_locale(crate::i18n::Locale::Ru);
    assert_eq!(v2.locale(), crate::i18n::Locale::Ru);
    assert_eq!(v2.config().locale, crate::i18n::Locale::Ru);
}

// ── Public popup-trigger API ────────────────────────────────────────────────

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
fn show_settings_round_trip() {
    let mut v = HexViewer::new("test");
    assert!(!v.show_settings, "popup starts closed");
    v.show_settings = true;
    assert!(v.show_settings);
    v.show_settings = false;
    assert!(!v.show_settings);
}

#[test]
fn address_flash_is_initially_none() {
    let v = HexViewer::new("test");
    assert!(v.address_flash.is_none());
}

#[test]
fn component_center_initialised_to_origin() {
    let v = HexViewer::new("test");
    assert_eq!(v.component_center, [0.0, 0.0]);
    assert_eq!(v.popup_open_pos, [0.0, 0.0]);
}

// ── VA-native API ───────────────────────────────────────────────────────────

#[test]
fn contains_va_and_cursor_address_track_base_address() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 16]);
    v.config_mut().base_address = 0x1000;
    assert!(v.contains_va(0x1000));
    assert!(v.contains_va(0x100F));
    assert!(!v.contains_va(0x0FFF), "below base");
    assert!(!v.contains_va(0x1010), "at base + len (exclusive)");
    v.set_cursor(4);
    assert_eq!(v.cursor_address(), 0x1004);
}

#[test]
fn contains_va_false_on_empty_buffer() {
    let v = HexViewer::new("test");
    assert!(!v.contains_va(0));
    assert!(!v.contains_va(0x1000));
}

#[test]
fn goto_address_inside_buffer_moves_cursor() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 32]);
    v.config_mut().base_address = 0x4000;
    v.goto_address(0x4010);
    assert_eq!(v.cursor(), 0x10);
    assert_eq!(v.cursor_address(), 0x4010);
}
