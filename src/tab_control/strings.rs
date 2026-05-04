//! Localizable strings for tab control.

#![allow(missing_docs)] // string field names are self-explanatory

/// User-facing strings — override for localization.
#[derive(serde::Serialize, serde::Deserialize)]
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

impl Default for TabStrings {
    fn default() -> Self {
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
}
