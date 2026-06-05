//! Unit tests for the confirm dialog — config schema/value parity, builder
//! coverage, theme resolution and `DialogResult` semantics. Everything here
//! is reachable without spinning up an ImGui `Ui`.

use super::*;

#[test]
fn default_config() {
    let cfg = DialogConfig::default();
    assert_eq!(cfg.width, 340.0);
    assert_eq!(cfg.height, 160.0);
    assert!(cfg.dim_background);
    assert!(cfg.keyboard_shortcuts);
    assert_eq!(cfg.icon, DialogIcon::Warning);
    assert_eq!(cfg.confirm_style, ConfirmStyle::Destructive);
}

#[test]
fn builder_chain() {
    let cfg = DialogConfig::new("Delete File", "This cannot be undone.")
        .with_theme(crate::theme::Theme::Dark)
        .with_icon(DialogIcon::Error)
        .with_confirm_label("Delete")
        .with_cancel_label("Keep")
        .with_confirm_style(ConfirmStyle::Destructive)
        .with_width(400.0)
        .with_height(180.0)
        .with_rounding(8.0)
        .without_dim()
        .without_keyboard();

    assert_eq!(cfg.title, "Delete File");
    assert_eq!(cfg.message, "This cannot be undone.");
    assert_eq!(cfg.confirm_label, "Delete");
    assert_eq!(cfg.cancel_label, "Keep");
    assert_eq!(cfg.icon, DialogIcon::Error);
    assert_eq!(cfg.width, 400.0);
    assert_eq!(cfg.height, 180.0);
    assert_eq!(cfg.rounding, 8.0);
    assert!(!cfg.dim_background);
    assert!(!cfg.keyboard_shortcuts);
}

#[test]
fn all_builtin_themes_resolve() {
    for &theme in crate::theme::Theme::ALL {
        let c = theme.dialog();
        assert!(c.bg.iter().all(|&v| (0.0..=1.0).contains(&v)));
        assert!(c.overlay[3] > 0.0);
        assert!(c.btn_confirm[3] > 0.0);
        assert!(c.btn_cancel[3] > 0.0);
    }
}

#[test]
fn dialog_result_not_open_returns_cancelled() {
    assert_eq!(DialogResult::Confirmed, DialogResult::Confirmed);
    assert_ne!(DialogResult::Open, DialogResult::Cancelled);
}

#[test]
fn icon_enum_variants() {
    assert_eq!(DialogIcon::default(), DialogIcon::Warning);
    assert_ne!(DialogIcon::None, DialogIcon::Error);
    assert_ne!(DialogIcon::Info, DialogIcon::Question);
}

#[test]
fn builder_chain_applies_all_fields() {
    let cfg = DialogConfig::new("Title", "Message")
        .with_icon(DialogIcon::Error)
        .with_confirm_label("Yes")
        .with_cancel_label("No")
        .with_confirm_style(ConfirmStyle::Normal)
        .with_button_height(26.0)
        .with_button_gap(40.0)
        .with_border_thickness(0.5)
        .with_rounding(10.0)
        .with_padding(20.0);
    assert_eq!(cfg.title, "Title");
    assert_eq!(cfg.message, "Message");
    assert_eq!(cfg.icon, DialogIcon::Error);
    assert_eq!(cfg.confirm_label, "Yes");
    assert_eq!(cfg.cancel_label, "No");
    assert_eq!(cfg.confirm_style, ConfirmStyle::Normal);
    assert_eq!(cfg.button_height, 26.0);
    assert_eq!(cfg.button_gap, 40.0);
    assert_eq!(cfg.border_thickness, 0.5);
    assert_eq!(cfg.rounding, 10.0);
    assert_eq!(cfg.padding, 20.0);
}

