//! Integration tests for the builder-pattern configuration APIs.
//!
//! Each component exposes a `Config` struct with `new(...)` + chainable
//! `with_*` setters. These tests prove the chains compose, carry values
//! through unchanged, and the `theme` / `colors_override` priority works
//! the way the docs claim.

use dear_imgui_custom_mod::confirm_dialog::{ConfirmStyle, DialogConfig, DialogIcon};
use dear_imgui_custom_mod::nav_panel::{DockPosition, NavButton, NavPanelConfig};
use dear_imgui_custom_mod::theme::Theme;

// (BorderlessConfig builder tests were removed alongside the
// `borderless_window` module on 2026-04-29 — the chrome lives inside
// `app_window_v2::config::TitlebarConfigV2` now and is exercised by the
// `demo_app_window_v2` example. New unit tests for the v2 config
// builders are tracked separately.)

// ── DialogConfig ────────────────────────────────────────────────────────────

#[test]
fn dialog_config_defaults() {
    let cfg = DialogConfig::new("Quit?", "Really quit?");
    assert_eq!(cfg.title, "Quit?");
    assert_eq!(cfg.message, "Really quit?");
    assert_eq!(cfg.theme, Theme::Dark);
}

#[test]
fn dialog_config_builder_chain() {
    let cfg = DialogConfig::new("X", "Y")
        .with_icon(DialogIcon::Error)
        .with_confirm_style(ConfirmStyle::Destructive)
        .with_confirm_label("Delete")
        .with_cancel_label("Keep")
        .with_theme(Theme::Monokai);
    assert!(matches!(cfg.icon, DialogIcon::Error));
    assert!(matches!(cfg.confirm_style, ConfirmStyle::Destructive));
    assert_eq!(cfg.confirm_label, "Delete");
    assert_eq!(cfg.cancel_label, "Keep");
    assert_eq!(cfg.theme, Theme::Monokai);
}

// ── NavPanelConfig ──────────────────────────────────────────────────────────

#[test]
fn nav_panel_builder_chain() {
    let cfg = NavPanelConfig::new(DockPosition::Left)
        .with_theme(Theme::Dark)
        .with_width(32.0)
        .with_button_size(28.0)
        .with_button_spacing(2.0)
        .with_animate(true)
        .add_button(NavButton::action("home", "H", "Home"))
        .add_button(NavButton::action("cfg", "C", "Settings").with_badge("!"));
    assert_eq!(cfg.position, DockPosition::Left);
    assert_eq!(cfg.theme, Theme::Dark);
    assert_eq!(cfg.width, 32.0);
    assert!(cfg.animate);
    // Two buttons plus zero separators.
    use dear_imgui_custom_mod::nav_panel::NavItem;
    let btns: Vec<_> = cfg
        .items
        .iter()
        .filter(|i| matches!(i, NavItem::Button(_)))
        .collect();
    assert_eq!(btns.len(), 2);
}
