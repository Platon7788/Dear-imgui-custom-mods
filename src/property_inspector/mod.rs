//! # PropertyInspector
//!
//! Hierarchical property editor — two-column tree-table for editing
//! typed key-value pairs. Supports 15+ value types, categories,
//! search/filter, diff highlighting, and nested objects.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::property_inspector::{
//!     PropertyInspector, PropertyNode, PropertyValue,
//! };
//!
//! let mut inspector = PropertyInspector::new("##props");
//! inspector.add_category("Transform");
//! inspector.add("position", PropertyValue::Vec3([0.0, 0.0, 0.0]));
//! inspector.add("rotation", PropertyValue::F32(0.0));
//! // In render loop: inspector.render(ui);
//! ```

pub mod config;
pub mod value;

pub use config::InspectorConfig;
pub use value::PropertyValue;

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── Property node ───────────────────────────────────────────────────────────

/// A single property in the inspector.
#[derive(Debug, Clone, Default)]
pub struct PropertyNode {
    /// Key / label.
    pub key: String,
    /// Value.
    pub value: PropertyValue,
    /// Whether this property is read-only.
    pub read_only: bool,
    /// Whether this property was recently changed (for diff highlighting).
    pub changed: bool,
    /// Children (for Object/Array types).
    pub children: Vec<PropertyNode>,
    /// Whether the node is expanded (for Object/Array).
    pub expanded: bool,
    /// Nesting depth.
    depth: u32,
}

impl PropertyNode {
    pub fn new(key: impl Into<String>, value: PropertyValue) -> Self {
        Self {
            key: key.into(),
            value,
            read_only: false,
            changed: false,
            children: Vec::new(),
            expanded: false,
            depth: 0,
        }
    }

    pub fn with_readonly(mut self, ro: bool) -> Self {
        self.read_only = ro;
        self
    }

    pub fn with_changed(mut self, c: bool) -> Self {
        self.changed = c;
        self
    }

    pub fn with_child(mut self, child: PropertyNode) -> Self {
        self.children.push(child);
        self
    }
}

// ── Category ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Category {
    name: String,
    collapsed: bool,
    properties: Vec<PropertyNode>,
}

// ── Events ──────────────────────────────────────────────────────────────────

/// Event emitted when a property value changes.
#[derive(Debug, Clone)]
pub struct PropertyChangedEvent {
    /// Key path (e.g. "Transform.position").
    pub key: String,
    /// New value display string.
    pub new_value: String,
}

// ── PropertyInspector ───────────────────────────────────────────────────────

/// Hierarchical property editor widget.
pub struct PropertyInspector {
    id: String,
    categories: Vec<Category>,
    /// Current active category for `add()` calls.
    active_category: usize,
    /// Filter text.
    filter: String,
    /// Configuration.
    pub config: InspectorConfig,
}

