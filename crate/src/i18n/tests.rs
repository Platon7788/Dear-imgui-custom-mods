//! `crate::i18n` tests — `Locale` round-trip, the `From` bridge to
//! `disasm_knowledge::Locale`, the historic `*_strings_resolve`
//! canaries, format-helper localisation, and per-widget EN/RU parity
//! sanity checks.

use super::*;

#[test]
fn locale_default_is_english() {
    assert_eq!(Locale::default(), Locale::En);
}

#[test]
fn locale_tag_round_trip() {
    assert_eq!(Locale::En.tag(), "en");
    assert_eq!(Locale::Ru.tag(), "ru");
}

#[test]
fn hex_viewer_strings_resolve() {
    let en = hex_viewer::strings(Locale::En);
    let ru = hex_viewer::strings(Locale::Ru);
    // English defaults must match the historic literals; a single
    // canary check guards against accidental EN-side renaming.
    assert_eq!(en.action_go, "Go");
    assert_eq!(en.header_address, "Address");
    // Russian must be different (translation actually exists).
    assert_ne!(en.action_go, ru.action_go);
    assert_ne!(en.header_address, ru.header_address);
}

#[test]
fn disasm_view_strings_resolve() {
    let en = disasm_view::strings(Locale::En);
    let ru = disasm_view::strings(Locale::Ru);
    assert_eq!(en.action_close, "Close");
    assert_eq!(en.flow_normal, "Normal (sequential)");
    assert_ne!(en.action_close, ru.action_close);
    assert_ne!(en.flow_normal, ru.flow_normal);
}

#[test]
fn hex_viewer_format_helpers_change_per_locale() {
    let en = hex_viewer::result_n_of_m(Locale::En, 3, 7);
    let ru = hex_viewer::result_n_of_m(Locale::Ru, 3, 7);
    assert_eq!(en, "Result 3/7");
    assert_eq!(ru, "Результат 3/7");
}

#[test]
fn disasm_view_format_helpers_change_per_locale() {
    let en = disasm_view::pattern_too_short(Locale::En, 3, 5);
    let ru = disasm_view::pattern_too_short(Locale::Ru, 3, 5);
    assert!(en.contains("3 / 5") && en.contains("Pattern"));
    assert!(ru.contains("3 / 5") && ru.contains("Шаблон"));

    let en = disasm_view::copy_n_instructions(Locale::En, 4);
    let ru = disasm_view::copy_n_instructions(Locale::Ru, 4);
    assert!(en.contains('4') && en.contains("Instructions"));
    assert!(ru.contains('4') && ru.contains("инструкций"));
}

#[test]
fn timeline_strings_resolve() {
    assert_eq!(timeline::strings(Locale::En).category_label, "Category: ");
    assert_eq!(timeline::strings(Locale::Ru).category_label, "Категория: ");
}

#[test]
fn timeline_start_end_helper_localises() {
    let en = timeline::start_end(Locale::En, 1.2345_f64, 6.7890_f64);
    let ru = timeline::start_end(Locale::Ru, 1.2345_f64, 6.7890_f64);
    assert!(en.contains("Start") && en.contains("End"));
    assert!(ru.contains("Начало") && ru.contains("Конец"));
    assert!(en.contains("1.2345") && ru.contains("1.2345"));
}

#[test]
fn diff_viewer_strings_resolve() {
    assert_eq!(
        diff_viewer::strings(Locale::En).prev_button,
        "Prev (Shift+F7)"
    );
    assert_eq!(
        diff_viewer::strings(Locale::Ru).prev_button,
        "Назад (Shift+F7)"
    );
}

#[test]
fn nav_panel_strings_resolve() {
    assert_eq!(nav_panel::strings(Locale::En).show_panel, "Show panel");
    assert_eq!(nav_panel::strings(Locale::Ru).show_panel, "Показать панель");
}

#[test]
fn code_editor_strings_resolve() {
    assert_eq!(code_editor::strings(Locale::En).menu_cut, "Cut");
    assert_eq!(code_editor::strings(Locale::Ru).menu_cut, "Вырезать");
    assert_eq!(code_editor::strings(Locale::En).submenu_view, "View");
    assert_eq!(code_editor::strings(Locale::Ru).submenu_view, "Вид");
}

#[test]
fn code_editor_cursor_info_localises() {
    let en = code_editor::cursor_info(Locale::En, 12, 5, 100);
    let ru = code_editor::cursor_info(Locale::Ru, 12, 5, 100);
    assert!(en.contains("Ln 12") && en.contains("Col 5") && en.contains("100 lines"));
    assert!(ru.contains("Стр 12") && ru.contains("Кол 5") && ru.contains("всего 100"));
}

#[test]
fn force_graph_strings_resolve() {
    assert_eq!(force_graph::strings(Locale::En).section_filters, "Filters");
    assert_eq!(force_graph::strings(Locale::Ru).section_filters, "Фильтры");
    assert_eq!(
        force_graph::strings(Locale::En).btn_pause_resume,
        "Pause/Resume"
    );
    assert_eq!(
        force_graph::strings(Locale::Ru).btn_pause_resume,
        "Пауза/Возобн."
    );
}

