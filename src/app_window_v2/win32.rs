//! Windows-specific helpers — minimal v1-equivalent set, delegating to
//! the canonical implementation in [`crate::borderless_window::platform`].
//!
//! `app_window_v2` always creates `WS_POPUP + WS_THICKFRAME` windows
//! (`with_decorations(false)` in [`super`]). That style has no caption,
//! no system menu, no DWM chrome — DWM has nothing to draw or tint on
//! focus change, so there is no inactive-window dimming to fight.
//!
//! This module provides only the *app-host*-specific extensions:
//! 1. `WS_EX_TOOLWINDOW` for tool-window kinds (excludes from Alt-Tab).
//! 2. A `WM_GETMINMAXINFO` subclass that clamps a maximised
//!    `WS_THICKFRAME` window to the monitor work area so it doesn't
//!    cover the taskbar.
//! 3. `set_opacity` — toggles `WS_EX_LAYERED`.
//! 4. `debug_log` — `OutputDebugStringW` so messages survive
//!    `windows_subsystem = "windows"` (where stderr is detached).
//!
//! Everything else (HWND extraction, dark-mode attribute, rounded
//! corners, Win11 detection) is re-exported from
//! [`crate::borderless_window::platform`] so there is one source of
//! truth across the crate.

#![cfg(windows)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, LWA_ALPHA, MINMAXINFO, SetLayeredWindowAttributes,
    SetWindowLongPtrW, WM_DESTROY, WM_GETMINMAXINFO, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
};

// ── Re-exports of the canonical helpers ──────────────────────────────────────

pub(super) use crate::borderless_window::platform::{
    hwnd_of, is_win11_dwm_active as is_win11, update_rounded_region,
};

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
    crate::borderless_window::platform::set_titlebar_dark_mode(hwnd, true);
    crate::borderless_window::platform::set_rounded_corners(hwnd, opts.corner_radius);
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
