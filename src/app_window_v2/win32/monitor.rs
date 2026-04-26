//! Monitor work-area helper.
//!
//! Used by `WM_GETMINMAXINFO` to constrain a maximized borderless window to
//! the work area of the monitor that contains it — without this, a window
//! with `WM_NCCALCSIZE → 0` maximizes to the **full** monitor area
//! including the taskbar (a known borderless-on-Windows quirk).

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};

/// Returns the work-area `RECT` of the monitor that currently contains the
/// given window. Falls back to the nearest monitor when the window is
/// off-screen.
///
/// Returns `None` when neither query succeeds (extremely rare; safe to
/// skip the work-area clamp in that case — the OS default will be used).
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn work_area_of(hwnd: HWND) -> Option<RECT> {
    if hwnd.is_null() {
        return None;
    }
    // SAFETY: MonitorFromWindow accepts any HWND and returns NULL on failure;
    // we check below.
    let monitor: HMONITOR = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return None;
    }
    let mut info: MONITORINFO = MONITORINFO {
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
    // SAFETY: `info` is a stack-allocated MONITORINFO with cbSize set.
    let ok = unsafe { GetMonitorInfoW(monitor, &mut info) };
    if ok == 0 {
        return None;
    }
    Some(info.rcWork)
}
