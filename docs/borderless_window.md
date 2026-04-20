# borderless_window

Reusable borderless-window titlebar component for Rust + Dear ImGui on Windows.

## Overview

`borderless_window` provides a fully custom titlebar rendered via Dear ImGui draw lists. It replaces the OS window chrome with minimize / maximize / close buttons, drag-to-move, 8-direction edge resize detection, color themes, and optional extras — all without any OS titlebar artifacts.

## Features

- **5 built-in themes**: Dark, Light, Midnight, Solarized, Monokai (via the unified [`Theme`](theme.md) enum) + per-instance custom palette via `colors_override`
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
- **Overlay variant** — `render_titlebar_overlay` draws through the foreground draw list without a host ImGui window

## Quick Start

```rust
use dear_imgui_custom_mod::borderless_window::{
    BorderlessConfig, CloseMode, TitlebarState, WindowAction, render_titlebar,
};
use dear_imgui_custom_mod::theme::Theme;

// Config (create once)
let cfg = BorderlessConfig::new("My App")
    .with_theme(Theme::Dark)
    .with_close_mode(CloseMode::Confirm);

// State (persistent across frames)
let mut state = TitlebarState::new();

// Inside a full-screen zero-padding Dear ImGui window each frame:
// let res = render_titlebar(ui, &cfg, &mut state);
//
// if let Some(edge) = res.hover_edge {
//     window.set_cursor(cursor_for_edge(edge));
// }
//
// match res.action {
//     WindowAction::Close          => event_loop.exit(),
//     WindowAction::CloseRequested => show_confirm_dialog(&mut state),
//     WindowAction::Minimize       => window.set_minimized(true),
//     WindowAction::Maximize       => { window.set_maximized(!state.maximized); }
//     WindowAction::DragStart      => { window.drag_window().ok(); }
//     WindowAction::ResizeStart(e) => { window.drag_resize_window(to_winit(e)).ok(); }
//     WindowAction::Extra(id)      => { /* custom button */ }
//     WindowAction::IconClick      => { /* icon clicked */ }
//     WindowAction::None           => {}
// }
```

`TitlebarResult` is `#[must_use]` — dropping the return value means dropping user input (clicks, resize starts, close requests).

## Configuration

### `BorderlessConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `title` | `String` | `"Application"` | Window title text |
| `titlebar_height` | `f32` | `28.0` | Titlebar height in pixels |
| `resize_zone` | `f32` | `6.0` | Edge/corner resize detection zone width (px) |
| `separator_height` | `f32` | `1.0` | Height of the separator line below the titlebar (px) |
| `theme` | `Theme` | `Dark` | Color theme selector |
| `colors_override` | `Option<Box<TitlebarColors>>` | `None` | Per-instance custom palette (bypasses `theme`) |
| `title_align` | `TitleAlign` | `Left` | Title text alignment: `Left` or `Center` |
| `icon` | `Option<String>` | `None` | Unicode glyph shown before the title |
| `buttons` | `ButtonConfig` | default | Minimize/maximize/close + extra buttons |
| `double_click_maximize` | `bool` | `true` | Maximize window on titlebar double-click |
| `title_padding_left` | `f32` | `10.0` | Left padding before icon / title (px) |
| `close_mode` | `CloseMode` | `Immediate` | Close behavior: `Immediate` or `Confirm` |
| `separator_visible` | `bool` | `true` | Show the 1-px separator line below the titlebar |
| `show_drag_hint` | `bool` | `true` | Highlight the drag-move zone on hover |
| `focus_dim` | `bool` | `false` | Dim titlebar colors when the window loses OS focus |

### Builder Methods

```rust
use dear_imgui_custom_mod::borderless_window::{
    BorderlessConfig, ButtonConfig, CloseMode, ExtraButton, TitleAlign,
};
use dear_imgui_custom_mod::theme::Theme;

let cfg = BorderlessConfig::new("My App")
    .with_title("New Title")                      // change title
    .with_theme(Theme::Solarized)                 // color theme
    .with_titlebar_height(32.0)                   // px height
    .with_resize_zone(8.0)                        // edge resize zone width (px)
    .with_title_align(TitleAlign::Center)         // center title
    .with_icon("\u{2302}")                        // house icon before title
    .with_close_mode(CloseMode::Confirm)          // ask before closing
    .with_focus_dim()                             // dim when unfocused
    .without_focus_dim()                          // explicitly disable focus dim
    .without_drag_hint()                          // no hover hint
    .without_separator()                          // no bottom line
    .without_minimize()                           // hide minimize button
    .without_maximize()                           // hide maximize button
    .with_buttons(
        ButtonConfig::default()
            .add_extra(
                ExtraButton::new("theme", "\u{263D}", [0.8, 0.8, 0.5, 1.0])
                    .with_tooltip("Toggle theme"),
            ),
    );
```

All builder methods:

