//! Clipboard write helper + Windows keyboard-layout switching.
//!
//! Layout-independent shortcut handling lives in [`crate::input::keyboard`]
//! and is wired in by `app_window` (and any host that mirrors that
//! pattern in their `WindowEvent::KeyboardInput` arm). The per-widget
//! VK-fallback code that used to live here was removed — see commit
//! history before 2026-04-29 for context.

// Module-wide `dead_code` allow because `activate_english_layout` /
// `restore_keyboard_layout` (lines below the `#[cfg(target_os =
// "windows")]` gate) become invisible to the linker on non-Windows
// targets — Rust then warns the windows-only `static` and unsafe FFI
// bodies are never used. The three functions defined here are all
// genuinely used (set_clipboard from disasm_view / hex_viewer /
// virtual_table / force_graph / code_editor; the layout switchers
// from hex_viewer/input). Tighter item-level allows on the windows
// statics would be cleaner — left as a follow-up nit.
#![allow(dead_code)]

/// Copy text to the system clipboard via Dear ImGui's C API.
///
/// NUL bytes are stripped (Dear ImGui treats them as string terminators).
pub(crate) fn set_clipboard(text: &str) {
    let sanitized: String = text.chars().filter(|&c| c != '\0').collect();
    if let Ok(c_str) = std::ffi::CString::new(sanitized) {
        unsafe {
            dear_imgui_rs::sys::igSetClipboardText(c_str.as_ptr());
        }
    }
}

// ── Keyboard Layout Management (Windows only) ──────────────────────────────

/// Saved keyboard layout handle for restore after editing.
#[cfg(target_os = "windows")]
static SAVED_LAYOUT: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Switch to English (US) keyboard layout, saving the current one.
/// Call `restore_keyboard_layout()` when done.
pub(crate) fn activate_english_layout() {
    #[cfg(target_os = "windows")]
    {
        unsafe extern "system" {
            fn GetKeyboardLayout(idThread: u32) -> usize;
            fn ActivateKeyboardLayout(hkl: usize, flags: u32) -> usize;
        }
        let current = unsafe { GetKeyboardLayout(0) };
        SAVED_LAYOUT.store(current as isize, std::sync::atomic::Ordering::Relaxed);
        // 0x0409 = English (US). Low word is LANGID.
        let en_us: usize = 0x0409_0409;
        unsafe {
            ActivateKeyboardLayout(en_us, 0);
        }
    }
}

/// Restore the keyboard layout saved by `activate_english_layout()`.
pub(crate) fn restore_keyboard_layout() {
    #[cfg(target_os = "windows")]
    {
        unsafe extern "system" {
            fn ActivateKeyboardLayout(hkl: usize, flags: u32) -> usize;
        }
        let saved = SAVED_LAYOUT.load(std::sync::atomic::Ordering::Relaxed);
        if saved != 0 {
            unsafe {
                ActivateKeyboardLayout(saved as usize, 0);
            }
            SAVED_LAYOUT.store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
