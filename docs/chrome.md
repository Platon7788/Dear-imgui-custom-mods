# chrome

Borderless-window helpers — **stateless / explicit-state design**. Replaces
the removed `app_window` module (see [ADR-029](../../Documents/Obsidian%20Vault/dear-imgui-mods/decisions/029-chrome-replaces-app-window.md)).

The chrome doesn't own a runner, an event loop, or a wgpu/ImGui context —
it just provides:

- A custom Dear ImGui titlebar (min / max / close / drag / double-click).
- Resize-edge detection with cursor mapping.
- Win32 helpers (DWM dark mode, rounded corners, opacity, Win10 region sync).
- A small `Chrome` convenience wrapper that bundles per-frame state.

The host wires these helpers into its own `winit` + `wgpu` event loop.

## Why no runner?

The old `app_window` module duplicated everything `dear-app` already does
(event loop, wgpu surface lifecycle, ImGui platform / renderer init,
font atlas, frame scheduling). Every Windows quirk we hit (`SWP_FRAMECHANGED`
client-area expansion, borderless-fullscreen heuristic on small screens,
DPI / monitor-switch font rebuild, `Suboptimal` swap-chain reconfigure)
had to be re-discovered and patched in our runner. Meanwhile `test-dear-imgui-rs`
— which uses `dear-app` directly + a thin chrome layer — never had any
of those bugs.

The 2026-05 refactor (session 044) deletes the runner entirely and exposes
chrome as helper functions. Hosts drive their own `winit` + `wgpu` loop (see
`demo_chrome`), keeping full control of window/surface lifecycle; we maintain
less code. Note: `dear-app` 0.17 can't host chrome — its frame context no
longer exposes a per-frame window handle.

## Architecture

Method `impl`s + stateless functions are split across files (each < 500
lines) but share one public surface via the parent `mod.rs`:

```
chrome/
├── mod.rs       Public types, Chrome struct, monitor-clamp math + tests
├── render.rs    render_titlebar(), whole_window_resize() (stateless)
├── state.rs     impl Chrome — builders, setters, on_setup/on_event/render
├── config.rs    TitlebarConfig, Buttons, TitleAlign, CloseMode
├── edge.rs      ResizeEdge, edge_at(), cursor_for_edge(), resize_direction()
├── glyph.rs     Vector glyphs for min / max / restore / close (DPI-crisp)
└── win32.rs     setup_window(), sync_region(), set_opacity(), hwnd_of()
```

Public types:

- [`TitlebarConfig`] — height, alignment, buttons, close mode.
- [`Buttons`] — which buttons (min / max / close), width, icon radius,
  hover-zoom scale.
- [`TitleAlign`] — Left / Center.
- [`CloseMode`] — Immediate / Confirm.
- [`ResizeEdge`] — N / S / E / W + 4 corners.
- [`TitlebarAction`] — None / Minimize / Maximize / Close / DragStart /
  ResizeStart.
- [`Chrome`] — stateful wrapper bundling cursor / hover-edge / maximize
  tracking, dispatches actions to `winit::Window` automatically.

## Two integration patterns

### 1. Stateless helpers (full control)

For hosts with their own root window setup:

```rust
use dear_imgui_custom_mod::chrome::{render_titlebar, TitlebarConfig};

let cfg = TitlebarConfig::default();
let palette = Theme::Dark.titlebar();

ui.window("##my_root").build(|| {
    let result = render_titlebar(
        ui,
        &cfg,
        "My App",
        &palette,
        is_maximized,
        6.0,        // resize-zone in logical pixels
        true,       // os_resizable
    );
    // dispatch result.action yourself…
});
```

### 2. `Chrome` wrapper (recommended)

Bundles state + dispatch. Drive it from your own `winit` + `wgpu` loop —
`chrome` needs the `Arc<Window>` and per-frame `&Ui`, which a `winit`
`ApplicationHandler` provides directly. (It can't run under `dear-app`,
whose 0.17 frame context exposes no per-frame window handle.) See
`examples-app/examples/demo_chrome.rs` for a complete, runnable reference.

