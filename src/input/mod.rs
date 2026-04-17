//! # input
//!
//! Keyboard / IME helpers for `dear-imgui-winit` integration.
//!
//! These fixes solve three well-known issues when hosting Dear ImGui inside a
//! winit event loop:
//!
//! 1. **Non-Latin keyboard layouts** — on a Russian / French / German layout
//!    `key_without_modifiers()` returns a layout-dependent character (e.g.
//!    Cyrillic "с" for the physical `C` key), which breaks `Ctrl+C/V/X/A/Z`
//!    because `to_imgui_key()` cannot map it and the raw character is injected
//!    into the active input field instead of being recognised as a shortcut.
//! 2. **Numpad digits** — `dear-imgui-winit` maps numpad digits to
//!    `Key::Keypad0..9` which Dear ImGui treats as navigation, not text input,
//!    so numpad-typed digits never appear in the focused text field.
//! 3. **IME (Input Method Editor)** — `dear-imgui-winit` ignores
//!    `WindowEvent::Ime` entirely, which means CJK / dead-key composition never
//!    commits to the input field.
//!
//! Usage pattern inside `window_event(...)`:
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::input::keyboard::*;
//!
//! match event {
//!     WindowEvent::KeyboardInput { event: ref ke, .. } => {
//!         let io = context.io_mut();
//!         if try_inject_numpad_text(io, ke)             { return; }
//!         if try_inject_ctrl_alt_shortcut(io, ke)       { return; }
//!         // forward to platform afterwards:
//!         // platform.handle_event(...);
//!         // reinforce_physical_key_state(io, ke);
//!     }
//!     WindowEvent::Ime(Ime::Commit(text)) => {
//!         inject_ime_commit(context.io_mut(), text);
//!         return;
//!     }
//!     _ => {}
//! }
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod keyboard;

pub use keyboard::{
    inject_ime_commit, physical_key_to_imgui, reinforce_physical_key_state,
    try_inject_ctrl_alt_shortcut, try_inject_numpad_text,
};