impl PropertyInspector {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            categories: vec![Category {
                name: String::new(),
                collapsed: false,
                properties: Vec::new(),
            }],
            active_category: 0,
            filter: String::new(),
            config: InspectorConfig::default(),
        }
    }

    /// Add a category header. Subsequent `add()` calls go into this category.
    pub fn add_category(&mut self, name: impl Into<String>) -> &mut Self {
        self.categories.push(Category {
            name: name.into(),
            collapsed: false,
            properties: Vec::new(),
        });
        self.active_category = self.categories.len() - 1;
        self
    }

    /// Add a property to the current category.
    pub fn add(&mut self, key: impl Into<String>, value: PropertyValue) -> &mut Self {
        let node = PropertyNode::new(key, value);
        self.categories[self.active_category].properties.push(node);
        self
    }

    /// Add a full property node.
    pub fn add_node(&mut self, node: PropertyNode) -> &mut Self {
        self.categories[self.active_category].properties.push(node);
        self
    }

    /// Clear all categories and properties.
    pub fn clear(&mut self) {
        self.categories.clear();
        self.categories.push(Category {
            name: String::new(),
            collapsed: false,
            properties: Vec::new(),
        });
        self.active_category = 0;
    }

    /// Total number of properties across all categories.
    pub fn property_count(&self) -> usize {
        self.categories.iter().map(|c| c.properties.len()).sum()
    }

    /// Render the inspector. Returns change events.
    pub fn render(&mut self, ui: &Ui) -> Vec<PropertyChangedEvent> {
        let events = Vec::new();
        let cfg = self.config; // Copy, not clone

        let _id_tok = ui.push_id(&self.id);

        // Filter bar
        if cfg.show_filter {
            ui.set_next_item_width(-1.0);
            ui.input_text("##filter", &mut self.filter).build();
        }

        let avail = ui.content_region_avail();
        let key_w = avail[0] * cfg.key_width_ratio;

        ui.child_window("##inspector_scroll")
            .size(avail)
            .build(ui, || {
                let draw = ui.get_window_draw_list();
                let win_pos = ui.cursor_screen_pos();
                let win_w = ui.content_region_avail()[0];

                let mouse_pos = ui.io().mouse_pos();
                let is_clicked = ui.is_mouse_clicked(MouseButton::Left);
                let window_hovered = ui.is_window_hovered();

                let mut y = win_pos[1];
                let mut row_idx = 0usize;
                let filter_lower = self.filter.to_lowercase();

                for cat_idx in 0..self.categories.len() {
                    // Category header
                    if cfg.show_categories && !self.categories[cat_idx].name.is_empty() {
                        // Category background
                        draw.add_rect(
                            [win_pos[0], y],
                            [win_pos[0] + win_w, y + cfg.row_height],
                            col32(cfg.color_category_bg),
                        ).filled(true).build();

                        // Hover highlight on category header
                        let cat_row_hovered = mouse_pos[0] >= win_pos[0]
                            && mouse_pos[0] < win_pos[0] + win_w
                            && mouse_pos[1] >= y
                            && mouse_pos[1] < y + cfg.row_height;
                        if cat_row_hovered {
                            draw.add_rect(
                                [win_pos[0], y],
                                [win_pos[0] + win_w, y + cfg.row_height],
                                col32([1.0, 1.0, 1.0, 0.04]),
                            ).filled(true).build();
                        }

                        let arrow = if self.categories[cat_idx].collapsed {
                            "\u{25B8}"
                        } else {
                            "\u{25BE}"
                        };
                        let text = format!("{} {}", arrow, self.categories[cat_idx].name);
                        let ty = y + (cfg.row_height - calc_text_size(&text)[1]) * 0.5;
                        draw.add_text(
                            [win_pos[0] + 4.0, ty],
                            col32(cfg.color_category_text),
                            &text,
                        );

                        // Click detection for category collapse toggle
                        let cat_hovered = window_hovered
                            && mouse_pos[0] >= win_pos[0]
                            && mouse_pos[0] < win_pos[0] + win_w
                            && mouse_pos[1] >= y
                            && mouse_pos[1] < y + cfg.row_height;
                        if cat_hovered && is_clicked {
                            self.categories[cat_idx].collapsed =
                                !self.categories[cat_idx].collapsed;
                        }

                        y += cfg.row_height;
                    }

                    if self.categories[cat_idx].collapsed {
                        continue;
                    }

                    for prop_idx in 0..self.categories[cat_idx].properties.len() {
                        // Filter
                        if !filter_lower.is_empty()
                            && !self.categories[cat_idx].properties[prop_idx]
                                .key
                                .to_lowercase()
                                .contains(&filter_lower)
                            && !self.categories[cat_idx].properties[prop_idx]
                                .value
                                .display()
                                .to_lowercase()
                                .contains(&filter_lower)
                        {
                            continue;
                        }

                        Self::render_property(
                            &draw,
                            ui,
                            &mut self.categories[cat_idx].properties[prop_idx],
                            &mut y,
                            &mut row_idx,
                            win_pos,
                            win_w,
                            key_w,
                            &cfg,
                            mouse_pos,
                            is_clicked,
                            window_hovered,
                        );
                    }
                }

                // Dummy for scroll
                ui.set_cursor_pos([0.0, y - win_pos[1]]);
                ui.dummy([1.0, 1.0]);
            });

        events
    }

    /// Render a single property row and its children recursively.
    #[allow(clippy::too_many_arguments, clippy::only_used_in_recursion)]
    fn render_property(
        draw: &dear_imgui_rs::DrawListMut<'_>,
        ui: &Ui,
        prop: &mut PropertyNode,
        y: &mut f32,
        row_idx: &mut usize,
        win_pos: [f32; 2],
        win_w: f32,
        key_w: f32,
        cfg: &InspectorConfig,
        mouse_pos: [f32; 2],
        is_clicked: bool,
        window_hovered: bool,
    ) {
        // Alternate row background
        if *row_idx % 2 == 1 {
            draw.add_rect(
                [win_pos[0], *y],
                [win_pos[0] + win_w, *y + cfg.row_height],
                col32(cfg.color_bg_alt),
            ).filled(true).build();
        }

        // Changed highlight
        if cfg.highlight_changes && prop.changed {
            draw.add_rect(
                [win_pos[0], *y],
                [win_pos[0] + win_w, *y + cfg.row_height],
                col32(cfg.color_changed),
            ).filled(true).build();
        }

        // Hover highlight
        let row_hovered = mouse_pos[0] >= win_pos[0]
            && mouse_pos[0] < win_pos[0] + win_w
            && mouse_pos[1] >= *y
            && mouse_pos[1] < *y + cfg.row_height;
        if row_hovered {
            draw.add_rect(
                [win_pos[0], *y],
                [win_pos[0] + win_w, *y + cfg.row_height],
                col32([1.0, 1.0, 1.0, 0.04]),
            ).filled(true).build();
        }

        let indent = prop.depth as f32 * cfg.indent;
        let ty = *y + (cfg.row_height - calc_text_size("A")[1]) * 0.5;

        // Expand arrow for Object/Array
        let has_children = !prop.children.is_empty()
            || matches!(prop.value, PropertyValue::Object | PropertyValue::Array(_));
        if has_children {
            let arrow = if prop.expanded { "\u{25BE}" } else { "\u{25B8}" };
            draw.add_text(
                [win_pos[0] + indent + 2.0, ty],
                col32(cfg.color_key),
                arrow,
            );
        }

        // Click detection for expand/collapse on property rows with children
        if has_children {
            let prop_hovered = window_hovered
                && mouse_pos[0] >= win_pos[0]
                && mouse_pos[0] < win_pos[0] + win_w
                && mouse_pos[1] >= *y
                && mouse_pos[1] < *y + cfg.row_height;
            if prop_hovered && is_clicked {
                prop.expanded = !prop.expanded;
            }
        }

        // Key
        let key_x = win_pos[0] + indent + if has_children { 16.0 } else { 4.0 };
        draw.add_text(
            [key_x, ty],
            col32(cfg.color_key),
            &prop.key,
        );

        // Separator
        draw.add_line(
            [win_pos[0] + key_w, *y],
            [win_pos[0] + key_w, *y + cfg.row_height],
            col32(cfg.color_separator),
        ).build();

        // Value
        let val_x = win_pos[0] + key_w + 4.0;
        let val_text = prop.value.display();
        let val_color = if prop.read_only {
            cfg.color_readonly
        } else {
            cfg.color_value
        };

        // Color swatch for Color3/Color4
        match &prop.value {
            PropertyValue::Color3(c) => {
                draw.add_rect(
                    [val_x, *y + 2.0],
                    [val_x + 14.0, *y + cfg.row_height - 2.0],
                    col32([c[0], c[1], c[2], 1.0]),
                ).filled(true).build();
                draw.add_text(
                    [val_x + 18.0, ty],
                    col32(val_color),
                    &val_text,
                );
            }
            PropertyValue::Color4(c) => {
                draw.add_rect(
                    [val_x, *y + 2.0],
                    [val_x + 14.0, *y + cfg.row_height - 2.0],
                    col32(*c),
                ).filled(true).build();
                draw.add_text(
                    [val_x + 18.0, ty],
                    col32(val_color),
                    &val_text,
                );
            }
            _ => {
                draw.add_text(
                    [val_x, ty],
                    col32(val_color),
                    &val_text,
                );
            }
        }

        // Type badge (dimmed, right-aligned)
        let type_badge = prop.value.type_name();
        let badge_x = win_pos[0] + win_w - calc_text_size(type_badge)[0] - 6.0;
        draw.add_text(
            [badge_x, ty],
            col32([0.35, 0.38, 0.45, 1.0]),
            type_badge,
        );

        *y += cfg.row_height;
        *row_idx += 1;

        // Recursively render children if expanded
        if prop.expanded && !prop.children.is_empty() {
            for child_idx in 0..prop.children.len() {
                prop.children[child_idx].depth = prop.depth + 1;
                // We need to split the borrow: take child out temporarily
                // to satisfy the borrow checker with mutable recursion.
                let mut child = std::mem::take(&mut prop.children[child_idx]);
                child.depth = prop.depth + 1;
                Self::render_property(
                    draw,
                    ui,
                    &mut child,
                    y,
                    row_idx,
                    win_pos,
                    win_w,
                    key_w,
                    cfg,
                    mouse_pos,
                    is_clicked,
                    window_hovered,
                );
                prop.children[child_idx] = child;
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_add() {
        let mut pi = PropertyInspector::new("##test");
        pi.add("name", PropertyValue::String("hello".into()));
        pi.add("count", PropertyValue::I32(42));
        assert_eq!(pi.property_count(), 2);
    }

    #[test]
    fn categories() {
        let mut pi = PropertyInspector::new("##test");
        pi.add_category("A");
        pi.add("x", PropertyValue::F32(1.0));
        pi.add_category("B");
        pi.add("y", PropertyValue::F32(2.0));
        assert_eq!(pi.categories.len(), 3); // default + A + B
        assert_eq!(pi.property_count(), 2);
    }

    #[test]
    fn clear() {
        let mut pi = PropertyInspector::new("##test");
        pi.add("a", PropertyValue::Bool(true));
        pi.clear();
        assert_eq!(pi.property_count(), 0);
    }

    #[test]
    fn node_builders() {
        let node = PropertyNode::new("test", PropertyValue::I32(10))
            .with_readonly(true)
            .with_changed(true)
            .with_child(PropertyNode::new("child", PropertyValue::Bool(false)));
        assert!(node.read_only);
        assert!(node.changed);
        assert_eq!(node.children.len(), 1);
    }

    #[test]
    fn value_display() {
        assert_eq!(PropertyValue::Bool(true).display(), "true");
        assert_eq!(PropertyValue::I32(-5).display(), "-5");
        assert_eq!(PropertyValue::String("hi".into()).display(), "hi");
        assert_eq!(PropertyValue::Object.display(), "{...}");
        assert_eq!(PropertyValue::Array(3).display(), "[3 items]");
    }

    #[test]
    fn value_type_name() {
        assert_eq!(PropertyValue::Bool(true).type_name(), "bool");
        assert_eq!(PropertyValue::F32(1.0).type_name(), "f32");
        assert_eq!(PropertyValue::Color4([0.0; 4]).type_name(), "color4");
    }

    #[test]
    fn enum_display() {
        let v = PropertyValue::Enum(1, vec!["A".into(), "B".into(), "C".into()]);
        assert_eq!(v.display(), "B");
    }

    #[test]
    fn flags_display() {
        let v = PropertyValue::Flags(0xFF, vec!["a".into()]);
        assert_eq!(v.display(), "0xFF");
    }

    #[test]
    fn config_defaults() {
        let cfg = InspectorConfig::default();
        assert!((cfg.key_width_ratio - 0.4).abs() < 0.01);
        assert!(cfg.show_filter);
        assert!(cfg.show_categories);
    }

    #[test]
    fn vec_display() {
        assert!(PropertyValue::Vec2([1.0, 2.0]).display().contains("1.00"));
        assert!(PropertyValue::Vec3([1.0, 2.0, 3.0]).display().contains("3.00"));
    }
}
