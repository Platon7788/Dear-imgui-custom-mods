//! `crate::timeline` localisation strings.

#![allow(missing_docs)]

use super::Locale;

/// Tooltip labels rendered by [`crate::timeline::Timeline`] when
/// `cfg.show_tooltip` is on.
#[derive(Debug)]
pub struct Strings {
    pub category_label: &'static str,     // "Category: "
    pub source_label: &'static str,       // "Source: "
    pub start_end_template: &'static str, // "Start: {:.4}ms  End: {:.4}ms"
    pub depth_label: &'static str,        // "Depth: "
}

pub const EN: Strings = Strings {
    category_label: "Category: ",
    source_label: "Source: ",
    start_end_template: "Start: {start:.4}ms  End: {end:.4}ms",
    depth_label: "Depth: ",
};

pub const RU: Strings = Strings {
    category_label: "Категория: ",
    source_label: "Источник: ",
    start_end_template: "Начало: {start:.4}мс  Конец: {end:.4}мс",
    depth_label: "Глубина: ",
};

pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}

/// `"Start: 1.2345ms  End: 6.7890ms"` for the tooltip's time line.
/// Accepts `f64` because `Span::start` / `Span::end` are stored as
/// `f64` for sub-millisecond precision over long captures.
pub fn start_end(locale: Locale, start_ms: f64, end_ms: f64) -> String {
    match locale {
        Locale::En => format!("Start: {start_ms:.4}ms  End: {end_ms:.4}ms"),
        Locale::Ru => format!("Начало: {start_ms:.4}мс  Конец: {end_ms:.4}мс"),
    }
}
