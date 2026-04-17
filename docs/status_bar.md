# StatusBar

Composable bottom status bar for Dear ImGui with left/center/right sections, status indicators, clickable items, and progress bars.

## Overview

`StatusBar` renders a horizontal bar at the bottom of a window with three alignment sections. Items can display plain text, colored indicator dots, clickable labels, icons, and progress bars.

## Features

- **3-section layout**: left-aligned, center-aligned, right-aligned items
- **Status indicators**: colored dots (Success/Warning/Error/Info) before text
- **Clickable items** — emits events on click with hover/active highlighting
- **Progress bars** — inline 60px progress bar with label
- **Icon prefix** — Unicode icon text before label
- **Tooltips** — hover tooltip on any item
- **Color override** — per-item text color
- **Separator lines** between items (configurable)
- **Unique item IDs** — auto-generated for event tracking

## Quick Start

```rust
use dear_imgui_custom_mod::status_bar::{StatusBar, StatusItem, Indicator};

let mut bar = StatusBar::new("##status");
bar.left(StatusItem::indicator("Connected", Indicator::Success));
bar.left(StatusItem::text("Ln 42, Col 15"));
bar.center(StatusItem::text("main.rs"));
bar.right(StatusItem::text("UTF-8"));
bar.right(StatusItem::text("Rust"));

// In render loop:
let events = bar.render(ui);
for event in events {
    println!("Clicked: {} (id: {})", event.label, event.item_id);
}
```

### Clickable Items

```rust
bar.left(StatusItem::clickable("Errors: 3")
    .with_color([0.9, 0.3, 0.3, 1.0])
    .with_tooltip("Click to open error panel"));
```

### Progress Bar

```rust
bar.right(StatusItem::progress("Indexing", 0.65)
    .with_tooltip("65% complete"));
```

### Icon Prefix

```rust
bar.left(StatusItem::text("main")
    .with_icon("\u{F0214}"));  // file icon
```

## Public API

### Construction

| Method | Description |
|--------|-------------|
| `new(id)` | Create a new status bar |

### Adding Items

| Method | Description |
|--------|-------------|
| `left(item)` | Add item to the left section |
| `center(item)` | Add item to the center section |
| `right(item)` | Add item to the right section |
| `clear()` | Remove all items from all sections |

### Rendering

| Method | Description |
|--------|-------------|
| `render(ui) -> Vec<StatusBarEvent>` | Render inside the current ImGui window using the cursor + `content_region_avail()` |
| `render_overlay(ui, origin, size) -> Vec<StatusBarEvent>` | Overlay variant — draws via `ui.get_foreground_draw_list()` at an explicit screen position, no host window required |

### Overlay variant

`render_overlay(ui, origin, size)` draws through the foreground draw list at
an explicit screen-space position, so it does not need a host ImGui window
to live inside.

- `origin` — top-left of the bar in **screen** coordinates.
- `size` — `[width, height]` in logical pixels. `size[1]` overrides
  `config.height` for this call.

Hover detection is position-only (no `is_window_hovered` check), so
clickable items stay responsive even when no ImGui window covers the bar
region.

Use this variant when your application already has content windows on
screen and you do not want a fullscreen host ImGui layer sitting above them
and swallowing mouse clicks — the typical pattern when you compose your own
event loop / layout rather than using
[`app_window::AppWindow`](app_window.md). For the in-window case (render
flows with regular ImGui layout) stick with `render`.

## StatusItem

### Constructors

| Method | Description |
|--------|-------------|
| `StatusItem::text(label)` | Plain text item |
| `StatusItem::indicator(label, ind)` | Text with colored status dot |
| `StatusItem::clickable(label)` | Clickable text (emits events) |
| `StatusItem::progress(label, value)` | Progress bar (0.0..=1.0, clamped) |

### Builders

| Method | Description |
|--------|-------------|
| `.with_tooltip(text)` | Set hover tooltip |
| `.with_color([r,g,b,a])` | Override text color |
| `.with_icon(text)` | Unicode icon prefix |

## Indicator

| Variant | Color |
|---------|-------|
| `None` | No dot |
| `Success` | Green |
| `Warning` | Yellow |
| `Error` | Red |
| `Info` | Blue |

## Events

```rust
pub struct StatusBarEvent {
    pub label: String,  // clicked item's label
    pub item_id: u32,   // unique item ID
}
```

Only emitted for items created with `StatusItem::clickable()`.

## Configuration

```rust
let cfg = &mut bar.config;

cfg.height = 22.0;           // bar height in pixels
cfg.item_padding = 8.0;      // horizontal padding between items
cfg.separator_width = 1.0;   // separator line width
cfg.show_separators = true;  // show separator lines between items
```

### Colors

| Field | Description |
|-------|-------------|
| `color_bg` | Bar background |
| `color_text` | Default text color |
| `color_text_dim` | Dimmed/secondary text (progress labels) |
| `color_separator` | Separator line color |
| `color_hover` | Hovered clickable item background |
| `color_active` | Pressed clickable item background |
| `color_success` | Success indicator dot (green) |
| `color_warning` | Warning indicator dot (yellow) |
| `color_error` | Error indicator dot (red) |
| `color_info` | Info indicator dot (blue) |

## Architecture

```
status_bar/
  mod.rs      StatusBar struct, StatusItem, Indicator, rendering
  config.rs   StatusBarConfig, Alignment
```
