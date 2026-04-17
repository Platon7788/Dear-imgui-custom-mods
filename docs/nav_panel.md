# nav_panel

Modern navigation panel (activity bar) for Rust + Dear ImGui.

## Overview

`nav_panel` provides a dockable navigation bar rendered via Dear ImGui draw lists. Supports left/right vertical icon strips (VS Code activity bar style) and top horizontal bars. Font-independent — icons are passed as text glyphs.

## Features

- **3 docking positions**: Left, Right, Top (Bottom reserved for StatusBar)
- **Left/Right**: vertical icon strip with active indicator bar
- **Top**: horizontal bar with `IconOnly`, `IconWithLabel`, or `LabelOnly` modes
- **Flyout submenu** on any button with icons, shortcuts, separators
- **Auto-hide** with slide animation + auto-show on edge hover
- **Toggle arrow** button (double chevron, direction-aware)
- **Active indicator bar** — vertical for sides, underline for top
- **Badge** (notification counter / dot) anchored to button corner
- **Button spacing** — configurable gap between buttons
- **Button separators** — optional thin lines between buttons
- **Per-button tooltip control** + global tooltip toggle
- **Custom icon colors** per button
- **5 built-in themes** via the unified [`Theme`](theme.md) enum + per-instance custom palette via `colors_override`
- **content_offset_y** for correct edge detection with borderless titlebar
- **Overlay variant** — `render_nav_panel_overlay` draws through the foreground draw list without a host ImGui window

## Quick Start

```rust
use dear_imgui_custom_mod::nav_panel::{
    DockPosition, NavButton, NavPanelConfig, NavPanelState, SubMenuItem,
};
use dear_imgui_custom_mod::theme::Theme;

let _cfg = NavPanelConfig::new(DockPosition::Left)
    .with_theme(Theme::Dark)
    .add_button(NavButton::action("home", "H", "Home")
        .with_color([0.3, 0.6, 1.0, 1.0]))
    .add_button(NavButton::action("search", "S", "Search")
        .with_color([0.4, 0.8, 0.3, 1.0]))
    .add_separator()
    .add_button(NavButton::submenu("settings", "*", "Settings")
        .add_item(SubMenuItem::new("theme", "Theme"))
        .add_item(SubMenuItem::separator())
        .add_item(SubMenuItem::new("about", "About")));

let mut state = NavPanelState::new();
state.set_active("home");

// In render loop:
// let result = render_nav_panel(ui, &cfg, &mut state);
// for event in &result.events {
//     match event {
//         NavEvent::ButtonClicked(id) => { /* navigate */ }
//         NavEvent::SubMenuClicked(btn_id, item_id) => { /* handle */ }
//         NavEvent::ToggleClicked(visible) => { /* panel toggled */ }
//     }
// }
// // Offset content by result.occupied_size
```

`NavPanelResult` is `#[must_use]` — its `events` vec is how button clicks
and submenu selections reach you.

## Configuration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `position` | `DockPosition` | `Left` | Left, Right, or Top |
| `theme` | `Theme` | `Dark` | Color theme selector |
| `colors_override` | `Option<Box<NavColors>>` | `None` | Per-instance palette override |
| `width` | `f32` | `28.0` | Panel width for Left/Right (min 16) |
| `height` | `f32` | `20.0` | Panel height for Top (min 16) |
| `button_size` | `f32` | `24.0` | Button cell size (min 14) |
| `button_spacing` | `f32` | `4.0` | Gap between buttons |
| `button_style` | `ButtonStyle` | `IconOnly` | For Top: IconOnly/IconWithLabel/LabelOnly |
| `indicator_thickness` | `f32` | `3.0` | Active indicator bar thickness |
| `button_rounding` | `f32` | `6.0` | Hover highlight rounding |
| `separator_padding` | `f32` | `4.0` | Padding around NavItem::Separator |
| `show_button_separators` | `bool` | `true` | Thin lines between every button |
| `show_toggle` | `bool` | `false` | Show toggle arrow button |
| `auto_hide` | `bool` | `false` | Hide when cursor leaves |
| `auto_show_on_hover` | `bool` | `true` | Show when cursor enters edge zone |
| `animate` | `bool` | `true` | Enable slide animation |
| `animation_speed` | `f32` | `6.0` | Animation speed (progress/sec) |
| `show_tooltips` | `bool` | `true` | Global tooltip toggle |
| `content_offset_y` | `f32` | `0.0` | Y offset for edge detection (titlebar) |

## Themes

Themes come from the unified [`Theme`](theme.md) enum. For a one-off custom
palette that does not fit any built-in theme, use `.with_colors(NavColors)`
— it takes priority over the `Theme` selector for that instance.

| Variant | Description |
|---------|-------------|
| `Theme::Dark` | Deep navy (default) |
| `Theme::Light` | Clean white / light-grey |
| `Theme::Midnight` | Near-black, high-contrast |
| `Theme::Solarized` | Solarized dark |
| `Theme::Monokai` | Monokai Pro |

## Events

| Event | Description |
|-------|-------------|
| `NavEvent::ButtonClicked(id)` | Action button clicked |
| `NavEvent::SubMenuClicked(btn_id, item_id)` | Submenu item clicked |
| `NavEvent::ToggleClicked(visible)` | Toggle arrow clicked |

## Rendering variants

### `render_nav_panel(ui, cfg, state) -> NavPanelResult`

Draws inside the current ImGui window using `ui.cursor_screen_pos()` +
`ui.content_region_avail()` for geometry. Use this when your layout is built
from regular ImGui windows and you want the panel to flow with them.

### `render_nav_panel_overlay(ui, cfg, state, origin, size) -> NavPanelResult`

Overlay variant: draws through `ui.get_foreground_draw_list()` at an
explicit screen-space position without requiring a host ImGui window.

- `origin` — top-left of the panel region in **screen** coordinates.
- `size` — `[width, height]` of the region reserved for the panel.

The submenu flyout still spawns its own ImGui window (it needs input focus),
but the panel surface itself draws on the foreground draw list so content
windows behind it remain clickable.

Use this variant when you already have regular content windows in the frame
and you do not want a fullscreen host ImGui window sitting above them and
swallowing mouse clicks — the typical case when you roll your own event loop
and layout rather than using [`app_window::AppWindow`](app_window.md).

## Integration with StatusBar

NavPanel supports Left, Right, Top. Bottom is reserved for StatusBar. Layout order:

```
+-------------------------------+
|        Titlebar (28px)        |
+---+---------------------------+
| N |                           |
| A |      Content area         |
| V |                           |
+---+---------------------------+
| StatusBar                     |
+-------------------------------+
```

Wrap NavPanel + content in a child_window sized to `avail_h - status_bar_height`, then render StatusBar after.

## NavButton API

```rust
use dear_imgui_custom_mod::nav_panel::{NavButton, SubMenuItem};

let _btn = NavButton::action("id", "icon", "tooltip");    // plain click
let _sub = NavButton::submenu("id", "icon", "tooltip")    // opens flyout
    .add_item(SubMenuItem::new("a", "Label"))
    .add_item(SubMenuItem::separator())
    .add_item(SubMenuItem::new("b", "Label").with_shortcut("Ctrl+B"))
    .with_color([1.0, 1.0, 1.0, 1.0])
    .with_badge("3")
    .without_tooltip();
```
