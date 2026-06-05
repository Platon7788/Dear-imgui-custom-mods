//! Unit tests for the timeline widget. All tests here run without an
//! ImGui context — they exercise the pure data, coordinate and config
//! layers only.

use super::*;
use crate::i18n::Locale;

// ── Data-type smoke tests ────────────────────────────────────────────────────

#[test]
fn span_basic() {
    let s = Span::new(1, 0.0, 0.050, 0, "frame");
    assert_eq!(s.id, 1);
    assert!((s.duration() - 0.050).abs() < 1e-12);
    assert_eq!(s.label, "frame");
}

#[test]
fn span_builders() {
    let s = Span::new(2, 0.0, 1.0, 0, "test")
        .with_category("cat")
        .with_color([1.0, 0.0, 0.0, 1.0])
        .with_source("main.rs:42");
    assert_eq!(s.category, "cat");
    assert!(s.color.is_some());
    assert_eq!(s.source.as_deref(), Some("main.rs:42"));
}

#[test]
fn span_swaps_reversed_times() {
    let s = Span::new(1, 1.0, 0.0, 0, "rev");
    assert!(s.start <= s.end);
    assert!((s.start - 0.0).abs() < 1e-12);
    assert!((s.end - 1.0).abs() < 1e-12);
}

#[test]
fn span_non_finite_times_clamp_to_zero() {
    let s = Span::new(1, f64::NAN, 1.0, 0, "bad");
    assert_eq!(s.start, 0.0);
    assert_eq!(s.end, 0.0);
    let s = Span::new(2, 0.0, f64::INFINITY, 0, "bad");
    assert_eq!(s.start, 0.0);
    assert_eq!(s.end, 0.0);
}

#[test]
fn track_add_span_sorted() {
    let mut t = Track::new("test");
    t.add_span(Span::new(1, 0.5, 1.0, 0, "b"));
    t.add_span(Span::new(2, 0.0, 0.3, 0, "a"));
    t.add_span(Span::new(3, 0.2, 0.8, 1, "c"));
    assert_eq!(t.spans[0].label, "a");
    assert_eq!(t.spans[1].label, "c");
    assert_eq!(t.spans[2].label, "b");
}

#[test]
fn track_max_depth() {
    let mut t = Track::new("test");
    t.add_span(Span::new(1, 0.0, 1.0, 0, "a"));
    t.add_span(Span::new(2, 0.0, 0.5, 1, "b"));
    t.add_span(Span::new(3, 0.0, 0.2, 3, "c"));
    assert_eq!(t.max_depth(), 3);
    assert_eq!(t.depth_rows(), 4);
}

#[test]
fn track_time_range() {
    let mut t = Track::new("t");
    assert!(t.time_range().is_none());
    t.add_span(Span::new(1, 0.1, 0.5, 0, "a"));
    t.add_span(Span::new(2, 0.3, 0.9, 0, "b"));
    let (lo, hi) = t.time_range().unwrap();
    assert!((lo - 0.1).abs() < 1e-12);
    assert!((hi - 0.9).abs() < 1e-12);
}

#[test]
fn marker_basic() {
    let m = Marker::new(0.016, "frame").with_color([1.0, 1.0, 0.0, 1.0]);
    assert!((m.time - 0.016).abs() < 1e-12);
    assert!(m.color.is_some());
}

// ── Data range / fit ─────────────────────────────────────────────────────────

#[test]
fn timeline_data_range() {
    let mut tl = Timeline::new("##test");
    assert_eq!(tl.data_time_range(), (0.0, 1.0));

    let mut t = Track::new("main");
    t.add_span(Span::new(1, 0.01, 0.05, 0, "a"));
    tl.add_track(t);

    let (lo, hi) = tl.data_time_range();
    assert!((lo - 0.01).abs() < 1e-12);
    assert!((hi - 0.05).abs() < 1e-12);
}

#[test]
fn timeline_fit_to_content() {
    let mut tl = Timeline::new("##test");
    let mut t = Track::new("main");
    t.add_span(Span::new(1, 0.0, 0.1, 0, "a"));
    tl.add_track(t);
    tl.fit_to_content(1000.0);
    // usable = 1000 - 120 (label) = 880; duration = 0.1 → pps = 8800.
    assert!((tl.vp.pixels_per_second - 8800.0).abs() < 1.0);
}

