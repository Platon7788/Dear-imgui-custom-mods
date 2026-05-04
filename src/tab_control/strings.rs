//! Localizable strings for tab control.

#![allow(missing_docs)] // string field names are self-explanatory

/// User-facing strings — override for localization.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TabStrings {
    pub cancel: String,
    pub close: String,
    pub close_confirm: String,
    /// Confirmation text shown when the tab being closed has unsaved changes
    /// (status `Dirty`). More urgent phrasing than [`Self::close_confirm`].
    pub close_confirm_dirty: String,
    pub no_tabs: String,
    pub empty_hint: String,
    pub overflow_tooltip: String,
    pub add_tab: String,
}

impl TabStrings {
    /// English catalogue — historic default.
    #[must_use]
    pub fn en() -> Self {
        Self {
            cancel: String::from("Cancel"),
            close: String::from("Close"),
            close_confirm: String::from("Close this tab?"),
            close_confirm_dirty: String::from(
                "This tab has unsaved changes. Discard and close?",
            ),
            no_tabs: String::from("No tabs"),
            empty_hint: String::from("Add a tab to begin\u{2026}"),
            overflow_tooltip: String::from("All tabs"),
            add_tab: String::from("New tab"),
        }
    }

    /// Russian catalogue. Switching to this requires the host to bake
    /// `GlyphRanges::Cyrillic` (or a superset) into the active font
    /// atlas — without that, non-ASCII characters render as `?`.
    #[must_use]
    pub fn ru() -> Self {
        Self {
            cancel: String::from("Отмена"),
            close: String::from("Закрыть"),
            close_confirm: String::from("Закрыть эту вкладку?"),
            close_confirm_dirty: String::from(
                "Во вкладке есть несохранённые изменения. Закрыть без сохранения?",
            ),
            no_tabs: String::from("Нет вкладок"),
            empty_hint: String::from("Добавьте вкладку, чтобы начать\u{2026}"),
            overflow_tooltip: String::from("Все вкладки"),
            add_tab: String::from("Новая вкладка"),
        }
    }

    /// Resolve the catalogue for the given [`crate::i18n::Locale`].
    #[must_use]
    pub fn for_locale(locale: crate::i18n::Locale) -> Self {
        match locale {
            crate::i18n::Locale::En => Self::en(),
            crate::i18n::Locale::Ru => Self::ru(),
        }
    }
}

impl Default for TabStrings {
    /// English catalogue. Use [`Self::for_locale`] to pick a different
    /// language; the [`crate::tab_control::TabControl::with_locale`]
    /// builder hooks straight into that.
    fn default() -> Self {
        Self::en()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Locale;

    #[test]
    fn en_and_ru_diverge_on_translatable_keys() {
        let en = TabStrings::en();
        let ru = TabStrings::ru();
        assert_ne!(en.cancel, ru.cancel);
        assert_ne!(en.close, ru.close);
        assert_ne!(en.no_tabs, ru.no_tabs);
        assert_eq!(en.cancel, "Cancel");
        assert_eq!(ru.cancel, "Отмена");
    }

    #[test]
    fn for_locale_resolves() {
        assert_eq!(TabStrings::for_locale(Locale::En).cancel, "Cancel");
        assert_eq!(TabStrings::for_locale(Locale::Ru).cancel, "Отмена");
    }

    #[test]
    fn default_is_english() {
        assert_eq!(TabStrings::default().cancel, "Cancel");
    }
}
