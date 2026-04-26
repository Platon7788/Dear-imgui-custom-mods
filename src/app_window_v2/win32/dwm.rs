//! DWM (Desktop Window Manager) helpers for v2 borderless windows.
//!
//! - [`set_immersive_dark_mode`] — dark titlebar attribute, prevents the
//!   white-flash on startup.
//! - [`enable_dwm_rounded_corners`] — Win11 native rounded corners
//!   (`DWMWA_WINDOW_CORNER_PREFERENCE`). No-op / silent fail on Win10.
//! - [`extend_frame_into_client`] — canonical `DwmExtendFrameIntoClientArea`
//!   call with `{1,1,1,1}` margins so DWM composes the native drop-shadow
//!   and rounded-corner anti-aliasing inside the client area.
//! - [`suppress_caption_color`] — sets `DWMWA_CAPTION_COLOR = NONE` to
//!   prevent Win11 from applying a caption tint over our client area.
//! - [`suppress_system_backdrop`] — sets `DWMWA_SYSTEMBACKDROP_TYPE = NONE`
//!   to prevent Win11 22H2+ from applying Mica/Acrylic over the window.
//! - [`is_win11_dwm_corners`] — cached probe; `true` when on Win11 with
//!   rounded-corner support.

use std::sync::OnceLock;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows_sys::Win32::UI::Controls::MARGINS;

/// Set after the first probe. `true` = Win11 with rounded-corner support.
static WIN11_CORNERS: OnceLock<bool> = OnceLock::new();

/// Apply (or remove) the DWM immersive-dark-mode attribute.
///
/// Call this **before** showing the window to avoid the white-flash that
/// happens when the OS draws the default light frame first.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn set_immersive_dark_mode(hwnd: HWND, dark: bool) {
    if hwnd.is_null() {
        return;
    }
    let value: u32 = if dark { 1 } else { 0 };
    // SAFETY: stable Win32 DWM API. cbAttribute exactly matches sizeof(BOOL=u32).
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            (&value as *const u32).cast(),
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// Enable Win11 native rounded corners.
///
/// Returns `true` if the call succeeded (Win11), `false` on Win10 (no
/// corner attribute support — the window will have square corners, matching
/// every other Win10 application). Result is cached; subsequent calls just
/// return the cached value.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn enable_dwm_rounded_corners(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }
    if let Some(&cached) = WIN11_CORNERS.get() {
        if cached {
            apply_round_attr(hwnd);
        }
        return cached;
    }
    let ok = apply_round_attr(hwnd);
    let _ = WIN11_CORNERS.set(ok);
    ok
}

fn apply_round_attr(hwnd: HWND) -> bool {
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;
    let pref: u32 = DWMWCP_ROUND;
    // SAFETY: stable Win32 DWM API. cbAttribute exactly matches sizeof(u32).
    let hr = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&pref as *const u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    hr == 0
}

/// Returns `true` when the last [`enable_dwm_rounded_corners`] call succeeded.
pub fn is_win11_dwm_corners() -> bool {
    WIN11_CORNERS.get().copied().unwrap_or(false)
}

/// Tell DWM to extend the system frame into the **entire** client area.
///
/// `MARGINS {-1,-1,-1,-1}` means "full-glass mode": DWM composites its frame
/// (shadow, rounded corners, active border) across the whole window area.
/// Our `CompositeAlphaMode::Opaque` swap chain renders completely on top,
/// so the DWM glass is invisible and the window appears fully opaque.
///
/// This is the correct value for borderless windows on both Win10 and Win11:
///   - Using `{1,1,1,1}` instead causes Win11 DWM to render its "title bar
///     region" (~30 px) as a black layer OVER the swap chain, because DWM
///     still reserves caption space for its compositing even when WM_NCCALCSIZE
///     returns 0. Full-glass mode (`{-1,-1,-1,-1}`) tells DWM the entire
///     client area is part of the frame, eliminating the reserved caption zone.
///   - On Win10, full-glass puts the HWND into Aero-glass mode. The swap chain
///     is composited as fully opaque (CompositeAlphaMode::Opaque), hiding the
///     glass completely — no transparency visible.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn extend_frame_into_client(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    let margins = MARGINS {
        cxLeftWidth:    -1,
        cxRightWidth:   -1,
        cyTopHeight:    -1,
        cyBottomHeight: -1,
    };
    // SAFETY: stable Win32 DWM API.
    unsafe {
        DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
}

/// Suppress the Win11 caption color tint.
///
/// On Win11, DWM can apply a "caption color" over the client area. Setting
/// `DWMWA_CAPTION_COLOR = DWMWA_COLOR_NONE` (0xFFFFFFFE) disables this so
/// the tint does not bleed onto our custom-rendered titlebar. Silently fails
/// on Win10 (the attribute is not recognised there).
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn suppress_caption_color(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    const DWMWA_CAPTION_COLOR: u32 = 35;
    const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;
    let color: u32 = DWMWA_COLOR_NONE;
    // SAFETY: Win11 DWM API. Silently returns non-zero HRESULT on Win10.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            (&color as *const u32).cast(),
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// Disable the Win11 22H2+ system backdrop (Mica / Acrylic).
///
/// Without this call, Win11 may composite a Mica or Acrylic backdrop over
/// the window. Setting `DWMWA_SYSTEMBACKDROP_TYPE = DWMSBT_NONE` (1) opts
/// out. Silently fails on Win10 and pre-22H2 Win11 (attribute not present).
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn suppress_system_backdrop(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
    const DWMSBT_NONE: u32 = 1;
    let value: u32 = DWMSBT_NONE;
    // SAFETY: Win11 22H2+ DWM API. Silently fails on earlier builds.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&value as *const u32).cast(),
            std::mem::size_of::<u32>() as u32,
        );
    }
}