#[test]
fn fit_to_content_respects_zoom_clamps() {
    // Regression: fit() used to skip the [min_zoom, max_zoom] clamp the
    // interactive zoom path enforces. A near-instant capture (huge pps)
    // must be capped at max_zoom; an extremely long one floored at
    // min_zoom.
    let mut tl = Timeline::new("##clamp");
    tl.config.max_zoom = 1_000.0;
    tl.config.min_zoom = 1.0;

    // Tiny duration → would-be pps far above max_zoom.
    let mut short = Track::new("s");
    short.add_span(Span::new(1, 0.0, 1e-6, 0, "tiny"));
    tl.add_track(short);
    tl.fit_to_content(1000.0);
    assert!(tl.vp.pixels_per_second <= tl.config.max_zoom);
    assert!((tl.vp.pixels_per_second - tl.config.max_zoom).abs() < 1e-6);

    // Huge duration → would-be pps below min_zoom.
    tl.clear_tracks();
    let mut long = Track::new("l");
    long.add_span(Span::new(2, 0.0, 1e9, 0, "huge"));
    tl.add_track(long);
    tl.fit_to_content(1000.0);
    assert!(tl.vp.pixels_per_second >= tl.config.min_zoom);
}

#[test]
fn fit_to_content_ignores_tiny_viewport() {
    let mut tl = Timeline::new("##tiny");
    let mut t = Track::new("t");
    t.add_span(Span::new(1, 0.0, 1.0, 0, "a"));
    tl.add_track(t);
    let before = tl.vp.pixels_per_second;
    // usable = 5 - 120 < 10 → no change.
    tl.fit_to_content(5.0);
    assert_eq!(tl.vp.pixels_per_second, before);
}

// ── Coordinate round-trip ────────────────────────────────────────────────────

#[test]
fn time_pixel_round_trip() {
    let mut tl = Timeline::new("##rt");
    tl.vp.time_start = 1.25;
    tl.vp.pixels_per_second = 4_000.0;
    let content_x = 64.0_f32;

    for &t in &[1.25_f64, 1.30, 2.0, 5.5] {
        let x = tl.time_to_x(t, content_x);
        let back = tl.x_to_time(x, content_x);
        // f32 pixel narrowing limits precision; allow a relaxed epsilon.
        assert!(
            (back - t).abs() < 1e-3,
            "round trip drifted: t={t}, back={back}"
        );
    }
}

#[test]
fn x_to_time_survives_zero_pps() {
    // Division-by-zero guard: pps floored at 1e-9, no inf / NaN.
    let mut tl = Timeline::new("##div0");
    tl.vp.pixels_per_second = 0.0;
    let t = tl.x_to_time(200.0, 0.0);
    assert!(t.is_finite());
}

#[test]
fn time_to_x_monotonic() {
    let mut tl = Timeline::new("##mono");
    tl.vp.time_start = 0.0;
    tl.vp.pixels_per_second = 1_000.0;
    let a = tl.time_to_x(0.0, 0.0);
    let b = tl.time_to_x(0.5, 0.0);
    let c = tl.time_to_x(1.0, 0.0);
    assert!(a < b && b < c);
}

// ── Tick generation bounds ───────────────────────────────────────────────────

#[test]
fn adaptive_ticks_basic() {
    let (interval, _unit) = adaptive_ticks(1.0, 1000.0);
    assert!(interval > 0.0);
}

#[test]
fn adaptive_ticks_degenerate_inputs() {
    // Non-positive / non-finite visible spans must not yield a 0 or NaN
    // step (which would spin the ruler's tick loop).
    for &v in &[0.0_f64, -1.0, f64::NAN, f64::INFINITY] {
        let (interval, _) = adaptive_ticks(v, 1000.0);
        assert!(interval.is_finite() && interval > 0.0, "v={v}");
    }
    let (interval, _) = adaptive_ticks(1.0, f32::NAN);
    assert!(interval.is_finite() && interval > 0.0);
}

#[test]
fn adaptive_ticks_bounded_count_at_extreme_zoom() {
    // The tick loop draws `(visible / interval)` ticks. The "nice"
    // selection must keep that count bounded even at an extreme zoom-out
    // (a huge visible span). Mirror the loop's iteration bound check.
    let visible = 1e6_f64; // a million seconds visible
    let (interval, _) = adaptive_ticks(visible, 1200.0);
    assert!(interval > 0.0);
    let count = visible / interval;
    assert!(
        count < 2000.0,
        "tick count {count} would overrun safety guard"
    );
}