```rust,ignore
use std::sync::Arc;
use dear_imgui_custom_mod::chrome::{Chrome, TitlebarConfig};

// Once, after creating the borderless `Arc<winit::window::Window>`:
let mut chrome = Chrome::new(TitlebarConfig::default())
    .with_title("My App")
    .with_corner_radius(8);
chrome.on_setup(&window);

// In `ApplicationHandler::window_event`, before your own handling —
// chrome expects a full `winit::event::Event`, then forward to the platform:
let wrapped = winit::event::Event::WindowEvent { window_id, event: event.clone() };
chrome.on_event(&wrapped, &window, &mut imgui_context);
let _ = platform.handle_window_event(&mut imgui_context, &window, &event);

// Each frame (RedrawRequested), inside the Dear ImGui frame:
chrome.render(ui, &window, |ui, area| {
    // your UI inside the content area …
    ui.text(format!("Content goes here, area = {:?}", area.size));
});
if chrome.take_close_request().is_some() {
    std::process::exit(0);
}
```

## Single-window architecture

`Chrome::render` wraps the host content in **one** full-display ImGui
root window. The titlebar paints into the same window (top of its draw
list) and the content callback fires inside it. Three reasons:

1. **One Z-layer.** Foreground / background draw-list overlays from
   `status_bar::render_overlay` and `nav_panel::render_nav_panel_overlay`
   composite cleanly relative to the host content.
2. **Full-edge resize hit-testing.** `edge_at` runs against
   `ui.window_size()` which equals the display — every edge / corner
   surfaces a hover cursor and dispatches `drag_resize_window`.
3. **No click absorption.** Two stacked root windows with
   `NO_BRING_TO_FRONT_ON_FOCUS` are easy to mis-arrange (the lower one
   silently swallows hover state).

## Limitations

### Cyrillic Ctrl+C / V / X requires runner-level event interception

Dear-imgui-winit derives Dear ImGui keys from the *logical* (post-keyboard-
layout) key. On Cyrillic / CJK / Greek layouts physical `KeyC` arrives as
e.g. Cyrillic `с` — neither maps to `Key::C` nor reaches `InputText` as a
shortcut, so `Ctrl+C` silently does nothing.

The `crate::input::keyboard` module has the fix (`try_dispatch_ctrl_alt_shortcut`,
`try_dispatch_numpad_text`, `dispatch_ime_commit`), but it requires the runner
to call `platform.handle_event` **conditionally** — when our injection
fires, the platform handler must be skipped, otherwise both fire and the
Cyrillic `с` gets typed into the InputText after the Ctrl+C copy.

`dear-app`'s current `on_event` API runs unconditionally before
`platform.handle_event` and provides no way to suppress it, so under
`dear-app` hosts the Cyrillic shortcut fix is **not active**. Hosts with
their own winit event loop (or a forked dear-app with a `bool` return on
`on_event`) can wire `crate::input::keyboard::*` directly.

Tracking: file an upstream issue with `dear-app` to add `consumed: bool`
return to `on_event`.

### Themes

`Chrome::render` hardcodes the Dark titlebar palette
(`Theme::Dark.titlebar()`). Hosts that want a different palette can call
[`render_titlebar`] directly and manage their own root window. The
hardcoded palette is a deliberate scope cut — any host that needs runtime
theme switching is already maintaining their own root window.

### Splash / chrome-less windows

Use [`whole_window_resize`] from inside your own root window when
`config.chrome` is conceptually `None`. There's no built-in `Chrome`
wrapper for the splash case — splash configs are too varied (no
titlebar, custom drag, auto-close timer) to abstract usefully.

## Win32 details

[`win32::setup_window`] applies one-shot setup after `set_decorations(false)`:

- DWM dark mode (Alt-Tab thumbnail tint).
- Rounded corners — `DWMWA_WINDOW_CORNER_PREFERENCE` on Win11, fallback
  to `SetWindowRgn` with a rounded-rect region on Win10.

[`win32::sync_region`] — Win10 only. The rounded-rect region must be
cleared (`SetWindowRgn(hwnd, NULL, 1)`) when the window maximises so the
taskbar-side corners don't get clipped against the monitor edges (the
"black corner" / "shadow eats the bottom" artefact). Re-applied when the
window restores. No-op on Win11 (DWM owns the corners natively).

[`win32::set_opacity`] toggles `WS_EX_LAYERED` only when alpha < 1.0 — the
style flag is removed when opaque to avoid the perf hit of redirected
layered surfaces.

## Test reference

`test-dear-imgui-rs` (sibling repo) is the canonical end-to-end example —
borderless window, custom titlebar, status bar overlay, nav panel,
keyboard fix, all wired through `dear-app` and the chrome helpers.
