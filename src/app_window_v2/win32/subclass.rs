//! Win32 WndProc subclass for non-client area handling.
//!
//! The subclass intercepts these messages and lets everything else flow to
//! the default subclass procedure (which falls through to winit's WndProc):
//!
//! 1. **`WM_NCHITTEST`** — converts cursor screen coords to window-local
//!    coords and reports the semantic region under the cursor: `HTCAPTION`
//!    (drag), `HTMAXBUTTON` / `HTCLOSE` (system buttons — the OS draws Snap
//!    Layouts on Win11 over `HTMAXBUTTON`), `HTLEFT` … `HTBOTTOMRIGHT`
//!    (resize edges), or `HTCLIENT` for everything else.
//!    **NOTE**: The minimize button returns `HTCLIENT` (not `HTMINBUTTON`) so
//!    that ImGui can handle the click and apply the Win11 restore-before-
//!    minimize workaround (see `pending_remax` in `app.rs`).
//! 2. **`WM_GETMINMAXINFO`** — clamps `ptMaxPosition` / `ptMaxSize` to the
//!    work area of the monitor, so a maximized borderless window does not
//!    cover the taskbar.
//! 3. **`WM_NCACTIVATE`** — returns `TRUE` (1) directly, **without** calling
//!    `DefWindowProc`. Any path through `DefWindowProc` for `WM_NCACTIVATE`
//!    causes Win11 22H2+ DWM to composite native caption chrome (title bar +
//!    min/max/close buttons) over the client area. Returning here prevents
//!    DWM from seeing the message. `WS_POPUP` has no visible NC area anyway.
//! 4. **`WM_NCPAINT`** — forwarded to `DefSubclassProc` (not suppressed).
//!    With `WS_POPUP` and no caption style, the default handler paints nothing
//!    visible, and intercepting with `return 0` causes Win11 to continuously
//!    re-send `WM_NCPAINT` because the NC region is never validated.
//! 5. **`WM_DESTROY`** — frees the `SharedHitRegions` payload box and removes
//!    the subclass so no dangling pointer is left after window destruction.
//!
//! **`WM_NCCALCSIZE` is intentionally NOT intercepted.**
//! Intercepting it and returning 0 (client = full window rect) causes Win11
//! DWM to render a permanent ~30 px black strip at the top of the window:
//! DWM interprets the explicit override as "this app manages its own NC area"
//! and composites its caption layer on top of the client. With `WS_POPUP` the
//! default `DefSubclassProc` already makes client ≈ window rect (no visible
//! NC area), so no override is needed.

use std::panic::{AssertUnwindSafe, catch_unwind};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::UI::Shell::{
    DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCLIENT, HTCLOSE, HTLEFT, HTMAXBUTTON,
    HTMINBUTTON, HTNOWHERE, HTRIGHT, HTSYSMENU, HTTOP, HTTOPLEFT, HTTOPRIGHT, MINMAXINFO,
    WM_DESTROY, WM_GETMINMAXINFO, WM_NCACTIVATE, WM_NCHITTEST,
    WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_NCPAINT,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
};

use super::super::hit_test::{HoveredNcButton, SharedHitRegions};
use super::monitor::work_area_of;

/// Subclass identifier — unique within the (hwnd, proc) pair.
const SUBCLASS_ID: usize = 0x_AF_E1_BD_71;

/// Install the subclass on `hwnd`, attaching the given hit-region handle.
///
/// Returns `true` on success, `false` if the subclass install failed (very
/// rare — typically only when comctl32 is not loaded or `hwnd` is null).
/// The subclass owns the `SharedHitRegions` box; it is freed automatically
/// when the window is destroyed (via the `WM_DESTROY` handler).
///
/// # Safety
/// `hwnd` must be a valid Win32 window handle owned by the calling thread.
pub unsafe fn install(hwnd: HWND, regions: SharedHitRegions) -> bool {
    if hwnd.is_null() {
        return false;
    }
    let boxed: Box<SharedHitRegions> = Box::new(regions);
    let raw = Box::into_raw(boxed) as usize;
    // SAFETY: SetWindowSubclass is the documented entry point for installing
    // a subclass. `subclass_proc` matches the SUBCLASSPROC ABI. dwRefData is
    // an opaque payload that we round-trip back unchanged in the WndProc.
    let ok = unsafe {
        SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, raw)
    };
    if ok == 0 {
        // Re-claim the box to avoid a leak on failure.
        // SAFETY: `raw` was just produced by Box::into_raw and never aliased.
        unsafe { drop(Box::from_raw(raw as *mut SharedHitRegions)); }
        false
    } else {
        true
    }
}