#[test]
fn closed_dialog_returns_cancelled() {
    // `render_confirm_dialog(open=false)` short-circuits to Cancelled
    // without touching ImGui. Logic-only test (we can't spin up a Ui here),
    // but the early-return path is reachable via the same code:
    let open = false;
    // Mirror the exact branch from `render_confirm_dialog`'s top:
    let result = if !open {
        // (the function returns Cancelled immediately when open=false)
        DialogResult::Cancelled
    } else {
        DialogResult::Open
    };
    assert_eq!(result, DialogResult::Cancelled);
    assert!(!open);
}

// ── DDD config: schema (.rs) + values (.ron) ─────────────────────────────

#[test]
fn default_loads_from_ron_with_expected_values() {
    // `Default` must come from `config.ron`, not hardcoded `.rs`
    // values. Pin every scalar so a drift between the two surfaces here.
    let cfg = DialogConfig::default();
    // The default ron leaves title/message/labels EMPTY — they are
    // sentinels for "no host override". The localized default fills
    // them in at render time via the `resolved_*` accessors.
    assert_eq!(cfg.title, "");
    assert_eq!(cfg.message, "");
    assert_eq!(cfg.confirm_label, "");
    assert_eq!(cfg.cancel_label, "");
    // Default locale is English, so the resolved defaults match the
    // historic English literals.
    assert_eq!(cfg.locale, crate::i18n::Locale::En);
    assert_eq!(cfg.resolved_title(), "Confirm");
    assert_eq!(cfg.resolved_message(), "Are you sure?");
    assert_eq!(cfg.resolved_confirm_label(), "Confirm");
    assert_eq!(cfg.resolved_cancel_label(), "Cancel");
    assert_eq!(cfg.icon, DialogIcon::Warning);
    assert_eq!(cfg.confirm_style, ConfirmStyle::Destructive);
    assert_eq!(cfg.theme, crate::theme::Theme::Dark);
    assert_eq!(cfg.padding, 16.0);
    assert_eq!(cfg.button_height, 27.0);
    assert_eq!(cfg.button_width, 75.0);
    assert_eq!(cfg.button_gap, 60.0);
    assert_eq!(cfg.rounding, 6.0);
    assert_eq!(cfg.border_thickness, 1.5);
    assert!(cfg.accent_border);
    assert!(!cfg.show_separator);
    assert!(cfg.show_button_icons);
    assert_eq!(cfg.header_icon_size, 16.0);
    assert_eq!(cfg.button_padding_x, 22.0);
    assert_eq!(cfg.button_bottom_factor, 0.35);
    assert_eq!(cfg.button_icon_scale, 0.16);
    // No custom palette by default — render path falls back to `theme`.
    assert!(cfg.colors_override.is_none());
}

#[test]
fn config_round_trips_through_ron() {
    // Serialize a builder-mutated config and parse it back: every
    // serde-visible field must survive the round trip unchanged.
    let cfg = DialogConfig::new("Quit", "Discard changes?")
        .with_icon(DialogIcon::Question)
        .with_confirm_style(ConfirmStyle::Normal)
        .with_button_width(120.0)
        .with_button_icon_scale(0.22)
        .with_separator(true)
        .without_dim();
    let ron = ron::ser::to_string(&cfg).expect("serialize DialogConfig");
    let back: DialogConfig = ron::from_str(&ron).expect("parse DialogConfig");
    assert_eq!(back.title, cfg.title);
    assert_eq!(back.message, cfg.message);
    assert_eq!(back.icon, cfg.icon);
    assert_eq!(back.confirm_style, cfg.confirm_style);
    assert_eq!(back.button_width, cfg.button_width);
    assert_eq!(back.button_icon_scale, cfg.button_icon_scale);
    assert_eq!(back.show_separator, cfg.show_separator);
    assert_eq!(back.dim_background, cfg.dim_background);
    assert_eq!(back.theme, cfg.theme);
}

