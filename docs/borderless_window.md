# borderless_window

Reusable borderless-window titlebar component for Rust + Dear ImGui on Windows.

## Overview

`borderless_window` provides a fully custom titlebar rendered via Dear ImGui draw lists. It replaces the OS window chrome with minimize / maximize / close buttons, drag-to-move, 8-direction edge resize detection, color themes, and optional extras — all without any OS titlebar artifacts.

## Features

- **6 built-in themes**: Dark, Light, Midnight, Nord, Solarized, Monokai + fully custom palette
- **Minimize / Maximize / Close** — crisp icon-only buttons drawn as draw-list primitives
- **8-direction edge resize detection** — returns `ResizeEdge` every frame for cursor updates
- **Focused / unfocused dimming** — optional dimmed colors when window loses OS focus
- **Close-confirmation mode** — `CloseMode::Confirm` delays close until your dialog calls `confirm_close()`
- **Custom extra buttons** — add arbitrary icon buttons left of the standard buttons
- **Icon before title** — Unicode glyph or short label prefix
- **Title alignment** — left or centered
- **Drag-zone hover hint** — subtle background tint on drag area
- **Separator toggle** — show or hide the 1-px line below the titlebar
- **Icon click** — click on the window icon triggers `WindowAction::IconClick`

## Quick Start

```rust
use dear_imgui_custom_mod::borderless_window::{
    BorderlessConfig, TitlebarState, WindowAction, render_titlebar,
};

// Config (create once)
let cfg = BorderlessConfig::new("My App")
    .with_theme(TitlebarTheme::Dark)
    .with_close_mode(CloseMode::Confirm);

// State (persistent across frames)
let mut state = TitlebarState::new();

// Inside a full-screen zero-padding Dear ImGui window each frame:
let res = render_titlebar(ui, &cfg, &mut state);

// Cursor update (every frame, no click needed)
if let Some(edge) = res.hover_edge {
    window.set_cursor(cursor_for_edge(edge));
}

// Handle actions
match res.action {
    WindowAction::Close          => event_loop.exit(),
    WindowAction::CloseRequested => show_confirm_dialog(&mut state),
    WindowAction::Minimize       => window.set_minimized(true),
    WindowAction::Maximize       => { window.set_maximized(!state.maximized); }
    WindowAction::DragStart      => { window.drag_window().ok(); }
    WindowAction::ResizeStart(e) => { window.drag_resize_window(to_winit(e)).ok(); }
    WindowAction::Extra(id)      => { /* custom button */ }
    WindowAction::IconClick      => { /* icon clicked */ }
    WindowAction::None           => {}
}
```

## Configuration

```rust
let cfg = BorderlessConfig::new("My App")
    .with_theme(TitlebarTheme::Nord)          // color theme
    .with_titlebar_height(32.0)               // px height
    .with_title_align(TitleAlign::Center)     // center title
    .with_icon("\u{2302}")                    // house icon before title
    .with_close_mode(CloseMode::Confirm)      // ask before closing
    .with_focus_dim()                         // dim when unfocused
    .without_drag_hint()                      // no hover hint
    .without_separator()                      // no bottom line
    .with_buttons(
        ButtonConfig::default()
            .add_extra(
                ExtraButton::new("theme", "\u{263D}", [0.8, 0.8, 0.5, 1.0])
                    .with_tooltip("Toggle theme"),
            ),
    );
```

## Themes

| Variant | Description |
|---------|-------------|
| `TitlebarTheme::Dark` | Deep navy, pastel accent buttons (default) |
| `TitlebarTheme::Light` | Clean white / light-grey |
| `TitlebarTheme::Midnight` | Near-black, VS Code dark+ inspired |
| `TitlebarTheme::Nord` | Nordic `#2E3440` palette |
| `TitlebarTheme::Solarized` | Solarized dark |
| `TitlebarTheme::Monokai` | Monokai Pro |
| `TitlebarTheme::Custom(colors)` | Fully custom `TitlebarColors` |

### Custom theme

```rust
use dear_imgui_custom_mod::borderless_window::{TitlebarTheme, TitlebarColors};

let colors = TitlebarColors {
    bg:                 [0.10, 0.10, 0.14, 1.0],
    title:              [0.90, 0.90, 0.90, 1.0],
    separator:          [0.20, 0.20, 0.26, 1.0],
    btn_minimize:       [1.00, 0.75, 0.00, 1.0],
    btn_maximize:       [0.30, 0.75, 1.00, 1.0],
    btn_close:          [1.00, 0.33, 0.33, 1.0],
    btn_hover_bg:       [0.22, 0.22, 0.32, 0.85],
    btn_close_hover_bg: [0.50, 0.10, 0.10, 0.90],
    icon:               [0.90, 0.90, 0.90, 1.0],
    bg_erase:           [0.10, 0.10, 0.14, 1.0],
    drag_hint:          [0.18, 0.18, 0.26, 0.30],
    bg_inactive:        [0.08, 0.08, 0.10, 1.0],
    title_inactive:     [0.45, 0.45, 0.50, 1.0],
};

let cfg = BorderlessConfig::new("My App")
    .with_theme(TitlebarTheme::Custom(colors));
```

## Close Confirmation

```rust
// Config: use Confirm mode
let cfg = BorderlessConfig::new("App").with_close_mode(CloseMode::Confirm);

// In render loop:
match res.action {
    WindowAction::CloseRequested => {
        // Show your own dialog...
        if user_clicked_ok {
            state.confirm_close(); // triggers WindowAction::Close next frame
        }
    }
    WindowAction::Close => event_loop.exit(),
    _ => {}
}
```

## State

`TitlebarState` tracks:

| Field | Type | Description |
|-------|------|-------------|
| `maximized` | `bool` | Whether the window is maximized |
| `focused` | `bool` | Whether the window has OS focus |

```rust
state.set_focused(focused);      // call from WindowEvent::Focused
state.set_maximized(true);       // update after OS maximize
state.confirm_close();           // trigger close after user confirmation
```

## API Reference

### `render_titlebar(ui, cfg, state) -> TitlebarResult`

Renders the titlebar row and returns:
- `result.action` — the `WindowAction` for this frame
- `result.hover_edge` — the `ResizeEdge` (or `None`) the cursor is over

### `WindowAction`

| Variant | When |
|---------|------|
| `None` | No actionable event |
| `Minimize` | Minimize button pressed |
| `Maximize` | Maximize / restore toggled |
| `Close` | Close confirmed |
| `CloseRequested` | Close button pressed (`CloseMode::Confirm` only) |
| `DragStart` | Mouse pressed on drag area |
| `ResizeStart(edge)` | Mouse pressed on resize edge |
| `Extra(id)` | Custom extra button clicked |
| `IconClick` | Window icon clicked |
