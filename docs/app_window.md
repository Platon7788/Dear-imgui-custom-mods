# app_window

Zero-boilerplate borderless application window for Rust + Dear ImGui.

## Overview

`app_window` bundles wgpu, winit, Dear ImGui, and `borderless_window` into a single `AppWindow::run()` call. Your application only implements the `AppHandler` trait — all GPU init, event loop, DPI handling, and frame rendering is handled automatically.

## Features

- **Zero boilerplate** — one `run()` call replaces ~300 lines of setup code
- **Auto GPU backend selection** — tries DX12 → Vulkan → GL on Windows, falls back to software adapter
- **Auto surface format detection** — prefers sRGB, gracefully falls back
- **Auto HiDPI** — DPI scale clamped to `[1.0, 3.0]`, font scaled accordingly
- **FPS cap** — configurable via `fps_limit` (default 60), `0` = unlimited (Poll mode)
- **Window start position** — `CenterScreen`, `TopLeft`, or `Custom(x, y)`
- **Full theme system** — `AppState::set_theme()` updates both titlebar colors and full ImGui widget palette
- **Clean event routing** — `on_close_requested`, `on_extra_button`, `on_icon_click`, `on_theme_changed` callbacks

## Quick Start

```rust
use dear_imgui_custom_mod::app_window::{AppConfig, AppHandler, AppState, AppWindow};
use dear_imgui_rs::Ui;

struct MyApp;

impl AppHandler for MyApp {
    fn render(&mut self, ui: &Ui, _state: &mut AppState) {
        ui.window("Hello").build(|| {
            ui.text("Hello from AppWindow!");
        });
    }
}

fn main() {
    AppWindow::new(AppConfig::new("My App", 1024.0, 768.0))
        .run(MyApp)
        .expect("event loop error");
}
```

## Configuration

```rust
use dear_imgui_custom_mod::app_window::{AppConfig, StartPosition};
use dear_imgui_custom_mod::theme::Theme;

let _config = AppConfig::new("My App", 1100.0, 680.0)
    .with_min_size(640.0, 400.0)
    .with_fps_limit(60)                              // 0 = unlimited
    .with_font_size(15.0)                            // logical px
    .with_start_position(StartPosition::CenterScreen)
    .with_theme(Theme::Dark);
```

### `StartPosition`

| Variant | Behavior |
|---------|----------|
| `CenterScreen` | Centered on primary monitor (default) |
| `TopLeft` | Top-left corner of primary monitor |
| `Custom(x, y)` | Explicit physical-pixel coordinates |

## AppHandler Trait

All methods have default implementations — only override what you need:

```rust
impl AppHandler for MyApp {
    /// Called every frame inside the full-screen root window, below the titlebar.
    fn render(&mut self, ui: &Ui, state: &mut AppState) { }

    /// Called when close is requested (Close button or OS close).
    /// Default: calls `state.exit()` immediately.
    fn on_close_requested(&mut self, state: &mut AppState) {
        state.exit();
    }

    /// Called when a custom extra titlebar button is clicked.
    fn on_extra_button(&mut self, id: &'static str, state: &mut AppState) { }

    /// Called when the window icon is clicked.
    fn on_icon_click(&mut self, state: &mut AppState) { }

    /// Called after a theme change is applied (style already updated).
    fn on_theme_changed(&mut self, theme: &Theme, state: &mut AppState) { }
}
```

`Theme` is re-exported from `dear_imgui_custom_mod::app_window` for
convenience; it is the same type as [`dear_imgui_custom_mod::theme::Theme`](theme.md).

## AppState

`AppState` is passed to every callback and `render()`:

```rust
// Exit the application
state.exit();

// Toggle window maximize
state.toggle_maximized();
state.set_maximized(true);

// Request a theme change (applied at end of frame)
state.set_theme(Theme::Solarized);

// Access titlebar state (read-only in most cases)
let is_maximized = state.titlebar.maximized;
let is_focused   = state.titlebar.focused;
```

## Theme System

Calling `state.set_theme()` inside `render()` will, at the end of the current frame:
1. Update the `borderless_window` titlebar colors
2. Reapply the full Dear ImGui widget color palette (derived from the theme)
3. Call `on_theme_changed()` on the handler

```rust,ignore
fn render(&mut self, ui: &Ui, state: &mut AppState) {
    if ui.button("Switch to Solarized") {
        state.set_theme(Theme::Solarized);
    }
}
```

## Close Confirmation Dialog

Override `on_close_requested` to show your own dialog instead of exiting:

```rust
fn on_close_requested(&mut self, _state: &mut AppState) {
    self.show_confirm = true; // don't call state.exit() yet
}

fn render(&mut self, ui: &Ui, state: &mut AppState) {
    if self.show_confirm {
        // render centered dialog...
        if ui.button("Close") { state.exit(); }
        if ui.button("Cancel") { self.show_confirm = false; }
    }
}
```

## GPU Backend Selection (Windows)

On Windows, backends are tried in this order:
1. **DX12** — preferred for best performance
2. **Vulkan** — fallback
3. **GL** (ANGLE) — software fallback

A software adapter (`force_fallback_adapter = true`) is tried if all preferred adapters fail.

## Architecture

```
AppWindow::run(handler)
  └── winit EventLoop
        ├── resumed()         → init_wgpu() + init_imgui()
        ├── window_event()
        │     ├── Focused      → titlebar.set_focused()
        │     ├── Resized      → reconfigure surface
        │     ├── CloseReq.    → handler.on_close_requested()
        │     └── Redraw       → render_frame()
        │           ├── render_titlebar()   (borderless_window)
        │           ├── handler.render()
        │           ├── apply pending_theme
        │           └── dispatch OS actions
        └── about_to_wait()   → WaitUntil(fps_interval) or Poll
```

## API Reference

### `AppWindow`

| Method | Description |
|--------|-------------|
| `AppWindow::new(config)` | Create a window with the given configuration |
| `run<H: AppHandler>(handler)` | Start the event loop (blocks until window closes) |

### `AppConfig` builder

| Method | Default | Description |
|--------|---------|-------------|
| `new(title, w, h)` | — | Create config with title and size |
| `with_min_size(w, h)` | `640×400` | Minimum window size |
| `with_fps_limit(fps)` | `60` | Frame rate cap (`0` = unlimited) |
| `with_font_size(px)` | `15.0` | Base font size in logical pixels |
| `with_start_position(p)` | `CenterScreen` | Where to place the window on startup |
| `with_theme(theme)` | `Dark` | Initial color theme |
| `with_titlebar(cfg)` | default | Replace the entire titlebar config |
| `with_corner_radius(r: i32)` | `8` | Rounded-corner radius for Win10 fallback path (Win11 DWM ignores this) |
| `with_mdi_icons()` | `false` | Merge Material Design Icons font into atlas — required for MDI codepoints (U+F0000–U+F1FFF) in nav panel buttons etc. |