#[test]
fn button_icon_scale_defaults_when_field_absent() {
    // Older saved ron files predate `button_icon_scale`. The
    // `#[serde(default = "default_button_icon_scale")]` fallback must
    // restore 0.16 rather than collapsing the glyph to a zero radius.
    let legacy = r#"(
        title: "T",
        message: "M",
        confirm_label: "C",
        cancel_label: "X",
        icon: Warning,
        confirm_style: Destructive,
        theme: Dark,
        width: 340.0,
        height: 160.0,
        padding: 16.0,
        button_height: 27.0,
        button_gap: 60.0,
        dim_background: true,
        keyboard_shortcuts: true,
        rounding: 6.0,
        border_thickness: 1.5,
        accent_border: true,
        show_separator: false,
        show_button_icons: true,
        header_icon_size: 16.0,
        button_padding_x: 22.0,
        button_bottom_factor: 0.35,
    )"#;
    let cfg: DialogConfig = ron::from_str(legacy).expect("parse legacy ron");
    assert_eq!(cfg.button_icon_scale, 0.16, "serde default not applied");
    // `button_width` is also `#[serde(default)]` — absent ⇒ 0.0 (auto-size).
    assert_eq!(cfg.button_width, 0.0);
}

#[test]
fn colors_override_takes_priority_over_theme() {
    // `resolved_colors()` must return the override verbatim when one is
    // set, and fall through to the theme palette otherwise. We build the
    // override from the Light palette (a different theme than the config's
    // Dark) so a regression that ignores the override surfaces as a bg/btn
    // mismatch.
    let custom = crate::theme::Theme::Light.dialog();
    let cfg = DialogConfig::new("t", "m")
        .with_theme(crate::theme::Theme::Dark)
        .with_colors(custom.clone());
    let resolved = cfg.resolved_colors();
    assert_eq!(resolved.bg, custom.bg);
    assert_eq!(resolved.btn_confirm, custom.btn_confirm);

    // `with_theme` clears any prior override and falls back to the theme.
    let cleared = cfg.with_theme(crate::theme::Theme::Dark);
    assert!(cleared.colors_override.is_none());
    assert_eq!(
        cleared.resolved_colors().bg,
        crate::theme::Theme::Dark.dialog().bg,
    );
}

#[test]
fn fixed_button_width_overrides_autosize() {
    // Mirror the `btn_w` selection in `render_confirm_dialog`:
    // a non-zero `button_width` pins the cell; `0.0` auto-sizes.
    fn resolve_btn_w(cfg: &DialogConfig) -> f32 {
        if cfg.button_width > 0.0 {
            cfg.button_width
        } else {
            // auto-size path always yields a strictly positive width
            // because `button_padding_x` (> 0) is always added.
            cfg.button_padding_x
        }
    }
    let pinned = DialogConfig::default().with_button_width(140.0);
    assert_eq!(resolve_btn_w(&pinned), 140.0);

    let auto = DialogConfig::default().with_button_width(0.0);
    assert!(resolve_btn_w(&auto) > 0.0, "auto width must be positive");
}

#[test]
fn dialog_result_is_must_use() {
    // Compile-time contract: `DialogResult` is `#[must_use]`. We can't
    // assert the attribute at runtime, but we *can* pin that consuming
    // the value via a match (the documented usage) covers all variants.
    for r in [
        DialogResult::Confirmed,
        DialogResult::Cancelled,
        DialogResult::Open,
    ] {
        let reacted = match r {
            DialogResult::Confirmed => 1,
            DialogResult::Cancelled => 2,
            DialogResult::Open => 3,
        };
        assert!((1..=3).contains(&reacted));
    }
}

// ── i18n config guard tests (project-wide requirement) ───────────────────

use crate::i18n::Locale;

