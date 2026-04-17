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

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;

pub use config::ToolbarConfig;

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── Toolbar item types ──────────────────────────────────────────────────────

/// Toolbar item variant.
#[derive(Debug, Clone)]
pub enum ToolbarItemKind {
    /// Clickable button.
    Button,
    /// Toggle button (on/off state).
    Toggle { on: bool },
    /// Visual separator line.
    Separator,
    /// Flexible spacer (pushes items to the right).
    Spacer,
    /// Dropdown (click → emits event, dropdown menu is handled externally).
    Dropdown { options: Vec<String>, selected: usize },
}

/// A single toolbar item.
#[derive(Debug, Clone)]
pub struct ToolbarItem {
    /// Display label.
    pub label: String,
    /// Unicode icon text (empty = no icon).
    pub icon: String,
    /// Kind of item.
    pub kind: ToolbarItemKind,
    /// Tooltip text (shown on hover).
    pub tooltip: String,
    /// Whether this item is enabled.
    pub enabled: bool,
}

impl ToolbarItem {
    /// Create a button.
    pub fn button(label: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: String::new(),
            kind: ToolbarItemKind::Button,
            tooltip: tooltip.into(),
            enabled: true,
        }
    }

    /// Create a toggle button.
    pub fn toggle(label: impl Into<String>, on: bool, tooltip: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: String::new(),
            kind: ToolbarItemKind::Toggle { on },
            tooltip: tooltip.into(),
            enabled: true,
        }
    }

    /// Create a separator.
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            icon: String::new(),
            kind: ToolbarItemKind::Separator,
            tooltip: String::new(),
            enabled: true,
        }
    }

    /// Create a spacer.
    pub fn spacer() -> Self {
        Self {
            label: String::new(),
            icon: String::new(),
            kind: ToolbarItemKind::Spacer,
            tooltip: String::new(),
            enabled: true,
        }
    }

    /// Create a dropdown.
    pub fn dropdown(
        label: impl Into<String>,
        options: Vec<String>,
        selected: usize,
        tooltip: impl Into<String>,
    ) -> Self {
        let clamped = if options.is_empty() { 0 } else { selected.min(options.len() - 1) };
        Self {
            label: label.into(),
            icon: String::new(),
            kind: ToolbarItemKind::Dropdown { options, selected: clamped },
            tooltip: tooltip.into(),
            enabled: true,
        }
    }

    /// Builder: set enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Builder: set icon text (Unicode glyph).
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

// ── Events ──────────────────────────────────────────────────────────────────

/// Event emitted by toolbar interaction.
#[derive(Debug, Clone)]
pub enum ToolbarEvent {
    /// A button was clicked.
    ButtonClicked { index: usize, label: String },
    /// A toggle was toggled (new state).
    Toggled { index: usize, label: String, on: bool },
    /// A dropdown selection changed.
    DropdownChanged { index: usize, label: String, selected: usize },
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build the display string by combining icon and label.
///
/// Returns a `Cow<str>` to avoid allocation when only one part is present.
fn display_text(item: &ToolbarItem) -> std::borrow::Cow<'_, str> {
    display_text_ref(&item.icon, &item.label)
}

