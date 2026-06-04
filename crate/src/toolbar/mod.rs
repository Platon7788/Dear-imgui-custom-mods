//! # Toolbar
//!
//! Configurable horizontal toolbar with buttons, toggles, separators,
//! dropdowns, and spacers. Builder-pattern API for declarative layout.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::toolbar::{Toolbar, ToolbarItem};
//!
//! let mut toolbar = Toolbar::new("##toolbar");
//! toolbar.add(ToolbarItem::button("New", "Create new file"));
//! toolbar.add(ToolbarItem::button("Open", "Open file"));
//! toolbar.add(ToolbarItem::separator());
//! toolbar.add(ToolbarItem::toggle("Bold", false, "Toggle bold"));
//! toolbar.add(ToolbarItem::spacer());
//! toolbar.add(ToolbarItem::button("Settings", "Open settings"));
//! // In render loop: let events = toolbar.render(ui);
//! ```
//!
//! ## Module layout
//!
//! - [`item`] — [`ToolbarItem`] / [`ToolbarItemKind`] data + builders.
//! - [`events`] — [`ToolbarEvent`] emitted per frame.
//! - [`layout`] — pure (context-free) width / spacer math.
//! - [`render`] — the per-frame [`Toolbar::render`] implementation.
//!
//! ## Localization
//!
//! The toolbar draws **only host-supplied** strings — button/toggle
//! labels, tooltips, and dropdown option text all come from the caller.
//! It owns no user-visible vocabulary of its own, so (per the crate i18n
//! policy) it carries no `Locale` field and no string catalogue: the
//! host translates the labels it passes in.

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
pub mod events;
pub mod item;
mod layout;
mod render;

pub use config::ToolbarConfig;
pub use events::ToolbarEvent;
pub use item::{ToolbarItem, ToolbarItemKind};

use crate::utils::color::rgba_f32;

/// Pack an RGBA float color into ImGui's packed `u32`.
pub(crate) fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── Toolbar widget ──────────────────────────────────────────────────────────

/// Configurable horizontal toolbar.
pub struct Toolbar {
    id: String,
    items: Vec<ToolbarItem>,
    pub config: ToolbarConfig,
}

