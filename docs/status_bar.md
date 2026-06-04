# StatusBar

Composable bottom status bar for Dear ImGui with left/center/right sections, status indicators, clickable items, and progress bars.

## Overview

`StatusBar` renders a horizontal bar at the bottom of a window with three alignment sections. Items can display plain text, colored indicator dots, clickable labels, icons, and progress bars.

## Features

- **3-section layout**: left-aligned, center-aligned, right-aligned items
- **Status indicators**: colored dots (Success/Warning/Error/Info) before text
- **Clickable items** — minimalist text-buttons that emit events on click
- **Static visuals** — the bar never paints a built-in hover/active fill; it
  stays fully flat. Clicks still fire and tooltips still show on hover, so the
  host can layer its own item highlighting if desired.
- **Progress bars** — inline 60px progress bar with label
- **Icon prefix** — Unicode icon text before label
- **Tooltips** — hover tooltip on any item
- **Color override** — per-item text color
- **Separator lines** between items (configurable)
- **Position-based events** — clicks are addressed by `(section, index)`

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
    println!("Clicked: {} ({:?} #{})", event.label, event.section, event.index);
}
```

### Clickable Items (minimalist text-buttons)

```rust
bar.left(StatusItem::clickable("Errors: 3")
    .with_color([0.9, 0.3, 0.3, 1.0])
    .with_tooltip("Click to open error panel"));

// Optional: icon + label looks like a tiny toolbar button
bar.right(StatusItem::clickable("Build")
    .with_icon("\u{25B6}")     // ▶
    .with_tooltip("Ctrl+B"));

// In the render loop — click events are emitted regardless of `highlight_hover`:
for ev in bar.render(ui) {
    match ev.label.as_str() {
        "Build" => { /* trigger action */ }
        _ => {}
    }
}
```

Clickable items are intentionally frameless — the bar is a miniature UI, so
buttons stay as plain text. The bar paints no built-in hover/active fill; if
you want mouse-over feedback, wrap the item yourself in the host layer.

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
| `render_overlay(ui, origin, size) -> Vec<StatusBarEvent>` | Overlay variant — draws via `ui.get_background_draw_list()` at an explicit screen position, no host window required. Sits **below** every ImGui popup. |
| `render_overlay_foreground(ui, origin, size) -> Vec<StatusBarEvent>` | Same as `render_overlay` but draws via `ui.get_foreground_draw_list()` — sits **above** every popup. Use only for HUD/kiosk bars that must always be readable. |

### Overlay variants

`render_overlay(ui, origin, size)` draws through the **background** draw list
at an explicit screen-space position, so it does not need a host ImGui window
to live inside.

- `origin` — top-left of the bar in **screen** coordinates.
- `size` — `[width, height]` in logical pixels. `size[1]` overrides
  `config.height` for this call.

Hover detection is position-only (no `is_window_hovered` check), so
clickable items stay responsive even when no ImGui window covers the bar
region.

**Z-order:** `render_overlay` paints into the *background* draw list, which
sits above the page surface but **below** every ImGui popup (tooltips,
context menus, modals) — so a tooltip raised by another widget can never be
clipped by the bar. When the host hosts a full-window root behind every
frame (e.g. [`chrome::Chrome`](chrome.md)), that root's `WindowBg` fill would
clobber the background bar; in that case use `render_overlay_foreground`,
which paints into the *foreground* draw list above all windows. Its per-item
tooltips are painted manually into the same foreground draw list, anchored
above the cursor, so the bar strip can't slice them.

Use an overlay variant when your application already has content windows on
screen and you do not want a fullscreen host ImGui layer sitting above them
and swallowing mouse clicks. For the in-window case (render flows with
regular ImGui layout) stick with `render`.

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
pub enum StatusSection { Left, Center, Right }

pub struct StatusBarEvent {
    pub label: String,        // clicked item's label
    pub section: StatusSection, // which section the item lives in
    pub index: usize,         // 0-based position within its section
}
```

Only emitted for items created with `StatusItem::clickable()`. Identify the
clicked item by `(section, index)` — no global ID counter (the old `item_id:
u32` was removed in 0.9 in favour of position-based addressing).

## Configuration

Layout defaults live in `config.ron` (the DDD schema/values split:
`config.rs` declares the struct, `config.ron` holds the values; `Default`
loads the ron via `ron::from_str(include_str!(..))`). Override at runtime:

```rust
let cfg = &mut bar.config;

cfg.height = 22.0;                  // bar height in pixels
cfg.item_padding = 8.0;             // horizontal padding between items
cfg.separator_width = 1.0;          // separator line width
cfg.show_separators = true;         // show separator lines between items
cfg.show_top_border = true;         // 1px line along the bar's top edge
cfg.top_border_offset_left = 0.0;   // skip a left-docked nav panel's slice
cfg.top_border_offset_right = 0.0;  // skip a right-docked nav panel's slice
cfg.progress_width = 60.0;          // inline progress-bar width (px)
cfg.progress_height = 8.0;          // inline progress-bar height (px)
```

The bar paints no built-in hover/active fill, so there is no hover-toggle
config — the bar is always visually static.

### Colors

The colour palette lives in [`theme::StatusBarColors`](theme.md) (a separate
type so custom themes can build one without depending on this widget). It is
`#[serde(skip)]` in `StatusBarConfig` — re-derived from
`StatusBarColors::default()` (NxT-dark) on load, or supplied via
`Theme::statusbar_colors()`.

| Field | Description |
|-------|-------------|
| `bg` | Bar background |
| `text` | Default text color |
| `text_dim` | Dimmed/secondary text (progress labels) |
| `separator` | Separator line + top-border color |
| `hover` | Hovered item background (theme-level; bar itself draws no hover) |
| `active` | Pressed item background (theme-level) |
| `success` | Success indicator dot (green) |
| `warning` | Warning indicator dot (yellow) |
| `error` | Error indicator dot (red) |
| `info` | Info indicator dot (blue) + progress-bar fill |

## Architecture

```
status_bar/
  mod.rs       StatusBar struct + constructors, StatusItem, Indicator,
               StatusSection/StatusBarEvent, tests
  render.rs    render / render_overlay / render_overlay_foreground +
               the shared layout body and per-item paint/measure
  tooltip.rs   foreground-draw-list tooltip painter (overlay-foreground path)
  config.rs    StatusBarConfig schema + Alignment
  config.ron   StatusBarConfig default values
```
