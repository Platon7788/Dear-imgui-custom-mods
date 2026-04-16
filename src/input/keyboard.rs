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
/// - Function keys `F1`..`F12`
/// - Top-row digits `Digit0`..`Digit9` (mapped to the Dear ImGui `Key0`..`Key9` variants)
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
        KeyCode::KeyA => Key::A, KeyCode::KeyB => Key::B, KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D, KeyCode::KeyE => Key::E, KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G, KeyCode::KeyH => Key::H, KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J, KeyCode::KeyK => Key::K, KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M, KeyCode::KeyN => Key::N, KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P, KeyCode::KeyQ => Key::Q, KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S, KeyCode::KeyT => Key::T, KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V, KeyCode::KeyW => Key::W, KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y, KeyCode::KeyZ => Key::Z,
        // Function keys
        KeyCode::F1  => Key::F1,  KeyCode::F2  => Key::F2,  KeyCode::F3  => Key::F3,
        KeyCode::F4  => Key::F4,  KeyCode::F5  => Key::F5,  KeyCode::F6  => Key::F6,
        KeyCode::F7  => Key::F7,  KeyCode::F8  => Key::F8,  KeyCode::F9  => Key::F9,
        KeyCode::F10 => Key::F10, KeyCode::F11 => Key::F11, KeyCode::F12 => Key::F12,
        // Top-row digits (Key0..9 — distinct from Keypad0..9)
        KeyCode::Digit0 => Key::Key0, KeyCode::Digit1 => Key::Key1,
        KeyCode::Digit2 => Key::Key2, KeyCode::Digit3 => Key::Key3,
        KeyCode::Digit4 => Key::Key4, KeyCode::Digit5 => Key::Key5,
        KeyCode::Digit6 => Key::Key6, KeyCode::Digit7 => Key::Key7,
        KeyCode::Digit8 => Key::Key8, KeyCode::Digit9 => Key::Key9,
        // Navigation / editing
        KeyCode::Escape    => Key::Escape,
        KeyCode::Tab       => Key::Tab,
        KeyCode::Enter     => Key::Enter,
        KeyCode::Space     => Key::Space,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::ArrowUp    => Key::UpArrow,
        KeyCode::ArrowDown  => Key::DownArrow,
        KeyCode::ArrowLeft  => Key::LeftArrow,
        KeyCode::ArrowRight => Key::RightArrow,
        KeyCode::Home      => Key::Home,
        KeyCode::End       => Key::End,
        KeyCode::PageUp    => Key::PageUp,
        KeyCode::PageDown  => Key::PageDown,
        KeyCode::Insert    => Key::Insert,
        KeyCode::Delete    => Key::Delete,
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

/// If a modifier (Ctrl or Alt) is held AND the physical key maps to a Dear ImGui
/// [`Key`], inject the key event directly on the physical code.
///
/// Solves the non-Latin-layout shortcut problem: on a Russian layout
/// `dear-imgui-winit` sees the Cyrillic character "с" for the physical `C` key
/// and neither maps it to `Key::C` nor treats it as a shortcut — it just
/// injects "с" into the focused input field while the user is trying to copy.
/// This function injects `Key::C` based on the physical scan code so
/// `Ctrl+C` (and friends) work on any layout.
///
/// Call BEFORE forwarding the event to `platform.handle_event(...)`.
/// Returns `true` if the shortcut was injected; caller must then skip the
/// platform forward to avoid duplicating the raw character into input fields.
pub fn try_inject_ctrl_alt_shortcut(io: &mut Io, event: &KeyEvent) -> bool {
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_map_to_imgui_keys() {
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::KeyA)), Some(Key::A));
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::KeyC)), Some(Key::C));
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::KeyZ)), Some(Key::Z));
    }

    #[test]
    fn function_keys_map() {
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::F1)),  Some(Key::F1));
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::F5)),  Some(Key::F5));
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::F12)), Some(Key::F12));
    }

    #[test]
    fn digit_keys_map_to_key_variants() {
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::Digit0)), Some(Key::Key0));
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::Digit5)), Some(Key::Key5));
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::Digit9)), Some(Key::Key9));
    }

    #[test]
    fn navigation_keys_map() {
        assert_eq!(
            physical_key_to_imgui(PhysicalKey::Code(KeyCode::Escape)),
            Some(Key::Escape)
        );
        assert_eq!(
            physical_key_to_imgui(PhysicalKey::Code(KeyCode::ArrowLeft)),
            Some(Key::LeftArrow)
        );
        assert_eq!(
            physical_key_to_imgui(PhysicalKey::Code(KeyCode::Home)),
            Some(Key::Home)
        );
        assert_eq!(
            physical_key_to_imgui(PhysicalKey::Code(KeyCode::Delete)),
            Some(Key::Delete)
        );
    }

    #[test]
    fn unmapped_key_returns_none() {
        // PrintScreen is not in the covered set.
        assert_eq!(physical_key_to_imgui(PhysicalKey::Code(KeyCode::PrintScreen)), None);
    }

    #[test]
    fn unidentified_returns_none() {
        use winit::keyboard::NativeKeyCode;
        assert_eq!(
            physical_key_to_imgui(PhysicalKey::Unidentified(NativeKeyCode::Unidentified)),
            None
        );
    }

    #[test]
    fn numpad_control_key_detection() {
        use winit::keyboard::Key as WKey;
        assert!(is_numpad_control_key(&WKey::Named(NamedKey::Enter)));
        assert!(is_numpad_control_key(&WKey::Named(NamedKey::ArrowUp)));
        assert!(is_numpad_control_key(&WKey::Named(NamedKey::PageDown)));
        // Character keys are NOT control keys — they produce text.
        assert!(!is_numpad_control_key(&WKey::Character("1".into())));
        assert!(!is_numpad_control_key(&WKey::Character("+".into())));
    }
}
