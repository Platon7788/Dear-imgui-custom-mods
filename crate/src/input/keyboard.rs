//! Layout-independent keyboard helpers for `dear-imgui-winit`.
//!
//! See the module-level docs in [`crate::input`] for rationale.

use dear_imgui_rs::{Io, Key};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, KeyLocation, NamedKey, PhysicalKey};

// ── Physical key → Dear ImGui Key ─────────────────────────────────────────────

/// Map a physical-key scan code to a Dear ImGui [`Key`].
///
/// Layout-independent — always returns the same [`Key`] for a given physical key
/// regardless of the active keyboard layout (US, Russian, German, French, …).
///
/// Covers:
/// - Letters `KeyA`..`KeyZ`
/// - Function keys `F1`..`F12` (Dear ImGui has no F13-F24 variants)
/// - Top-row digits `Digit0`..`Digit9` (mapped to `Key0`..`Key9`)
/// - Numpad digits `Numpad0`..`Numpad9` (mapped to `Keypad0`..`Keypad9`)
/// - Numpad operators: `+`, `-`, `*`, `/`, `=`, `.`, `Enter`
/// - `Escape`, `Tab`, `Enter`, `Space`, `Backspace`
/// - Arrow keys, `Home`, `End`, `PageUp`, `PageDown`, `Insert`, `Delete`
///
/// Returns `None` for unmapped or unidentified keys.
pub fn physical_key_to_imgui(physical: PhysicalKey) -> Option<Key> {
    let code = match physical {
        PhysicalKey::Code(c) => c,
        PhysicalKey::Unidentified(_) => return None,
    };
    Some(match code {
        // Letters
        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        // Function keys
        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        // Top-row digits (Key0..9 — distinct from Keypad0..9)
        KeyCode::Digit0 => Key::Key0,
        KeyCode::Digit1 => Key::Key1,
        KeyCode::Digit2 => Key::Key2,
        KeyCode::Digit3 => Key::Key3,
        KeyCode::Digit4 => Key::Key4,
        KeyCode::Digit5 => Key::Key5,
        KeyCode::Digit6 => Key::Key6,
        KeyCode::Digit7 => Key::Key7,
        KeyCode::Digit8 => Key::Key8,
        KeyCode::Digit9 => Key::Key9,
        // Numpad digits — distinct from Digit0..9 so apps can tell them apart.
        KeyCode::Numpad0 => Key::Keypad0,
        KeyCode::Numpad1 => Key::Keypad1,
        KeyCode::Numpad2 => Key::Keypad2,
        KeyCode::Numpad3 => Key::Keypad3,
        KeyCode::Numpad4 => Key::Keypad4,
        KeyCode::Numpad5 => Key::Keypad5,
        KeyCode::Numpad6 => Key::Keypad6,
        KeyCode::Numpad7 => Key::Keypad7,
        KeyCode::Numpad8 => Key::Keypad8,
        KeyCode::Numpad9 => Key::Keypad9,
        // Numpad operators
        KeyCode::NumpadAdd => Key::KeypadAdd,
        KeyCode::NumpadSubtract => Key::KeypadSubtract,
        KeyCode::NumpadMultiply => Key::KeypadMultiply,
        KeyCode::NumpadDivide => Key::KeypadDivide,
        KeyCode::NumpadDecimal => Key::KeypadDecimal,
        KeyCode::NumpadEqual => Key::KeypadEqual,
        KeyCode::NumpadEnter => Key::KeypadEnter,
        // Navigation / editing
        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Enter => Key::Enter,
        KeyCode::Space => Key::Space,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::ArrowUp => Key::UpArrow,
        KeyCode::ArrowDown => Key::DownArrow,
        KeyCode::ArrowLeft => Key::LeftArrow,
        KeyCode::ArrowRight => Key::RightArrow,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Insert => Key::Insert,
        KeyCode::Delete => Key::Delete,
        _ => return None,
    })
}

// ── Numpad text injection ─────────────────────────────────────────────────────

