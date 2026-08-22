//! Windows-specific helpers for borderless chrome.
//!
//! These functions are pure helpers — they take a winit `Window` and
//! perform Win32 API calls. They do **not** own an event loop or a wgpu
//! context; the host wires them into its own winit + wgpu loop (see
//! [`super::Chrome::on_setup`]).
//!
//! Provides:
//! 1. HWND extraction from a `winit::Window`.
//! 2. DWM dark-mode attribute (Alt-Tab thumbnail tint, [`set_titlebar_dark_mode`]).
//! 3. Rounded corners (Win11 DWM, Win10 `SetWindowRgn` fallback).
//! 4. [`set_opacity`] — toggles `WS_EX_LAYERED`.
//! 5. [`sync_region`] — keeps the Win10 clip region in sync with maximise/restore.

#![cfg(windows)]

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetClientRect, GetWindowLongPtrW, LWA_ALPHA, SetLayeredWindowAttributes,
    SetWindowLongPtrW, WS_EX_LAYERED,
};

// ── HWND extraction ──────────────────────────────────────────────────────────

/// Extract the HWND from a winit window. Returns `None` if the window's
/// raw handle is not a Win32 handle.
pub fn hwnd_of(window: &winit::window::Window) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(h) = window.window_handle()
        && let RawWindowHandle::Win32(w) = h.as_raw()
    {
        return Some(w.hwnd.get());
    }
    None
}

// ── Rounded corners + Win11 detection ────────────────────────────────────────

/// Process-wide cache for the Win11 DWM rounded-corners probe. Set by the
/// first successful `set_rounded_corners` call; read by `sync_region` so
/// it can skip `SetWindowRgn` on Win11 — where mixing SetWindowRgn with
/// the DWM rounded frame causes a phantom caption strip to appear above
/// the client area.
static WIN11_DWM_CORNERS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Returns `true` when the Win11 DWM rounded-corners path was successfully
/// applied during the last `set_rounded_corners` call.
pub fn is_win11() -> bool {
    WIN11_DWM_CORNERS.get().copied().unwrap_or(false)
}

/// Apply rounded corners. On Win11 uses the DWM corner-preference attribute;
/// on Win10 falls back to `SetWindowRgn` with a rounded-rect region. Returns
/// `true` if the Win11 path succeeded.
fn set_rounded_corners(hwnd: isize, radius: i32) -> bool {
    if hwnd == 0 {
        return false;
    }
    // Win11: DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2.
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;
    let pref: u32 = DWMWCP_ROUND;
    // SAFETY: stable Win32 DWM API. cbAttribute matches size_of::<u32>().
    let hr = unsafe {
        DwmSetWindowAttribute(
            hwnd as _,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        )
    };
    let win11 = hr == 0;
    let _ = WIN11_DWM_CORNERS.set(win11);
    if win11 {
        return true;
    }
    apply_rounded_region_raw(hwnd, radius);
    false
}

/// Keep the Win10 clip region in sync with the maximised/windowed state.
///
/// - **Win11:** no-op. DWM owns the corners (`DWMWA_WINDOW_CORNER_PREFERENCE`)
///   and any `SetWindowRgn` here would clip its frame.
/// - **Win10 maximised:** clear the region (`SetWindowRgn(hwnd, NULL, 1)`) so
///   the window paints the full work-area rect. Without this the rounded
///   region cuts the top-left / bottom-left corners against the monitor
///   edges, producing the "black corner" artefact.
/// - **Win10 windowed:** re-apply the rounded region against the current
///   client rect, with `corner_radius` scaled by the window's DPI.
///
/// Call this from your event-loop callback whenever the window's maximised
/// state or size changes.
pub fn sync_region(window: &winit::window::Window, corner_radius: i32, is_maximized: bool) {
    let Some(hwnd) = hwnd_of(window) else { return };
    if hwnd == 0 || is_win11() {
        return;
    }
    if is_maximized {
        // SAFETY: passing a null region with redraw=TRUE clears the
        // current region and immediately repaints. Stable Win32 GDI call.
        unsafe {
            SetWindowRgn(hwnd as _, std::ptr::null_mut(), 1);
        }
        return;
    }
    let scaled = scale_radius_for_dpi(corner_radius, window.scale_factor());
    apply_rounded_region_raw(hwnd, scaled);
}

/// Scale a logical corner-radius up to physical pixels for `SetWindowRgn`,
/// which always works in physical coordinates regardless of DPI.
///
/// Reads `window.scale_factor()` (1.0 / 1.25 / 1.5 / 2.0 / …) and
/// multiplies. A configured `corner_radius = 8` looks like 8 logical
/// pixels at every DPI, instead of being silently halved on a 200 % DPI
/// monitor.
#[inline]
fn scale_radius_for_dpi(radius_logical: i32, scale: f64) -> i32 {
    if scale <= 0.0 {
        return radius_logical;
    }
    let scaled = (radius_logical as f64 * scale).round();
    scaled.clamp(0.0, i32::MAX as f64) as i32
}

