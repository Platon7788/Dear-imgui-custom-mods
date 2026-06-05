//! `crate::nav_panel` localisation strings.

#![allow(missing_docs)]

use super::Locale;

/// Panel-toggle button tooltips. The nav buttons themselves carry
/// host-supplied labels via `NavItem::label` — those stay
/// host-driven (the host owns the user's mental model of "what
/// each button means").
#[derive(Debug)]
pub struct Strings {
    pub show_panel: &'static str,   // "Show panel"
    pub toggle_panel: &'static str, // "Toggle panel"
}

pub const EN: Strings = Strings {
    show_panel: "Show panel",
    toggle_panel: "Toggle panel",
};

pub const RU: Strings = Strings {
    show_panel: "Показать панель",
    toggle_panel: "Скрыть/показать панель",
};

pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}
