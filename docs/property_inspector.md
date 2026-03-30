# PropertyInspector

Hierarchical property editor for Dear ImGui — two-column tree-table for editing typed key-value pairs with 15+ value types.

## Overview

`PropertyInspector` displays a list of categorized properties in a key-value layout. Supports nested objects/arrays with expand/collapse, search filtering, read-only fields, diff highlighting, color swatches, and type badges.

## Features

- **15+ value types**: Bool, I32, I64, F32, F64, String, Color3, Color4, Vec2, Vec3, Vec4, Enum, Flags, Object, Array
- **Categories** — collapsible section headers to group related properties
- **Nested properties** — Object/Array nodes with recursive expand/collapse
- **Search filter** — filter properties by key or value text
- **Diff highlighting** — mark recently changed properties with accent color
- **Read-only support** — dimmed display for non-editable properties
- **Color swatches** — inline color preview for Color3/Color4 values
- **Type badges** — right-aligned type name for each property
- **Alternating row colors** — striped rows for readability
- **Hover highlighting** — subtle highlight on mouse-over
- **Builder pattern** — fluent API for constructing property nodes

## Quick Start

```rust
use dear_imgui_custom_mod::property_inspector::{
    PropertyInspector, PropertyNode, PropertyValue,
};

let mut inspector = PropertyInspector::new("##props");
inspector.add_category("Transform");
inspector.add("position", PropertyValue::Vec3([0.0, 1.0, 2.0]));
inspector.add("rotation", PropertyValue::F32(45.0));
inspector.add("scale", PropertyValue::Vec3([1.0, 1.0, 1.0]));

inspector.add_category("Material");
inspector.add("color", PropertyValue::Color4([0.8, 0.2, 0.2, 1.0]));
inspector.add("metallic", PropertyValue::F32(0.5));

// In render loop:
let events = inspector.render(ui);
for event in events {
    println!("Changed: {} = {}", event.key, event.new_value);
}
```

### Nested Properties

```rust
let node = PropertyNode::new("transform", PropertyValue::Object)
    .with_child(PropertyNode::new("x", PropertyValue::F32(0.0)))
    .with_child(PropertyNode::new("y", PropertyValue::F32(1.0)))
    .with_child(PropertyNode::new("z", PropertyValue::F32(2.0)));
inspector.add_node(node);
```

### Read-Only with Diff Highlight

```rust
let node = PropertyNode::new("id", PropertyValue::I64(42))
    .with_readonly(true)
    .with_changed(true);
inspector.add_node(node);
```

## Public API

### Construction & Data

| Method | Description |
|--------|-------------|
| `new(id)` | Create a new property inspector |
| `add_category(name)` | Add a category header; subsequent `add()` calls go into this category |
| `add(key, value)` | Add a property to the current category |
| `add_node(node)` | Add a full `PropertyNode` with all options |
| `clear()` | Remove all categories and properties |
| `property_count() -> usize` | Total number of properties across all categories |

### Rendering

| Method | Description |
|--------|-------------|
| `render(ui) -> Vec<PropertyChangedEvent>` | Render the inspector. Returns change events |

## PropertyNode

```rust
pub struct PropertyNode {
    pub key: String,                  // property label
    pub value: PropertyValue,         // typed value
    pub read_only: bool,              // non-editable (dimmed)
    pub changed: bool,                // diff highlight
    pub children: Vec<PropertyNode>,  // nested properties
    pub expanded: bool,               // expand/collapse state
}
```

Builder methods:

| Method | Description |
|--------|-------------|
| `new(key, value)` | Create a property node |
| `.with_readonly(bool)` | Set read-only flag |
| `.with_changed(bool)` | Set changed flag (diff highlight) |
| `.with_child(node)` | Add a child property |

## PropertyValue

| Variant | Display | Type Badge |
|---------|---------|-----------|
| `Bool(bool)` | `true` / `false` | `bool` |
| `I32(i32)` | `-42` | `i32` |
| `I64(i64)` | `123456` | `i64` |
| `F32(f32)` | `3.142` | `f32` |
| `F64(f64)` | `3.141593` | `f64` |
| `String(String)` | raw text | `string` |
| `Color3([f32; 3])` | `[0.80, 0.20, 0.20]` + swatch | `color3` |
| `Color4([f32; 4])` | `[0.80, 0.20, 0.20, 1.00]` + swatch | `color4` |
| `Vec2([f32; 2])` | `[1.00, 2.00]` | `vec2` |
| `Vec3([f32; 3])` | `[1.00, 2.00, 3.00]` | `vec3` |
| `Vec4([f32; 4])` | `[1.00, 2.00, 3.00, 4.00]` | `vec4` |
| `Enum(index, options)` | selected option text | `enum` |
| `Flags(u64, names)` | `0xFF` | `flags` |
| `Object` | `{...}` | `object` |
| `Array(count)` | `[3 items]` | `array` |

## Events

```rust
pub struct PropertyChangedEvent {
    pub key: String,       // key path (e.g. "Transform.position")
    pub new_value: String, // new value display string
}
```

## Configuration

```rust
let cfg = &mut inspector.config;

// Layout
cfg.key_width_ratio = 0.40;   // key column width (0.0..1.0)
cfg.row_height = 22.0;        // row height in pixels
cfg.indent = 16.0;            // indent per nesting level

// Features
cfg.show_filter = true;       // search/filter bar at top
cfg.show_categories = true;   // category headers
cfg.highlight_changes = false; // highlight changed values
```

### Colors

| Field | Description |
|-------|-------------|
| `color_bg` | Background |
| `color_bg_alt` | Alternate row background |
| `color_key` | Key/label text |
| `color_value` | Value text |
| `color_readonly` | Read-only value (dimmed) |
| `color_category_bg` | Category header background |
| `color_category_text` | Category header text |
| `color_changed` | Changed value highlight |
| `color_separator` | Key-value separator line |

## Architecture

```
property_inspector/
  mod.rs      PropertyInspector struct, rendering, category management
  config.rs   InspectorConfig with colors and layout
  value.rs    PropertyValue enum (15+ variants) with display and type_name
```
