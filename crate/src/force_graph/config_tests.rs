//! Unit tests for [`super::ViewerConfig`] / [`super::ForceConfig`].
//!
//! Split out of `config.rs` to keep that file under the 500-line limit.
//! Included via `#[cfg(test)] #[path = "config_tests.rs"] mod tests;` so
//! `super::*` reaches the private `ConfigRon` loader struct.

use super::*;

#[test]
fn default_force_config_values_from_spec() {
    let f = ForceConfig::default();
    assert!((f.repulsion - 120.0).abs() < f32::EPSILON);
    assert!((f.attraction - 0.04).abs() < f32::EPSILON);
    assert!((f.center_pull - 0.002).abs() < f32::EPSILON);
    assert!((f.velocity_decay - 0.6).abs() < f32::EPSILON);
    assert!((f.barnes_hut_theta - 0.9).abs() < f32::EPSILON);
    assert!((f.collision_radius - 20.0).abs() < f32::EPSILON);
    assert!((f.link_distance - 80.0).abs() < f32::EPSILON);
    assert!(f.radius_by_degree);
    assert!((f.radius_base - 4.0).abs() < f32::EPSILON);
    assert!((f.radius_per_degree - 1.5).abs() < f32::EPSILON);
}

#[test]
fn viewer_config_builder_chain() {
    let c = ViewerConfig {
        show_labels: LabelVisibility::Always,
        ..ViewerConfig::default()
    };
    assert!(matches!(c.show_labels, LabelVisibility::Always));
    assert_eq!(c.lod_threshold, 5000);
    assert!(!c.minimap);
    assert!(c.background_grid);
    assert!(c.show_orphans);
    assert!(!c.cluster_hulls);
    assert!((c.node_size_multiplier - 1.0).abs() < f32::EPSILON);
}

#[test]
fn viewer_config_default_theme_is_dark() {
    let c = ViewerConfig::default();
    assert_eq!(c.theme, Theme::Dark);
}

#[test]
fn force_config_clone() {
    let f = ForceConfig::default();
    let g = f.clone();
    assert!((f.repulsion - g.repulsion).abs() < f32::EPSILON);
}

#[test]
fn color_mode_debug_does_not_panic() {
    let _ = format!("{:?}", ColorMode::Static);
    let _ = format!("{:?}", ColorMode::ByTag);
    let _ = format!("{:?}", ColorMode::ByCommunity);
    let _ = format!("{:?}", ColorMode::ByPageRank);
    let _ = format!("{:?}", ColorMode::ByBetweenness);
    let _ = format!("{:?}", ColorMode::Custom(Box::new(|_, _| [1.0; 4])));
}

// ── i18n guard tests (one of the 9 localised widgets) ───────────────────

/// `ViewerConfig::default()` must resolve to English so a host that never
/// calls `with_locale` gets the default-language UI.
#[test]
fn default_locale_is_english() {
    assert_eq!(ViewerConfig::default().locale, crate::i18n::Locale::En);
}

/// The `locale` field must survive a ron serialize → deserialize round-trip
/// so saved viewer configs preserve the chosen language.
#[test]
fn locale_round_trips_through_ron() {
    let cfg = ViewerConfig {
        locale: crate::i18n::Locale::Ru,
        ..ViewerConfig::default()
    };
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: ViewerConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.locale, crate::i18n::Locale::Ru);
}

/// Older `config.ron` files written before the `locale` field existed must
/// still parse — `#[serde(default)]` falls the field back to English.
#[test]
fn locale_field_optional_in_ron() {
    // Start from the canonical default ron, strip the `locale:` line.
    let full = include_str!("config.ron");
    let without_locale: String = full
        .lines()
        .filter(|l| !l.trim_start().starts_with("locale:"))
        .collect::<Vec<_>>()
        .join("\n");
    let cfg: ConfigRon =
        ron::from_str(&without_locale).expect("config.ron without `locale:` must still parse");
    assert_eq!(cfg.viewer.locale, crate::i18n::Locale::En);
}