impl Toolbar {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            config: ToolbarConfig::default(),
        }
    }

    /// The toolbar's ImGui id string.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Add an item to the toolbar.
    pub fn add(&mut self, item: ToolbarItem) -> &mut Self {
        self.items.push(item);
        self
    }

    /// Access items mutably (e.g. to update toggle states).
    pub fn items_mut(&mut self) -> &mut Vec<ToolbarItem> {
        &mut self.items
    }

    pub fn items(&self) -> &[ToolbarItem] {
        &self.items
    }

    /// Get a specific item by index.
    pub fn get(&self, index: usize) -> Option<&ToolbarItem> {
        self.items.get(index)
    }

    /// Get a specific item mutably by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut ToolbarItem> {
        self.items.get_mut(index)
    }

    /// Remove an item by index, returning it.
    ///
    /// Returns `None` (and leaves the toolbar unchanged) when `index` is
    /// out of range, so a stale index from the caller can never panic.
    pub fn remove(&mut self, index: usize) -> Option<ToolbarItem> {
        if index < self.items.len() {
            Some(self.items.remove(index))
        } else {
            None
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the toolbar has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::item::{display_text, display_text_ref};
    use super::*;

    #[test]
    fn button_item() {
        let item = ToolbarItem::button("New", "Create new");
        assert_eq!(item.label, "New");
        assert!(item.enabled);
        assert!(matches!(item.kind, ToolbarItemKind::Button));
    }

    #[test]
    fn toggle_item() {
        let item = ToolbarItem::toggle("Bold", true, "Toggle bold");
        assert!(matches!(item.kind, ToolbarItemKind::Toggle { on: true }));
    }

    #[test]
    fn separator_item() {
        let item = ToolbarItem::separator();
        assert!(matches!(item.kind, ToolbarItemKind::Separator));
    }

    #[test]
    fn spacer_item() {
        let item = ToolbarItem::spacer();
        assert!(matches!(item.kind, ToolbarItemKind::Spacer));
    }

    #[test]
    fn dropdown_item() {
        let item = ToolbarItem::dropdown(
            "Mode",
            vec!["Debug".into(), "Release".into()],
            0,
            "Select mode",
        );
        assert!(matches!(item.kind, ToolbarItemKind::Dropdown { .. }));
    }

    /// Regression: an out-of-range `selected` must be clamped at
    /// construction so the renderer never indexes past `options`.
    #[test]
    fn dropdown_clamps_out_of_range_selected() {
        let item = ToolbarItem::dropdown("Mode", vec!["A".into(), "B".into()], 99, "");
        match item.kind {
            ToolbarItemKind::Dropdown { selected, .. } => assert_eq!(selected, 1),
            _ => panic!("expected dropdown"),
        }
    }

    /// Regression: an empty option list must not underflow
    /// `options.len() - 1` and must clamp `selected` to `0`.
    #[test]
    fn dropdown_empty_options_selected_is_zero() {
        let item = ToolbarItem::dropdown("Mode", Vec::new(), 5, "");
        match item.kind {
            ToolbarItemKind::Dropdown { selected, options } => {
                assert_eq!(selected, 0);
                assert!(options.is_empty());
            }
            _ => panic!("expected dropdown"),
        }
    }

    #[test]
    fn disabled_item() {
        let item = ToolbarItem::button("X", "").with_enabled(false);
        assert!(!item.enabled);
    }

    #[test]
    fn with_icon_sets_icon() {
        let item = ToolbarItem::button("Save", "").with_icon("\u{F0193}");
        assert_eq!(item.icon, "\u{F0193}");
    }

    #[test]
    fn toolbar_add_clear() {
        let mut tb = Toolbar::new("##test");
        tb.add(ToolbarItem::button("A", ""));
        tb.add(ToolbarItem::separator());
        tb.add(ToolbarItem::button("B", ""));
        assert_eq!(tb.items().len(), 3);
        tb.clear();
        assert!(tb.items().is_empty());
    }

    #[test]
    fn toolbar_id_round_trips() {
        let tb = Toolbar::new("##my_bar");
        assert_eq!(tb.id(), "##my_bar");
    }

    #[test]
    fn toolbar_len_and_empty() {
        let mut tb = Toolbar::new("##t");
        assert!(tb.is_empty());
        assert_eq!(tb.len(), 0);
        tb.add(ToolbarItem::button("A", ""));
        assert_eq!(tb.len(), 1);
        assert!(!tb.is_empty());
    }

    /// Regression: out-of-range `remove` returns `None` instead of
    /// panicking (`Vec::remove` would panic).
    #[test]
    fn remove_out_of_range_is_none() {
        let mut tb = Toolbar::new("##t");
        tb.add(ToolbarItem::button("A", ""));
        assert!(tb.remove(5).is_none());
        assert_eq!(tb.len(), 1);
    }

    #[test]
    fn remove_in_range_returns_item() {
        let mut tb = Toolbar::new("##t");
        tb.add(ToolbarItem::button("A", ""));
        tb.add(ToolbarItem::button("B", ""));
        let removed = tb.remove(0).expect("index 0 in range");
        assert_eq!(removed.label, "A");
        assert_eq!(tb.len(), 1);
        assert_eq!(tb.items()[0].label, "B");
    }

    #[test]
    fn get_and_get_mut() {
        let mut tb = Toolbar::new("##t");
        tb.add(ToolbarItem::button("A", ""));
        assert_eq!(tb.get(0).map(|i| i.label.as_str()), Some("A"));
        assert!(tb.get(1).is_none());
        tb.get_mut(0).unwrap().label = "Z".into();
        assert_eq!(tb.items()[0].label, "Z");
    }

    #[test]
    fn config_defaults() {
        let cfg = ToolbarConfig::default();
        assert_eq!(cfg.height, 30.0);
        assert_eq!(cfg.button_rounding, 3.0);
    }

    /// The config schema lives in `config.rs`; the values live in
    /// `config.ron`. `Default` must load the ron, and the whole struct
    /// must round-trip through ron (DDD config pattern guard).
    #[test]
    fn config_round_trips_through_ron() {
        let cfg = ToolbarConfig::default();
        let serialized = ron::to_string(&cfg).expect("serialize");
        let restored: ToolbarConfig = ron::from_str(&serialized).expect("deserialize");
        assert_eq!(cfg.height, restored.height);
        assert_eq!(cfg.item_spacing, restored.item_spacing);
        assert_eq!(cfg.color_bg, restored.color_bg);
        assert_eq!(
            cfg.hover_underline_thickness,
            restored.hover_underline_thickness
        );
    }

    #[test]
    fn item_labels_distinct() {
        let a = ToolbarItem::button("a", "");
        let b = ToolbarItem::button("b", "");
        assert_ne!(a.label, b.label);
    }

    // ── display-text helper behaviour (zero-alloc Cow paths) ──────────

    #[test]
    fn display_text_label_only_borrows() {
        let item = ToolbarItem::button("Save", "");
        let s = display_text(&item);
        assert!(matches!(s, std::borrow::Cow::Borrowed(_)));
        assert_eq!(s, "Save");
    }

    #[test]
    fn display_text_icon_only_borrows() {
        let s = display_text_ref("\u{F0193}", "");
        assert!(matches!(s, std::borrow::Cow::Borrowed(_)));
        assert_eq!(s, "\u{F0193}");
    }

    #[test]
    fn display_text_icon_and_label_joins() {
        let s = display_text_ref("I", "Save");
        assert!(matches!(s, std::borrow::Cow::Owned(_)));
        assert_eq!(s, "I Save");
    }
}
