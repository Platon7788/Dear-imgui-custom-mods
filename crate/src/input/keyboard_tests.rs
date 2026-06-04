//! Unit tests for the layout-independent keyboard helpers.
//!
//! Split out of `keyboard.rs` to keep that file under the 500-line cap
//! (CLAUDE.md). This is a child module of `keyboard`, so `use super::*`
//! reaches both the public helpers and the private `is_numpad_control_key`.

use super::*;

/// Shorthand: physical `KeyCode` → mapped Dear ImGui `Key`.
fn map(code: KeyCode) -> Option<Key> {
    physical_key_to_imgui(PhysicalKey::Code(code))
}

#[test]
fn letters_map_to_imgui_keys() {
    assert_eq!(map(KeyCode::KeyA), Some(Key::A));
    assert_eq!(map(KeyCode::KeyC), Some(Key::C));
    assert_eq!(map(KeyCode::KeyZ), Some(Key::Z));
}

/// Every physical letter `KeyA`..`KeyZ` maps to its matching `Key::A`..`Key::Z`,
/// and the 26 results are all distinct (no accidental collision in the match).
#[test]
fn all_letters_map_distinctly() {
    let pairs = [
        (KeyCode::KeyA, Key::A),
        (KeyCode::KeyB, Key::B),
        (KeyCode::KeyC, Key::C),
        (KeyCode::KeyD, Key::D),
        (KeyCode::KeyE, Key::E),
        (KeyCode::KeyF, Key::F),
        (KeyCode::KeyG, Key::G),
        (KeyCode::KeyH, Key::H),
        (KeyCode::KeyI, Key::I),
        (KeyCode::KeyJ, Key::J),
        (KeyCode::KeyK, Key::K),
        (KeyCode::KeyL, Key::L),
        (KeyCode::KeyM, Key::M),
        (KeyCode::KeyN, Key::N),
        (KeyCode::KeyO, Key::O),
        (KeyCode::KeyP, Key::P),
        (KeyCode::KeyQ, Key::Q),
        (KeyCode::KeyR, Key::R),
        (KeyCode::KeyS, Key::S),
        (KeyCode::KeyT, Key::T),
        (KeyCode::KeyU, Key::U),
        (KeyCode::KeyV, Key::V),
        (KeyCode::KeyW, Key::W),
        (KeyCode::KeyX, Key::X),
        (KeyCode::KeyY, Key::Y),
        (KeyCode::KeyZ, Key::Z),
    ];
    let mut seen = Vec::with_capacity(pairs.len());
    for (code, key) in pairs {
        assert_eq!(map(code), Some(key), "{code:?} should map to {key:?}");
        assert!(!seen.contains(&key), "duplicate mapping for {key:?}");
        seen.push(key);
    }
    assert_eq!(seen.len(), 26);
}

#[test]
fn function_keys_map() {
    let pairs = [
        (KeyCode::F1, Key::F1),
        (KeyCode::F2, Key::F2),
        (KeyCode::F3, Key::F3),
        (KeyCode::F4, Key::F4),
        (KeyCode::F5, Key::F5),
        (KeyCode::F6, Key::F6),
        (KeyCode::F7, Key::F7),
        (KeyCode::F8, Key::F8),
        (KeyCode::F9, Key::F9),
        (KeyCode::F10, Key::F10),
        (KeyCode::F11, Key::F11),
        (KeyCode::F12, Key::F12),
    ];
    for (code, key) in pairs {
        assert_eq!(map(code), Some(key), "{code:?} should map to {key:?}");
    }
}

#[test]
fn digit_keys_map_to_key_variants() {
    let pairs = [
        (KeyCode::Digit0, Key::Key0),
        (KeyCode::Digit1, Key::Key1),
        (KeyCode::Digit2, Key::Key2),
        (KeyCode::Digit3, Key::Key3),
        (KeyCode::Digit4, Key::Key4),
        (KeyCode::Digit5, Key::Key5),
        (KeyCode::Digit6, Key::Key6),
        (KeyCode::Digit7, Key::Key7),
        (KeyCode::Digit8, Key::Key8),
        (KeyCode::Digit9, Key::Key9),
    ];
    for (code, key) in pairs {
        assert_eq!(map(code), Some(key), "{code:?} should map to {key:?}");
    }
}

#[test]
fn numpad_digits_map_to_keypad_variants() {
    let pairs = [
        (KeyCode::Numpad0, Key::Keypad0),
        (KeyCode::Numpad1, Key::Keypad1),
        (KeyCode::Numpad2, Key::Keypad2),
        (KeyCode::Numpad3, Key::Keypad3),
        (KeyCode::Numpad4, Key::Keypad4),
        (KeyCode::Numpad5, Key::Keypad5),
        (KeyCode::Numpad6, Key::Keypad6),
        (KeyCode::Numpad7, Key::Keypad7),
        (KeyCode::Numpad8, Key::Keypad8),
        (KeyCode::Numpad9, Key::Keypad9),
    ];
    for (code, key) in pairs {
        assert_eq!(map(code), Some(key), "{code:?} should map to {key:?}");
    }
    // Distinct from top-row Key0..9.
    assert_ne!(map(KeyCode::Numpad5), map(KeyCode::Digit5));
}

