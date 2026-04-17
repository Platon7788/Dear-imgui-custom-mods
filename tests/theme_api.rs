//! Integration tests for the unified `Theme` enum public API.
//!
//! These run against the installed library — they catch breakage in the
//! Theme surface (missing variant, silent behavior change in `.next()`,
//! sub-palette dispatch regressions) without spinning up a live ImGui
//! context.

use dear_imgui_custom_mod::theme::Theme;

#[test]
fn all_variants_are_iterable() {
    // `Theme::ALL` is the canonical iteration order — used by the theme
    // picker in demos. Size must match the enum; duplicates or omissions
    // are public-API regressions.
    assert_eq!(Theme::ALL.len(), 5, "Theme should have exactly 5 variants");
    assert!(Theme::ALL.contains(&Theme::Dark));
    assert!(Theme::ALL.contains(&Theme::Light));
    assert!(Theme::ALL.contains(&Theme::Midnight));
    assert!(Theme::ALL.contains(&Theme::Solarized));
    assert!(Theme::ALL.contains(&Theme::Monokai));
}

#[test]
fn all_themes_have_distinct_display_names() {
    // Display names are shown in theme pickers — two themes mapping to
    // the same user-facing string would confuse users.
    let names: Vec<_> = Theme::ALL.iter().map(|t| t.display_name()).collect();
    let unique: std::collections::HashSet<_> = names.iter().collect();
    assert_eq!(
        names.len(),
        unique.len(),
        "display_name collisions: {names:?}"
    );
}

#[test]
fn display_names_are_non_empty() {
    for t in Theme::ALL {
        let name = t.display_name();
        assert!(!name.is_empty(), "Theme::{t:?} has empty display_name");
        // No leading / trailing whitespace — it'd mis-render in ImGui.
        assert_eq!(name.trim(), name, "Theme::{t:?} display_name has whitespace");
    }
}

#[test]
fn next_cycles_through_all_variants() {
    // Calling `.next()` ALL.len() times should bring us back to the start.
    let start = Theme::Dark;
    let mut cur = start;
    for _ in 0..Theme::ALL.len() {
        cur = cur.next();
    }
    assert_eq!(cur, start, ".next() cycle should be closed");
}

#[test]
fn next_visits_every_variant_exactly_once() {
    let mut seen = std::collections::HashSet::new();
    let mut cur = Theme::Dark;
    for _ in 0..Theme::ALL.len() {
        seen.insert(cur);
        cur = cur.next();
    }
    assert_eq!(seen.len(), Theme::ALL.len(), ".next() skipped a variant");
}

#[test]
fn every_theme_resolves_all_sub_palettes() {
    // Calling the dispatch methods should never panic and should return
    // a distinct struct per theme (i.e. each theme really does supply its
    // own palette, rather than falling back to some default).
    let mut titlebar_bgs = Vec::new();
    let mut nav_bgs = Vec::new();
    let mut dialog_bgs = Vec::new();

    for t in Theme::ALL {
        titlebar_bgs.push(t.titlebar().bg);
        nav_bgs.push(t.nav().bg);
        dialog_bgs.push(t.dialog().bg);
        // `statusbar()` returns a full config — just ensure it doesn't panic.
        let _sb = t.statusbar();
    }

    // Not every theme has a visually distinct background from every other
    // (Dark + Midnight may legitimately share a very dark bg), but at least
    // two of the five should differ per component — otherwise something is
    // wrong with the dispatch.
    assert!(
        titlebar_bgs.windows(2).any(|w| w[0] != w[1]),
        "all titlebar bgs collapsed to one color"
    );
    assert!(
        nav_bgs.windows(2).any(|w| w[0] != w[1]),
        "all nav bgs collapsed to one color"
    );
    assert!(
        dialog_bgs.windows(2).any(|w| w[0] != w[1]),
        "all dialog bgs collapsed to one color"
    );
}

#[test]
fn default_is_dark() {
    // Documented contract — `Theme::default()` is Dark. If this ever
    // changes, downstream consumers (NxT persists `AppTheme::default()`)
    // need to be told.
    assert_eq!(Theme::default(), Theme::Dark);
}

#[test]
fn theme_is_copy_and_eq() {
    // Theme is `Copy + PartialEq + Eq` — lets it flow through configs
    // and live in HashSets without jumping through hoops.
    fn require_copy_eq<T: Copy + PartialEq + Eq + std::hash::Hash>(_: T) {}
    require_copy_eq(Theme::Dark);
}