/// Returns `true` if `key` is a numpad navigation / control key that must NOT be
/// injected as a text character (Enter, arrows, Home/End, etc.).
fn is_numpad_control_key(key: &winit::keyboard::Key) -> bool {
    matches!(
        key,
        winit::keyboard::Key::Named(
            NamedKey::Enter
                | NamedKey::Delete
                | NamedKey::Insert
                | NamedKey::Home
                | NamedKey::End
                | NamedKey::PageUp
                | NamedKey::PageDown
                | NamedKey::ArrowUp
                | NamedKey::ArrowDown
                | NamedKey::ArrowLeft
                | NamedKey::ArrowRight
        )
    )
}

/// Inject numpad-digit text into the Dear ImGui input stream.
///
/// `dear-imgui-winit` maps numpad digits to `Key::Keypad0..9` which Dear ImGui
/// treats as navigation, not text input — so numpad-typed digits never appear
/// in focused text fields. This function bridges the gap.
///
/// Call BEFORE forwarding the event to `platform.handle_event(...)`.
/// Returns `true` if the event was handled; the caller must then NOT forward it.
/// Returns `false` for released keys, non-numpad keys, control keys, and events
/// without printable text.
pub fn try_inject_numpad_text(io: &mut Io, event: &KeyEvent) -> bool {
    if event.location != KeyLocation::Numpad {
        return false;
    }
    if !event.state.is_pressed() {
        return false;
    }
    if is_numpad_control_key(&event.logical_key) {
        return false;
    }
    let Some(txt) = &event.text else { return false };
    if txt.is_empty() {
        return false;
    }
    for ch in txt.chars() {
        io.add_input_character(ch);
    }
    true
}

// ── IME commit injection ──────────────────────────────────────────────────────

/// Inject an IME commit string as Dear ImGui input characters.
///
/// `dear-imgui-winit` ignores `WindowEvent::Ime` events entirely, which means
/// CJK / dead-key composition never reaches the focused input field. Call this
/// from `WindowEvent::Ime(Ime::Commit(text))`.
pub fn inject_ime_commit(io: &mut Io, text: &str) {
    for ch in text.chars() {
        io.add_input_character(ch);
    }
}

// ── Ctrl/Alt shortcut injection (non-Latin layout fix) ────────────────────────

/// Returns `true` if `code` is a letter (`KeyA`..`KeyZ`) or top-row digit
/// (`Digit0`..`Digit9`) — the only physical keys whose *logical* key
/// changes on a non-Latin/non-US layout (Cyrillic letters, AZERTY's
/// shifted digit row, …) and therefore need the physical-scan-code
/// workaround in [`try_inject_ctrl_alt_shortcut`].
///
/// Navigation/control keys (Tab, Escape, Enter, arrows, F-keys, numpad, …)
/// are layout-independent — winit reports the same logical key for them on
/// every layout — so they must NOT be routed through this override. Doing
/// so would needlessly bypass the normal, platform-forwarded path for keys
/// where the workaround provides no benefit, and would silently re-inject
/// Tab/Escape whenever Ctrl or Alt happens to be held (e.g. during an
/// Alt+Tab or Alt+Escape window-switch gesture), duplicating a class of bug
/// that must instead be fixed at the point where those keys are consumed
/// (see the `alt` guards in `code_editor::input`).
fn is_layout_sensitive_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::KeyA
            | KeyCode::KeyB
            | KeyCode::KeyC
            | KeyCode::KeyD
            | KeyCode::KeyE
            | KeyCode::KeyF
            | KeyCode::KeyG
            | KeyCode::KeyH
            | KeyCode::KeyI
            | KeyCode::KeyJ
            | KeyCode::KeyK
            | KeyCode::KeyL
            | KeyCode::KeyM
            | KeyCode::KeyN
            | KeyCode::KeyO
            | KeyCode::KeyP
            | KeyCode::KeyQ
            | KeyCode::KeyR
            | KeyCode::KeyS
            | KeyCode::KeyT
            | KeyCode::KeyU
            | KeyCode::KeyV
            | KeyCode::KeyW
            | KeyCode::KeyX
            | KeyCode::KeyY
            | KeyCode::KeyZ
            | KeyCode::Digit0
            | KeyCode::Digit1
            | KeyCode::Digit2
            | KeyCode::Digit3
            | KeyCode::Digit4
            | KeyCode::Digit5
            | KeyCode::Digit6
            | KeyCode::Digit7
            | KeyCode::Digit8
            | KeyCode::Digit9
    )
}

