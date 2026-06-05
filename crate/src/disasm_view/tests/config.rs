//! Config defaults / ron round-trip / locale + verbosity guards, watchpoint API, shrink-clamp regression, bulk bookmark restore.

use super::super::*;
use super::sample_provider;

// ── Session 035 audit follow-ups ─────────────────────────────────
//
// Watchpoint plumbing test — the `RW` gutter glyph and the
// single context-menu entry hang off a provider trait method
// that defaults to a no-op; pin the round-trip through
// `VecDisasmProvider` so a future refactor can't silently
// break the `Instruction::has_watchpoint` ↔ `toggle_watchpoint`
// contract. (Earlier sessions had separate `R` / `W` toggles —
// collapsed into a single watchpoint by user request:
// host-side engine sorts read-only / write-only on its side.)

#[test]
fn vec_provider_toggle_watchpoint_round_trip() {
    let mut p = sample_provider();
    let addr = 0x401000;
    let idx = p.index_of_address(addr).unwrap();
    assert!(!p.instruction(idx).unwrap().has_watchpoint());
    assert!(p.toggle_watchpoint(addr));
    assert!(p.instruction(idx).unwrap().has_watchpoint());
    assert!(!p.toggle_watchpoint(addr));
    assert!(!p.instruction(idx).unwrap().has_watchpoint());
}

#[test]
fn vec_provider_watchpoint_independent_of_breakpoint() {
    // Pin: setting a watchpoint does NOT touch the breakpoint
    // flag and vice versa. Renderer priority at draw.rs uses
    // `has_watchpoint` first, falling back to `bp_number > 0`,
    // and assumes the two are independent booleans.
    let mut p = sample_provider();
    let addr = 0x401004;
    assert!(p.toggle_watchpoint(addr));
    let idx = p.index_of_address(addr).unwrap();
    let i = p.instruction(idx).unwrap();
    assert!(i.has_watchpoint());
    assert!(!i.has_breakpoint(), "watchpoint must not flip breakpoint");
}

#[test]
fn provider_default_watchpoint_toggle_is_noop_false() {
    // Trait default: hosts that opt out of the watchpoint API
    // (e.g. simple read-only disassemblers) get a false-returning
    // no-op toggle so the context-menu entry doesn't crash.
    // Pin the default behaviour so a future trait refactor
    // doesn't accidentally make it required.
    struct ReadOnly;
    impl DisasmDataProvider for ReadOnly {
        fn instruction_count(&self) -> usize {
            0
        }
        fn instruction(&self, _idx: usize) -> Option<&dyn Instruction> {
            None
        }
        fn toggle_breakpoint(&mut self, _addr: u64) -> bool {
            false
        }
        fn decode_range(&mut self, _start_addr: u64, _max_count: usize) {}
        fn index_of_address(&self, _addr: u64) -> Option<usize> {
            None
        }
    }
    let mut ro = ReadOnly;
    assert!(!ro.toggle_watchpoint(0x1000));
}

#[test]
fn icons_available_default_is_true() {
    // Pin: MDI glyphs (BOOKMARK_CHECK_OUTLINE, wrench-cog)
    // render by default. Hosts without the MDI atlas opt out
    // by setting `view.config.icons_available = false`.
    let view = DisasmView::new("test");
    assert!(view.config.icons_available);
}

#[test]
fn first_row_clamped_when_count_shrinks() {
    // Defensive guard at mod.rs:1015 prevents `last_row -
    // first_row` from underflowing when the provider shrinks
    // between frames. Mirror the math here as a regression
    // pin (the actual call lives inside `render`, which can't
    // run without an ImGui context, but the saturation arithmetic
    // is independently verifiable).
    let scroll_y: f32 = 1000.0;
    let line_h: f32 = 18.0;
    let count: usize = 5;
    let first_row = ((scroll_y / line_h) as usize).min(count);
    assert!(first_row <= count);
    let visible_count = 30;
    let last_row = (first_row + visible_count).min(count);
    assert!(last_row >= first_row, "last_row must not underflow");
}

