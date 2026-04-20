# notifications

Modern, flexible toast-notification center for Rust + Dear ImGui.

## Overview

`notifications` provides a stateful [`NotificationCenter`] that renders a
stack of modern toast popups every frame. Each toast has a severity level
(driving its color + icon), an optional auto-dismiss timer with progress bar,
optional action buttons, and fade / slide-in entry / exit animations. Icons
are drawn as draw-list primitives — no icon font needed.

The center maintains its own state between frames; you `push()` notifications
from anywhere in your app (event handlers, async callbacks, error paths) and
call `render()` once per frame to advance timers, honor hover-pause, and draw
the visible stack.

## Features

- **5 severity levels** — `Info`, `Success`, `Warning`, `Error`, `Debug`,
  each with a distinct color and font-independent draw-list icon.
- **6 stack placements** — 4 corners (`TopRight`, `TopLeft`, `BottomRight`,
  `BottomLeft`) + `TopCenter` and `BottomCenter`, with customizable margin
  and inter-toast spacing.
- **Auto-dismiss** via `Duration::Timed(f32)` seconds, or `Duration::Sticky`
  for user-closed toasts. Optional bottom progress bar tracks time remaining.
- **Pause-on-hover** — the auto-dismiss timer freezes while the cursor is
  over the toast, so long bodies are readable.
- **Animations** — `Fade`, `SlideIn` (from the anchor edge), or `None`
  (instant); configurable duration.
- **Action buttons** with caller-defined ids surfaced via
  `NotificationEvent::ActionClicked`.
- **Manual `×` close** button with hover highlight.
- **Custom accent color** per toast — override the severity default.
- **Max-visible cap** with graceful overflow — older toasts fade out as new
  ones arrive.
- **5 built-in themes** (Dark, Light, Midnight, Solarized, Monokai) via the
  unified [`Theme`](theme.md) enum + per-instance `colors_override` for
  fully custom palettes.
- **Zero per-frame allocations** on the hot path beyond the Vec<Event>
  returned (typically empty).

## Quick Start

```rust
use dear_imgui_custom_mod::notifications::{
    Notification, NotificationCenter, NotificationEvent,
};

// Persistent state — lives in your app/handler struct.
let mut center = NotificationCenter::new();

// Push from anywhere — buttons, event handlers, async callbacks.
center.push(Notification::success("Saved"));
center.push(
    Notification::error("Sync failed")
        .with_body("The remote rejected the push.")
        .with_action(1, "Retry")
        .sticky(),
);

# fn frame(ui: &dear_imgui_rs::Ui, center: &mut NotificationCenter, dt: f32) {
// Render every frame — after other UI so toasts stay on top.
for event in center.render(ui, dt) {
    match event {
        NotificationEvent::Dismissed(id)                     => { /* timer / × / action */ }
        NotificationEvent::ActionClicked { id, action_id }   => { /* handle action */ }
        NotificationEvent::Clicked(id)                       => { /* body click */ }
    }
}
# }
```

## Configuration

```rust
use dear_imgui_custom_mod::notifications::{
    AnimationKind, CenterConfig, NotificationCenter, Placement,
};
use dear_imgui_custom_mod::theme::Theme;

let cfg = CenterConfig::new()
    .with_placement(Placement::BottomRight)
    .with_max_visible(6)
    .with_width(360.0)
    .with_spacing(10.0)
    .with_margin(20.0, 20.0)
    .with_padding(12.0, 10.0)
    .with_rounding(8.0)
    .with_animation(AnimationKind::SlideIn)
    .with_animation_duration(0.25)
    .with_pause_on_hover(true)
    .with_theme(Theme::Midnight);

let mut center = NotificationCenter::with_config(cfg);
```

## Notification Builder

```rust
use dear_imgui_custom_mod::notifications::Notification;

// Severity-specific constructors set the default accent color + icon.
Notification::info("New message");
Notification::success("Saved");
Notification::warning("Low disk");
Notification::error("Failed");
Notification::debug("trace:frame");

// Full builder chain.
let _n = Notification::warning("Unsaved changes")
    .with_body("Do you want to save before closing?")
    .with_duration_secs(6.0)               // or .sticky()
    .with_action(10, "Save")
    .with_action(11, "Discard")
    .with_custom_color([0.86, 0.30, 0.78, 1.0])  // override severity accent
    .not_closable()                        // no × button
    .without_icon()
    .without_progress();
```

## Placements

Toasts stack from the anchor edge outward. Newest toasts appear closest to
the anchor; older ones are pushed away.

| Placement | Anchor | Stack grows |
|-----------|--------|-------------|
| `TopRight` (default) | Top-right corner | Downward |
| `TopLeft` | Top-left corner | Downward |
| `BottomRight` | Bottom-right corner | Upward |
| `BottomLeft` | Bottom-left corner | Upward |
| `TopCenter` | Top-center | Downward |
| `BottomCenter` | Bottom-center | Upward |

> **Custom titlebars:** `notifications` uses `io.display_size()` for anchor
> math and does not know about host-window chrome. If your app draws a
> custom titlebar (`borderless_window`, ~28 px by default), raise the top
> margin to clear it — e.g. `cfg.margin = [16.0, 44.0]` for `TopRight`.

