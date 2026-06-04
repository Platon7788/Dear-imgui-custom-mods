//! Unit tests for `nav_panel` — config defaults / builders, state
//! transitions, theme resolution, i18n locale guards, and the pure
//! flyout-alignment layout math (no ImGui context required).

use super::*;

#[test]
fn default_config() {
    let cfg = NavPanelConfig::default();
    assert_eq!(cfg.position, DockPosition::Left);
    assert_eq!(cfg.width, 28.0);
    assert!(!cfg.auto_hide);
    assert!(!cfg.show_toggle);
    assert!(cfg.items.is_empty());
}

#[test]
fn builder_chain() {
    let cfg = NavPanelConfig::new(DockPosition::Right)
        .with_theme(crate::theme::Theme::Dark)
        .with_width(48.0)
        .with_auto_hide(true)
        .with_toggle_button(true)
        .with_animation_speed(10.0)
        .add_button(NavButton::action("home", "H", "Home").with_color([1.0, 0.0, 0.0, 1.0]))
        .add_separator()
        .add_button(
            NavButton::submenu("cfg", "C", "Config")
                .add_item(SubMenuItem::new("a", "Item A").with_icon("*"))
                .add_item(SubMenuItem::separator())
                .add_item(SubMenuItem::new("b", "Item B").with_shortcut("Ctrl+B")),
        );

    assert_eq!(cfg.position, DockPosition::Right);
    assert_eq!(cfg.width, 48.0);
    assert!(cfg.auto_hide);
    assert!(cfg.show_toggle);
    assert_eq!(cfg.items.len(), 3);
}

#[test]
fn state_active() {
    let mut s = NavPanelState::new();
    assert!(s.active.is_none());
    s.set_active("home");
    // Compare via deref so the test stays terse — `Cow<'static, str>`
    // derefs to `str`, and `Option::as_deref()` gives `Option<&str>`.
    assert_eq!(s.active.as_deref(), Some("home"));
    s.clear_active();
    assert!(s.active.is_none());
}

#[test]
fn state_active_accepts_runtime_string() {
    // The whole point of `Cow<'static, str>`: we can hand it a `String`
    // built at runtime — no Box::leak, no &'static str hacks.
    let mut s = NavPanelState::new();
    let runtime_id = format!("page_{}", 42);
    s.set_active(runtime_id);
    assert_eq!(s.active.as_deref(), Some("page_42"));
}

#[test]
fn state_visibility() {
    let mut s = NavPanelState::new();
    assert!(s.visible);
    s.hide();
    assert!(!s.visible);
    s.show();
    assert!(s.visible);
    s.toggle();
    assert!(!s.visible);
}

#[test]
fn state_close_submenu() {
    let mut s = NavPanelState::new();
    s.open_submenu = Some("cfg".into());
    s.close_submenu();
    assert!(s.open_submenu.is_none());
}

#[test]
fn all_builtin_themes_resolve() {
    for &theme in crate::theme::Theme::ALL {
        let c = theme.nav();
        assert!(c.bg.iter().all(|&v| (0.0..=1.0).contains(&v)));
        assert!(c.indicator[3] > 0.0);
    }
}

#[test]
fn hover_zoom_default_is_macos_dock_band() {
    // The default zoom factor sits in the visible-but-not-jarring
    // band (~`1.15`–`1.30`) — same magnification feel as the
    // macOS Dock. Set to exactly `1.0` to disable the effect.
    let cfg = NavPanelConfig::default();
    assert!(cfg.hover_zoom_scale > 1.0 && cfg.hover_zoom_scale <= 1.5);
}

#[test]
fn hover_zoom_scale_is_clamped() {
    // 1.0 floor — anything below collapses to "no zoom" / makes
    // no sense (would shrink the glyph). 3.0 ceiling — beyond
    // that the glyph overflows the button cell and clips visibly.
    let too_small = NavPanelConfig::default().with_hover_zoom_scale(0.5);
    assert!(too_small.hover_zoom_scale >= 1.0);
    let too_big = NavPanelConfig::default().with_hover_zoom_scale(10.0);
    assert!(too_big.hover_zoom_scale <= 3.0);
    // In-range values pass through unchanged.
    let mid = NavPanelConfig::default().with_hover_zoom_scale(1.35);
    assert_eq!(mid.hover_zoom_scale, 1.35);
}