// ── Locale on `DisasmViewConfig` ─────────────────────────────────

#[test]
fn config_default_locale_is_english() {
    let cfg = DisasmViewConfig::default();
    assert_eq!(cfg.locale, crate::i18n::Locale::En);
}

#[test]
fn config_locale_round_trips_through_ron() {
    let cfg = DisasmViewConfig {
        locale: crate::i18n::Locale::Ru,
        ..DisasmViewConfig::default()
    };
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: DisasmViewConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.locale, crate::i18n::Locale::Ru);
}

#[test]
fn with_locale_updates_config_field() {
    // The view-level builder forwards into `config.locale`, so
    // `set_locale` mutations persist into the saved ron payload.
    let view = DisasmView::new("test").with_locale(crate::i18n::Locale::Ru);
    assert_eq!(view.config.locale, crate::i18n::Locale::Ru);
    assert_eq!(view.locale(), crate::i18n::Locale::Ru);
}

#[test]
fn columns_inline_in_config_ron_matches_canonical() {
    // `disasm_view/config.ron` inlines `columns:(...)` because ron
    // 0.8 has no `include`. This drift-test makes sure the inline
    // block stays in lock-step with `column_widths.ron`.
    let canonical = super::ColumnWidths::default();
    let cfg = DisasmViewConfig::default();
    assert_eq!(cfg.columns.margin, canonical.margin);
    assert_eq!(cfg.columns.arrows, canonical.arrows);
    assert_eq!(cfg.columns.address, canonical.address);
    assert_eq!(cfg.columns.bytes, canonical.bytes);
    assert_eq!(cfg.columns.mnemonic, canonical.mnemonic);
    assert_eq!(cfg.columns.operands, canonical.operands);
    assert_eq!(cfg.columns.comment, canonical.comment);
}

#[test]
fn config_locale_field_optional_in_ron() {
    // Older configs (saved before the locale field landed) still
    // parse — `#[serde(default)]` falls back to English. Pre-0.10.x
    // hosts depend on this for forward compatibility.
    let cfg: DisasmViewConfig = ron::from_str(
        r#"(
                columns: (
                    margin: 26.0,
                    arrows: 36.0,
                    address: 150.0,
                    bytes: 200.0,
                    mnemonic: 80.0,
                    operands: 220.0,
                    comment: 100.0,
                ),
                show_bytes: true,
                show_comments: true,
                show_arrows: true,
                show_breakpoints: true,
                show_bookmarks: true,
                icons_available: true,
                show_block_tints: false,
                show_header: true,
                show_column_dividers: true,
                uppercase: true,
                address_width_64: true,
                byte_category_colors: true,
                editable: false,
                follow_execution: false,
                base_address: 0,
                max_arrows: 256,
            )"#,
    )
    .expect("disasm_view config without `locale` field must still parse");
    assert_eq!(cfg.locale, crate::i18n::Locale::En);
}

#[test]
fn config_empty_ron_falls_back_to_per_field_defaults() {
    // Forward-compat regression: an empty `()` (the smallest
    // possible ron file the host could save against this schema)
    // must deserialise — every field is `#[serde(default = "fn")]`
    // since the audit pass, and each helper returns the
    // `config.ron`-canonical value.
    let cfg: DisasmViewConfig =
        ron::from_str("()").expect("empty config must deserialise via field defaults");
    let canon = DisasmViewConfig::default();
    assert_eq!(cfg.show_bytes, canon.show_bytes);
    assert_eq!(cfg.show_comments, canon.show_comments);
    assert_eq!(cfg.show_arrows, canon.show_arrows);
    assert_eq!(cfg.show_breakpoints, canon.show_breakpoints);
    assert_eq!(cfg.show_bookmarks, canon.show_bookmarks);
    assert_eq!(cfg.icons_available, canon.icons_available);
    assert_eq!(cfg.show_block_tints, canon.show_block_tints);
    assert_eq!(cfg.show_header, canon.show_header);
    assert_eq!(cfg.show_column_dividers, canon.show_column_dividers);
    assert_eq!(cfg.uppercase, canon.uppercase);
    assert_eq!(cfg.address_width_64, canon.address_width_64);
    assert_eq!(cfg.byte_category_colors, canon.byte_category_colors);
    assert_eq!(cfg.editable, canon.editable);
    assert_eq!(cfg.follow_execution, canon.follow_execution);
    assert_eq!(cfg.base_address, canon.base_address);
    assert_eq!(cfg.max_arrows, canon.max_arrows);
    assert_eq!(cfg.columns.margin, canon.columns.margin);
    assert_eq!(cfg.locale, canon.locale);
}

