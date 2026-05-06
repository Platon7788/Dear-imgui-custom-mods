//! Windows-specific helpers for the borderless app-host.
//!
//! `app_window` creates the window with normal `WS_OVERLAPPEDWINDOW`
//! decorations (winit `with_decorations(true)`); after wgpu init the
//! framework calls `window.set_decorations(false)` from `startup.rs`,
//! which flips winit's `MARKER_DECORATIONS` flag and triggers a
//! `SetWindowPos(SWP_FRAMECHANGED)` — winit's own `WM_NCCALCSIZE`
//! handler then returns `0` for every NC pass, killing the visual
//! frame. This is the post-creation route; verified working on
//! laptops, desktops and VMs against the reference at
//! `D:\\GitHub\\Rust_Projects\\test-dear-imgui-rs`.
//!
//! Provides:
//! 1. HWND extraction from a `winit::Window`.
//! 2. DWM dark-mode attribute (Alt-Tab thumbnail tint).
//! 3. Rounded corners (Win11 DWM, Win10 `SetWindowRgn` fallback).
//! 4. `WS_EX_TOOLWINDOW` for tool-window kinds (excludes from Alt-Tab).
//! 5. `set_opacity` — toggles `WS_EX_LAYERED`.
//! 6. `debug_log` — `OutputDebugStringW` so messages survive
//!    `windows_subsystem = "windows"` (where stderr is detached).
//!
//! Deliberately NOT done here:
//! - **No `WM_GETMINMAXINFO` subclass.** winit's `WM_NCCALCSIZE` handler
//!   already sets `rgrc[0] = monitorInfo.rcWork` on maximise (see
//!   `winit/src/platform_impl/windows/event_loop.rs` ~line 1170), so
//!   the taskbar stays visible automatically. Adding our own MINMAX
//!   clamp on top double-constrained the window and produced visible
//!   gap+clip artifacts on configurations where the OS already
//!   accounted for the work area.
//! - **No `WM_NCCALCSIZE` override.** winit owns that handler — we let
//!   it do its job.
//! - **No `WM_NCHITTEST` override.** winit + `WS_OVERLAPPEDWINDOW`
//!   handles native edge-resize through `DefWindowProc`. The titlebar
//!   drag is handled at the app layer through
//!   `winit::Window::drag_window`.

#![cfg(windows)]

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetClientRect, GetWindowLongPtrW, LWA_ALPHA, SetLayeredWindowAttributes,
    SetWindowLongPtrW, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
};

// ── HWND extraction ──────────────────────────────────────────────────────────

/// Extract the HWND from a winit window. Returns `None` if the window's
/// raw handle is not a Win32 handle.
pub(super) fn hwnd_of(window: &winit::window::Window) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(h) = window.window_handle()
        && let RawWindowHandle::Win32(w) = h.as_raw()
    {
        return Some(w.hwnd.get());
    }
    None
}

// ── Rounded corners + Win11 detection ────────────────────────────────────────

// Process-wide cache for the Win11 DWM rounded-corners probe. Set by the first
// successful `set_rounded_corners` call; read by `update_rounded_region` so it
// can skip `SetWindowRgn` on Win11 — where mixing SetWindowRgn with the DWM
// rounded frame causes a phantom caption strip to appear above the client area.
static WIN11_DWM_CORNERS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Returns `true` when the Win11 DWM rounded-corners path was successfully
/// applied during the last `set_rounded_corners` call. Used by the
/// `restore-from-minimised-when-maximised` workaround in `gpu/mod.rs`.
pub(super) fn is_win11() -> bool {
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

/// Re-apply the rounded window region after a resize (Win10-only path —
/// no-op on Win11 because the DWM owns the corners and `SetWindowRgn`
/// would clip its frame).
pub(super) fn update_rounded_region(hwnd: isize, radius: i32) {
    if hwnd == 0 || is_win11() {
        return;
    }
    apply_rounded_region_raw(hwnd, radius);
}

fn apply_rounded_region_raw(hwnd: isize, radius: i32) {
    let mut rect: RECT = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: GetClientRect writes into our stack-allocated RECT. hwnd is the caller's responsibility.
    let ok = unsafe { GetClientRect(hwnd as _, &mut rect) };
    if ok == 0 {
        return;
    }
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    if w <= 0 || h <= 0 {
        return;
    }
    let r = radius.max(0);
    // SAFETY: SetWindowRgn takes ownership of the GDI region (redraw=TRUE),
    // so the OS frees it on window destruction. If SetWindowRgn fails we
    // leak one region per failed call — acceptable for a rare edge case.
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
pub(super) fn set_titlebar_dark_mode(hwnd: isize, dark: bool) {
    if hwnd == 0 {
        return;
    }
    let value: u32 = if dark { 1 } else { 0 };
    // SAFETY: DwmSetWindowAttribute reads `cbAttribute` bytes from the pointer.
    // We pass a stack u32 and its size — matches the documented layout.
    unsafe {
        DwmSetWindowAttribute(
            hwnd as _,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &value as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

// ── Setup options ─────────────────────────────────────────────────────────────

/// Options for [`setup_window`].
#[derive(Debug, Clone, Copy)]
pub(super) struct SetupOptions {
    pub tool_window: bool,
    pub corner_radius: i32,
}

/// Apply every Win32-side adjustment our framework cares about, in the
/// order required for them not to fight each other:
/// 1. Dark mode for the Alt-Tab thumbnail.
/// 2. Rounded corners (Win11 DWM, Win10 region fallback).
/// 3. `WS_EX_TOOLWINDOW` (tool kinds only).
pub(super) fn setup_window(hwnd: isize, opts: SetupOptions) {
    if hwnd == 0 {
        return;
    }
    set_titlebar_dark_mode(hwnd, true);
    set_rounded_corners(hwnd, opts.corner_radius);
    apply_extended_styles(hwnd as HWND, opts.tool_window);
}

// ── WS_EX_TOOLWINDOW ─────────────────────────────────────────────────────────

fn apply_extended_styles(hwnd: HWND, tool_window: bool) {
    if !tool_window {
        return;
    }
    unsafe {
        let cur = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let want = cur | WS_EX_TOOLWINDOW;
        if want != cur {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, want as isize);
        }
    }
}

// ── Opacity (WS_EX_LAYERED) ──────────────────────────────────────────────────

pub(super) fn set_opacity(hwnd: isize, alpha: f32) {
    if hwnd == 0 {
        return;
    }
    let h = hwnd as HWND;
    let alpha = alpha.clamp(0.0, 1.0);
    unsafe {
        let cur = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
        if alpha >= 0.999 {
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

// ── Debug logging ────────────────────────────────────────────────────────────

/// Send a debug message that survives `windows_subsystem = "windows"` (where
/// stderr is detached). Routed via `OutputDebugStringW`, visible in DebugView /
/// the IDE debug output pane.
pub(crate) fn debug_log(msg: &str) {
    unsafe extern "system" {
        fn OutputDebugStringW(lpOutputString: *const u16);
    }
    let mut wide: Vec<u16> = msg.encode_utf16().collect();
    wide.push(b'\n' as u16);
    wide.push(0);
    unsafe {
        OutputDebugStringW(wide.as_ptr());
    }
}
