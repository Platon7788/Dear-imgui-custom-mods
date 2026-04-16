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
//! if let Some(hwnd) = hwnd_of(&window) {
//!     dear_imgui_custom_mod::borderless_window::platform::set_titlebar_dark_mode(hwnd, true);
//!     dear_imgui_custom_mod::borderless_window::platform::set_rounded_corners(hwnd, 8);
//! }
//! ```
//!
//! # Rounded corners
//!
//! [`set_rounded_corners`] applies native rounded corners on Windows 11 via the
//! DWM `DWMWA_WINDOW_CORNER_PREFERENCE` attribute; on Windows 10 it falls back
//! to `SetWindowRgn` with a rounded-rectangle region. Call [`update_rounded_region`]
//! after every `WindowEvent::Resized` to keep the Win10 region in sync with the
//! client area (Win11 ignores this call — it is a no-op on that path).
//!
//! # Cursors
//!
//! [`cursor_icon_for_edge`] returns the matching [`winit::window::CursorIcon`]
//! for a given [`super::ResizeEdge`]. [`set_os_resize_cursor`] bypasses winit
//! and calls the Win32 `SetCursor` directly — useful when you need to force the
//! resize cursor during an OS drag operation that winit would otherwise reset.

use super::actions::ResizeEdge;

// ── HWND extraction ───────────────────────────────────────────────────────────

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

// ── DWM dark title bar ────────────────────────────────────────────────────────

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
    if hwnd == 0 {
        return;
    }
    let value: u32 = if dark { 1 } else { 0 };
    // SAFETY: DwmSetWindowAttribute reads `cbAttribute` bytes from the pointer.
    // We pass a stack-allocated u32 and its size, which matches the expected layout.
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

// ── Rounded corners ───────────────────────────────────────────────────────────

/// Apply rounded corners to a borderless window.
///
/// On **Windows 11** this uses the DWM `DWMWA_WINDOW_CORNER_PREFERENCE` attribute
/// with `DWMWCP_ROUND` (value 2). The system chooses the radius — `radius` is ignored.
/// On **Windows 10** this falls back to `SetWindowRgn` with a rounded-rectangle
/// region created via `CreateRoundRectRgn`, using the given `radius` (in client-area pixels).
/// After a resize, call [`update_rounded_region`] to re-apply the Win10 region.
///
/// Returns `true` if the Win11 DWM path succeeded, `false` if the Win10 fallback
/// was used (or on non-Windows / `hwnd == 0`).
///
/// Safe to call with `hwnd == 0` (returns `false` immediately).
#[cfg(windows)]
pub fn set_rounded_corners(hwnd: isize, radius: i32) -> bool {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    if hwnd == 0 {
        return false;
    }

    // Win11: DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2.
    // Returns S_OK (0) when supported; non-zero HRESULT on Win10.
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
    if hr == 0 {
        return true;
    }

    // Win10 fallback
    apply_rounded_region_raw(hwnd, radius);
    false
}

#[cfg(not(windows))]
#[allow(unused_variables)]
/// No-op on non-Windows platforms.
pub fn set_rounded_corners(_hwnd: isize, _radius: i32) -> bool { false }

/// Re-apply the rounded window region after a resize (Win10-only path).
///
/// On Windows 11 this is a no-op because the DWM draws the rounded corners itself.
/// On Windows 10 it recreates the region from the current `GetClientRect` so the
/// rounded shape follows the new window size.
///
/// Safe to call with `hwnd == 0` (returns immediately).
#[cfg(windows)]
pub fn update_rounded_region(hwnd: isize, radius: i32) {
    if hwnd == 0 {
        return;
    }
    apply_rounded_region_raw(hwnd, radius);
}

#[cfg(not(windows))]
#[allow(unused_variables)]
/// No-op on non-Windows platforms.
pub fn update_rounded_region(_hwnd: isize, _radius: i32) {}

#[cfg(windows)]
fn apply_rounded_region_raw(hwnd: isize, radius: i32) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    let mut rect: RECT = RECT { left: 0, top: 0, right: 0, bottom: 0 };
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
    // SAFETY: CreateRoundRectRgn allocates a GDI region; SetWindowRgn takes ownership
    // (redraw = TRUE) so the region is freed by the OS on window destruction. If
    // SetWindowRgn fails we leak one region per failed call, which is acceptable for
    // a rare edge case (unlike dropping our own handle, we cannot call DeleteObject
    // without risking a double-free).
    unsafe {
        let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, r, r);
        if !rgn.is_null() {
            SetWindowRgn(hwnd as _, rgn, 1);
        }
    }
}

// ── Resize cursors ────────────────────────────────────────────────────────────

/// Return the [`winit::window::CursorIcon`] matching a resize edge.
///
/// Cross-platform. Returns [`CursorIcon::Default`](winit::window::CursorIcon::Default)
/// for `None` so callers can use the result directly:
///
/// ```rust,ignore
/// window.set_cursor(cursor_icon_for_edge(result.hover_edge));
/// ```
pub fn cursor_icon_for_edge(edge: Option<ResizeEdge>) -> winit::window::CursorIcon {
    use winit::window::CursorIcon;
    match edge {
        None                        => CursorIcon::Default,
        Some(ResizeEdge::North)     => CursorIcon::NResize,
        Some(ResizeEdge::South)     => CursorIcon::SResize,
        Some(ResizeEdge::East)      => CursorIcon::EResize,
        Some(ResizeEdge::West)      => CursorIcon::WResize,
        Some(ResizeEdge::NorthEast) => CursorIcon::NeResize,
        Some(ResizeEdge::NorthWest) => CursorIcon::NwResize,
        Some(ResizeEdge::SouthEast) => CursorIcon::SeResize,
        Some(ResizeEdge::SouthWest) => CursorIcon::SwResize,
    }
}