/// Top-row digits and numpad digits must never collapse to the same `Key`
/// for any digit 0..9 — apps rely on telling `Key5` from `Keypad5`.
#[test]
fn numpad_and_toprow_digits_never_alias() {
    let toprow = [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    let numpad = [
        KeyCode::Numpad0,
        KeyCode::Numpad1,
        KeyCode::Numpad2,
        KeyCode::Numpad3,
        KeyCode::Numpad4,
        KeyCode::Numpad5,
        KeyCode::Numpad6,
        KeyCode::Numpad7,
        KeyCode::Numpad8,
        KeyCode::Numpad9,
    ];
    for (t, n) in toprow.into_iter().zip(numpad) {
        assert_ne!(map(t), map(n), "{t:?} aliased {n:?}");
    }
}

#[test]
fn numpad_operators_map() {
    assert_eq!(map(KeyCode::NumpadAdd), Some(Key::KeypadAdd));
    assert_eq!(map(KeyCode::NumpadSubtract), Some(Key::KeypadSubtract));
    assert_eq!(map(KeyCode::NumpadMultiply), Some(Key::KeypadMultiply));
    assert_eq!(map(KeyCode::NumpadDivide), Some(Key::KeypadDivide));
    assert_eq!(map(KeyCode::NumpadDecimal), Some(Key::KeypadDecimal));
    assert_eq!(map(KeyCode::NumpadEnter), Some(Key::KeypadEnter));
    assert_eq!(map(KeyCode::NumpadEqual), Some(Key::KeypadEqual));
}

#[test]
fn navigation_keys_map() {
    let pairs = [
        (KeyCode::Escape, Key::Escape),
        (KeyCode::Tab, Key::Tab),
        (KeyCode::Enter, Key::Enter),
        (KeyCode::Space, Key::Space),
        (KeyCode::Backspace, Key::Backspace),
        (KeyCode::ArrowUp, Key::UpArrow),
        (KeyCode::ArrowDown, Key::DownArrow),
        (KeyCode::ArrowLeft, Key::LeftArrow),
        (KeyCode::ArrowRight, Key::RightArrow),
        (KeyCode::Home, Key::Home),
        (KeyCode::End, Key::End),
        (KeyCode::PageUp, Key::PageUp),
        (KeyCode::PageDown, Key::PageDown),
        (KeyCode::Insert, Key::Insert),
        (KeyCode::Delete, Key::Delete),
    ];
    for (code, key) in pairs {
        assert_eq!(map(code), Some(key), "{code:?} should map to {key:?}");
    }
}

/// `NumpadEnter` (the keypad Return) must NOT collapse onto the main `Enter`
/// key — they are distinct Dear ImGui keys (`KeypadEnter` vs `Enter`).
#[test]
fn numpad_enter_distinct_from_main_enter() {
    assert_eq!(map(KeyCode::NumpadEnter), Some(Key::KeypadEnter));
    assert_eq!(map(KeyCode::Enter), Some(Key::Enter));
    assert_ne!(map(KeyCode::NumpadEnter), map(KeyCode::Enter));
}

#[test]
fn unmapped_key_returns_none() {
    // Keys outside the covered set fall through to `None`.
    assert_eq!(map(KeyCode::PrintScreen), None);
    assert_eq!(map(KeyCode::ScrollLock), None);
    assert_eq!(map(KeyCode::Pause), None);
    // Modifier keys are intentionally unmapped: `try_inject_ctrl_alt_shortcut`
    // relies on these returning `None` so a bare Ctrl/Alt press is never
    // re-injected as a spurious shortcut key.
    assert_eq!(map(KeyCode::ControlLeft), None);
    assert_eq!(map(KeyCode::AltLeft), None);
    assert_eq!(map(KeyCode::ShiftLeft), None);
    assert_eq!(map(KeyCode::SuperLeft), None);
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
    // Every navigation / control NamedKey that NumLock-off numpads emit.
    for named in [
        NamedKey::Enter,
        NamedKey::Delete,
        NamedKey::Insert,
        NamedKey::Home,
        NamedKey::End,
        NamedKey::PageUp,
        NamedKey::PageDown,
        NamedKey::ArrowUp,
        NamedKey::ArrowDown,
        NamedKey::ArrowLeft,
        NamedKey::ArrowRight,
    ] {
        assert!(
            is_numpad_control_key(&WKey::Named(named)),
            "{named:?} should be a numpad control key"
        );
    }
    // Character keys produce text and must NOT be treated as control keys.
    assert!(!is_numpad_control_key(&WKey::Character("1".into())));
    assert!(!is_numpad_control_key(&WKey::Character("+".into())));
    assert!(!is_numpad_control_key(&WKey::Character(".".into())));
    // A NamedKey that is NOT in the control set (e.g. Space) is also text-like.
    assert!(!is_numpad_control_key(&WKey::Named(NamedKey::Space)));
}