/// If a modifier (Ctrl or Alt) is held AND the physical key is a letter or
/// top-row digit, inject the key event directly on the physical code.
///
/// Solves the non-Latin-layout shortcut problem: on a Russian layout
/// `dear-imgui-winit` sees the Cyrillic character "с" for the physical `C` key
/// and neither maps it to `Key::C` nor treats it as a shortcut — it just
/// injects "с" into the focused input field while the user is trying to copy.
/// This function injects `Key::C` based on the physical scan code so
/// `Ctrl+C` (and friends) work on any layout.
///
/// Deliberately scoped to letters/digits only (see
/// [`is_layout_sensitive_key`]) — navigation/control keys are layout-independent
/// and must fall through to the normal platform-forwarded path instead.
///
/// Call BEFORE forwarding the event to `platform.handle_event(...)`.
/// Returns `true` if the shortcut was injected; caller must then skip the
/// platform forward to avoid duplicating the raw character into input fields.
pub fn try_inject_ctrl_alt_shortcut(io: &mut Io, event: &KeyEvent) -> bool {
    let PhysicalKey::Code(code) = event.physical_key else {
        return false;
    };
    if !is_layout_sensitive_key(code) {
        return false;
    }
    let Some(imgui_key) = physical_key_to_imgui(event.physical_key) else {
        return false;
    };
    if !io.key_ctrl() && !io.key_alt() {
        return false;
    }
    let pressed = event.state == ElementState::Pressed;
    io.add_key_event(imgui_key, pressed);
    true
}

/// Idempotent re-injection of physical-key state AFTER `platform.handle_event`.
///
/// Dear ImGui deduplicates same-state `add_key_event` calls, so this is safe to
/// call every frame. It ensures correct key-release tracking on non-Latin
/// layouts where the user may release Ctrl before the letter key — without
/// this, the platform-forwarded release event uses the Cyrillic logical key
/// (which does not map to the same Dear ImGui Key), leaving `Key::C` "stuck"
/// in the down state.
pub fn reinforce_physical_key_state(io: &mut Io, event: &KeyEvent) {
    if let Some(imgui_key) = physical_key_to_imgui(event.physical_key) {
        let pressed = event.state == ElementState::Pressed;
        io.add_key_event(imgui_key, pressed);
    }
}

// ── High-level dispatcher ─────────────────────────────────────────────────────

/// One-call layout-independent dispatcher for `WinitPlatform`-based hosts.
///
/// Replaces the boilerplate of:
///
/// 1. Calling [`try_inject_numpad_text`] / [`try_inject_ctrl_alt_shortcut`]
///    on every `WindowEvent::KeyboardInput` and skipping the platform
///    forward when one of them consumes the event.
/// 2. Calling [`inject_ime_commit`] on `WindowEvent::Ime(Ime::Commit(_))`.
/// 3. Calling [`reinforce_physical_key_state`] **after** the platform
///    forward when the helpers did not consume the event.
///
/// Use this from `ApplicationHandler::window_event`:
///
/// ```rust,ignore
/// fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
///     dear_imgui_custom_mod::input::keyboard::dispatch_window_event(
///         &mut gpu.context,
///         &mut gpu.platform,
///         &gpu.window,
///         &event,
///     );
///     match event {
///         WindowEvent::CloseRequested => event_loop.exit(),
///         WindowEvent::RedrawRequested => { /* draw */ }
///         _ => {}
///     }
/// }
/// ```
///
/// **For `dear-app` users** see [`dispatch_dear_app_event`] — `dear-app`
/// does not expose a consumed-flag return from `on_event`, so it cannot
/// fully replicate this dispatcher; the lighter helper installs only the
/// non-destructive injections.
pub fn dispatch_window_event(
    context: &mut dear_imgui_rs::Context,
    platform: &mut dear_imgui_winit::WinitPlatform,
    window: &winit::window::Window,
    event: &winit::event::WindowEvent,
) {
    use winit::event::{Ime, WindowEvent};

    match event {
        WindowEvent::KeyboardInput { event: ke, .. } => {
            let io = context.io_mut();
            if try_inject_numpad_text(io, ke) || try_inject_ctrl_alt_shortcut(io, ke) {
                return;
            }
            platform.handle_window_event(context, window, event);
            // Reinforce only on physical-key release tracking. `add_key_event`
            // is idempotent, so this is a no-op when the platform layer
            // already injected the matching state.
            reinforce_physical_key_state(context.io_mut(), ke);
        }
        WindowEvent::Ime(Ime::Commit(text)) => {
            inject_ime_commit(context.io_mut(), text);
        }
        _ => {
            platform.handle_window_event(context, window, event);
        }
    }
}