#[test]
fn locale_serde_round_trip() {
    // ron round-trip: makes sure saved configs can carry a locale.
    let s = ron::ser::to_string(&Locale::Ru).unwrap();
    let back: Locale = ron::from_str(&s).unwrap();
    assert_eq!(back, Locale::Ru);
}

// ── From-bridge to disasm_knowledge::Locale ─────────────────────────────────

/// The headless `disasm_knowledge` crate carries its own `Locale`; the
/// UI crate bridges into it via `From` (impl lives in
/// `crate::disasm_view`). Pin both variants so a future enum-variant
/// rename can't silently break the bridge.
///
/// `disasm-knowledge` is an optional dependency pulled in by the
/// `disasm_view` feature, so this test is feature-gated to keep
/// `--no-default-features` builds compiling.
#[cfg(feature = "disasm_view")]
#[test]
fn locale_bridges_into_disasm_knowledge() {
    use disasm_knowledge::Locale as KLocale;
    assert_eq!(KLocale::from(Locale::En), KLocale::En);
    assert_eq!(KLocale::from(Locale::Ru), KLocale::Ru);
}

// ── EN/RU parity sanity (per widget) ────────────────────────────────────────
//
// The `Strings` struct already forces structural parity at compile time
// (every field must be set in both EN and RU). These tests guard the
// *value* invariants the compiler can't: that both catalogues resolve,
// that key fields are non-empty, and that translated fields actually
// diverge (no stale EN-copied RU values). Technical abbreviations that
// legitimately stay identical (Hex / Dec / ASCII / little-endian …) are
// deliberately not asserted-different.

#[test]
fn hex_viewer_parity_key_fields_nonempty() {
    let en = hex_viewer::strings(Locale::En);
    let ru = hex_viewer::strings(Locale::Ru);
    for s in [
        en.header_address,
        en.settings_title,
        en.action_go,
        en.action_find,
    ] {
        assert!(!s.is_empty());
    }
    for s in [
        ru.header_address,
        ru.settings_title,
        ru.action_go,
        ru.action_find,
    ] {
        assert!(!s.is_empty());
    }
    // Translated fields diverge.
    assert_ne!(en.settings_title, ru.settings_title);
    assert_ne!(en.action_find, ru.action_find);
}

#[test]
fn disasm_view_parity_key_fields_nonempty() {
    let en = disasm_view::strings(Locale::En);
    let ru = disasm_view::strings(Locale::Ru);
    for s in [
        en.settings_title,
        en.goto_title,
        en.no_matches,
        en.flow_invalid,
    ] {
        assert!(!s.is_empty());
    }
    for s in [
        ru.settings_title,
        ru.goto_title,
        ru.no_matches,
        ru.flow_invalid,
    ] {
        assert!(!s.is_empty());
    }
    assert_ne!(en.settings_title, ru.settings_title);
    assert_ne!(en.no_matches, ru.no_matches);
}

#[test]
fn force_graph_parity_key_fields_nonempty() {
    let en = force_graph::strings(Locale::En);
    let ru = force_graph::strings(Locale::Ru);
    for s in [
        en.section_filters,
        en.section_physics,
        en.menu_pin,
        en.btn_reset_layout,
    ] {
        assert!(!s.is_empty());
    }
    for s in [
        ru.section_filters,
        ru.section_physics,
        ru.menu_pin,
        ru.btn_reset_layout,
    ] {
        assert!(!s.is_empty());
    }
    assert_ne!(en.section_filters, ru.section_filters);
    assert_ne!(en.menu_pin, ru.menu_pin);
}

#[test]
fn code_editor_parity_key_fields_nonempty() {
    let en = code_editor::strings(Locale::En);
    let ru = code_editor::strings(Locale::Ru);
    for s in [en.menu_cut, en.submenu_view, en.btn_replace, en.no_matches] {
        assert!(!s.is_empty());
    }
    for s in [ru.menu_cut, ru.submenu_view, ru.btn_replace, ru.no_matches] {
        assert!(!s.is_empty());
    }
    assert_ne!(en.menu_cut, ru.menu_cut);
    assert_ne!(en.btn_replace, ru.btn_replace);
}

#[test]
fn small_widget_parity_diverges() {
    // timeline / diff_viewer / nav_panel — all-translatable catalogues.
    assert_ne!(
        timeline::strings(Locale::En).depth_label,
        timeline::strings(Locale::Ru).depth_label
    );
    assert_ne!(
        diff_viewer::strings(Locale::En).next_button,
        diff_viewer::strings(Locale::Ru).next_button
    );
    assert_ne!(
        nav_panel::strings(Locale::En).toggle_panel,
        nav_panel::strings(Locale::Ru).toggle_panel
    );
}

/// Format-template fields must carry matching placeholders across EN/RU
/// so `format!`-equivalent substitution lines up in both languages.
#[test]
fn format_templates_have_matching_placeholders() {
    // timeline.start_end_template — {start} and {end}.
    for cat in [
        timeline::EN.start_end_template,
        timeline::RU.start_end_template,
    ] {
        assert!(cat.contains("{start") && cat.contains("{end"));
    }
    // disasm_view.search_pattern_too_short_template — {n} and {min}.
    for cat in [
        disasm_view::EN.search_pattern_too_short_template,
        disasm_view::RU.search_pattern_too_short_template,
    ] {
        assert!(cat.contains("{n}") && cat.contains("{min}"));
    }
}