#[test]
fn active_style_default_is_ring_with_orange() {
    // As of 2026-04-29 the default active-state visual is a
    // transparent ring around the icon (no background fill, no
    // indicator strip), tinted warm amber so it reads as
    // "orange focus" on every built-in theme. Flip back to the
    // historic filled-bar look via
    // `with_active_style(ActiveStyle::Bar)`.
    let cfg = NavPanelConfig::default();
    assert_eq!(cfg.active_style, ActiveStyle::Ring);
    let ring = cfg
        .active_ring_color
        .expect("ring colour must be Some by default");
    // Warm amber: red ≫ green > blue, fully opaque.
    assert!(
        ring[0] > 0.8,
        "default ring should be predominantly red/orange"
    );
    assert!(
        ring[0] > ring[1] && ring[1] > ring[2],
        "warm hue ordering R > G > B"
    );
    assert!(
        (ring[3] - 1.0).abs() < f32::EPSILON,
        "ring colour must be opaque"
    );
    assert!(cfg.active_ring_thickness > 0.0);
    assert!(cfg.active_ring_padding >= 0.0);
}

#[test]
fn active_style_can_opt_back_into_bar() {
    // Backwards-compatibility escape hatch — callers that want the
    // pre-2026-04-29 filled-bar look just call this builder.
    let cfg = NavPanelConfig::default().with_active_style(ActiveStyle::Bar);
    assert_eq!(cfg.active_style, ActiveStyle::Bar);
}

#[test]
fn active_style_ring_builders() {
    let cfg = NavPanelConfig::new(DockPosition::Left)
        .with_active_style(ActiveStyle::Ring)
        .with_active_ring_color([1.0, 0.65, 0.20, 1.0])
        .with_active_ring_thickness(2.0)
        .with_active_ring_padding(6.0);
    assert_eq!(cfg.active_style, ActiveStyle::Ring);
    assert_eq!(cfg.active_ring_color, Some([1.0, 0.65, 0.20, 1.0]));
    assert_eq!(cfg.active_ring_thickness, 2.0);
    assert_eq!(cfg.active_ring_padding, 6.0);
    // `without_active_ring_color` clears back to None (palette
    // indicator falls back through).
    let cleared = cfg.without_active_ring_color();
    assert!(cleared.active_ring_color.is_none());
}

#[test]
fn active_ring_thickness_clamped_above_min() {
    // 0.0 / negative values would render an invisible (or
    // backwards) stroke — clamp keeps the ring visible.
    let cfg = NavPanelConfig::default().with_active_ring_thickness(0.0);
    assert!(cfg.active_ring_thickness >= 0.5);
    let cfg = NavPanelConfig::default().with_active_ring_thickness(-3.0);
    assert!(cfg.active_ring_thickness >= 0.5);
}

#[test]
fn nav_button_builders() {
    let btn = NavButton::action("test", "T", "Test")
        .with_color([1.0, 0.5, 0.0, 1.0])
        .with_badge("3");
    assert_eq!(btn.id, "test");
    assert_eq!(btn.color, Some([1.0, 0.5, 0.0, 1.0]));
    assert_eq!(btn.badge.as_deref(), Some("3"));
    assert!(btn.submenu.is_empty());
}

#[test]
fn submenu_items() {
    let btn = NavButton::submenu("menu", "M", "Menu")
        .add_item(
            SubMenuItem::new("a", "Alpha")
                .with_icon("*")
                .with_shortcut("Ctrl+A"),
        )
        .add_separator()
        .add_item(SubMenuItem::new("b", "Beta"));
    assert_eq!(btn.submenu.len(), 3);
    assert!(matches!(&btn.submenu[1], SubMenuItem::Separator));
}

#[test]
fn dock_positions() {
    assert_eq!(DockPosition::default(), DockPosition::Left);
    assert_ne!(DockPosition::Left, DockPosition::Right);
}

#[test]
fn button_styles() {
    assert_eq!(ButtonStyle::default(), ButtonStyle::IconOnly);
    assert_ne!(ButtonStyle::IconOnly, ButtonStyle::IconWithLabel);
}

// ── i18n config guard tests (project-wide requirement) ───────────────────

#[test]
fn default_locale_is_english() {
    let cfg = NavPanelConfig::default();
    assert_eq!(cfg.locale, crate::i18n::Locale::En);
}

#[test]
fn locale_round_trips_through_ron() {
    let cfg = NavPanelConfig {
        locale: crate::i18n::Locale::Ru,
        ..NavPanelConfig::default()
    };
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: NavPanelConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.locale, crate::i18n::Locale::Ru);
}