/// Remove the subclass installed by [`install`] and free its payload.
///
/// Normally the subclass removes itself in `WM_DESTROY`. Call this only for
/// early teardown (e.g. recreating the GPU device while keeping the window).
///
/// # Safety
/// `hwnd` must be a valid Win32 window handle owned by the calling thread.
#[allow(dead_code)]
pub unsafe fn uninstall(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    let mut data: usize = 0;
    // SAFETY: GetWindowSubclass writes to the optional `pdwRefData` out-param.
    let found = unsafe { GetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, &mut data) };
    if found == 0 {
        return;
    }
    // SAFETY: documented Win32 API.
    let _ = unsafe { RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID) };
    if data != 0 {
        // SAFETY: `data` is the same pointer we put in via Box::into_raw.
        unsafe { drop(Box::from_raw(data as *mut SharedHitRegions)); }
    }
}

/// The actual subclass procedure. Wrapped in `catch_unwind` because Win32
/// would unwind across an FFI boundary on a Rust panic — undefined behavior.
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    dwrefdata: usize,
) -> LRESULT {
    let result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the data we put in via Box::into_raw lives for as long as
        // the subclass is installed; WM_DESTROY reclaims it after
        // RemoveWindowSubclass returns (which synchronously drains in-flight
        // WndProc invocations before returning).
        let regions: &SharedHitRegions = unsafe {
            (dwrefdata as *const SharedHitRegions)
                .as_ref()
                .expect("subclass: null dwRefData")
        };
        match umsg {
            // Forward to default — with WS_POPUP and no caption style, the
            // default handler paints nothing visible. Returning 0 here would
            // leave the NC region permanently invalid, causing Win11 to
            // continuously re-send WM_NCPAINT and burning CPU.
            WM_NCPAINT => unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) },

            // Return TRUE (keep active appearance) without forwarding to
            // DefWindowProc. Forwarding — even with (wParam=1, lParam=-1) —
            // causes Win11 22H2+ DWM to composite native caption chrome
            // (title bar + min/max/close) over the client area whenever it
            // receives WM_NCACTIVATE. Returning here directly prevents DWM
            // from ever seeing the message and rendering the strip.
            WM_NCACTIVATE => 1,

            WM_NCHITTEST => handle_nc_hit_test(hwnd, lparam, regions),
            WM_GETMINMAXINFO => handle_min_max_info(hwnd, lparam),
            WM_NCMOUSEMOVE => handle_nc_mouse_move(hwnd, lparam, regions),
            WM_NCMOUSELEAVE => {
                regions.set_hovered_button(HoveredNcButton::None);
                regions.set_pressed_button(HoveredNcButton::None);
                // SAFETY: documented Win32 API.
                unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) }
            }
            WM_NCLBUTTONDOWN => {
                // Record the timestamp so the app layer can debounce Focused(false).
                regions.mark_nc_lbuttondown();
                let btn = nc_button_for_hit_code(wparam as i32);
                regions.set_pressed_button(btn);
                // SAFETY: documented Win32 API.
                unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) }
            }
            WM_NCLBUTTONUP => {
                regions.set_pressed_button(HoveredNcButton::None);
                // SAFETY: documented Win32 API.
                unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) }
            }
            WM_DESTROY => {
                // Remove the subclass and free the payload box. Must remove
                // FIRST so that any re-entrant messages fired by DefSubclassProc
                // below do not re-enter this handler with a freed pointer.
                // SAFETY: documented Win32 API. Removing from within the proc
                // is explicitly supported by comctl32.
                unsafe {
                    let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                    if dwrefdata != 0 {
                        drop(Box::from_raw(dwrefdata as *mut SharedHitRegions));
                    }
                    DefSubclassProc(hwnd, umsg, wparam, lparam)
                }
            }
            // SAFETY: documented Win32 API.
            _ => unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) },
        }
    }));
    match result {
        Ok(lr) => lr,
        // On panic, fall through to the default — at least the window stays alive.
        Err(_) => unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) },
    }
}