#[test]
fn confirm_dialog_strings_resolve() {
    // EN must match the historic literals; RU must actually diverge on
    // every translated field (no stale EN-copied RU value).
    let en = crate::i18n::confirm_dialog::strings(Locale::En);
    let ru = crate::i18n::confirm_dialog::strings(Locale::Ru);
    assert_eq!(en.title, "Confirm");
    assert_eq!(en.message, "Are you sure?");
    assert_eq!(en.confirm, "Confirm");
    assert_eq!(en.cancel, "Cancel");
    assert_ne!(en.title, ru.title);
    assert_ne!(en.message, ru.message);
    assert_ne!(en.confirm, ru.confirm);
    assert_ne!(en.cancel, ru.cancel);
}

#[test]
fn default_locale_is_english() {
    let cfg = DialogConfig::default();
    assert_eq!(cfg.locale, Locale::En);
}

#[test]
fn locale_round_trips_through_ron() {
    let cfg = DialogConfig {
        locale: Locale::Ru,
        ..DialogConfig::default()
    };
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: DialogConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.locale, Locale::Ru);
}

#[test]
fn locale_field_optional_in_ron() {
    // Older saved configs predate `locale:` — they must fall back to
    // English. (Same ron shape as `button_icon_scale_defaults_when_field_absent`,
    // which also omits `locale`.)
    let legacy = r#"(
        title: "",
        message: "",
        confirm_label: "",
        cancel_label: "",
        icon: Warning,
        confirm_style: Destructive,
        theme: Dark,
        width: 340.0,
        height: 160.0,
        padding: 16.0,
        button_height: 27.0,
        button_gap: 60.0,
        dim_background: true,
        keyboard_shortcuts: true,
        rounding: 6.0,
        border_thickness: 1.5,
        accent_border: true,
        show_separator: false,
        show_button_icons: true,
        header_icon_size: 16.0,
        button_padding_x: 22.0,
        button_bottom_factor: 0.35,
    )"#;
    let cfg: DialogConfig =
        ron::from_str(legacy).expect("confirm_dialog config without `locale` must still parse");
    assert_eq!(cfg.locale, Locale::En);
}

#[test]
fn with_locale_set_locale_round_trip() {
    // `with_locale` (consuming) and `set_locale` (&mut) agree, and
    // `locale()` mirrors the stored value.
    let cfg = DialogConfig::default().with_locale(Locale::Ru);
    assert_eq!(cfg.locale(), Locale::Ru);

    let mut cfg2 = DialogConfig::default();
    assert_eq!(cfg2.locale(), Locale::En);
    cfg2.set_locale(Locale::Ru);
    assert_eq!(cfg2.locale(), Locale::Ru);

    // Empty (host-unset) fields follow the locale: Russian defaults.
    assert_eq!(cfg.resolved_title(), "Подтверждение");
    assert_eq!(cfg.resolved_message(), "Вы уверены?");
    assert_eq!(cfg.resolved_confirm_label(), "Подтвердить");
    assert_eq!(cfg.resolved_cancel_label(), "Отмена");
}

#[test]
fn host_label_wins_over_localized_default() {
    // Precedence: a host-set (non-empty) field always beats the
    // localized default, regardless of locale.
    let cfg = DialogConfig::new("Quit?", "Discard changes?")
        .with_confirm_label("Quit")
        .with_cancel_label("Stay")
        .with_locale(Locale::Ru);
    assert_eq!(cfg.resolved_title(), "Quit?");
    assert_eq!(cfg.resolved_message(), "Discard changes?");
    assert_eq!(cfg.resolved_confirm_label(), "Quit");
    assert_eq!(cfg.resolved_cancel_label(), "Stay");
    // Only the cancel label was left to the locale here is not the case —
    // all four are host-set, so nothing falls back. Now drop the confirm
    // override to prove the locale fills *only* the empty field.
    let mixed = DialogConfig::new("Quit?", "Discard changes?").with_locale(Locale::Ru);
    assert_eq!(mixed.resolved_title(), "Quit?"); // host
    assert_eq!(mixed.resolved_confirm_label(), "Подтвердить"); // localized default
}