#[test]
fn first_tick_snap_is_at_or_before_view_start() {
    // The ruler snaps the first tick to floor(time_start / interval) *
    // interval. It must never start *after* time_start (would leave a
    // gap at the left edge).
    let interval = 0.05_f64;
    for &start in &[0.0_f64, 0.123, -0.07, 12.34] {
        let first = (start / interval).floor() * interval;
        assert!(first <= start + 1e-12, "first={first} start={start}");
        assert!(start - first < interval + 1e-12);
    }
}

#[test]
fn format_duration_ranges() {
    let (v, s) = format_duration(0.5e-9);
    assert!(s == "ns");
    assert!(v > 0.0);

    let (v, s) = format_duration(500e-6);
    assert!(s == "\u{00B5}s");
    assert!((v - 500.0).abs() < 0.1);

    let (v, s) = format_duration(0.042);
    assert!(s == "ms");
    assert!((v - 42.0).abs() < 0.1);

    let (v, s) = format_duration(2.5);
    assert!(s == "s");
    assert!((v - 2.5).abs() < 0.01);
}

#[test]
fn str_hash_deterministic() {
    assert_eq!(str_hash("update"), str_hash("update"));
    assert_ne!(str_hash("update"), str_hash("render"));
}

// ── Config ───────────────────────────────────────────────────────────────────

#[test]
fn config_defaults() {
    let cfg = TimelineConfig::default();
    assert_eq!(cfg.row_height, 20.0);
    assert!(cfg.show_ruler);
    assert!(cfg.show_tooltip);
    assert_eq!(cfg.span_palette.len(), 10);
}

#[test]
fn config_round_trips_through_ron() {
    let cfg = TimelineConfig::default();
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: TimelineConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.row_height, cfg.row_height);
    assert_eq!(back.span_palette.len(), cfg.span_palette.len());
    assert_eq!(back.min_zoom, cfg.min_zoom);
    assert_eq!(back.max_zoom, cfg.max_zoom);
    assert_eq!(back.color_mode, cfg.color_mode);
}

// ── Color resolution ─────────────────────────────────────────────────────────

#[test]
fn color_by_name() {
    let mut tl = Timeline::new("##test");
    let mut t = Track::new("t");
    t.add_span(Span::new(1, 0.0, 1.0, 0, "a"));
    t.add_span(Span::new(2, 0.0, 1.0, 0, "b"));
    tl.add_track(t);
    tl.config.color_mode = ColorMode::ByName;

    let dr = tl.data_time_range();
    let c1 = tl.span_color(&tl.tracks[0].spans[0], dr);
    let c2 = tl.span_color(&tl.tracks[0].spans[1], dr);
    assert!(c1[3] > 0.0);
    assert!(c2[3] > 0.0);
}

#[test]
fn color_by_duration() {
    let mut tl = Timeline::new("##test");
    let mut t = Track::new("t");
    t.add_span(Span::new(1, 0.0, 0.001, 0, "short"));
    t.add_span(Span::new(2, 0.0, 1.0, 0, "long"));
    tl.add_track(t);
    tl.config.color_mode = ColorMode::ByDuration;

    let dr = tl.data_time_range();
    let cs = tl.span_color(&tl.tracks[0].spans[0], dr);
    let cl = tl.span_color(&tl.tracks[0].spans[1], dr);
    assert!(cs[2] > cl[2]);
    assert!(cl[0] > cs[0]);
}

#[test]
fn color_by_depth_index_safe() {
    // ByDepth indexes the palette by `depth % palette.len()`; a depth far
    // beyond the palette length must wrap, never panic.
    let mut tl = Timeline::new("##depth");
    let mut t = Track::new("t");
    t.add_span(Span::new(1, 0.0, 1.0, 9_999, "deep"));
    tl.add_track(t);
    tl.config.color_mode = ColorMode::ByDepth;
    let dr = tl.data_time_range();
    let c = tl.span_color(&tl.tracks[0].spans[0], dr);
    assert!(c[3] > 0.0);
}

#[test]
fn color_empty_palette_falls_back() {
    let mut tl = Timeline::new("##empty");
    tl.config.span_palette.clear();
    let span = Span::new(1, 0.0, 1.0, 0, "x");
    let c = tl.span_color(&span, (0.0, 1.0));
    assert_eq!(c, [0.5, 0.5, 0.5, 0.9]);
}

// ── Index / state safety ─────────────────────────────────────────────────────

