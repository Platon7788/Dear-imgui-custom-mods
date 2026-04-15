//! Platform-specific helpers for borderless windows.
//!
//! # Windows — DWM dark title bar
//!
//! Call [`set_titlebar_dark_mode`] **before** making the window visible to avoid
//! the white-flash that occurs when the OS draws the default light chrome first.
//!
//! ```rust,ignore
//! // Extract the HWND from a winit window:
//! #[cfg(windows)]
//! fn hwnd_of(window: &winit::window::Window) -> Option<isize> {
//!     use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
//!     if let Ok(h) = window.window_handle() {
//!         if let RawWindowHandle::Win32(w) = h.as_raw() {
//!             return Some(w.hwnd.get() as isize);
//!         }
//!     }
//!     None
//! }
//!
//! // Then, after creating the window but before set_visible(true):
//! #[cfg(windows)]
//! if let Some(hwnd) = hwnd_of(&window) {
//!     dear_imgui_custom_mod::borderless_window::platform::set_titlebar_dark_mode(hwnd, true);
//! }
//! ```

/// Extract the HWND from a winit window.
///
/// Returns `None` on non-Win32 window handles.
#[cfg(windows)]
pub fn hwnd_of(window: &winit::window::Window) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(h) = window.window_handle()
        && let RawWindowHandle::Win32(w) = h.as_raw()
    {
        return Some(w.hwnd.get());
    }
    None
}

/// Extract the HWND from a winit window (no-op on non-Windows).
#[cfg(not(windows))]
#[allow(unused_variables)]
pub fn hwnd_of(_window: &winit::window::Window) -> Option<isize> { None }

/// Apply (or remove) the DWM immersive-dark-mode attribute on the OS titlebar.
///
/// Even with `with_decorations(false)`, Windows still renders a small OS-level
/// drop-shadow; dark mode prevents the brief white flash on startup.
///
/// Pass `dark = true` for dark mode, `false` to revert to light mode.
///
/// Only available on Windows. On other platforms this is a no-op that compiles away.
#[cfg(windows)]
pub fn set_titlebar_dark_mode(hwnd: isize, dark: bool) {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };
    let value: u32 = if dark { 1 } else { 0 };
    unsafe {
        DwmSetWindowAttribute(
            hwnd as _,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &value as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(windows))]
#[allow(unused_variables)]
/// No-op on non-Windows platforms.
pub fn set_titlebar_dark_mode(_hwnd: isize, _dark: bool) {}
