//! Integration tests for the builder-pattern configuration APIs.
//!
//! Each component exposes a `Config` struct with `new(...)` + chainable
//! `with_*` setters. These tests prove the chains compose, carry values
//! through unchanged, and the `theme` / `colors_override` priority works
//! the way the docs claim.

use dear_imgui_custom_mod::borderless_window::{
    BorderlessConfig, ButtonConfig, CloseMode, TitleAlign,
};
use dear_imgui_custom_mod::confirm_dialog::{ConfirmStyle, DialogConfig, DialogIcon};
use dear_imgui_custom_mod::nav_panel::{DockPosition, NavButton, NavPanelConfig};
use dear_imgui_custom_mod::theme::Theme;

// ── BorderlessConfig ────────────────────────────────────────────────────────

#[test]
fn borderless_config_defaults() {
    let cfg = BorderlessConfig::new("Test");
    assert_eq!(cfg.title, "Test");
    assert_eq!(cfg.theme, Theme::Dark);
    assert!(cfg.colors_override.is_none());
}

#[test]
fn borderless_config_with_theme_clears_override() {
    let mut colors = Theme::Light.titlebar();
    colors.bg = [1.0, 0.0, 0.0, 1.0];
    let cfg = BorderlessConfig::new("T")
        .with_colors(colors)
        .with_theme(Theme::Midnight);
    // with_theme after with_colors should clear the override so the new
    // theme's palette is used (documented contract).
    assert_eq!(cfg.theme, Theme::Midnight);
    assert!(
        cfg.colors_override.is_none(),
        "with_theme should clear override"
    );
}

#[test]
fn borderless_config_builder_chain() {
    let cfg = BorderlessConfig::new("NxT")
        .with_theme(Theme::Solarized)
        .with_titlebar_height(40.0)
        .with_resize_zone(8.0)
        .with_title_align(TitleAlign::Center)
        .with_close_mode(CloseMode::Confirm)
        .with_buttons(ButtonConfig {
            show_close: true,
            show_maximize: false,
            show_minimize: true,
            width: 50.0,
            icon_radius: 5.0,
            icon_hover_pad: 3.0,
            extra: Vec::new(),
        });
    assert_eq!(cfg.theme, Theme::Solarized);
    assert_eq!(cfg.titlebar_height, 40.0);
    assert_eq!(cfg.resize_zone, 8.0);
    assert_eq!(cfg.title_align, TitleAlign::Center);
    assert!(matches!(cfg.close_mode, CloseMode::Confirm));
    assert!(!cfg.buttons.show_maximize);
}

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
