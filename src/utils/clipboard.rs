//! Clipboard, physical-key helpers, and keyboard layout management.

/// Returns `true` if the physical key with the given VK code is currently held.
///
/// On Windows this bypasses the logical-key layer so that hotkeys work
/// regardless of the active keyboard layout (Russian, Greek, etc.).
/// On other platforms always returns `false`.
pub(crate) fn vk_down(vk: i32) -> bool {
    #[cfg(target_os = "windows")]
    {
        unsafe extern "system" {
            fn GetAsyncKeyState(vkey: i32) -> i16;
        }
        // SAFETY: GetAsyncKeyState is a safe Win32 API. Bit 15 = key down.
        unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = vk;
        false
    }
}

// Virtual key codes for common keys.
pub(crate) const VK_C: i32 = 0x43;
pub(crate) const VK_A: i32 = 0x41;
pub(crate) const VK_F: i32 = 0x46;
pub(crate) const VK_G: i32 = 0x47;
pub(crate) const VK_Y: i32 = 0x59;
pub(crate) const VK_Z: i32 = 0x5A;

/// Returns `true` if the physical **C key** (VK_C / 0x43) is currently held.
pub(crate) fn c_key_down_physical() -> bool {
    vk_down(VK_C)
}

/// Copy text to the system clipboard via Dear ImGui's C API.
///
/// NUL bytes are stripped (Dear ImGui treats them as string terminators).
pub(crate) fn set_clipboard(text: &str) {
    let sanitized: String = text.chars().filter(|&c| c != '\0').collect();
    if let Ok(c_str) = std::ffi::CString::new(sanitized) {
        unsafe { dear_imgui_rs::sys::igSetClipboardText(c_str.as_ptr()); }
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
        unsafe { ActivateKeyboardLayout(en_us, 0); }
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
            unsafe { ActivateKeyboardLayout(saved as usize, 0); }
            SAVED_LAYOUT.store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
