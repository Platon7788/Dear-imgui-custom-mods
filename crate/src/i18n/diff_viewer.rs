//! `crate::diff_viewer` localisation strings.

#![allow(missing_docs)]

use super::Locale;

/// Toolbar button labels.
#[derive(Debug)]
pub struct Strings {
    pub prev_button: &'static str, // "Prev (Shift+F7)"
    pub next_button: &'static str, // "Next (F7)"
}

pub const EN: Strings = Strings {
    prev_button: "Prev (Shift+F7)",
    next_button: "Next (F7)",
};

pub const RU: Strings = Strings {
    prev_button: "Назад (Shift+F7)",
    next_button: "Вперёд (F7)",
};

pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}