/// Map a [`ResizeEdge`] to the winit [`ResizeDirection`](winit::window::ResizeDirection)
/// required by `Window::drag_resize_window`.
///
/// Cross-platform.
pub fn resize_direction_of(edge: ResizeEdge) -> winit::window::ResizeDirection {
    use winit::window::ResizeDirection;
    match edge {
        ResizeEdge::North     => ResizeDirection::North,
        ResizeEdge::South     => ResizeDirection::South,
        ResizeEdge::East      => ResizeDirection::East,
        ResizeEdge::West      => ResizeDirection::West,
        ResizeEdge::NorthEast => ResizeDirection::NorthEast,
        ResizeEdge::NorthWest => ResizeDirection::NorthWest,
        ResizeEdge::SouthEast => ResizeDirection::SouthEast,
        ResizeEdge::SouthWest => ResizeDirection::SouthWest,
    }
}

/// Force the OS cursor to a resize arrow bypassing winit.
///
/// Useful when winit would otherwise reset the cursor between frames (e.g. during
/// an ongoing drag-resize). On non-Windows this is a no-op.
///
/// Pass `None` to leave the current cursor untouched.
#[cfg(windows)]
pub fn set_os_resize_cursor(edge: Option<ResizeEdge>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, LoadCursorW, SetCursor,
    };
    let id = match edge {
        None => return,
        Some(ResizeEdge::North) | Some(ResizeEdge::South)         => IDC_SIZENS,
        Some(ResizeEdge::East)  | Some(ResizeEdge::West)          => IDC_SIZEWE,
        Some(ResizeEdge::NorthWest) | Some(ResizeEdge::SouthEast) => IDC_SIZENWSE,
        Some(ResizeEdge::NorthEast) | Some(ResizeEdge::SouthWest) => IDC_SIZENESW,
    };
    // SAFETY: LoadCursorW with NULL hInstance + system cursor id returns a shared
    // handle owned by the OS; we do not destroy it. SetCursor is a simple state set.
    unsafe {
        let h = LoadCursorW(std::ptr::null_mut(), id);
        if !h.is_null() {
            SetCursor(h);
        }
    }
}

#[cfg(not(windows))]
#[allow(unused_variables)]
/// No-op on non-Windows platforms.
pub fn set_os_resize_cursor(_edge: Option<ResizeEdge>) {}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use winit::window::{CursorIcon, ResizeDirection};

    #[test]
    fn cursor_icon_none_is_default() {
        assert_eq!(cursor_icon_for_edge(None), CursorIcon::Default);
    }

    #[test]
    fn cursor_icon_covers_all_edges() {
        assert_eq!(cursor_icon_for_edge(Some(ResizeEdge::North)),     CursorIcon::NResize);
        assert_eq!(cursor_icon_for_edge(Some(ResizeEdge::South)),     CursorIcon::SResize);
        assert_eq!(cursor_icon_for_edge(Some(ResizeEdge::East)),      CursorIcon::EResize);
        assert_eq!(cursor_icon_for_edge(Some(ResizeEdge::West)),      CursorIcon::WResize);
        assert_eq!(cursor_icon_for_edge(Some(ResizeEdge::NorthEast)), CursorIcon::NeResize);
        assert_eq!(cursor_icon_for_edge(Some(ResizeEdge::NorthWest)), CursorIcon::NwResize);
        assert_eq!(cursor_icon_for_edge(Some(ResizeEdge::SouthEast)), CursorIcon::SeResize);
        assert_eq!(cursor_icon_for_edge(Some(ResizeEdge::SouthWest)), CursorIcon::SwResize);
    }

    #[test]
    fn resize_direction_covers_all_edges() {
        assert!(matches!(resize_direction_of(ResizeEdge::North),     ResizeDirection::North));
        assert!(matches!(resize_direction_of(ResizeEdge::South),     ResizeDirection::South));
        assert!(matches!(resize_direction_of(ResizeEdge::East),      ResizeDirection::East));
        assert!(matches!(resize_direction_of(ResizeEdge::West),      ResizeDirection::West));
        assert!(matches!(resize_direction_of(ResizeEdge::NorthEast), ResizeDirection::NorthEast));
        assert!(matches!(resize_direction_of(ResizeEdge::NorthWest), ResizeDirection::NorthWest));
        assert!(matches!(resize_direction_of(ResizeEdge::SouthEast), ResizeDirection::SouthEast));
        assert!(matches!(resize_direction_of(ResizeEdge::SouthWest), ResizeDirection::SouthWest));
    }

    // Platform-gated smoke tests: must not panic on a null HWND.
    #[cfg(windows)]
    #[test]
    fn set_titlebar_dark_mode_null_hwnd_is_safe() {
        set_titlebar_dark_mode(0, true);
        set_titlebar_dark_mode(0, false);
    }

    #[cfg(windows)]
    #[test]
    fn rounded_corners_null_hwnd_is_safe() {
        assert!(!set_rounded_corners(0, 8));
        update_rounded_region(0, 8);
    }

    #[cfg(windows)]
    #[test]
    fn set_os_resize_cursor_is_safe() {
        // Must not panic even with None.
        set_os_resize_cursor(None);
    }
}