// ── Message handlers ─────────────────────────────────────────────────────────

/// Determine which semantic region the cursor is over and report the
/// corresponding `HT*` code so the OS can drive the appropriate behavior.
fn handle_nc_hit_test(hwnd: HWND, lparam: LPARAM, regions: &SharedHitRegions) -> LRESULT {
    // Convert screen coords from the LPARAM to client-local coords.
    // Client coords match the space the titlebar renderer uses (ImGui root
    // window origin = client-area top-left). If we used GetWindowRect-based
    // window-local coords here instead, buttons would appear shifted by the
    // WS_THICKFRAME inset on any system where client != window origin.
    let (screen_x, screen_y) = screen_coords_from_lparam(lparam);
    let mut pt = POINT { x: screen_x, y: screen_y };
    // SAFETY: documented Win32 API.
    let ok = unsafe { windows_sys::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pt) };
    if ok == 0 {
        return HTNOWHERE as LRESULT;
    }
    let lx = pt.x;
    let ly = pt.y;

    // For resize edge detection we need the window dimensions in window-local
    // coords (edges are at the window boundary regardless of client origin).
    let mut win_rect: RECT = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: documented Win32 API.
    let _ = unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut win_rect)
    };

    let r = regions.read();

    if r.passthrough {
        return HTCLIENT as LRESULT;
    }

    // Resize edges have the highest priority — they use window-local coords.
    let win_lx = screen_x - win_rect.left;
    let win_ly = screen_y - win_rect.top;
    let win_w  = win_rect.right  - win_rect.left;
    let win_h  = win_rect.bottom - win_rect.top;
    let rz = r.resize_zone.max(1);

    if !r.is_maximized {
        let on_left   = win_lx < rz;
        let on_right  = win_lx >= win_w - rz;
        let on_top    = win_ly < rz;
        let on_bottom = win_ly >= win_h - rz;
        match (on_top, on_bottom, on_left, on_right) {
            (true,  _,     true,  _    ) => return HTTOPLEFT     as LRESULT,
            (true,  _,     _,     true ) => return HTTOPRIGHT    as LRESULT,
            (_,     true,  true,  _    ) => return HTBOTTOMLEFT  as LRESULT,
            (_,     true,  _,     true ) => return HTBOTTOMRIGHT as LRESULT,
            (true,  _,     _,     _    ) => return HTTOP         as LRESULT,
            (_,     true,  _,     _    ) => return HTBOTTOM      as LRESULT,
            (_,     _,     true,  _    ) => return HTLEFT        as LRESULT,
            (_,     _,     _,     true ) => return HTRIGHT       as LRESULT,
            _ => {}
        }
    }

    // System buttons — checked before HTCAPTION so a click on the maximize
    // button doesn't start a drag, and so Win11 shows Snap Layouts on hover.
    if !r.close_btn.is_empty() && r.close_btn.contains(lx, ly) {
        return HTCLOSE as LRESULT;
    }
    if !r.max_btn.is_empty() && r.max_btn.contains(lx, ly) {
        return HTMAXBUTTON as LRESULT;
    }
    // Return HTCLIENT for the minimize button so ImGui handles the click.
    // This lets app.rs apply the Win11 restore-before-minimize workaround
    // (pending_remax) before calling window.set_minimized(true).
    if !r.min_btn.is_empty() && r.min_btn.contains(lx, ly) {
        return HTCLIENT as LRESULT;
    }
    if !r.icon_btn.is_empty() && r.icon_btn.contains(lx, ly) {
        return HTSYSMENU as LRESULT;
    }
    // Custom extra buttons → HTCLIENT so winit's mouse pipeline and ImGui
    // both see the clicks normally.
    for extra in &r.extras {
        if extra.contains(lx, ly) {
            return HTCLIENT as LRESULT;
        }
    }

    // The drag area is HTCLIENT — drag is initiated by app code via
    // window.drag_window() when ImGui detects a click. Returning HTCAPTION
    // from WM_NCHITTEST causes Win11 DWM to continuously composite its own
    // caption layer over that area, producing a black strip at the top.
    HTCLIENT as LRESULT
}

