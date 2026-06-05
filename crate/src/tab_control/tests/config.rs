//! Config / palette default pins, popup-id scoping, and the smooth-scroll
//! constant. These guard against silent regressions in `config.ron` /
//! `colors.rs` / `render`.

use super::super::*;

// ─── popup IDs ──────────────────────────────────────────────────────────────

#[test]
fn popup_ids_are_scoped_to_imgui_id() {
    let pc1: TabControl<super::Spy> = TabControl::new("##first");
    let pc2: TabControl<super::Spy> = TabControl::new("##second");
    assert_ne!(pc1.close_popup_id, pc2.close_popup_id);
    assert_ne!(pc1.overflow_popup_id, pc2.overflow_popup_id);
    assert!(pc1.close_popup_id.contains("##first"));
    assert!(pc2.close_popup_id.contains("##second"));
}

// ─── TabStatus / status_color ───────────────────────────────────────────────

#[test]
fn tab_status_default_is_active() {
    assert_eq!(TabStatus::default(), TabStatus::Active);
}

#[test]
fn tab_status_none_returns_neutral_color_via_palette() {
    let palette = TabColors::default();
    // None must not crash and must return a sensible neutral (status_inactive).
    let none_col = palette.status_color(TabStatus::None);
    let inactive_col = palette.status_color(TabStatus::Inactive);
    assert_eq!(none_col, inactive_col);
}

#[test]
fn show_status_dot_config_default_is_true() {
    let cfg = TabControlConfig::default();
    assert!(cfg.show_status_dot);
}

// ─── body-inset defaults ────────────────────────────────────────────────────

#[test]
fn body_inset_defaults_are_four_pixel_inset_enabled() {
    // Pin the contract: 4-pixel outer-edge inset on by default so a visible
    // gap sits between the outer window edges and the body child-rect. The
    // `[2.0, 2.0]` default was too subtle for the body-frame visual to
    // register; bumped to `[4.0, 4.0]` per user feedback 2026-04-30.
    let cfg = TabControlConfig::default();
    assert!(cfg.body_inset_enabled);
    assert_eq!(cfg.body_inset, [4.0, 4.0]);
}

#[test]
fn body_inset_border_defaults_off() {
    // Active-pane outline is opt-in by design.
    let cfg = TabControlConfig::default();
    assert!(!cfg.body_inset_border);
}

#[test]
fn body_inset_border_thickness_default_is_one_and_a_half() {
    let cfg = TabControlConfig::default();
    assert_eq!(cfg.body_inset_border_thickness, 1.5);
}

// ─── palette default pins ───────────────────────────────────────────────────

#[test]
fn body_bg_default_differs_from_strip_bg_for_visible_frame() {
    // body_bg is intentionally distinct from strip_bg so the body-frame visual
    // works (outer rect = strip_bg fills the gap, inner rect = body_bg fills
    // the body).
    let palette = TabColors::default();
    assert_ne!(palette.body_bg, palette.strip_bg);
}

#[test]
fn frame_bg_default_mirrors_strip_bg() {
    // Body-inset gap fills with `frame_bg`; the default mirror keeps the strip
    // + frame chrome visually unified.
    let palette = TabColors::default();
    assert_eq!(palette.frame_bg, palette.strip_bg);
}

#[test]
fn frame_border_default_matches_accent() {
    // Active-pane border defaults to the `accent` hue.
    let palette = TabColors::default();
    assert_eq!(palette.frame_border, palette.accent);
}

// ─── smooth-scroll constant ─────────────────────────────────────────────────

#[test]
fn smooth_scroll_coef_pinned_at_28() {
    // The smooth-scroll coefficient was bumped from the legacy `14.0` to
    // `28.0` in session 033. The constant lives in `render`'s private scope;
    // this test imports the `pub(super)` re-export to keep the value pinned at
    // the module boundary.
    use super::super::render::SMOOTH_SCROLL_COEF;
    assert!(
        (SMOOTH_SCROLL_COEF - 28.0).abs() < f32::EPSILON,
        "SMOOTH_SCROLL_COEF must stay at 28.0; got {SMOOTH_SCROLL_COEF}"
    );
}
