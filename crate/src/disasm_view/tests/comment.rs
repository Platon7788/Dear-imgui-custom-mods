//! Theme-palette swap + `set_comment` round-trip.

use super::super::*;
use super::sample_provider;

// ── Theme integration ───────────────────────────────────────────────

use crate::theme::Theme;

#[test]
fn config_with_theme_replaces_palette() {
    // `with_theme` is a builder shortcut — it must replace the
    // embedded palette with the named theme's disasm-view colours.
    let dark = DisasmViewConfig::default().with_theme(Theme::Dark);
    let light = DisasmViewConfig::default().with_theme(Theme::Light);

    // Different themes => different surface colours (at minimum).
    assert_ne!(
        dark.colors.separator, light.colors.separator,
        "Dark and Light should expose distinct separator colours",
    );
    assert_ne!(
        dark.colors.header, light.colors.header,
        "Dark and Light should expose distinct header colours",
    );
}

#[test]
fn config_default_matches_dark_theme() {
    // Bare `DisasmViewConfig::default()` must reuse
    // `Theme::Dark.disasm_view_colors()` so a host that doesn't
    // pick a theme still gets the canonical Dark look.
    let default_cfg = DisasmViewConfig::default();
    let dark_palette = Theme::Dark.disasm_view_colors();
    assert_eq!(
        default_cfg.colors.mnemonic_normal,
        dark_palette.mnemonic_normal
    );
    assert_eq!(default_cfg.colors.address, dark_palette.address);
    assert_eq!(default_cfg.colors.selection_bg, dark_palette.selection_bg);
    assert_eq!(default_cfg.colors.breakpoint, dark_palette.breakpoint);
}

// ── set_comment / Comment edit round-trip ────────────────────────────

#[test]
fn set_comment_round_trip_via_vec_provider() {
    // Mutate-then-read: writing a comment via the trait method
    // must be visible through `Instruction::comment()` on the
    // very next frame (no buffering / async).
    let mut p = sample_provider();
    let addr = p.instruction(2).unwrap().address(); // 0x401004
    assert_eq!(p.instruction(2).unwrap().comment(), None);

    assert!(p.set_comment(addr, "stack alloca"));
    assert_eq!(p.instruction(2).unwrap().comment(), Some("stack alloca"));
}

#[test]
fn set_comment_clears_on_empty_string() {
    // Empty / whitespace-only input clears the comment so the
    // user can wipe a note by opening the editor and pressing
    // Enter on a blank buffer.
    let mut p = sample_provider();
    let addr = p.instruction(3).unwrap().address(); // 0x401008 (call)
    // Sample provider already attached "some_function" here.
    assert_eq!(p.instruction(3).unwrap().comment(), Some("some_function"));

    assert!(p.set_comment(addr, ""));
    assert_eq!(p.instruction(3).unwrap().comment(), None);

    // Whitespace-only must also clear (trim semantics).
    assert!(p.set_comment(addr, "first"));
    assert!(p.set_comment(addr, "   \t  "));
    assert_eq!(p.instruction(3).unwrap().comment(), None);
}

#[test]
fn set_comment_trims_surrounding_whitespace() {
    // Trim guards against accidental trailing whitespace from
    // clipboard pastes — stored value is canonicalised.
    let mut p = sample_provider();
    let addr = p.instruction(0).unwrap().address();
    assert!(p.set_comment(addr, "   prologue  "));
    assert_eq!(p.instruction(0).unwrap().comment(), Some("prologue"));
}

#[test]
fn set_comment_returns_false_for_unknown_address() {
    // Unknown address → no-op + false. Caller can then surface
    // the "address not decoded" diagnostic to the user.
    let mut p = sample_provider();
    assert!(!p.set_comment(0xDEAD_BEEF, "ghost"));
}