fn apply_rounded_region_raw(hwnd: isize, radius_physical: i32) {
    let mut rect: RECT = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: GetClientRect writes into our stack-allocated RECT.
    let ok = unsafe { GetClientRect(hwnd as _, &mut rect) };
    if ok == 0 {
        return;
    }
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    if w <= 0 || h <= 0 {
        return;
    }
    // Clamp the radius so it never exceeds half of the shorter side —
    // CreateRoundRectRgn would otherwise produce a degenerate "pill"
    // shape (full circle on tiny windows) when host code passes a
    // legacy-large radius like 32 to a 22-px tool window.
    let r = radius_physical.max(0).min(w.min(h) / 2);
    // SAFETY: SetWindowRgn takes ownership of the GDI region (redraw=TRUE),
    // so the OS frees it on window destruction.
    unsafe {
        let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, r, r);
        if !rgn.is_null() {
            SetWindowRgn(hwnd as _, rgn, 1);
        }
    }
}

// ── DWM dark titlebar ────────────────────────────────────────────────────────

/// Apply the DWM immersive-dark-mode attribute. Affects the Alt-Tab
/// thumbnail and the brief OS-rendered frame during minimise/restore
/// animations, which can otherwise flash white on dark themes.
///
/// Public so hosts can re-apply at runtime when their theme switches
/// between dark and light modes — `setup_window` only sets it once at
/// boot.
pub fn set_titlebar_dark_mode(window: &winit::window::Window, dark: bool) {
    let Some(hwnd) = hwnd_of(window) else { return };
    if hwnd == 0 {
        return;
    }
    let value: u32 = if dark { 1 } else { 0 };
    // SAFETY: DwmSetWindowAttribute reads `cbAttribute` bytes from the pointer.
    unsafe {
        DwmSetWindowAttribute(
            hwnd as _,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &value as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

// ── Setup ─────────────────────────────────────────────────────────────────────

/// One-shot Win32 chrome setup: dark mode + rounded corners.
/// Call this once after `window.set_decorations(false)` from your runner's
/// post-GPU-init callback. [`crate::chrome::Chrome::on_setup`] does this
/// automatically.
///
/// `corner_radius` is in **logical pixels** — it gets multiplied by the
/// window's scale factor before being handed to GDI. Only consulted on
/// Win10; Win11 DWM owns the corner preference natively (the value is
/// either rounded or square, no radius).
pub fn setup_window(window: &winit::window::Window, corner_radius: i32) {
    let Some(hwnd) = hwnd_of(window) else { return };
    if hwnd == 0 {
        return;
    }
    set_titlebar_dark_mode(window, true);
    let scaled = scale_radius_for_dpi(corner_radius, window.scale_factor());
    set_rounded_corners(hwnd, scaled);
}

// ── Opacity (WS_EX_LAYERED) ──────────────────────────────────────────────────

/// Set the window's opacity (`0.0..=1.0`). Toggles `WS_EX_LAYERED`
/// transparently — the style flag is removed at exactly `alpha == 1.0`
/// to avoid the perf hit of redirected layered surfaces while opaque.
/// Anything below 1.0 (down to 0.0) keeps the layered style and applies
/// the alpha byte.
pub fn set_opacity(window: &winit::window::Window, alpha: f32) {
    let Some(hwnd) = hwnd_of(window) else { return };
    if hwnd == 0 {
        return;
    }
    let h = hwnd as HWND;
    let alpha = alpha.clamp(0.0, 1.0);
    unsafe {
        let cur = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
        // Strict equality: anything < 1.0 keeps the layered style. The
        // previous `>= 0.999` short-circuit was wrong — at alpha = 0.9995
        // the user wants near-opaque, not "drop the style flag entirely".
        if alpha >= 1.0 {
            if cur & WS_EX_LAYERED != 0 {
                SetWindowLongPtrW(h, GWL_EXSTYLE, (cur & !WS_EX_LAYERED) as isize);
            }
            return;
        }
        if cur & WS_EX_LAYERED == 0 {
            SetWindowLongPtrW(h, GWL_EXSTYLE, (cur | WS_EX_LAYERED) as isize);
        }
        let byte = (alpha * 255.0).round() as u8;
        SetLayeredWindowAttributes(h, 0, byte, LWA_ALPHA);
    }
}

#[cfg(test)]
mod tests {
    use super::scale_radius_for_dpi;

    #[test]
    fn scale_radius_identity_at_100_percent() {
        assert_eq!(scale_radius_for_dpi(8, 1.0), 8);
    }

    #[test]
    fn scale_radius_doubles_at_200_percent() {
        assert_eq!(scale_radius_for_dpi(8, 2.0), 16);
    }

    #[test]
    fn scale_radius_handles_125_percent() {
        // 8 * 1.25 = 10
        assert_eq!(scale_radius_for_dpi(8, 1.25), 10);
    }

    #[test]
    fn scale_radius_pass_through_on_invalid_scale() {
        assert_eq!(scale_radius_for_dpi(8, 0.0), 8);
        assert_eq!(scale_radius_for_dpi(8, -1.0), 8);
    }

    #[test]
    fn scale_radius_handles_zero() {
        assert_eq!(scale_radius_for_dpi(0, 1.5), 0);
    }
}
