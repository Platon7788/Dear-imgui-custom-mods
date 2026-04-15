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
- **6 built-in themes** + fully custom palette
- **content_offset_y** for correct edge detection with borderless titlebar

## Quick Start

```rust
use dear_imgui_custom_mod::nav_panel::*;

let cfg = NavPanelConfig::new(DockPosition::Left)
    .with_theme(NavTheme::Dark)
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
let result = render_nav_panel(ui, &cfg, &mut state);
for event in &result.events {
    match event {
        NavEvent::ButtonClicked(id) => { /* navigate */ }
        NavEvent::SubMenuClicked(btn_id, item_id) => { /* handle */ }
        NavEvent::ToggleClicked(visible) => { /* panel toggled */ }
    }
}
// Offset content by result.occupied_size
```

## Configuration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `position` | `DockPosition` | `Left` | Left, Right, or Top |
| `theme` | `NavTheme` | `Dark` | Color theme (6 presets + Custom) |
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

| Variant | Description |
|---------|-------------|
| `NavTheme::Dark` | Deep navy (default) |
| `NavTheme::Light` | Clean white / light-grey |
| `NavTheme::Midnight` | Near-black, high-contrast |
| `NavTheme::Nord` | Nordic #2E3440 palette |
| `NavTheme::Solarized` | Solarized dark |
| `NavTheme::Monokai` | Monokai Pro |
| `NavTheme::Custom(Box<NavColors>)` | Fully custom |

## Events

| Event | Description |
|-------|-------------|
| `NavEvent::ButtonClicked(id)` | Action button clicked |
| `NavEvent::SubMenuClicked(btn_id, item_id)` | Submenu item clicked |
| `NavEvent::ToggleClicked(visible)` | Toggle arrow clicked |

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
NavButton::action("id", "icon", "tooltip")    // plain click
NavButton::submenu("id", "icon", "tooltip")   // opens flyout
    .add_item(SubMenuItem::new("a", "Label"))
    .add_item(SubMenuItem::separator())
    .add_item(SubMenuItem::new("b", "Label").with_shortcut("Ctrl+B"))
    .with_color([r, g, b, a])
    .with_badge("3")
    .without_tooltip()
```
