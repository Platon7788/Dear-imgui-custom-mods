# confirm_dialog

Reusable modal confirmation dialog component for Rust + Dear ImGui.

## Overview

`confirm_dialog` provides a fully styled, theme-aware confirmation dialog rendered via Dear ImGui. It replaces ad-hoc inline dialog code with a single function call. Icons are drawn as draw-list primitives (no icon font needed).

## Features

- **6 built-in themes**: Dark, Light, Midnight, Nord, Solarized, Monokai + fully custom palette
- **4 icon types**: Warning (filled triangle), Error (circle + X), Info (circle + i), Question (circle + ?)
- **Fullscreen dim overlay** behind the dialog (toggleable)
- **Keyboard shortcuts**: Escape = cancel, Enter = confirm (toggleable)
- **Color-coded buttons**: green Cancel (safe), red Confirm (destructive)
- **Compact button layout**: centred, bottom-anchored, generous spacing
- **Builder-pattern configuration**: `DialogConfig::new(...).with_icon(...).with_theme(...)`
- **Font-independent**: all icons drawn as crisp draw-list primitives

## Quick Start

```rust
use dear_imgui_custom_mod::confirm_dialog::{
    DialogConfig, DialogIcon, DialogResult, DialogTheme, render_confirm_dialog,
};

let cfg = DialogConfig::new("Close Application", "Are you sure you want to close?")
    .with_icon(DialogIcon::Warning)
    .with_confirm_label("Close")
    .with_cancel_label("Cancel")
    .with_theme(DialogTheme::Dark);

let mut show = true;

// In render loop:
match render_confirm_dialog(ui, &cfg, &mut show) {
    DialogResult::Confirmed => { /* do the action */ }
    DialogResult::Cancelled => { /* user cancelled */ }
    DialogResult::Open      => { /* still showing */ }
}
```

## Configuration

```rust
let cfg = DialogConfig::new("Delete File", "This action cannot be undone.")
    .with_theme(DialogTheme::Nord)       // color theme
    .with_icon(DialogIcon::Error)        // icon type
    .with_confirm_label("Delete")        // red button text
    .with_cancel_label("Keep")           // green button text
    .with_confirm_style(ConfirmStyle::Destructive) // red confirm button
    .with_width(380.0)                   // dialog width
    .with_height(170.0)                  // dialog height
    .with_rounding(8.0)                  // border radius
    .without_dim()                       // no background overlay
    .without_keyboard();                 // no Esc/Enter shortcuts
```

## Themes

| Variant | Description |
|---------|-------------|
| `DialogTheme::Dark` | Deep navy, pastel accents (default) |
| `DialogTheme::Light` | Clean white / light-grey |
| `DialogTheme::Midnight` | Near-black, high-contrast |
| `DialogTheme::Nord` | Nordic #2E3440 palette |
| `DialogTheme::Solarized` | Solarized dark |
| `DialogTheme::Monokai` | Monokai Pro |
| `DialogTheme::Custom(colors)` | Fully custom `DialogColors` |

## Icons

| Variant | Visual | Description |
|---------|--------|-------------|
| `DialogIcon::Warning` | Filled triangle with "!" | Destructive / caution actions |
| `DialogIcon::Error` | Circle with X | Error state |
| `DialogIcon::Info` | Circle with "i" | Informational |
| `DialogIcon::Question` | Circle with "?" | User choice |
| `DialogIcon::None` | (no icon) | Text only |

## Button Styles

| Style | Cancel Button | Confirm Button |
|-------|--------------|----------------|
| `ConfirmStyle::Destructive` | Green (safe) | Red (danger) |
| `ConfirmStyle::Normal` | Green (safe) | Green (neutral) |

## API Reference

### `render_confirm_dialog(ui, cfg, open) -> DialogResult`

Renders the dialog if `*open` is `true`. Sets `*open = false` on confirm or cancel.

Returns:
- `DialogResult::Confirmed` — user clicked confirm or pressed Enter
- `DialogResult::Cancelled` — user clicked cancel or pressed Escape
- `DialogResult::Open` — dialog still visible, no action

### `DialogConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `title` | `String` | `"Confirm"` | Header text |
| `message` | `String` | `"Are you sure?"` | Body message |
| `confirm_label` | `String` | `"Confirm"` | Confirm button text |
| `cancel_label` | `String` | `"Cancel"` | Cancel button text |
| `icon` | `DialogIcon` | `Warning` | Icon type |
| `confirm_style` | `ConfirmStyle` | `Destructive` | Button color style |
| `theme` | `DialogTheme` | `Dark` | Color theme |
| `width` | `f32` | `340.0` | Dialog width (px) |
| `height` | `f32` | `160.0` | Dialog height (px) |
| `padding` | `f32` | `16.0` | Inner padding (px) |
| `button_height` | `f32` | `30.0` | Base button height (px) |
| `button_gap` | `f32` | `20.0` | Gap between buttons (px) |
| `dim_background` | `bool` | `true` | Draw overlay behind dialog |
| `keyboard_shortcuts` | `bool` | `true` | Esc/Enter handling |
| `rounding` | `f32` | `6.0` | Border radius (px) |

## Integration with app_window

```rust
use dear_imgui_custom_mod::app_window::{AppHandler, AppState, TitlebarTheme};
use dear_imgui_custom_mod::confirm_dialog::*;

struct MyApp { show_confirm: bool }

impl AppHandler for MyApp {
    fn render(&mut self, ui: &Ui, state: &mut AppState) {
        if self.show_confirm {
            let cfg = DialogConfig::new("Close", "Are you sure?")
                .with_icon(DialogIcon::Warning)
                .with_confirm_label("Close")
                .with_theme(DialogTheme::Dark);

            match render_confirm_dialog(ui, &cfg, &mut self.show_confirm) {
                DialogResult::Confirmed => state.exit(),
                _ => {}
            }
        }
    }

    fn on_close_requested(&mut self, _state: &mut AppState) {
        self.show_confirm = true;
    }
}
```