| Method | Description |
|--------|-------------|
| `with_title(t)` | Set window title text |
| `with_theme(t)` | Set color theme (clears `colors_override`) |
| `with_colors(c)` | Set per-instance `TitlebarColors` palette override |
| `with_titlebar_height(h)` | Titlebar height in pixels |
| `with_resize_zone(z)` | Edge/corner resize detection zone width (px) |
| `with_title_align(a)` | Title alignment (`Left` or `Center`) |
| `with_icon(s)` | Icon character/glyph before title |
| `with_buttons(b)` | Replace entire `ButtonConfig` |
| `with_close_mode(m)` | Close behavior (`Immediate` or `Confirm`) |
| `without_maximize()` | Hide the maximize/restore button |
| `without_minimize()` | Hide the minimize button |
| `without_separator()` | Hide the 1-px separator line |
| `without_drag_hint()` | Disable drag-zone hover highlight |
| `with_focus_dim()` | Enable titlebar dimming on focus loss |
| `without_focus_dim()` | Disable titlebar dimming on focus loss |

## Themes

Themes come from the unified [`Theme`](theme.md) enum. See `docs/theme.md`
for the full table of variants and methods.

```rust
use dear_imgui_custom_mod::borderless_window::BorderlessConfig;
use dear_imgui_custom_mod::theme::Theme;

let _cfg = BorderlessConfig::new("App").with_theme(Theme::Midnight);
```

| Variant | Description |
|---------|-------------|
| `Theme::Dark` | Deep navy, pastel accent buttons (default) |
| `Theme::Light` | Clean white / light-grey |
| `Theme::Midnight` | Near-black, Tokyo Night accent |
| `Theme::Solarized` | Solarized dark |
| `Theme::Monokai` | Monokai Pro |

### Per-instance custom palette

Use `.with_colors(TitlebarColors)` when you need a one-off palette that does
not fit any built-in `Theme`. The override takes priority over `theme`;
calling `.with_theme(...)` again resets it.

```rust
use dear_imgui_custom_mod::borderless_window::{BorderlessConfig, TitlebarColors};
use dear_imgui_custom_mod::theme::Theme;

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

let _cfg = BorderlessConfig::new("My App")
    .with_theme(Theme::Dark)    // baseline (ignored while override is set)
    .with_colors(colors);       // per-instance override
```

## Close Confirmation

```rust
use dear_imgui_custom_mod::borderless_window::{BorderlessConfig, CloseMode};

let _cfg = BorderlessConfig::new("App").with_close_mode(CloseMode::Confirm);

// In the render loop:
// match res.action {
//     WindowAction::CloseRequested => {
//         // Show your own dialog...
//         if user_clicked_ok {
//             state.confirm_close(); // triggers WindowAction::Close next frame
//         }
//     }
//     WindowAction::Close => event_loop.exit(),
//     _ => {}
// }
```

## State

`TitlebarState` tracks:

| Field | Type | Description |
|-------|------|-------------|
| `maximized` | `bool` | Whether the window is maximized |
| `focused` | `bool` | Whether the window has OS focus |

```rust
use dear_imgui_custom_mod::borderless_window::TitlebarState;

let mut state = TitlebarState::new();
state.set_focused(true);      // call from WindowEvent::Focused
state.set_maximized(true);    // update after OS maximize
state.confirm_close();        // trigger close after user confirmation
```

## API Reference

### `render_titlebar(ui, cfg, state) -> TitlebarResult`

Renders the titlebar at the current ImGui cursor position inside a host
window, advances the cursor past it, and returns:

- `result.action` — the `WindowAction` for this frame
- `result.hover_edge` — the `ResizeEdge` (or `None`) the cursor is over

### `render_titlebar_overlay(ui, cfg, state, origin, full_window_size) -> TitlebarResult`

Overlay variant: draws through `ui.get_foreground_draw_list()` at an explicit
position without needing a host ImGui window.

- `origin` — top-left of the titlebar strip in **screen** coordinates.
- `full_window_size` — outer OS window size in logical pixels. The 8-edge
  resize hit test covers this whole area (not just the titlebar strip), so
  resizing works on every edge of the window even though the titlebar itself
  is only the top row.

Use this variant when your application already has content windows in the
frame and you do not want a fullscreen host ImGui window sitting above them
and swallowing mouse clicks. If you use [`app_window::AppWindow`](app_window.md),
the regular `render_titlebar` is already wired up for you — reach for the
overlay form when you roll your own event loop / layout.

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

## Platform Utilities

### `platform::hwnd_of(window) -> Option<isize>`

Extracts the Win32 HWND from a winit `Window`. Returns `None` on non-Windows platforms.

### `platform::set_titlebar_dark_mode(hwnd, dark)`

Applies (or removes) the DWM immersive dark mode attribute. Call **before** `window.set_visible(true)` to avoid white flash on startup.

```rust,ignore
#[cfg(windows)]
if let Some(hwnd) = borderless_window::platform::hwnd_of(&window) {
    borderless_window::platform::set_titlebar_dark_mode(hwnd, true);
}
```
