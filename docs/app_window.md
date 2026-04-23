# app_window

Zero-boilerplate borderless application window for Rust + Dear ImGui.

## Overview

`app_window` bundles wgpu, winit, Dear ImGui, and `borderless_window` into a single `AppWindow::run()` call. Your application only implements the `AppHandler` trait — all GPU init, event loop, DPI handling, and frame rendering is handled automatically.

## Features

- **Zero boilerplate** — one `run()` call replaces ~300 lines of setup code
- **Power-aware GPU selection** — scores every surface-compatible adapter
  (DX12 > Vulkan > GL; Discrete > Integrated by default), cascades through
  candidates on `request_device` failure, and warns when falling back to a
  software renderer. `PowerMode::LowPower` flips the iGPU/dGPU priority
  for battery-sensitive UI apps; `PowerMode::HighPerformance` refuses
  silent fallback to WARP / llvmpipe.
- **Auto surface format detection** — prefers sRGB, gracefully falls back
- **Auto HiDPI** — DPI scale clamped to `[1.0, 3.0]`, font scaled accordingly
- **FPS strategy** — `FpsMode::{Auto, Fixed(n), Unlimited}` (default `Auto` = match monitor refresh via wgpu Fifo vsync)
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
use dear_imgui_custom_mod::app_window::{AppConfig, FpsMode, PowerMode, StartPosition};
use dear_imgui_custom_mod::theme::Theme;

let _config = AppConfig::new("My App", 1100.0, 680.0)
    .with_min_size(640.0, 400.0)
    .with_fps_mode(FpsMode::Auto)                    // or Fixed(60) / Unlimited
    .with_power_mode(PowerMode::Auto)                // or LowPower / HighPerformance
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

### `FpsMode`

| Variant | Behavior |
|---------|----------|
| `Auto` (default) | Match display refresh via wgpu Fifo vsync — no OS-timer stutter |
| `Fixed(u32)` | Cap to exactly N frames per second via `WaitUntil` |
| `Unlimited` | `Poll` mode, render as fast as possible (benchmarking only) |

### `PowerMode`

Preference only — the runtime still enumerates every adapter and picks
the best surface-compatible one. The mode flips iGPU/dGPU priority within
the scoring table and optionally filters software (CPU) renderers.

| Variant | Behavior | Best for |
|---------|----------|----------|
| `Auto` (default) | Discrete GPU preferred when available | Desktops, 3D / graphics apps |
| `LowPower` | Integrated GPU preferred; iGPU wins over dGPU | Battery-powered laptops, monitoring tools, editors |
| `HighPerformance` | Same priority as `Auto`, but refuses software fallback | Apps that must not silently run on WARP / llvmpipe |

On hybrid-GPU laptops (NVIDIA Optimus / AMD Hybrid), `LowPower` avoids
the ~200–500 ms dGPU power-on transition, keeps fans quiet, and extends
battery life. For a process monitor or editor it's usually the right
choice — rendering cost is trivial.

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
| `with_fps_mode(mode)` | `Auto` | FPS strategy: `Auto` (monitor Hz) / `Fixed(n)` / `Unlimited` |
| `with_fps_limit(fps)` | — | Shortcut for `with_fps_mode(FpsMode::Fixed(fps))` |
| `with_power_mode(mode)` | `Auto` | GPU preference: `Auto` (dGPU) / `LowPower` (iGPU) / `HighPerformance` (no software fallback) |
| `with_font_size(px)` | `15.0` | Base font size in logical pixels |
| `with_start_position(p)` | `CenterScreen` | Where to place the window on startup |
| `with_theme(theme)` | `Dark` | Initial color theme |
| `with_titlebar(cfg)` | default | Replace the entire titlebar config |
| `with_corner_radius(r: i32)` | `8` | Rounded-corner radius for Win10 fallback path (Win11 DWM ignores this) |
| `with_mdi_icons()` | `false` | Merge Material Design Icons font into atlas — required for MDI codepoints (U+F0000–U+F1FFF) in nav panel buttons etc. |
