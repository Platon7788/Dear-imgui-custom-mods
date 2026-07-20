//! `crate::nav_panel` localisation strings.

#![allow(missing_docs)]

use super::Locale;

/// Panel-toggle button tooltips + the right-click reposition menu. The
/// nav buttons themselves carry host-supplied labels via `NavItem::label`
/// — those stay host-driven (the host owns the user's mental model of
/// "what each button means").
#[derive(Debug)]
pub struct Strings {
    pub show_panel: &'static str,   // "Show panel"
    pub toggle_panel: &'static str, // "Toggle panel"

    // ── Right-click reposition menu ──────────────────────────────────
    /// Header row of the reposition context menu.
    pub position_title: &'static str,
    /// "Dock left" menu entry.
    pub dock_left: &'static str,
    /// "Dock right" menu entry.
    pub dock_right: &'static str,
    /// "Dock top" menu entry.
    pub dock_top: &'static str,
    /// Tooltip for the "Dock left" entry.
    pub dock_left_hint: &'static str,
    /// Tooltip for the "Dock right" entry.
    pub dock_right_hint: &'static str,
    /// Tooltip for the "Dock top" entry.
    pub dock_top_hint: &'static str,
}

pub const EN: Strings = Strings {
    show_panel: "Show panel",
    toggle_panel: "Toggle panel",
    position_title: "Panel position",
    dock_left: "Dock left",
    dock_right: "Dock right",
    dock_top: "Dock top",
    dock_left_hint: "Move the navigation panel to the left edge",
    dock_right_hint: "Move the navigation panel to the right edge",
    dock_top_hint: "Move the navigation panel to the top edge",
};

pub const RU: Strings = Strings {
    show_panel: "Показать панель",
    toggle_panel: "Скрыть/показать панель",
    position_title: "Расположение панели",
    dock_left: "Слева",
    dock_right: "Справа",
    dock_top: "Сверху",
    dock_left_hint: "Переместить панель навигации к левому краю",
    dock_right_hint: "Переместить панель навигации к правому краю",
    dock_top_hint: "Переместить панель навигации к верхнему краю",
};

pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}
