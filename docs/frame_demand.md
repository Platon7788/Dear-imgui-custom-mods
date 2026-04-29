# frame_demand

Crate-wide thread-local *render-budget* signal — the bridge between
animation widgets and event-driven hosts.

## What it is

A single-page module (`src/frame_demand.rs`, ~110 LoC) exposing three
public functions and one `thread_local!` `Cell<u8>`:

```rust
pub fn request(frames: u8);  // bump the demand to max(current, frames)
pub fn take()    -> u8;      // read + reset to 0
pub fn peek()    -> u8;      // read without resetting (diagnostics)
```

Storage cost: zero allocations, one branch + a `Cell::set` per call.

## Why it exists

In an event-driven render loop (the default for
[`app_window`](app_window.md)), the host **does not repaint** while
the user is reading the screen. CPU/GPU usage drops to ≈ 0 %. But there
are widgets that have **ongoing work without input**:

- Notification fade-in / fade-out animations
- Auto-dismiss countdown timers
- Modal scale / slide tweens (e.g. `confirm_dialog` if you add one)
- Custom user animations

Those widgets need a way to say *"I'm not done — give me one more frame
to finish the tween"*. `frame_demand::request(1)` is that signal.

## Usage from a widget

Inside any per-frame `render(ui, …)` method that has an animation in
flight, drop a single line:

```rust
fn render(&mut self, ui: &Ui) {
    if self.tween.is_running() {
        crate::frame_demand::request(1);
    }
    // …draw…
}
```

Calls **accumulate by max**, not sum — spamming `request(1)` every
render is safe and idempotent. `request(0)` is a no-op. The `u8` cap
(255) is plenty since any animation longer than ~4 s on 60 Hz should
re-arm itself per-frame anyway.

## Usage from user code via `AppState`

For application-level code that already holds an `&mut AppState`, the
ergonomic alias [`AppState::keep_alive(frames)`](app_window.md)
forwards to `frame_demand::request`:

```rust
impl AppHandler for MySplash {
    fn render(&mut self, ui: &Ui, state: &mut AppState) {
        state.keep_alive(1);                  // animation in flight
        // …draw progress bar…
    }
}
```

Both paths land in the same thread-local — no double-signal, no race.

## Usage from a host

`app_window/gpu/mod.rs::render_frame` reads it once per frame, **after**
the user's `handler.render()` returns:

```rust
let demanded   = crate::frame_demand::take();
let want_text  = ui.io().want_text_input();   // active InputText cursor
let mut keep   = demanded;
if want_text { keep = keep.max(1); }
if keep > 0 { gpu.pending_frames = gpu.pending_frames.max(keep); }
```

The non-zero `pending_frames` budget tells `about_to_wait` to re-arm
`Window::request_redraw()` on the next iteration of the event loop and
fall back to `ControlFlow::Wait` only when the budget is exhausted.

Continuous-render hosts (`RenderMode::Continuous`, game-style) ignore
the value entirely — `take()` is a cheap no-op when nothing called
`request`, so widgets do not need to know which scheduling mode is
active.

## Why `thread_local!` and not `&mut AppState`

Widgets receive `(&Ui, …)` and do not own the host's state. A free
function with thread-local storage:

- Keeps the call site terse (`frame_demand::request(1);` is one line).
- Works from any widget regardless of which host (or no host) drives it.
- Costs nothing on the hot path — single `Cell::set`, no atomic.
- Has no global mutable static visible to library users.

A `&mut AppState` would force every widget signature in the crate to
plumb the state through, conflicting with library independence
(`notifications` does not depend on `app_window`).

## Tests

Three unit-tests in the same file:

- `request_then_take` — basic round-trip, `peek` doesn't reset.
- `request_uses_max` — repeated `request(N)` calls saturate by max.
- `request_zero_is_noop` — `request(0)` does not perturb the value.

## Public API

| Function | Purpose |
|----------|---------|
| `request(frames: u8)` | Bump the per-frame demand to `max(current, frames)`. Idempotent across calls within one frame. |
| `take() -> u8` | Read the current demand and reset to 0. Hosts call once per frame after the user's render. |
| `peek() -> u8` | Read without resetting — useful for diagnostics or logging. |

The module is exposed at `dear_imgui_custom_mod::frame_demand` and is
**always compiled** (no feature gate) — it is shared infrastructure
used by `notifications` and any future animated widget.

## Related

- [`app_window`](app_window.md) — event-driven host that consumes
  `frame_demand` signals; `RenderMode` controls scheduling strategy.
- [`notifications`](notifications.md) — first internal consumer; calls
  `frame_demand::request(1)` while toasts are alive (animations or
  countdown timers).
