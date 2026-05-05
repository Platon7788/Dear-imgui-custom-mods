//! Windows-specific helpers for the borderless app-host.
//!
//! `app_window` always creates `WS_POPUP + WS_THICKFRAME` windows
//! (`with_decorations(false)` in [`super`]). That style has no caption,
//! no system menu, no DWM chrome — DWM has nothing to draw or tint on
//! focus change, so there is no inactive-window dimming to fight.
//!
//! Provides the full set of Win32 hooks the framework needs:
//! 1. HWND extraction from a `winit::Window`.
//! 2. DWM dark-mode attribute on the OS titlebar.
//! 3. Rounded corners (Win11 DWM, Win10 `SetWindowRgn` fallback).
//! 4. `WS_EX_TOOLWINDOW` for tool-window kinds (excludes from Alt-Tab).
//! 5. Subclass procedure that handles:
//!    - `WM_NCCALCSIZE` — return 0 with `rgrc[0]` left as the proposed
//!      window rect, so client-area = window-area (no NC frame).
//!      Critical on configurations where winit's own handler doesn't
//!      fully strip the non-client zone — typically high-DPI laptops,
//!      systems with basic theme / DWM composition off, or 3rd-party
//!      shell extensions like StarDock. Without this, Windows draws a
//!      phantom caption on top + a thin resize border on the left,
//!      visible above and beside our custom borderless chrome.
//!    - `WM_GETMINMAXINFO` — clamp a maximised `WS_THICKFRAME` window
//!      to the monitor work area so it doesn't cover the taskbar.
//! 6. `set_opacity` — toggles `WS_EX_LAYERED`.
//! 7. `debug_log` — `OutputDebugStringW` so messages survive
//!    `windows_subsystem = "windows"` (where stderr is detached).
//!
//! Before the v1 / `borderless_window` removal (2026-04-29), helpers
//! 1-3 lived in `borderless_window::platform`; they are inlined here
//! now so the framework is fully self-contained.

#![cfg(windows)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{
    CreateRoundRectRgn, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow, SetWindowRgn,
};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetClientRect, GetWindowLongPtrW, LWA_ALPHA, MINMAXINFO,
    SetLayeredWindowAttributes, SetWindowLongPtrW, WM_DESTROY, WM_GETMINMAXINFO, WM_NCCALCSIZE,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW,
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

/// Apply the DWM immersive-dark-mode attribute. Even with
/// `with_decorations(false)`, Windows still renders a small drop-shadow;
/// dark mode prevents the brief white flash on startup.
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
/// 4. `WM_GETMINMAXINFO` clamp subclass.
pub(super) fn setup_window(hwnd: isize, opts: SetupOptions) {
    if hwnd == 0 {
        return;
    }
    set_titlebar_dark_mode(hwnd, true);
    set_rounded_corners(hwnd, opts.corner_radius);
    apply_extended_styles(hwnd as HWND, opts.tool_window);
    install_minmax_subclass(hwnd as HWND);
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

// ── Maximise-clamp subclass ──────────────────────────────────────────────────

const SUBCLASS_ID: usize = 0xAFE1_BD72;

fn install_minmax_subclass(hwnd: HWND) {
    let ok = unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0) };
    if ok == 0 {
        debug_log("SetWindowSubclass failed");
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _refdata: usize,
) -> LRESULT {
    let result = catch_unwind(AssertUnwindSafe(|| match umsg {
        // `wparam == TRUE` form: lparam points to NCCALCSIZE_PARAMS,
        // where rgrc[0] is the proposed window rect. Returning 0 with
        // the rect untouched tells Windows "client area equals window
        // area" — no NC frame allocated, so the OS draws no caption,
        // no border, no resize edge bevel. This is the canonical
        // borderless pattern (Chrome / VS Code / Slack / Discord).
        //
        // Why we need this on top of winit's `with_decorations(false)`:
        // on high-DPI laptops, on systems with DWM composition off, or
        // on machines with 3rd-party shell extensions, winit's NCCALCSIZE
        // handler can leave a residual frame visible. ADR-028 (2026-05-05).
        //
        // `wparam == FALSE` falls through — Windows expects different
        // semantics (lparam is a plain RECT*) and we let DefSubclassProc
        // handle it.
        WM_NCCALCSIZE if wparam != 0 => 0,
        WM_GETMINMAXINFO => clamp_minmax(hwnd, lparam),
        WM_DESTROY => {
            unsafe {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
            }
            unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) }
        }
        _ => unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) },
    }));
    result.unwrap_or_else(|_| unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) })
}

fn clamp_minmax(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let Some(work) = work_area_of(hwnd) else {
        return unsafe { DefSubclassProc(hwnd, WM_GETMINMAXINFO, 0, lparam) };
    };
    let mmi: &mut MINMAXINFO = unsafe { &mut *(lparam as *mut MINMAXINFO) };
    mmi.ptMaxPosition = POINT {
        x: work.left,
        y: work.top,
    };
    mmi.ptMaxSize = POINT {
        x: work.right - work.left,
        y: work.bottom - work.top,
    };
    mmi.ptMaxTrackSize = POINT {
        x: (work.right - work.left).max(mmi.ptMinTrackSize.x),
        y: (work.bottom - work.top).max(mmi.ptMinTrackSize.y),
    };
    0
}

fn work_area_of(hwnd: HWND) -> Option<RECT> {
    let monitor: HMONITOR = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        rcMonitor: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        rcWork: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        dwFlags: 0,
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        None
    } else {
        Some(info.rcWork)
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
