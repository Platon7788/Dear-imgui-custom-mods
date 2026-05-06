# Upstream issue draft — `dear-app::on_event` should support
event consumption

> Готовый текст для подачи issue/PR в upstream репозиторий
> [`dear-app`](https://crates.io/crates/dear-app). Скопировать в форму
> issue. Не часть публичной документации крейта — лежит здесь как
> tracking-артефакт пока issue не подан.

---

## Title

`on_event` callback should be able to suppress the default platform handler (return `bool`)

## Body

### Problem

`AppBuilder::on_event` is called before `dear_imgui_winit::WinitPlatform::handle_event`,
which is exactly the right place to intercept events for keyboard / IME
fixes (layout-independent shortcuts, numpad text injection, IME commit
forwarding). But the callback signature is

```rust
FnMut(&winit::event::Event<()>, &Arc<Window>, &mut imgui::Context)
```

— there is no return value, so the callback **cannot tell the runner
to skip** `WinitPlatform::handle_event` for this event. The platform
handler then runs unconditionally, undoing or doubling our injection.

### Concrete impact

Hosts using non-Latin keyboard layouts (Cyrillic / Greek / CJK etc.)
need a "physical-scan-code → ImGui Key" injection because
`dear-imgui-winit` derives Dear ImGui keys from the *logical* (post-
layout) key. On a Russian keyboard, `KeyC` arrives as Cyrillic `с`,
which neither maps to `Key::C` (so `Ctrl+C` doesn't fire as a
shortcut) nor reaches `InputText` as a typeable character via
the default path.

The fix is straightforward: in the runner's event handler, when our
`on_event` callback detects a Cyrillic-layout `Ctrl+C`, it injects
`Key::C` directly into `Context::io` and **suppresses**
`WinitPlatform::handle_event` for that event. With the current API
we can do the first part but not the second — so the platform handler
still runs and `add_input_character('с')` gets called, and the typed
'с' replaces the user's selection right after `Ctrl+C` copies it.

This is a hard blocker for Russian / Ukrainian / Belarusian /
non-Latin users in any text-editing-heavy `dear-app` host.

### Proposed change

Make `on_event` return `bool` (default `false`):

```rust
pub fn on_event<
    F: FnMut(&winit::event::Event<()>, &Arc<Window>, &mut imgui::Context) -> bool + 'static,
>(
    mut self,
    f: F,
) -> Self {
    self.cbs.on_event = Some(Box::new(f));
    self
}
```

And in the runner's event-dispatch sites:

```rust
// crate/src/lib.rs ~ lines 925, 985
let consumed = if let Some(cb) = self.cbs.on_event.as_mut() {
    cb(&full_event, &window.window, &mut window.imgui.context)
} else {
    false
};
if !consumed {
    window.imgui.platform.handle_event(
        &mut window.imgui.context,
        &window.window,
        &full_event,
    );
}
```

`true` from the callback ⇒ the event is **fully consumed**; the
platform handler is skipped. `false` ⇒ unchanged behaviour
(backward-compatible default).

This is a single-line breaking change to the type signature but the
semantics are additive: consumers that ignore the return value can
just write `|_, _, _| false` (or hosts with a forwarding-only
callback can keep returning `()` if you want to make this strictly
backward-compatible by introducing a *new* method `on_event_filter`
or similar instead).

### Workarounds tried

- Injecting via `io.add_key_event` and accepting the doubled `add_input_character`:
  doesn't work — the Cyrillic char replaces selection right after copy.
- Pre-modifying the event before `on_event` fires: `on_event` receives
  `&Event` (immutable); cannot mutate.
- Post-frame fixup (clearing `io.input_queue` of unwanted chars): no
  public API in `dear-imgui-rs` for that, and would race with legitimate
  user input.
- Forking dear-app: feasible but disconnects us from upstream fixes.

A `bool` return on `on_event` is the only clean fix.

### Reference

The downstream library that needs this is
[`dear-imgui-custom-mod`](https://github.com/Platon7788/dear-imgui-custom-mod)
— `crate/src/input/keyboard.rs` has the full implementation
(physical-scan-code injection + IME commit + numpad text), but it can
only be wired by hosts with full event-loop control. With the proposed
change, `dear-app` users could opt in via `on_event` returning `true`
when our injection fires.

Happy to send a PR if the proposed signature is acceptable.
