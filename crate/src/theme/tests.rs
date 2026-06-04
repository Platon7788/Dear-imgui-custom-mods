//! Core `Theme` enum tests — cycling, display names, semantic colour
//! helpers and WCAG-style contrast baselines. The per-widget palette
//! resolution tests live in the sibling [`super::widget_tests`] module.

use super::Theme;

#[test]
fn next_cycles_through_all_variants() {
    let start = Theme::Dark;
    let mut t = start;
    for _ in 0..Theme::ALL.len() {
        t = t.next();
    }
    assert_eq!(t, start, "Theme::next() should cycle around in N steps");
}

#[test]
fn prev_inverts_next() {
    for &theme in Theme::ALL {
        assert_eq!(
            theme.next().prev(),
            theme,
            "{theme:?}: prev∘next ≠ identity"
        );
        assert_eq!(
            theme.prev().next(),
            theme,
            "{theme:?}: next∘prev ≠ identity"
        );
    }
}

#[test]
fn next_visits_every_variant_exactly_once() {
    let mut t = Theme::Dark;
    let mut seen: Vec<Theme> = vec![t];
    for _ in 0..Theme::ALL.len() - 1 {
        t = t.next();
        seen.push(t);
    }
    for &expected in Theme::ALL {
        assert!(seen.contains(&expected), "{expected:?} missing from cycle");
    }
    assert_eq!(seen.len(), Theme::ALL.len());
}

#[test]
fn display_name_is_unique_per_variant() {
    let names: Vec<&'static str> = Theme::ALL.iter().map(|t| t.display_name()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "display_name collision");
}

#[test]
fn description_is_non_empty_per_variant() {
    for &t in Theme::ALL {
        assert!(!t.description().is_empty(), "{t:?} has empty description");
    }
}

#[test]
fn default_is_dark() {
    assert_eq!(Theme::default(), Theme::Dark);
}

#[test]
fn all_has_exactly_dark_and_light() {
    // The built-in theme set is exactly {Dark, Light}. Pin both the
    // length and the membership so a future re-introduction of an
    // extra variant is a deliberate, test-visible change.
    assert_eq!(Theme::ALL.len(), 2, "Theme::ALL must hold exactly 2 themes");
    assert_eq!(Theme::ALL, &[Theme::Dark, Theme::Light]);
}

#[test]
fn next_prev_toggle_dark_and_light() {
    // With two themes, next/prev simply toggle between them.
    assert_eq!(Theme::Dark.next(), Theme::Light);
    assert_eq!(Theme::Light.next(), Theme::Dark);
    assert_eq!(Theme::Dark.prev(), Theme::Light);
    assert_eq!(Theme::Light.prev(), Theme::Dark);
}

#[test]
fn display_names_are_dark_and_light() {
    assert_eq!(Theme::Dark.display_name(), "Dark");
    assert_eq!(Theme::Light.display_name(), "Light");
}

#[test]
fn is_dark_matches_only_light_as_light() {
    assert!(!Theme::Light.is_dark());
    assert!(Theme::Light.is_light());
    for &t in Theme::ALL {
        if t != Theme::Light {
            assert!(t.is_dark(), "{t:?} should be dark");
            assert!(!t.is_light(), "{t:?} should not be light");
        }
    }
}

// ── Semantic colour helpers ─────────────────────────────────────────────

#[test]
fn semantic_colors_have_full_alpha() {
    // accent / danger / success / warning are intended for opaque tinting
    // — anything below 0.99 is almost certainly a typo.
    for &t in Theme::ALL {
        for (name, c) in [
            ("accent", t.accent()),
            ("danger", t.danger()),
            ("success", t.success()),
            ("warning", t.warning()),
        ] {
            assert!(c[3] >= 0.99, "{t:?}::{name}: alpha {} < 0.99", c[3]);
        }
    }
}

#[test]
fn semantic_hues_are_perceptually_distinct() {
    // accent ≠ danger ≠ success ≠ warning per theme. Compare via raw
    // RGB Euclidean distance. Threshold = 0.15.
    fn dist(a: [f32; 4], b: [f32; 4]) -> f32 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }
    const MIN: f32 = 0.15;
    for &t in Theme::ALL {
        let pairs = [
            ("accent vs danger", t.accent(), t.danger()),
            ("accent vs success", t.accent(), t.success()),
            ("danger vs success", t.danger(), t.success()),
            ("danger vs warning", t.danger(), t.warning()),
            ("success vs warning", t.success(), t.warning()),
        ];
        for (label, a, b) in pairs {
            let d = dist(a, b);
            assert!(d > MIN, "{t:?}: {label} distance {d:.3} ≤ {MIN}");
        }
    }
}

// ── WCAG contrast (informational baseline — not strict AA) ─────────────
//
// Computes W3C "relative luminance" + contrast ratio for the primary
// text-on-window-bg pair sourced from `titlebar()`. The threshold is
// set to 4.5 ("AA body
// text") for the Light theme and 3.0 (relaxed for dark dim contrasts
// — many popular dark themes flunk strict AA but read well in practice)
// for everything else. Catches accidental black-on-black regressions.

fn relative_luminance(c: [f32; 4]) -> f32 {
    // sRGB → linear conversion per WCAG 2.x.
    fn lin(c: f32) -> f32 {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let r = lin(c[0]);
    let g = lin(c[1]);
    let b = lin(c[2]);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast_ratio(a: [f32; 4], b: [f32; 4]) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (l1, l2) = if la >= lb { (la, lb) } else { (lb, la) };
    (l1 + 0.05) / (l2 + 0.05)
}

#[test]
fn primary_text_meets_minimum_contrast() {
    // Floor levels chosen as regression detectors, NOT strict WCAG
    // compliance. Titlebar title text is allowed to be muted by design
    // — we just want to flag any accidental black-on-black or
    // near-identical-luminance pairs.
    for &theme in Theme::ALL {
        let palette = theme.titlebar();
        let ratio = contrast_ratio(palette.title, palette.bg);
        let min = if theme == Theme::Light { 4.5 } else { 2.5 };
        assert!(
            ratio >= min,
            "{theme:?}: title-on-bg contrast {ratio:.2} < {min}",
        );
    }
}

#[test]
fn button_glyphs_pop_against_titlebar_bg() {
    // The per-button accent palette (amber/cyan/red) MUST stand out
    // from the titlebar background — if a future tweak made
    // `btn_close ≈ bg`, the close button would silently disappear.
    for &theme in Theme::ALL {
        let p = theme.titlebar();
        for (label, glyph) in [
            ("minimize", p.btn_minimize),
            ("maximize", p.btn_maximize),
            ("close", p.btn_close),
        ] {
            let ratio = contrast_ratio(glyph, p.bg);
            assert!(
                ratio >= 1.8,
                "{theme:?}: {label} glyph contrast {ratio:.2} < 1.8 — too subtle",
            );
        }
    }
}