## Severities

| Severity | Default Color | Icon | Typical Use |
|----------|---------------|------|-------------|
| `Info` | Blue | Filled circle + "i" | General info |
| `Success` | Green | Filled circle + checkmark | Operation succeeded |
| `Warning` | Amber | Filled triangle + "!" | Caution, non-fatal |
| `Error` | Red | Filled circle + "×" | Failure / error |
| `Debug` | Gray | Outlined circle + "…" | Developer diagnostic |

All severity accents are resolved from the active `NotificationColors`
palette. Individual toasts can override the accent via
`with_custom_color([r, g, b, a])`.

## Themes

Themes come from the unified [`Theme`](theme.md) enum — each variant has a
matching `NotificationColors` palette tuned to the rest of the stack. For a
fully custom look use `CenterConfig::with_colors(NotificationColors { .. })`.

## API Reference

### `NotificationCenter`

| Method | Description |
|--------|-------------|
| `new()` / `default()` | Center with default config (TopRight, Fade, 5 visible). |
| `with_config(cfg)` | Center with explicit `CenterConfig`. |
| `push(n) -> u64` | Push a notification; returns its id. |
| `dismiss(id)` | Start exit animation for a specific toast. |
| `dismiss_all()` | Dismiss every active toast. |
| `count()` | Active count (including fading-out). |
| `config()` / `config_mut()` | Read / write configuration at runtime. |
| `render(ui, dt) -> Vec<NotificationEvent>` | Advance state, draw stack, collect events. |

### `NotificationEvent`

| Variant | Fires when |
|---------|------------|
| `Dismissed(u64)` | Toast was removed (timer expired, `×` clicked, or action click). |
| `ActionClicked { id, action_id }` | An action button inside a toast was clicked. |
| `Clicked(u64)` | The toast body (not a button) was clicked. |

### `CenterConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `placement` | `Placement` | `TopRight` | Stack anchor. |
| `max_visible` | `usize` | `5` | Cap on visible toasts at once. |
| `spacing` | `f32` | `8.0` | Vertical gap between toasts (px). |
| `margin` | `[f32; 2]` | `[16.0, 16.0]` | `[x, y]` from anchor edge. |
| `width` | `f32` | `340.0` | Toast width (px). |
| `padding` | `[f32; 2]` | `[12.0, 10.0]` | Inner padding (px). |
| `rounding` | `f32` | `6.0` | Corner radius (px). |
| `accent_strip` | `f32` | `4.0` | Leading accent-strip width (px). |
| `progress_height` | `f32` | `3.0` | Bottom progress-bar height (px). |
| `animation` | `AnimationKind` | `Fade` | `Fade` / `SlideIn` / `None`. |
| `animation_duration` | `f32` | `0.25` | Seconds for enter/exit anim. |
| `pause_on_hover` | `bool` | `true` | Freeze timer while hovered. |
| `theme` | `Theme` | `Dark` | Color theme. |
| `colors_override` | `Option<Box<NotificationColors>>` | `None` | Custom palette. |

### `Notification` Builder

| Method | Description |
|--------|-------------|
| `info(title)` / `success(title)` / `warning(title)` / `error(title)` / `debug(title)` | Severity constructors. |
| `with_body(text)` | Body text (word-wrapped, shown under the title). |
| `with_duration_secs(f32)` | Auto-dismiss after N seconds. |
| `sticky()` | Disable auto-dismiss (user must close). |
| `with_action(id, label)` | Append an action button — returns `ActionClicked { action_id: id }`. |
| `with_custom_color([r,g,b,a])` | Override the severity's default accent color. |
| `not_closable()` | Hide the `×` close button. |
| `without_icon()` | Hide the leading severity icon. |
| `without_progress()` | Hide the bottom progress bar (timer still ticks). |

## Integration with `app_window`

```rust,ignore
use dear_imgui_custom_mod::app_window::{AppHandler, AppState};
use dear_imgui_custom_mod::notifications::{
    Notification, NotificationCenter, NotificationEvent,
};
use dear_imgui_rs::Ui;

struct MyApp {
    center:    NotificationCenter,
    last_time: std::time::Instant,
}

impl AppHandler for MyApp {
    fn render(&mut self, ui: &Ui, _state: &mut AppState) {
        let now = std::time::Instant::now();
        let dt = (now - self.last_time).as_secs_f32();
        self.last_time = now;

        // ... your UI ...
        if ui.button("Save") {
            self.center.push(Notification::success("Saved"));
        }

        // Render last — toasts stay on top.
        for ev in self.center.render(ui, dt) {
            if let NotificationEvent::ActionClicked { id, action_id } = ev {
                let _ = (id, action_id);
            }
        }
    }
}
```

See [`examples/demo_app_window.rs`](../examples/demo_app_window.rs) for the
full integration: all 5 severities, every placement, live animation switch,
burst / dismiss-all, sticky + custom-color toasts, and theme sync.

## Tests

`notifications` ships with unit tests for:
- `push` assigns monotonic unique ids
- `dismiss` / `dismiss_all` set the exit flag
- Builder methods (`sticky`, `with_custom_color`, `with_action`)
- `Severity` labels and `Placement` orientation helpers

Run `cargo test --features=notifications` to execute them.