#[test]
fn config_partial_ron_keeps_explicit_overrides_per_field() {
    // Belt-and-braces: a partial config that overrides only a
    // few fields keeps those overrides and fills the rest from
    // the per-field helpers.
    let cfg: DisasmViewConfig =
        ron::from_str(r#"(show_bytes: false, max_arrows: 1024, uppercase: false)"#)
            .expect("partial config must deserialise");
    assert!(!cfg.show_bytes, "explicit override kept");
    assert!(!cfg.uppercase);
    assert_eq!(cfg.max_arrows, 1024);
    // Fallback for the rest.
    assert!(cfg.show_arrows);
    assert!(cfg.show_breakpoints);
    assert_eq!(cfg.columns.margin, 26.0);
}

#[test]
fn set_bookmarks_bulk_restore_caps_at_max() {
    // M1 from the audit — bulk restore replaces and caps.
    let mut v = DisasmView::new("test");
    let stored = v.set_bookmarks(0..(DisasmView::MAX_BOOKMARKS as u64 + 8));
    assert_eq!(stored, DisasmView::MAX_BOOKMARKS);
    assert_eq!(v.bookmark_count(), DisasmView::MAX_BOOKMARKS);
    // Replace with a smaller set — must drop the old contents.
    let stored2 = v.set_bookmarks([0x1000_u64, 0x2000, 0x3000]);
    assert_eq!(stored2, 3);
    assert_eq!(v.bookmark_count(), 3);
    assert!(v.is_bookmarked(0x2000));
    assert!(!v.is_bookmarked(0x0));
}

#[test]
fn with_config_replaces_entire_config() {
    // M2 from the audit — builder symmetry for the whole config.
    let cfg = DisasmViewConfig {
        show_arrows: false,
        max_arrows: 4,
        ..DisasmViewConfig::default()
    };
    let v = DisasmView::new("test").with_config(cfg);
    assert!(!v.config().show_arrows);
    assert_eq!(v.config().max_arrows, 4);
}

#[test]
fn config_mut_round_trips() {
    let mut v = DisasmView::new("test");
    assert!(v.config().show_bytes);
    v.config_mut().show_bytes = false;
    assert!(!v.config().show_bytes);
}

// ── HintVerbosity integration ───────────────────────────────

#[test]
fn config_default_verbosity_is_standard() {
    let cfg = DisasmViewConfig::default();
    assert_eq!(cfg.verbosity, super::HintVerbosity::Standard);
}

#[test]
fn config_verbosity_round_trips_through_ron() {
    let cfg = DisasmViewConfig {
        verbosity: super::HintVerbosity::Educational,
        ..DisasmViewConfig::default()
    };
    let s = ron::ser::to_string(&cfg).unwrap();
    let restored: DisasmViewConfig = ron::from_str(&s).unwrap();
    assert_eq!(restored.verbosity, super::HintVerbosity::Educational);
}

#[test]
fn config_verbosity_field_optional_in_ron() {
    // Forward-compat: older saved configs without `verbosity:`
    // still parse via `#[serde(default)]` and inherit Standard.
    let cfg: DisasmViewConfig = ron::from_str("()").expect("empty config");
    assert_eq!(cfg.verbosity, super::HintVerbosity::Standard);
}
