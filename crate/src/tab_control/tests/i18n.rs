//! Locale guard tests for `tab_control` — the canonical four required of every
//! i18n widget (`<widget>_strings_resolve`, `default_locale_is_english`,
//! `locale_round_trips_through_ron`, `locale_field_optional_in_ron`) plus
//! `with_locale` / `set_locale` builder coverage.

use super::super::*;
use super::Spy;
use crate::i18n::Locale;

// ─── strings resolution ─────────────────────────────────────────────────────

#[test]
fn tab_control_strings_resolve() {
    // EN and RU catalogues diverge on every translatable key and resolve
    // through `for_locale`.
    let en = TabStrings::for_locale(Locale::En);
    let ru = TabStrings::for_locale(Locale::Ru);
    assert_eq!(en.cancel, "Cancel");
    assert_eq!(ru.cancel, "Отмена");
    assert_ne!(en.close, ru.close);
    assert_ne!(en.close_confirm, ru.close_confirm);
    assert_ne!(en.close_confirm_dirty, ru.close_confirm_dirty);
    assert_ne!(en.no_tabs, ru.no_tabs);
    assert_ne!(en.empty_hint, ru.empty_hint);
    assert_ne!(en.overflow_tooltip, ru.overflow_tooltip);
    assert_ne!(en.add_tab, ru.add_tab);
}

// ─── locale default ─────────────────────────────────────────────────────────

#[test]
fn default_locale_is_english() {
    let cfg = TabControlConfig::default();
    assert_eq!(cfg.locale, Locale::En);
    // And a freshly-built control reports English.
    let pc: TabControl<Spy> = TabControl::new("##t");
    assert_eq!(pc.locale(), Locale::En);
}

// ─── ron round-trip ─────────────────────────────────────────────────────────

#[test]
fn locale_round_trips_through_ron() {
    let cfg = TabControlConfig {
        locale: Locale::Ru,
        ..TabControlConfig::default()
    };
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: TabControlConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.locale, Locale::Ru);
}

// ─── locale field optional in ron (forward-compat) ──────────────────────────

#[test]
fn locale_field_optional_in_ron() {
    // Older configs without `locale:` must fall back to English. Use the
    // canonical default but strip the locale line so forward-compat behaviour
    // is exercised.
    let canonical = ron::ser::to_string(&TabControlConfig::default()).unwrap();
    // Crude but sufficient: strip the trailing `,locale:En`.
    let stripped = canonical.replace(",locale:En", "");
    let cfg: TabControlConfig = ron::from_str(&stripped)
        .expect("tab_control config without `locale:` field must still parse");
    assert_eq!(cfg.locale, Locale::En);
}

// ─── builder / setter wiring ────────────────────────────────────────────────

#[test]
fn with_locale_sets_locale_and_refreshes_strings() {
    let pc: TabControl<Spy> = TabControl::new("##t").with_locale(Locale::Ru);
    assert_eq!(pc.locale(), Locale::Ru);
    // `with_locale` must auto-refresh the catalogue, not just the enum.
    assert_eq!(pc.config.strings.cancel, "Отмена");
}

#[test]
fn set_locale_switches_catalogue_mid_flight() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    assert_eq!(pc.config.strings.cancel, "Cancel");
    pc.set_locale(Locale::Ru);
    assert_eq!(pc.locale(), Locale::Ru);
    assert_eq!(pc.config.strings.cancel, "Отмена");
    pc.set_locale(Locale::En);
    assert_eq!(pc.config.strings.cancel, "Cancel");
}

#[test]
fn with_config_syncs_strings_to_locale() {
    // A config loaded with `locale: Ru` should come up Russian without the
    // host calling `set_locale` first (handled in `with_config`).
    let cfg = TabControlConfig {
        locale: Locale::Ru,
        ..TabControlConfig::default()
    };
    let pc: TabControl<Spy> = TabControl::with_config("##t", cfg);
    assert_eq!(pc.config.strings.cancel, "Отмена");
}