/// Constrain the maximize bounds to the work area of the window's current
/// monitor so the maximized window does not cover the taskbar.
fn handle_min_max_info(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let Some(work) = work_area_of(hwnd) else {
        return 0;
    };
    // SAFETY: lparam points to a valid MINMAXINFO for the duration of this call.
    let mmi: &mut MINMAXINFO = unsafe { &mut *(lparam as *mut MINMAXINFO) };
    mmi.ptMaxPosition = POINT { x: work.left, y: work.top };
    mmi.ptMaxSize     = POINT {
        x: work.right  - work.left,
        y: work.bottom - work.top,
    };
    mmi.ptMaxTrackSize = POINT {
        x: (work.right  - work.left).max(mmi.ptMinTrackSize.x),
        y: (work.bottom - work.top ).max(mmi.ptMinTrackSize.y),
    };
    0
}

/// Translate a hit-test code into our `HoveredNcButton` enum.
fn nc_button_for_hit_code(code: i32) -> HoveredNcButton {
    match code as u32 {
        x if x == HTMINBUTTON => HoveredNcButton::Min,
        x if x == HTMAXBUTTON => HoveredNcButton::Max,
        x if x == HTCLOSE     => HoveredNcButton::Close,
        _                     => HoveredNcButton::None,
    }
}

/// Track which non-client button the cursor is over for hover-highlight
/// rendering, and request `WM_NCMOUSELEAVE` so the highlight clears when
/// the cursor leaves.
fn handle_nc_mouse_move(hwnd: HWND, lparam: LPARAM, regions: &SharedHitRegions) -> LRESULT {
    let (lx, ly) = client_coords_from_screen_lparam(hwnd, lparam);
    let r = regions.read();

    let hovered = if !r.close_btn.is_empty() && r.close_btn.contains(lx, ly) {
        HoveredNcButton::Close
    } else if !r.max_btn.is_empty() && r.max_btn.contains(lx, ly) {
        HoveredNcButton::Max
    } else if !r.min_btn.is_empty() && r.min_btn.contains(lx, ly) {
        HoveredNcButton::Min
    } else {
        HoveredNcButton::None
    };
    if hovered != r.hovered_button {
        regions.set_hovered_button(hovered);
    }

    let mut tme = TRACKMOUSEEVENT {
        cbSize:      std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags:     TME_LEAVE | TME_NONCLIENT,
        hwndTrack:   hwnd,
        dwHoverTime: 0,
    };
    // SAFETY: `tme` is a fully-initialised stack-allocated TRACKMOUSEEVENT.
    unsafe { TrackMouseEvent(&mut tme); }

    // SAFETY: documented Win32 API.
    unsafe { DefSubclassProc(hwnd, WM_NCMOUSEMOVE, 0, lparam) }
}

// ── Coordinate helpers ────────────────────────────────────────────────────────

/// Unpack screen coordinates from a `WM_NC*` lParam, sign-extending the
/// 16-bit halves so negative coordinates (monitors left/above the primary)
/// are represented correctly.
#[inline]
fn screen_coords_from_lparam(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam as i32) & 0xFFFF;
    let y = ((lparam as i32) >> 16) & 0xFFFF;
    let x = if x & 0x8000 != 0 { x | !0xFFFF } else { x };
    let y = if y & 0x8000 != 0 { y | !0xFFFF } else { y };
    (x, y)
}

/// Convert the screen-coordinate cursor position from a `WM_NC*` lParam into
/// **client-local** pixels — the same coordinate space the titlebar renderer
/// uses to compute `HitRegions`.
fn client_coords_from_screen_lparam(hwnd: HWND, lparam: LPARAM) -> (i32, i32) {
    let (sx, sy) = screen_coords_from_lparam(lparam);
    let mut pt = POINT { x: sx, y: sy };
    // SAFETY: documented Win32 API.
    let ok = unsafe { windows_sys::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pt) };
    if ok == 0 { (sx, sy) } else { (pt.x, pt.y) }
}