/// Layout-independent shortcut fix for [`dear-app`]-style hosts.
///
/// `dear-app::AppBuilder::on_event` does not allow the callback to suppress
/// the subsequent `platform.handle_event` call, so the [full
/// dispatcher][`dispatch_window_event`] cannot be used there. This helper
/// installs only the **non-destructive** half of the fix:
///
/// - On `WindowEvent::KeyboardInput`, when Ctrl or Alt is held, injects
///   `Key::C` / `Key::V` / `Key::X` / `Key::A` / `Key::Z` / … based on the
///   **physical scan code** so Dear ImGui recognises the shortcut on RU /
///   DE / FR / any non-Latin keyboard layout.
///
/// This is safe to call alongside `dear-imgui-winit`'s own keyboard
/// handler — `io.add_key_event` is idempotent against the same (key, state)
/// pair, and the Cyrillic / dead-key logical key falls through
/// `winit_key_to_imgui_key` as `None`, so there is no competing key event.
///
/// `try_inject_numpad_text` is **not** called here: `dear-imgui-winit` 0.11
/// already adds numpad digit characters via `event.text`, so calling it
/// would double-inject the digit. Hosts who roll their own winit loop and
/// suppress the platform forward should use [`dispatch_window_event`]
/// instead — that path is structurally safe.
///
/// ### Known residual quirk
///
/// On non-Latin layouts the Cyrillic character ("с" for Ctrl+C, "в" for
/// Ctrl+V, …) is still added to the input character queue by
/// `dear-imgui-winit::handle_keyboard_input` because it does not filter
/// `event.text` against `io.key_ctrl`. The shortcut fires (Ctrl+C copies,
/// Ctrl+V pastes, etc.), but a stray Cyrillic letter may also land in the
/// focused `InputText`. Fixing this requires either an upstream patch in
/// `dear-imgui-winit` or a `consumed: bool` return from
/// `dear-app::on_event`. Tracked in `_CONTEXT.md` (session 044+).
///
/// ### Wiring
///
/// ```rust,ignore
/// AppBuilder::new()
///     .on_event(move |event, _window, ctx| {
///         dear_imgui_custom_mod::input::keyboard::dispatch_dear_app_event(ctx, event);
///         // … your own per-event handling …
///     })
///     .run().unwrap();
/// ```
///
/// `chrome::Chrome::on_event` already calls this internally, so apps using
/// the chrome wrapper do not need to wire it manually.
///
/// [`dear-app`]: https://crates.io/crates/dear-app
pub fn dispatch_dear_app_event(
    context: &mut dear_imgui_rs::Context,
    event: &winit::event::Event<()>,
) {
    let winit::event::Event::WindowEvent { event: we, .. } = event else {
        return;
    };
    if let winit::event::WindowEvent::KeyboardInput { event: ke, .. } = we {
        let _ = try_inject_ctrl_alt_shortcut(context.io_mut(), ke);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
//
// The test suite lives in a sibling file to keep this module under the
// 500-line cap (CLAUDE.md). `tests` is still a child of `keyboard`, so its
// `use super::*` reaches the private `is_numpad_control_key` helper directly.

#[cfg(test)]
#[path = "keyboard_tests.rs"]
mod tests;
