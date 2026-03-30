# Toolbar

Configurable horizontal toolbar for Dear ImGui with buttons, toggles, separators, dropdowns, and spacers.

## Overview

`Toolbar` renders a horizontal strip of interactive items. Supports buttons, toggles with on/off state, visual separators, flexible spacers, and cycling dropdowns — all with hover underlines, tooltips, and event reporting.

## Features

- **5 item types**: Button, Toggle, Separator, Spacer, Dropdown
- **Builder pattern** — fluent API for item construction
- **Flexible spacers** — push remaining items to the right
- **Hover underlines** — accent underline on hovered items
- **Tooltips** — hover tooltip on any interactive item
- **Disabled state** — per-item enable/disable
- **Icon support** — Unicode icon prefix on any item
- **Toggle state** — visual background for toggled-on items
- **Dropdown cycling** — click cycles through options
- **Mutable item access** — update toggle states externally

## Quick Start

```rust
use dear_imgui_custom_mod::toolbar::{Toolbar, ToolbarItem};

let mut toolbar = Toolbar::new("##toolbar");
toolbar.add(ToolbarItem::button("New", "Create new file"));
toolbar.add(ToolbarItem::button("Open", "Open file"));
toolbar.add(ToolbarItem::separator());
toolbar.add(ToolbarItem::toggle("Bold", false, "Toggle bold"));
toolbar.add(ToolbarItem::toggle("Italic", false, "Toggle italic"));
toolbar.add(ToolbarItem::spacer());
toolbar.add(ToolbarItem::dropdown(
    "Mode",
    vec!["Debug".into(), "Release".into()],
    0,
    "Build mode",
));
toolbar.add(ToolbarItem::button("Settings", "Open settings")
    .with_icon("\u{F0493}"));

// In render loop:
let events = toolbar.render(ui);
for event in events {
    match event {
        ToolbarEvent::ButtonClicked { index, label } => { /* handle click */ }
        ToolbarEvent::Toggled { index, label, on } => { /* handle toggle */ }
        ToolbarEvent::DropdownChanged { index, label, selected } => { /* handle dropdown */ }
    }
}
```

## Public API

### Construction

| Method | Description |
|--------|-------------|
| `new(id)` | Create a new toolbar |

### Items

| Method | Description |
|--------|-------------|
| `add(item)` | Add an item to the toolbar |
| `items() -> &[ToolbarItem]` | Read-only access to items |
| `items_mut() -> &mut Vec<ToolbarItem>` | Mutable access (e.g. update toggle states) |
| `clear()` | Remove all items |

### Rendering

| Method | Description |
|--------|-------------|
| `render(ui) -> Vec<ToolbarEvent>` | Render the toolbar. Returns events |

## ToolbarItem

### Constructors

| Method | Description |
|--------|-------------|
| `ToolbarItem::button(label, tooltip)` | Clickable button |
| `ToolbarItem::toggle(label, on, tooltip)` | Toggle button with on/off state |
| `ToolbarItem::separator()` | Visual separator line |
| `ToolbarItem::spacer()` | Flexible spacer (pushes items right) |
| `ToolbarItem::dropdown(label, options, selected, tooltip)` | Cycling dropdown |

### Builders

| Method | Description |
|--------|-------------|
| `.with_enabled(bool)` | Set enabled/disabled state |
| `.with_icon(text)` | Set Unicode icon prefix |

### Fields

```rust
pub struct ToolbarItem {
    pub label: String,
    pub icon: String,
    pub kind: ToolbarItemKind,
    pub tooltip: String,
    pub enabled: bool,
}
```

## ToolbarItemKind

| Variant | Description |
|---------|-------------|
| `Button` | Clickable button |
| `Toggle { on: bool }` | Toggle with on/off state |
| `Separator` | Visual separator line |
| `Spacer` | Flexible spacer |
| `Dropdown { options: Vec<String>, selected: usize }` | Cycling dropdown |

## Events

| Event | Fields | Description |
|-------|--------|-------------|
| `ButtonClicked` | `index`, `label` | Button was clicked |
| `Toggled` | `index`, `label`, `on` | Toggle state changed |
| `DropdownChanged` | `index`, `label`, `selected` | Dropdown selection changed |

## Configuration

```rust
let cfg = &mut toolbar.config;

// Layout
cfg.height = 30.0;                // toolbar height
cfg.item_spacing = 2.0;           // horizontal gap between items
cfg.button_padding = 6.0;         // padding inside each button
cfg.button_rounding = 3.0;        // button corner rounding
cfg.separator_width = 1.0;        // separator line width
cfg.separator_margin = 4.0;       // margin on each side of separator
cfg.hover_underline_thickness = 2.0;
```

### Colors

| Field | Description |
|-------|-------------|
| `color_bg` | Toolbar background |
| `color_text` | Button text/icon color |
| `color_disabled` | Disabled item color |
| `color_hover` | Hovered button background |
| `color_active` | Pressed button background |
| `color_toggled` | Toggled-on button background |
| `color_separator` | Separator line color |
| `color_border` | Bottom border line |
| `color_hover_underline` | Hover underline accent color |

## Architecture

```
toolbar/
  mod.rs      Toolbar struct, ToolbarItem, ToolbarEvent, rendering
  config.rs   ToolbarConfig with colors and layout
```