#[test]
fn track_mut_out_of_range_is_none() {
    let mut tl = Timeline::new("##idx");
    assert!(tl.track_mut(0).is_none());
    tl.add_track(Track::new("a"));
    assert!(tl.track_mut(0).is_some());
    assert!(tl.track_mut(99).is_none());
}

#[test]
fn track_collapsed() {
    let mut t = Track::new("t");
    t.add_span(Span::new(1, 0.0, 1.0, 0, "a"));
    t.collapsed = true;
    assert_eq!(t.depth_rows(), 1);
}

#[test]
fn total_content_height_accounts_for_collapse() {
    let cfg = TimelineConfig::default();
    let mut tl = Timeline::new("##h");
    let mut t = Track::new("a");
    t.add_span(Span::new(1, 0.0, 1.0, 0, "x"));
    t.add_span(Span::new(2, 0.0, 1.0, 2, "y")); // depth_rows = 3
    tl.add_track(t);

    let expanded = tl.total_content_height();
    tl.tracks[0].collapsed = true;
    let collapsed = tl.total_content_height();
    assert!(collapsed < expanded);
    assert!((collapsed - cfg.track_header_height).abs() < 1e-3);
}

#[test]
fn timeline_clear() {
    let mut tl = Timeline::new("##test");
    tl.add_track(Track::new("a"));
    tl.add_marker(Marker::new(0.0, "m"));
    tl.clear_tracks();
    tl.clear_markers();
    assert!(tl.tracks.is_empty());
    assert!(tl.markers.is_empty());
}

// ── i18n guard tests ─────────────────────────────────────────────────────────

#[test]
fn timeline_strings_resolve() {
    let en = crate::i18n::timeline::strings(Locale::En);
    let ru = crate::i18n::timeline::strings(Locale::Ru);
    assert_eq!(en.category_label, "Category: ");
    assert_eq!(ru.category_label, "Категория: ");
}

#[test]
fn default_locale_is_english() {
    let cfg = TimelineConfig::default();
    assert_eq!(cfg.locale, Locale::En);
    let tl = Timeline::new("##loc");
    assert_eq!(tl.locale(), Locale::En);
}

#[test]
fn locale_round_trips_through_ron() {
    let cfg = TimelineConfig {
        locale: Locale::Ru,
        ..TimelineConfig::default()
    };
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: TimelineConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.locale, Locale::Ru);
}

#[test]
fn locale_field_optional_in_ron() {
    // Older configs written before the `locale` field existed must still
    // parse and fall back to English (the field carries `#[serde(default)]`).
    // Hand-authored ron with NO `locale:` key.
    let back: TimelineConfig = ron::from_str(
        r#"(
            row_height: 20.0,
            row_gap: 1.0,
            ruler_height: 24.0,
            track_label_width: 120.0,
            min_span_width: 2.0,
            track_header_height: 22.0,
            mode: TopDown,
            color_mode: ByName,
            show_ruler: true,
            show_track_labels: true,
            show_tooltip: true,
            smooth_zoom: true,
            smooth_zoom_speed: 12.0,
            min_zoom: 0.000000001,
            max_zoom: 1000000.0,
            show_markers: true,
            color_bg: (0.12, 0.12, 0.14, 1.0),
            color_bg_alt: (0.14, 0.14, 0.16, 1.0),
            color_ruler_bg: (0.16, 0.18, 0.20, 1.0),
            color_ruler_text: (0.60, 0.65, 0.70, 1.0),
            color_track_label: (0.70, 0.75, 0.80, 1.0),
            color_track_separator: (0.25, 0.28, 0.32, 0.8),
            color_span_text: (0.95, 0.95, 0.95, 1.0),
            color_selection: (1.00, 0.85, 0.20, 1.0),
            color_hover: (0.80, 0.85, 1.00, 0.8),
            color_marker: (0.90, 0.30, 0.30, 0.7),
            color_tooltip_bg: (0.10, 0.10, 0.12, 0.95),
            color_tooltip_text: (0.90, 0.90, 0.92, 1.0),
            span_palette: [
                (0.35, 0.55, 0.85, 0.9),
            ],
        )"#,
    )
    .expect("config without locale: should parse");
    assert_eq!(back.locale, Locale::En);
}

#[test]
fn with_locale_sets_locale() {
    let tl = Timeline::new("##wl").with_locale(Locale::Ru);
    assert_eq!(tl.locale(), Locale::Ru);
    let mut tl = tl;
    tl.set_locale(Locale::En);
    assert_eq!(tl.locale(), Locale::En);
}
