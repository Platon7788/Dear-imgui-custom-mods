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
//!
//! ## Internationalisation (i18n)
//!
//! `PropertyInspector` is **N/A** for the crate i18n catalogue and is
//! intentionally absent from the nine localised widgets. Every
//! user-visible string it draws — category names, property keys, value
//! displays — is **host-supplied** through [`PropertyInspector::add`],
//! [`add_category`](PropertyInspector::add_category) and
//! [`PropertyNode`]. The widget owns **no chrome strings** of its own
//! (the only literals are the geometric collapse arrows `▸`/`▾` and the
//! type badges `bool`/`i32`/…, which are technical type identifiers and
//! stay untranslated by the same rule as `Hex`/`Dec`/`ASCII`). There is
//! therefore nothing to localise; a `locale` field would be dead state.

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
mod render;
pub mod value;

pub use config::InspectorConfig;
pub use value::PropertyValue;

/// Pack an `[f32; 4]` RGBA color into the draw-list `u32` (ABGR) format.
///
/// `pub(super)` so the sibling `render` module shares one definition.
#[inline]
pub(super) fn col32(c: [f32; 4]) -> u32 {
    crate::utils::color::col32(c)
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
    pub(super) depth: u32,
}

impl PropertyNode {
    #[must_use]
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

    #[must_use]
    pub fn with_readonly(mut self, ro: bool) -> Self {
        self.read_only = ro;
        self
    }

    #[must_use]
    pub fn with_changed(mut self, c: bool) -> Self {
        self.changed = c;
        self
    }

    #[must_use]
    pub fn with_child(mut self, child: PropertyNode) -> Self {
        self.children.push(child);
        self
    }

    /// `true` when this node has expandable child rows: either it
    /// carries explicit children, or its value is a container variant
    /// (`Object` / `Array`).
    #[must_use]
    pub fn has_children(&self) -> bool {
        !self.children.is_empty() || self.value.is_container()
    }

    /// `true` when the node's key or current value display contains
    /// `needle` (already lower-cased by the caller). An empty `needle`
    /// always matches.
    #[must_use]
    pub(super) fn matches_filter(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        self.key.to_lowercase().contains(needle)
            || self.value.display().to_lowercase().contains(needle)
    }
}

// ── Category ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct Category {
    pub(super) name: String,
    pub(super) collapsed: bool,
    pub(super) properties: Vec<PropertyNode>,
}

impl Category {
    fn empty_root() -> Self {
        Self {
            name: String::new(),
            collapsed: false,
            properties: Vec::new(),
        }
    }
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
    pub(super) id: String,
    pub(super) categories: Vec<Category>,
    /// Current active category for `add()` calls.
    pub(super) active_category: usize,
    /// Filter text.
    pub(super) filter: String,
    /// Configuration.
    pub config: InspectorConfig,
}

impl PropertyInspector {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            categories: vec![Category::empty_root()],
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
        self.add_node(PropertyNode::new(key, value))
    }

    /// Add a full property node to the current category.
    ///
    /// Robust against an out-of-range `active_category` (e.g. after a
    /// manual mutation): falls back to the last category so a stray
    /// index can never panic.
    pub fn add_node(&mut self, node: PropertyNode) -> &mut Self {
        let idx = self
            .active_category
            .min(self.categories.len().saturating_sub(1));
        if let Some(cat) = self.categories.get_mut(idx) {
            cat.properties.push(node);
        }
        self
    }

    /// Clear all categories and properties.
    pub fn clear(&mut self) {
        self.categories.clear();
        self.categories.push(Category::empty_root());
        self.active_category = 0;
    }

    /// Total number of properties across all categories (top-level only;
    /// nested children are not counted).
    #[must_use]
    pub fn property_count(&self) -> usize {
        self.categories.iter().map(|c| c.properties.len()).sum()
    }

    /// Number of categories, including the always-present unnamed root.
    #[must_use]
    pub fn category_count(&self) -> usize {
        self.categories.len()
    }

    /// Current filter text.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Set the filter text programmatically.
    pub fn set_filter(&mut self, filter: impl Into<String>) {
        self.filter = filter.into();
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
        assert_eq!(pi.category_count(), 3); // default + A + B
        assert_eq!(pi.property_count(), 2);
    }

    #[test]
    fn clear() {
        let mut pi = PropertyInspector::new("##test");
        pi.add("a", PropertyValue::Bool(true));
        pi.clear();
        assert_eq!(pi.property_count(), 0);
        assert_eq!(pi.category_count(), 1); // root survives
        assert_eq!(pi.active_category, 0);
    }

    #[test]
    fn add_node_survives_out_of_range_active_category() {
        // Corrupt the active index, then add — must not panic and must
        // land in the last category.
        let mut pi = PropertyInspector::new("##test");
        pi.add_category("A");
        pi.active_category = 999;
        pi.add("safe", PropertyValue::Bool(false));
        assert_eq!(pi.property_count(), 1);
        assert_eq!(pi.categories.last().unwrap().properties.len(), 1);
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
    fn has_children_detects_containers_and_explicit_kids() {
        // Container variant with no explicit kids still reports children.
        assert!(PropertyNode::new("o", PropertyValue::Object).has_children());
        assert!(PropertyNode::new("a", PropertyValue::Array(0)).has_children());
        // Scalar with no kids → no children.
        assert!(!PropertyNode::new("s", PropertyValue::I32(1)).has_children());
        // Scalar with an explicit child → children.
        let n = PropertyNode::new("s", PropertyValue::I32(1)).with_child(PropertyNode::default());
        assert!(n.has_children());
    }

    #[test]
    fn matches_filter_on_key_and_value() {
        let n = PropertyNode::new("Position", PropertyValue::String("origin".into()));
        assert!(n.matches_filter(""), "empty needle matches everything");
        assert!(n.matches_filter("posi"), "key substring matches");
        assert!(n.matches_filter("orig"), "value substring matches");
        assert!(!n.matches_filter("zzz"), "no match");
    }

    #[test]
    fn set_filter_round_trips() {
        let mut pi = PropertyInspector::new("##test");
        assert_eq!(pi.filter(), "");
        pi.set_filter("col");
        assert_eq!(pi.filter(), "col");
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
    fn config_round_trips_through_ron() {
        let cfg = InspectorConfig::default();
        let s = ron::ser::to_string(&cfg).expect("serialize");
        let back: InspectorConfig = ron::from_str(&s).expect("deserialize");
        assert_eq!(cfg.key_width_ratio, back.key_width_ratio);
        assert_eq!(cfg.row_height, back.row_height);
        assert_eq!(cfg.show_filter, back.show_filter);
        assert_eq!(cfg.color_changed, back.color_changed);
    }

    #[test]
    fn vec_display() {
        assert!(PropertyValue::Vec2([1.0, 2.0]).display().contains("1.00"));
        assert!(
            PropertyValue::Vec3([1.0, 2.0, 3.0])
                .display()
                .contains("3.00")
        );
    }
}
