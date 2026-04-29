# app_window

Borderless window framework — successor to [`app_window`](app_window.md).
Custom Dear ImGui titlebar; native OS resize, Aero Snap, drop shadow,
taskbar / Alt-Tab integration preserved.

## Why v2

- **Stable visuals on Win10 / Win11.** v1 had subtle DWM-driven artefacts
  (caption strip, focus dimming, white edges). v2 sidesteps DWM entirely
  via `WS_POPUP + WS_THICKFRAME`.
- **Event-driven render loop by default.** Idle CPU/GPU usage drops to
  ≈ 0 %; full refresh-rate (60 / 144 / 240 Hz) on input. See
  [Render mode](#render-mode) below.
- **Cross-thread wake-up** — [`AppProxy`](#cross-thread-wake-up) lets
  background threads / async tasks repaint the UI without busy-polling.
  Mandatory for HTTP / file-watch / IPC / async runtimes.
- **Raw event hook** —
  [`AppHandler::on_window_event`](#raw-event-hook) delivers winit
  `WindowEvent`s before ImGui sees them. Unlocks drag-drop file paths,
  layout-independent hotkeys, custom IME / touchpad gestures.
- **System clipboard wired by default.** `Ctrl+C` / `Ctrl+V` in any
  `InputText` round-trips through the OS clipboard; no boilerplate.
- **Multi-font Stack API** — `FontChoice::Stack` merges base font +
  icons + CJK overlays into a single atlas with per-layer glyph ranges.
- **HiDPI font rebuild** —
  [`WindowEvent::ScaleFactorChanged`](#hidpi-font-rebuild) automatically
  rebuilds the font atlas at the new monitor scale so dragging between
  100 % and 200 % displays does not leave blurry text.
- **Modular tree.** Config / chrome / gpu split into directories; every
  file under 500 LoC.
- **Per-button accent palette.** Vex0r-style amber/cyan/red on
  minimise/maximise/close out of the box.
- **Window icon API** — `with_window_icon_rgba(...)` for taskbar / Alt-Tab.

## Architecture (Windows)

All windows are created with `decorations=false`, so winit produces a
`WS_POPUP + WS_THICKFRAME` window. That style has *no* caption, *no*
system menu, *no* DWM chrome — meaning DWM has nothing to draw or tint
when the window loses focus. `WS_THICKFRAME` keeps native edge resize,
Aero Snap, and the DWM drop shadow.

All Win32-side helpers live in `app_window::win32` (~270 LoC):

| Helper | Purpose |
|--------|---------|
| `hwnd_of(window)` | Extract the HWND from a winit window. |
| `set_titlebar_dark_mode` | DWMWA_USE_IMMERSIVE_DARK_MODE — kills the white-flash on the Alt-Tab thumbnail. |
| `set_rounded_corners` | DWMWA_WINDOW_CORNER_PREFERENCE on Win11; `SetWindowRgn` rounded-rect fallback on Win10. |
| `update_rounded_region` | Re-apply the Win10 region after `WindowEvent::Resized` (no-op on Win11). |
| `is_win11` | Cached probe — true when the Win11 DWM rounded-corner path succeeded. |
| `WS_EX_TOOLWINDOW` | Excludes tool-window kinds from Alt-Tab. |
| `WM_GETMINMAXINFO` subclass | Clamps a maximised `WS_THICKFRAME` window to the monitor work area so it doesn't cover the taskbar. |
| `set_opacity` | Toggles `WS_EX_LAYERED`. |
| `debug_log` | `OutputDebugStringW` — survives `windows_subsystem = "windows"`. |

Before the v1 + `borderless_window` removal (2026-04-29) the first
five helpers lived in `borderless_window::platform`; they are inlined
in `win32.rs` now so the framework is fully self-contained.

The titlebar itself is pure Dear ImGui — buttons, drag, double-click are
all drawn into the ImGui draw list and dispatched to the OS via
`winit::window::Window::drag_window` / `drag_resize_window`.

## Module layout

```text
src/app_window/
├── mod.rs            ~580 — event loop, needs_redraw matcher,
│                            about_to_wait, on_window_event hook,
│                            user_event impl, ScaleFactorChanged
│                            font rebuild
├── handler.rs         91 — AppHandler trait (incl. on_window_event)
├── state.rs          158 — AppState (incl. keep_alive, proxy) +
│                            TitlebarState
├── proxy.rs          ~95 — AppProxy cross-thread wake-up +
│                           WakeError + 3 unit-tests
├── clipboard.rs      174 — SystemClipboardBackend (direct Win32
│                            CF_UNICODETEXT read+write, no
│                            ig*ClipboardText round-trip)
├── win32.rs          ~270 — all host Win32 glue (HWND extract,
│                            DWM dark mode, rounded corners,
│                            WS_EX_TOOLWINDOW, MinMax subclass,
│                            set_opacity, debug_log)
├── config/
│   ├── mod.rs        ~600 — AppConfig + presets + builders
│   │                        (incl. with_font_stack) + 11 unit-tests
│   ├── enums.rs      195 — WindowKind/Border/Form/Position/Close/Fps/
│   │                       Power/TitleAlign + RenderMode
│   ├── titlebar.rs   143 — TitlebarConfig + Chrome + Buttons +
│   │                       ExtraButton
│   ├── icon.rs        44 — WindowIcon
│   └── (FontChoice / FontLayer / GlyphRanges in mod.rs)
├── chrome/
│   ├── mod.rs        349 — public types + render_titlebar
│   ├── edge.rs       151 — edge_at + cursor_for_edge + 5 unit-tests
│   └── glyph.rs       51 — draw_close (circle-X) / max / restore /
│                            minimize
└── gpu/
    ├── mod.rs        348 — GpuState + render_frame (frame_demand
    │                       integration)
    ├── setup.rs      148 — init_wgpu + adapter selection
    ├── imgui.rs      ~270 — init_imgui (font stack, glyph ranges,
    │                       clipboard wiring), rebuild_fonts_for_scale,
    │                       resolve_glyph_ranges helper + 6 unit-tests
    └── position.rs    28 — position_window
```

Module-level test count: **20** (`proxy::tests` 3 + `gpu::imgui::tests` 6 +
`config::tests` 11). All run with `cargo test --features=full --lib
app_window`.

## Quick start

```rust,no_run
use dear_imgui_custom_mod::app_window::{
    AppConfig, AppHandler, AppState, AppWindow, Theme,
};
use dear_imgui_rs::Ui;

struct MyApp;
impl AppHandler for MyApp {
    fn render(&mut self, ui: &Ui, _state: &mut AppState) {
        ui.text("Hello, world!");
    }
}

fn main() {
    AppWindow::new(AppConfig::main("My App", 1100.0, 680.0))
        .run(MyApp).unwrap();
}
```

## Presets

| Preset | Border | Chrome | Notes |
|--------|--------|--------|-------|
| `AppConfig::splash(title, w, h)` | None | None | Borderless, centred, stays on top, often paired with `with_auto_close` |
| `AppConfig::tool(title, w, h)` | SizeToolWin | Compact custom | Stays on top, close-only |
| `AppConfig::dialog(title, w, h)` | Dialog | Compact custom | Fixed size, close-only, screen-centred |
| `AppConfig::main(title, w, h)` | Sizeable | Full custom | Default app skeleton |

## Builders (selected)

### Window / chrome

| Method | Effect |
|--------|--------|
| `with_theme(Theme)` | Built-in palette (Dark / Light / Midnight / Solarized / Monokai) |
| `with_corner_radius(i32)` | Rounded corners (Win10 `SetWindowRgn` fallback) |
| `with_power_mode(PowerMode)` | GPU adapter pick (`HighPerformance` default / `LowPower`) — see [GPU adapter strategy](#gpu-adapter-strategy) |
| `with_font_size(f32)` | Base font size in logical pixels |
| `with_builtin_font(BuiltinFont)` | Pick from Hack / JetBrains Mono / JetBrains Mono NL |
| `with_font_bytes(impl Into<Arc<[u8]>>)` | Custom TTF/OTF (e.g. `include_bytes!("Inter.ttf")`) |
| `with_mdi_icons()` | Merge Material Design Icons into the atlas |
| `with_auto_close(Duration)` | Auto-exit after duration (splash) |
| `with_opacity(f32)` | Initial window alpha 0.0–1.0 |
| `with_window_icon_rgba(rgba, w, h)` | Taskbar / Alt-Tab icon from raw pixels |
| `with_close_confirm()` | X click fires `on_close_requested` first |
| `with_extra_button(ExtraButton)` | Custom button left of the standard trio |
| `start_hidden()` | Create hidden; `state.show()` later |
| `stay_on_top()` | `WS_EX_TOPMOST` |
| `raw_content()` | Full-bleed content — skip default child wrapper / padding (see [Full-bleed content](#full-bleed-content) below) |

### Render scheduling

See [Render mode](#render-mode) for details. The default (event-driven,
2 s foreground / 5 s background pulse) is the right choice for almost
every desktop app — these builders are escape hatches.

| Method | Effect |
|--------|--------|
| `with_render_mode(RenderMode)` | Replace the entire scheduling strategy |
| `with_idle_pulse(Duration)` | Foreground refresh cadence in event-driven mode |
| `with_unfocused_idle_pulse(Duration)` | Background cadence (window not focused) |
| `without_idle_pulse()` | Disable foreground pulse — paint only on input + animation requests |
| `event_driven_minimal()` | Both pulses `None` — strictly zero-idle |
| `continuous_render()` | Switch to game-style continuous render at vsync |
| `with_fps_limit(u32)` | Continuous mode + explicit FPS cap |
| `with_unfocused_fps(u32)` | Continuous-mode background FPS cap |

## Render mode

`AppConfig` carries a [`RenderMode`] enum that picks one of two
scheduling strategies. The default is **event-driven** — repaint only
on input events, animation requests, or the optional periodic *idle
pulses* used for clocks / uptime / status metrics.

```rust
pub enum RenderMode {
    /// Default. Idle CPU/GPU ≈ 0 %.
    EventDriven {
        idle_pulse:           Option<Duration>,  // foreground; default 2 s
        unfocused_idle_pulse: Option<Duration>,  // background;  default 5 s
    },
    /// Game-style — every iteration repaints.
    Continuous {
        fps_mode:      FpsMode,  // Auto / Fixed(n) / Unlimited
        unfocused_fps: u32,        // 0 = same as foreground
    },
}
```

### How the event-driven loop schedules a frame

| Source | Effect |
|--------|--------|
| Input (`needs_redraw` matcher: cursor, mouse, key, IME, scale, theme, touch, drag-drop, gestures) | `pending_frames = 2` + `request_redraw()` |
| `Focused(true)` / `Focused(false)` | `pending_frames = 2` + `request_redraw()` (titlebar tint flips) |
| `Resized` | Reconfigure surface, `pending_frames = 2` + `request_redraw()` |
| `frame_demand::request(N)` (animation widget, e.g. `notifications`) | After current frame, `pending_frames` raised to `N` |
| `state.keep_alive(N)` (user code) | Same as `frame_demand::request(N)` |
| `ui.io().want_text_input()` (any active `InputText`) | One more frame for cursor blink |
| `idle_pulse` timer expires (window focused, idle) | Single redraw — clocks, uptime tick |
| `unfocused_idle_pulse` timer expires (window unfocused, idle) | Single redraw |
| Window minimized | `ControlFlow::Wait`, **no** `request_redraw` — 0 fps |

### Why this matters

| Scenario | Continuous (Poll + vsync) | Event-driven (default) |
|----------|---------------------------|------------------------|
| Idle, focused | matches refresh rate (60 / 144 / 240 Hz) | **0.5 fps** (one frame per `idle_pulse`) |
| Idle, unfocused | depends on cap | **0.2 fps** (one frame per `unfocused_idle_pulse`) |
| Window minimized | 0 fps | 0 fps |
| Mouse / keyboard input | full refresh | full refresh |
| `InputText` active | full refresh | full refresh (`want_text_input`) |
| Notification fading | full refresh | full refresh until fade ≤ 0 |
| `event_driven_minimal()` idle | — | **0 fps** literally (no pulse, no input) |

For a 144 Hz monitor, event-driven mode buys back ~5–10 % of one CPU
core when the user is reading the screen — typical for editors,
dashboards, chat / file managers. Continuous mode remains the right
choice for games, simulations, live previews.

### Animation contract

If a built-in or user widget has an animation in flight (fade, slide,
countdown), it is responsible for keeping the loop alive. Two equivalent
APIs:

```rust
// Inside a widget render — no AppState required:
crate::frame_demand::request(1);

// Inside a user `AppHandler::render` — through state:
state.keep_alive(1);
```

Both lock-step into the same thread-local counter (see
[`frame_demand`](frame_demand.md)). Continuous-render hosts ignore the
counter; the calls become a cheap no-op. Without these calls in
event-driven mode, the animation would freeze on the next idle pulse.

Built-in widgets that already do this:

- `notifications` — keeps alive while any toast is animating or its
  countdown timer is ticking.
- `confirm_dialog` — static; no keep-alive needed (re-rendered on user
  input via the `needs_redraw` matcher).

### Example configurations

```rust
// Default — usually right
AppConfig::main("App", 1100.0, 680.0)

// Live clock with a second hand: 2 fps idle
.with_idle_pulse(Duration::from_millis(500))

// Quieter background: 10 s unfocused pulse
.with_unfocused_idle_pulse(Duration::from_secs(10))

// Strictly zero-idle (no clocks, no time-driven UI)
.event_driven_minimal()

// Game-style: always vsync (auto-matches monitor refresh)
.continuous_render()

// Game-style with explicit cap
.with_fps_limit(60).with_unfocused_fps(15)
```

[`RenderMode`]: ../src/app_window/config/enums.rs

## Cross-thread wake-up

In event-driven mode the loop sleeps in `ControlFlow::Wait` until input
arrives. Background threads (HTTP, file watch, IPC, async runtimes) need
a way to **wake** the loop when their work completes — otherwise the UI
stalls until the next idle pulse (default: 2 s).

[`AppProxy`] (returned by [`AppState::proxy`]) is `Send + Sync +
Clone` and exposes a single `wake()` method. Calls are idempotent and
coalesce — multiple wakes between two iterations of the loop trigger
exactly one redraw cycle.

```rust,ignore
use dear_imgui_custom_mod::app_window::{AppHandler, AppState, AppProxy};
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

struct MyApp {
    counter: Arc<AtomicU32>,
    _bg: Option<std::thread::JoinHandle<()>>,
}

impl AppHandler for MyApp {
    fn on_ready(&mut self, state: &mut AppState) {
        let proxy: AppProxy = state.proxy();
        let counter = Arc::clone(&self.counter);
        self._bg = Some(std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            counter.fetch_add(1, Ordering::Relaxed);
            if proxy.wake().is_err() { break; }    // event loop closed
        }));
    }

    fn render(&mut self, ui: &dear_imgui_rs::Ui, _state: &mut AppState) {
        ui.text(format!("Tick: {}", self.counter.load(Ordering::Relaxed)));
    }
}
```

`wake()` returns `Result<(), WakeError>`; `WakeError::EventLoopClosed`
fires after the application has begun shutting down. Most callers
ignore the error.

## Raw event hook

`AppHandler::on_window_event(&self, event: &WindowEvent, state) ->
bool` receives **every** winit `WindowEvent` *before* the ImGui
platform layer processes it. Return `true` to **consume** the event so
Dear ImGui does not see it.

> **Contract change (session 021)** — `consumed = true` only suppresses
> the **ImGui platform handler**. The framework's own routing (`Resized`
> reconfigures the wgpu surface, `CloseRequested` exits, `Focused`
> updates titlebar tint, `RedrawRequested` runs `render_frame`,
> `ScaleFactorChanged` rebuilds the font atlas) **always runs**.
> Consuming a structural event is normally pointless; the previous
> "consume = skip everything" contract produced black-rect, stuck-
> close-button and stuck-tint bugs and has been replaced.

The headline use case: **drag-drop file paths**. The framework's
`needs_redraw` matcher includes `DroppedFile` so the window repaints
when a file is dropped — but the path itself is only delivered through
this hook.

```rust,ignore
use dear_imgui_custom_mod::winit::event::WindowEvent;

fn on_window_event(&mut self, event: &WindowEvent, _: &mut AppState) -> bool {
    if let WindowEvent::DroppedFile(path) = event {
        self.last_dropped = Some(path.clone());
        // …open the file, populate buffer, etc.
    }
    false   // don't consume — let ImGui still see hover / cursor moves
}
```

Other use cases:

- Layout-independent hotkeys (physical key codes before ImGui's keyboard layer).
- Custom IME composition handling.
- Touchpad gestures (`PinchGesture`, `PanGesture`, `RotationGesture`).
- Application-level shortcuts that bypass focused widgets ("F12 toggle dev console").

## System clipboard

`AppWindow` installs a system clipboard backend on the ImGui context
automatically (see `app_window/clipboard.rs`). Without this, every
`InputText` ends up with a private paste buffer that does not interact
with the OS — an immediate UX regression for end users.

Both `get` and `set` go **directly to the Win32 API**, bypassing
`igSetClipboardText` / `igGetClipboardText`. Routing through ImGui's
setter from inside a backend `set` would re-enter the same callback
and be silently short-circuited by `dear_imgui_rs::ClipboardBorrowGuard`
— meaning **the OS clipboard never actually gets the text**. (This was
a real bug fixed in session 021; if you write a custom backend, do the
same — talk to the OS directly.)

- **`set`** — `OpenClipboard(NULL)` → `EmptyClipboard()` →
  `GlobalAlloc(GHND, bytes)` → `GlobalLock` → `memcpy` UTF-16 →
  `GlobalUnlock` → `SetClipboardData(CF_UNICODETEXT, h_mem)` →
  `CloseClipboard()`. After `SetClipboardData` succeeds, the OS owns
  the handle (we do **not** free it).
- **`get`** — on Windows, `OpenClipboard(NULL)` →
  `GetClipboardData(CF_UNICODETEXT)` → `GlobalLock` → UTF-16 lookup
  with a 16 Mi-character defensive cap → `String::from_utf16_lossy`.
  On other platforms returns `None` (a future minor release will add
  platform-native getters for macOS / X11 / Wayland via opt-in
  `arboard` feature).

If you need a custom backend (e.g. piping through a remote-desktop
session), implement `dear_imgui_rs::ClipboardBackend` and install with
`context.set_clipboard_backend(your_backend)`. The framework's default
is installed during `init_imgui` so you'd need to swap it after — a
minor builder is on the roadmap.

## Multi-font Stack API

```rust,ignore
use dear_imgui_custom_mod::app_window::{
    AppConfig, FontChoice, FontLayer, GlyphRanges,
};

let cfg = AppConfig::main("App", 1100.0, 680.0)
    .with_font_stack(vec![
        // Base UI font (Latin only)
        FontLayer::base(include_bytes!("Inter.ttf").as_slice(), 15.0),
        // Material Design Icons merged on top — private-use plane
        FontLayer::merge(include_bytes!("MDI.ttf").as_slice(), 13.0)
            .with_glyph_ranges(GlyphRanges::Custom(vec![[0xF0001, 0xF1FFF]])),
        // Cyrillic overlay if the UI text needs Russian glyphs
        FontLayer::merge(include_bytes!("Inter-Cyrillic.ttf").as_slice(), 15.0)
            .with_glyph_ranges(GlyphRanges::Cyrillic),
    ]);
```

[`GlyphRanges`] presets cover Latin (`Default`), `Cyrillic`,
`Japanese`, `ChineseSimplified`/`Traditional`, `Korean`, `Thai`,
`Vietnamese`, plus `Custom(Vec<[u32; 2]>)` for arbitrary inclusive
ranges (icon fonts, math symbols, emoji). Ranges are inlined inside
the framework — decoupled from upstream Dear ImGui deprecation noise.

The first layer is always the **base** (its `merge` flag is ignored —
bases never merge); subsequent layers should set `merge = true`.

[`AppProxy`]: ../src/app_window/proxy.rs
[`AppState::proxy`]: ../src/app_window/state.rs
[`GlyphRanges`]: ../src/app_window/config/mod.rs

## GPU adapter strategy

The framework copies [`IMGUI_NXT`](https://github.com/Platon7788/IMGUI_NXT)'s
production-tested adapter-selection path: ask the OS GPU manager for the
preferred adapter, then fall back to the WARP / llvmpipe software
renderer if the primary path fails. This works on every tested setup —
integrated-only laptops (Intel UHD / AMD Vega), hybrid Optimus /
switchable graphics laptops, desktops with dedicated GPUs, and machines
without a working hardware-accelerated path.

### Why not enumerate-and-score?

Earlier versions of `app_window` used `wgpu::Instance::enumerate_adapters`
and a manual score (discrete GPU > integrated GPU > software). On hybrid
laptops where the display is routed through the iGPU but a discrete
GPU is also visible, the score picked the dGPU; `request_device` could
then fail or produce a black window. The new path uses
`request_adapter(compatible_surface = Some(&surface))`, which the OS
GPU manager resolves correctly because it knows the actual display
routing.

### Selection sequence

```text
1. request_adapter(HighPerformance, compatible_surface=Some(&surface))
       Ok  → use that adapter (driver picks display-routed GPU)
       Err ↓
2. request_adapter(force_fallback_adapter=true, compatible_surface=Some(&surface))
       Ok  → use software adapter (WARP on Win, llvmpipe on Linux)
       Err ↓
3. panic("wgpu: no usable adapter — primary + fallback both failed")
```

### `PowerMode`

Two variants — one strategy with one knob.

| Variant | Maps to | Use case |
|---|---|---|
| **`HighPerformance` (default)** | `wgpu::PowerPreference::HighPerformance` | NxT-proven default. Returns iGPU on integrated-only laptops (only adapter), display-routed GPU on hybrid laptops, dedicated GPU on desktops. |
| `LowPower` | `wgpu::PowerPreference::LowPower` | Battery-friendly opt-in for hybrid laptops where the user explicitly wants the iGPU even when a dGPU is available. On integrated-only machines this is identical to `HighPerformance`. |

The previous `Auto` variant was a duplicate alias to
`HighPerformance` and has been removed; users who wrote
`PowerMode::Auto` should switch to
`PowerMode::HighPerformance` (or simply
`PowerMode::default()`).

### Present-mode validation

`AppConfig.render_mode.fps_mode()` is mapped to a present-mode using
`surface.get_capabilities().present_modes` so the choice is **always
supported** by the actual adapter:

| `FpsMode` | Tries (in order) |
|---|---|
| `Auto` / `Fixed(n)` (vsync) | `FifoRelaxed` (adaptive) → `Fifo` (mandated by spec, always works) |
| `Unlimited` | `Mailbox` → `Immediate` → `Fifo` |

`Fifo` is mandated by the wgpu spec to be supported on every surface,
so the terminal fallback is guaranteed. This avoids
`surface.configure()` panics on older Intel iGPU drivers that report
only `Fifo` while we previously requested `AutoNoVsync`/`Mailbox`.

### Diagnostics

The selected adapter is logged to stderr **and**
`OutputDebugStringW` (visible in DebugView even when
`windows_subsystem = "windows"` detaches stderr):

```
wgpu: adapter "Intel(R) UHD Graphics 620" (IntegratedGpu, Dx12)
       | driver "Intel" "27.20.100.8729"
```

If the software fallback path triggers, an additional
`wgpu: WARNING: software renderer active` line follows so the user can
see why performance is poor.

## Layout-independent keyboard

`app_window` (and now also `app_window` v1) intercepts every
`WindowEvent::KeyboardInput` **before** Dear ImGui's platform layer
sees it and fixes three latent issues that would otherwise force the
user to switch keyboard layout:

| Problem | Fix |
|---|---|
| **Cyrillic / Greek / CJK layouts** — `Ctrl+C` arrives as `Ctrl+С` (Cyrillic) which `dear-imgui-winit` does not map to `Key::C`. The user has to switch to English to copy/paste. | `crate::input::keyboard::try_inject_ctrl_alt_shortcut` — if a modifier is held and the **physical** scan code maps to a Dear ImGui `Key`, inject that key directly. Skip the platform forward so the Cyrillic glyph is **not** typed into the focused text field. |
| **Numpad digits** never reach `InputText` — ImGui treats `Keypad0..9` as navigation, not text. | `try_inject_numpad_text` — for numpad-located printable events, push each character via `add_input_character`. |
| **IME composition** (Japanese / Korean / Chinese) — `dear-imgui-winit` ignores `WindowEvent::Ime` entirely. | On `Ime::Commit(text)`, push every `char` via `add_input_character`. |
| **Stuck `Key::C`** when the user releases `Ctrl` *before* the letter on a non-Latin layout (release event arrives with the wrong logical key). | After the platform forward, call `reinforce_physical_key_state` — idempotent re-injection that overrides the platform's mistaken release. |

All four fixes are wired automatically — handlers do nothing.
The pure helpers live in [`crate::input::keyboard`] in case a custom
host needs them too.

[`crate::input::keyboard`]: ../src/input/keyboard.rs

## HiDPI font rebuild

When the user drags the window between monitors of differing DPI
(typical: 100 % laptop screen ↔ 200 % external 4K), winit fires
[`WindowEvent::ScaleFactorChanged { scale_factor }`]. The framework
intercepts this and **rebuilds the font atlas at the new scale**
automatically — without this step text would render at the old physical
pixel size and look blurry through the OS upscale until app restart.

Implementation (`gpu/imgui.rs::rebuild_fonts_for_scale`):

1. `context.fonts().clear_fonts()` — drop every previously-added
   `ImFont` (texture data is rebuilt on demand by Dear ImGui 1.92+'s
   `RENDERER_HAS_TEXTURES` flag, which `dear_imgui_wgpu` sets).
2. Re-add fonts using the stored [`AppConfig.font`] at the new
   scale (`Builtin` / `Bytes` / `Stack` paths all supported).
3. `renderer.invalidate_device_objects()` — flush the wgpu pipeline
   cache, render resources, frame resources, texture manager. Cheap
   (rebuilt lazily next frame).
4. `io.set_font_global_scale(1.0 / new_hidpi)` — reciprocal scale so
   widget metrics keep their logical sizes; ImGui multiplies by
   `display_framebuffer_scale` internally.

Two frames are added to `pending_frames` so the rebuilt atlas reaches
the screen even in event-driven mode. New `cfg.merge_mdi_icons` flag
is honored on rebuild.

If you ever override font config at runtime (currently requires a
custom path — talk to the proxy and your handler), the same routine
fires.

[`WindowEvent::ScaleFactorChanged { scale_factor }`]: https://docs.rs/winit/latest/winit/event/enum.WindowEvent.html
[`AppConfig.font`]: ../src/app_window/config/mod.rs

## Per-button colour palette (default)

Every theme paints the standard buttons with distinct accents (Vex0r style):

| Button | Dark / Midnight | Light | Solarized | Monokai |
|--------|-----------------|-------|-----------|---------|
| Minimize (`─`) | `#fcbf00` amber | `#c99500` | `YELLOW` | `YELLOW` |
| Maximize (`□`) | `#4fc3f7` cyan | `#1976d2` | `CYAN` | `CYAN` |
| Close (`⊗`) | `#ef5350` red | `#c62828` | `RED` | `RED` |

Close is drawn as a **circle-X** (matches MDI `close-circle-outline`) — purely
draw-list primitives, no font dependency.

## State (runtime)

```rust
pub struct AppState {
    pub titlebar: TitlebarState,    // .maximized / .focused
    // ...mutators below queue actions for end-of-frame dispatch
}

impl AppState {
    fn exit(&mut self);
    fn minimize(&mut self);
    fn set_maximized(&mut self, v: bool);
    fn toggle_maximized(&mut self);
    fn set_theme(&mut self, t: Theme);
    fn confirm_close(&mut self);    // for CloseMode::Confirm
    fn show(&mut self) / hide(&mut self);
    fn set_title(&mut self, ...);
    fn set_opacity(&mut self, alpha: f32);

    /// Request that the host render at least `frames` more frames after
    /// this one. Use for animations that don't have input (timers,
    /// progress bars, splash sequences). In event-driven mode the
    /// difference between an animation playing and a frame freezing
    /// mid-tween. Continuous mode: no-op.
    fn keep_alive(&self, frames: u8);

    /// Cross-thread wake-up proxy. Clone freely; hand to background
    /// threads / async tasks; call `proxy.wake()` to repaint the UI.
    /// `Send + Sync + Clone`.
    fn proxy(&self) -> AppProxy;
}
```

`keep_alive` is a thin shim over [`crate::frame_demand::request`](frame_demand.md);
both paths land in the same thread-local. Built-in animation widgets
(`notifications`, anything with a fade/scale tween) call
`frame_demand::request` directly so they work regardless of which host
drives them.

## Handler trait

```rust
pub trait AppHandler {
    /// Per-frame render — the only required method.
    fn render(&mut self, ui: &Ui, state: &mut AppState);

    /// Window close requested (X button, Alt-F4, OS close).
    /// Default: `state.exit()`. Override for confirm-close UX.
    fn on_close_requested(&mut self, state: &mut AppState) { state.exit(); }

    /// Custom titlebar `ExtraButton` clicked.
    fn on_extra_button(&mut self, _id: &'static str, _state: &mut AppState) {}

    /// Titlebar icon glyph clicked (if set).
    fn on_icon_click(&mut self, _state: &mut AppState) {}

    /// Theme changed via `state.set_theme(...)`.
    fn on_theme_changed(&mut self, _theme: &Theme, _state: &mut AppState) {}

    /// Window is fully created and ready. Use `state.proxy()` here to
    /// grab the cross-thread wake-up handle for any background work.
    fn on_ready(&mut self, _state: &mut AppState) {}

    /// Raw winit `WindowEvent` hook — called BEFORE the event reaches
    /// Dear ImGui's platform layer. Return `true` to consume.
    /// Use for: drag-drop file paths, layout-independent hotkeys,
    /// custom IME, touchpad gestures, application-level shortcuts.
    fn on_window_event(&mut self, _event: &winit::event::WindowEvent,
                       _state: &mut AppState) -> bool { false }
}
```

## Layout helper

`render_frame` hosts the user UI in a child-window with `WindowPadding=[8,8]`
and `ItemSpacing=[6,4]` so widgets don't crash into the window edge. Inside
`handler.render`, `ui.content_region_avail()` gives the correct inner area
(client width minus titlebar minus padding).

## Full-bleed content

For widgets that need pixel-perfect control over the content rect — chart
viewers, video players, 3D viewports, `node_graph` host viewports,
full-bleed code editors — the default child-window wrapper and the 8-pixel
padding stand in the way.

Set [`AppConfig::raw_content()`] to opt out:

```rust,ignore
AppConfig::main("Chart Studio", 1280.0, 800.0)
    .raw_content()    // ← handler runs directly inside the root window
```

What changes:

- The `##app_content` child-window wrapper is **skipped**.
- `WindowPadding=[8,8]` and `ItemSpacing=[6,4]` are **not pushed**.
- `ui.set_cursor_pos([0.0, content_top])` still runs so the titlebar stays
  on top — the handler's `render` starts at the first pixel below the
  titlebar (or at `[0, 0]` when chrome is `Chrome::None`).
- `ui.content_region_avail()` now returns `[client_w, client_h - title_h]`
  with no padding subtraction.

The handler is responsible for any padding / spacing / scroll regions it
wants. The most common pattern is to push your own `WindowPadding` /
`ItemSpacing` styles or open a child-window with explicit `[w, h]`.

### Common layout patterns

#### Sidebar + main + status (dashboard skeleton)

```rust,ignore
fn render(&mut self, ui: &Ui, state: &mut AppState) {
    let avail = ui.content_region_avail();
    let sidebar_w = 240.0;
    let status_h  = 22.0;

    ui.child_window("##sidebar")
        .size([sidebar_w, avail[1] - status_h])
        .border(true)
        .build(ui, || self.sidebar.render(ui));
    ui.same_line();
    ui.child_window("##main")
        .size([avail[0] - sidebar_w, avail[1] - status_h])
        .build(ui, || self.main.render(ui));

    // Bottom status — pixel-zero, full width.
    ui.set_cursor_pos([0.0, avail[1] - status_h]);
    self.status_bar.render(ui);
}
```

#### Resizable splitter (free position)

```rust,ignore
let split_x = self.split_pos;     // user-controlled
ui.child_window("##left").size([split_x, 0.0])
    .build(ui, || self.left.render(ui));
ui.same_line();
self.draw_split_handle(ui, &mut self.split_pos);
ui.same_line();
ui.child_window("##right").size([0.0, 0.0])
    .build(ui, || self.right.render(ui));
```

#### Pixel-perfect chart / graph host

```rust,ignore
let avail = ui.content_region_avail();
let pos   = ui.cursor_screen_pos();
let dl    = ui.get_window_draw_list();
// Background fills the WHOLE content rect — no padding leak.
dl.add_rect([pos[0], pos[1]], [pos[0] + avail[0], pos[1] + avail[1]],
            0xFF101418).filled(true).build();
self.chart.render(ui, avail);     // chart owns every pixel
```

#### Centered modal / splash (free coordinates)

```rust,ignore
let avail   = ui.content_region_avail();
let modal_w = 480.0;
let modal_h = 280.0;
ui.set_cursor_pos([(avail[0] - modal_w) * 0.5, (avail[1] - modal_h) * 0.5]);
ui.child_window("##modal").size([modal_w, modal_h]).border(true)
    .build(ui, || self.confirm.render(ui));
```

#### Manually positioned widgets

```rust,ignore
let avail = ui.content_region_avail();
ui.set_cursor_pos([10.0, 10.0]);                  ui.button("Top-Left");
ui.set_cursor_pos([avail[0] - 80.0, 10.0]);       ui.button("Top-Right");
ui.set_cursor_pos([10.0, avail[1] - 30.0]);       ui.button("Bottom-Left");
```

### What `raw_content` does **not** remove

- **Titlebar (chrome).** Still rendered above `content_top`. To remove
  it use `Chrome::None` (or [`AppConfig::splash`] preset) — that's
  a separate axis from `raw_content`.
- **The single root ImGui window.** You're inside `##app_root`. Multiple
  independent ImGui top-level windows still work — call
  `ui.window("...").build(...)` from inside `render` for overlays.
- **Frame scheduling, clipboard, proxy, font stack, DPI rebuild.** All
  framework infrastructure keeps working.

### Composition

```rust,ignore
AppConfig::main("Studio", 1280.0, 800.0)
    .raw_content()                    // pixel-perfect layout
    .with_theme(Theme::Midnight)      // styling
    .continuous_render()              // game-style refresh (real-time chart)
    .with_font_stack(vec![ /* … */ ]) // multi-font
```

All builders compose. For a fully chrome-less full-bleed canvas (no
titlebar, no padding, custom window-icon-only Alt-Tab):

```rust,ignore
AppConfig::splash("Visualizer", 1280.0, 720.0)
    .without_chrome()
    .raw_content()
```

Default for `raw_content` is `false`; existing handlers stay on the
padded path with no migration cost.

[`AppConfig::raw_content()`]: ../src/app_window/config/mod.rs
[`AppConfig::splash`]: ../src/app_window/config/mod.rs

## Demo

```bash
cargo run --example demo_app_window -- main      # default
cargo run --example demo_app_window -- splash
cargo run --example demo_app_window -- tool
cargo run --example demo_app_window -- dialog
```

`main` showcases: chrome + nav_panel (left dock with notification badge) +
status_bar (bottom strip with clickable theme cycler) + confirm_dialog
(close-confirm modal with circle-X icon).

## Migration from v1

| v1 (`app_window`) | v2 (`app_window`) |
|-------------------|----------------------|
| `AppConfig` | `AppConfig` |
| `AppHandler` trait | `AppHandler` trait |
| `AppState` | `AppState` |
| `BorderlessConfig` (nested) | flat `Chrome::Custom(TitlebarConfig)` |
| `with_decorations(true)` + DWM hacks | `with_decorations(false)` always |
| `focus_dim` flag | removed (no DWM caption to dim) |
| `show_drag_hint` flag | removed (it confused users with translucent overlay) |
| `BuiltinFont` only | `FontChoice::{Builtin, Bytes}` |

The v1 module is **kept indefinitely** — switch when you specifically need
v2's features. `cargo build --features=app_window` and
`--features=app_window` are independent.