#[test]
fn locale_field_optional_in_ron() {
    // Older configs without `locale:` must fall back to English.
    let cfg: NavPanelConfig = ron::from_str(
        r#"(
            position: Left,
            theme: Dark,
            width: 28.0,
            height: 24.0,
            button_size: 24.0,
            button_spacing: 4.0,
            button_style: IconOnly,
            indicator_thickness: 3.0,
            button_rounding: 6.0,
            hover_zoom_scale: 1.2,
            active_style: Ring,
            active_ring_color: Some((0.95, 0.62, 0.20, 1.0)),
            active_ring_thickness: 1.5,
            active_ring_padding: 4.0,
            separator_padding: 4.0,
            show_button_separators: true,
            show_toggle: false,
            auto_hide: false,
            auto_show_on_hover: true,
            edge_zone: 6.0,
            animate: true,
            animation_speed: 6.0,
            show_tooltips: true,
            submenu_min_width: 160.0,
            submenu_item_height: 26.0,
            content_offset_y: 0.0,
            content_offset_x: 0.0,
            items: [],
        )"#,
    )
    .expect("config without locale field must parse");
    assert_eq!(cfg.locale, crate::i18n::Locale::En);
}

#[test]
fn config_ron_round_trips() {
    // The built-in `config.ron` (loaded by `Default`) must survive a
    // serialize → deserialize cycle unchanged on its scalar fields.
    let cfg = NavPanelConfig::default();
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: NavPanelConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.position, cfg.position);
    assert_eq!(back.width, cfg.width);
    assert_eq!(back.button_size, cfg.button_size);
    assert_eq!(back.active_style, cfg.active_style);
    assert_eq!(back.active_ring_color, cfg.active_ring_color);
    assert_eq!(back.show_button_separators, cfg.show_button_separators);
    assert_eq!(back.locale, cfg.locale);
}

// ── Flyout-alignment layout math (regression for the off-by-spacing bug) ──

#[test]
fn flyout_offset_accounts_for_button_spacing() {
    // REGRESSION: the submenu-flyout walk used to advance by
    // `button_size` alone, dropping `button_spacing`, so every
    // button BEFORE the open submenu nudged the flyout anchor one
    // gap out of place. With three preceding buttons and a 4px gap
    // the trigger sits at `3 * (btn_s + 4)`, not `3 * btn_s`.
    let cfg = NavPanelConfig::new(DockPosition::Left)
        .with_button_spacing(4.0)
        .add_button(NavButton::action("a", "A", "A"))
        .add_button(NavButton::action("b", "B", "B"))
        .add_button(NavButton::action("c", "C", "C"))
        .add_button(NavButton::submenu("menu", "M", "Menu").add_item(SubMenuItem::new("x", "X")));
    let btn_s = 24.0_f32;
    let off =
        render::flyout_button_offset(&cfg, btn_s, 0.0, "menu").expect("submenu trigger present");
    let step = render::button_advance(&cfg, btn_s); // 24 + 4 = 28
    assert!((off - 3.0 * step).abs() < f32::EPSILON);
    // And the buggy `btn_s`-only walk would have produced a smaller,
    // wrong offset — prove they differ.
    assert!((off - 3.0 * btn_s).abs() > 1.0);
}

#[test]
fn flyout_offset_includes_toggle_head_and_separators() {
    let cfg = NavPanelConfig::new(DockPosition::Left)
        .with_button_spacing(2.0)
        .add_button(NavButton::action("a", "A", "A"))
        .add_separator()
        .add_button(NavButton::submenu("menu", "M", "Menu").add_item(SubMenuItem::new("x", "X")));
    let btn_s = 24.0_f32;
    let head = 10.0_f32; // toggle chevron already consumed this much
    let off = render::flyout_button_offset(&cfg, btn_s, head, "menu").unwrap();
    let expected = head + render::button_advance(&cfg, btn_s) + render::separator_advance(&cfg);
    assert!((off - expected).abs() < f32::EPSILON);
}

#[test]
fn flyout_offset_none_for_missing_or_empty_submenu() {
    let cfg = NavPanelConfig::new(DockPosition::Left)
        // A plain action button with the matching id but NO submenu
        // must NOT be treated as a flyout trigger.
        .add_button(NavButton::action("menu", "M", "Menu"));
    assert!(render::flyout_button_offset(&cfg, 24.0, 0.0, "menu").is_none());
    assert!(render::flyout_button_offset(&cfg, 24.0, 0.0, "nope").is_none());
}