/// Build the display string from icon and label references.
///
/// Zero-alloc when only icon or label is present.
fn display_text_ref<'a>(icon: &'a str, label: &'a str) -> std::borrow::Cow<'a, str> {
    if icon.is_empty() {
        std::borrow::Cow::Borrowed(label)
    } else if label.is_empty() {
        std::borrow::Cow::Borrowed(icon)
    } else {
        std::borrow::Cow::Owned(format!("{} {}", icon, label))
    }
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

    /// Remove an item by index. Returns the removed item.
    pub fn remove(&mut self, index: usize) -> ToolbarItem {
        self.items.remove(index)
    }

    /// Number of items.
    pub fn len(&self) -> usize { self.items.len() }

    /// Whether the toolbar has no items.
    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Render the toolbar. Returns events for this frame.
    pub fn render(&mut self, ui: &Ui) -> Vec<ToolbarEvent> {
        let mut events = Vec::new();
        let cfg = self.config;

        let _id_tok = ui.push_id(&self.id);

        let avail_w = ui.content_region_avail()[0];
        let bar_h = cfg.height;
        let cursor = ui.cursor_screen_pos();
        let draw = ui.get_window_draw_list();

        // Background
        draw.add_rect(
            cursor,
            [cursor[0] + avail_w, cursor[1] + bar_h],
            col32(cfg.color_bg),
        ).filled(true).build();

        // Bottom border
        draw.add_line(
            [cursor[0], cursor[1] + bar_h - 1.0],
            [cursor[0] + avail_w, cursor[1] + bar_h - 1.0],
            col32(cfg.color_border),
        ).build();

        let mouse_pos = ui.io().mouse_pos();
        let window_hovered = ui.is_window_hovered();
        let btn_h = bar_h - 6.0;
        let btn_y = cursor[1] + 3.0;

        // First pass: compute spacer width
        let mut fixed_w = 0.0_f32;
        let mut spacer_count = 0;
        for item in &self.items {
            match &item.kind {
                ToolbarItemKind::Spacer => spacer_count += 1,
                ToolbarItemKind::Separator => {
                    fixed_w += cfg.separator_margin * 2.0 + cfg.separator_width;
                }
                ToolbarItemKind::Dropdown { options, selected } => {
                    let base = display_text(item);
                    let label = if *selected < options.len() {
                        format!("{} [{}]", base, options[*selected])
                    } else {
                        base.into_owned()
                    };
                    fixed_w += calc_text_size(&label)[0] + cfg.button_padding * 2.0
                        + cfg.item_spacing;
                }
                _ => {
                    let text = display_text(item);
                    fixed_w += calc_text_size(&text)[0] + cfg.button_padding * 2.0
                        + cfg.item_spacing;
                }
            }
        }

        let spacer_w = if spacer_count > 0 {
            ((avail_w - fixed_w) / spacer_count as f32).max(0.0)
        } else {
            0.0
        };

        // Second pass: render
        let mut x = cursor[0] + cfg.item_spacing;

        for (idx, item) in self.items.iter_mut().enumerate() {
            // Separator and Spacer have no display text — handle them first.
            match &mut item.kind {
                ToolbarItemKind::Separator => {
                    x += cfg.separator_margin;
                    draw.add_line(
                        [x, btn_y + 2.0],
                        [x, btn_y + btn_h - 2.0],
                        col32(cfg.color_separator),
                    ).build();
                    x += cfg.separator_width + cfg.separator_margin;
                    continue;
                }
                ToolbarItemKind::Spacer => {
                    x += spacer_w;
                    continue;
                }
                _ => {}
            }

            // Shared pre-computation for Button / Toggle / Dropdown
            let base_display = display_text_ref(&item.icon, &item.label);
            let full_display: std::borrow::Cow<'_, str> = match &item.kind {
                ToolbarItemKind::Dropdown { options, selected } => {
                    if *selected < options.len() {
                        std::borrow::Cow::Owned(format!("{} [{}]", base_display, options[*selected]))
                    } else {
                        base_display.clone()
                    }
                }
                _ => base_display.clone(),
            };
            let text_sz = calc_text_size(&full_display);
            let text_w = text_sz[0];
            let btn_w = text_w + cfg.button_padding * 2.0;

            let hovered = item.enabled
                && window_hovered
                && mouse_pos[0] >= x
                && mouse_pos[0] < x + btn_w
                && mouse_pos[1] >= btn_y
                && mouse_pos[1] < btn_y + btn_h;

            let text_color = if item.enabled {
                cfg.color_text
            } else {
                cfg.color_disabled
            };

            match &mut item.kind {
                ToolbarItemKind::Button => {
                    if hovered {
                        let bg = if ui.is_mouse_down(MouseButton::Left) {
                            cfg.color_active
                        } else {
                            cfg.color_hover
                        };
                        draw.add_rect(
                            [x, btn_y],
                            [x + btn_w, btn_y + btn_h],
                            col32(bg),
                        ).rounding(cfg.button_rounding).filled(true).build();

                        // Hover underline
                        let uy = btn_y + btn_h - 1.0;
                        draw.add_line(
                            [x + 2.0, uy],
                            [x + btn_w - 2.0, uy],
                            col32(cfg.color_hover_underline),
                        ).thickness(cfg.hover_underline_thickness).build();

                        if ui.is_mouse_clicked(MouseButton::Left) {
                            events.push(ToolbarEvent::ButtonClicked {
                                index: idx,
                                label: item.label.clone(), // clone only on event (not per-frame)
                            });
                        }

                        if !item.tooltip.is_empty() {
                            ui.tooltip_text(&item.tooltip);
                        }
                    }
                }

                ToolbarItemKind::Toggle { on } => {
                    // Toggle background
                    if *on {
                        draw.add_rect(
                            [x, btn_y],
                            [x + btn_w, btn_y + btn_h],
                            col32(cfg.color_toggled),
                        ).rounding(cfg.button_rounding).filled(true).build();
                    }

                    if hovered {
                        let bg = if ui.is_mouse_down(MouseButton::Left) {
                            cfg.color_active
                        } else {
                            cfg.color_hover
                        };
                        draw.add_rect(
                            [x, btn_y],
                            [x + btn_w, btn_y + btn_h],
                            col32(bg),
                        ).rounding(cfg.button_rounding).filled(true).build();

                        // Hover underline
                        let uy = btn_y + btn_h - 1.0;
                        draw.add_line(
                            [x + 2.0, uy],
                            [x + btn_w - 2.0, uy],
                            col32(cfg.color_hover_underline),
                        ).thickness(cfg.hover_underline_thickness).build();

                        if ui.is_mouse_clicked(MouseButton::Left) {
                            *on = !*on;
                            events.push(ToolbarEvent::Toggled {
                                index: idx,
                                label: item.label.clone(), // clone only on event (not per-frame)
                                on: *on,
                            });
                        }

                        if !item.tooltip.is_empty() {
                            ui.tooltip_text(&item.tooltip);
                        }
                    }
                }

                ToolbarItemKind::Dropdown { options, selected } => {
                    if hovered {
                        let bg = if ui.is_mouse_down(MouseButton::Left) {
                            cfg.color_active
                        } else {
                            cfg.color_hover
                        };
                        draw.add_rect(
                            [x, btn_y],
                            [x + btn_w, btn_y + btn_h],
                            col32(bg),
                        ).rounding(cfg.button_rounding).filled(true).build();

                        // Hover underline
                        let uy = btn_y + btn_h - 1.0;
                        draw.add_line(
                            [x + 2.0, uy],
                            [x + btn_w - 2.0, uy],
                            col32(cfg.color_hover_underline),
                        ).thickness(cfg.hover_underline_thickness).build();

                        if ui.is_mouse_clicked(MouseButton::Left) && !options.is_empty() {
                            *selected = (*selected + 1) % options.len();
                            events.push(ToolbarEvent::DropdownChanged {
                                index: idx,
                                label: item.label.clone(), // clone only on event (not per-frame)
                                selected: *selected,
                            });
                        }

                        if !item.tooltip.is_empty() {
                            ui.tooltip_text(&item.tooltip);
                        }
                    }
                }

                // Separator/Spacer already handled above via `continue`.
                _ => {}
            }

            // Draw the display text (shared across all interactive item types).
            let tx = x + (btn_w - text_sz[0]) * 0.5;
            let ty = btn_y + (btn_h - text_sz[1]) * 0.5;
            draw.add_text([tx, ty], col32(text_color), &full_display);

            x += btn_w + cfg.item_spacing;
        }

        // Advance cursor past the toolbar
        ui.set_cursor_pos([ui.cursor_pos()[0], ui.cursor_pos()[1] + bar_h]);
        ui.dummy([0.0, 0.0]);

        events
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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

    #[test]
    fn disabled_item() {
        let item = ToolbarItem::button("X", "").with_enabled(false);
        assert!(!item.enabled);
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
    fn config_defaults() {
        let cfg = ToolbarConfig::default();
        assert_eq!(cfg.height, 30.0);
        assert_eq!(cfg.button_rounding, 3.0);
    }

    #[test]
    fn item_labels_distinct() {
        let a = ToolbarItem::button("a", "");
        let b = ToolbarItem::button("b", "");
        assert_ne!(a.label, b.label);
    }
}
