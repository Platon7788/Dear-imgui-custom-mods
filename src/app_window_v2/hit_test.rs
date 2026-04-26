//! Hit-testing regions shared between the titlebar renderer and the Win32
//! `WM_NCHITTEST` handler.
//!
//! The titlebar renderer updates these rectangles every frame from its layout
//! pass; the WndProc reads them on every `WM_NCHITTEST` and reports which
//! semantic region the cursor is over so the OS can drive drag, resize,
//! Snap Layouts, system menu, and the rest of the non-client lifecycle
//! natively.
//!
//! All rectangles are in **physical (window-local) pixels** — the OS reports
//! cursor coordinates to `WM_NCHITTEST` in screen pixels, and we convert to
//! window-local before testing.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

/// Axis-aligned rectangle `[left, top, right, bottom]` in physical pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PixelRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PixelRect {
    pub const fn empty() -> Self {
        Self {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        }
    }

    pub const fn from_xywh(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        }
    }

    #[inline]
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }
}

/// Which non-client button (if any) the cursor is currently over.
///
/// Updated by the WndProc on `WM_NCMOUSEMOVE` / `WM_NCMOUSELEAVE` and read
/// by the titlebar renderer to draw the hover highlight. We need this
/// because once `WM_NCHITTEST` reports HTMINBUTTON / HTMAXBUTTON / HTCLOSE,
/// the OS owns the mouse — ImGui's `mouse_pos()` no longer reflects whether
/// the cursor is over those buttons, so we cannot derive the hover state
/// from it. The WndProc gives us first-party access via the `WM_NC*` events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HoveredNcButton {
    #[default]
    None,
    Min,
    Max,
    Close,
}

/// Live hit-testing data updated each frame by the titlebar and read by
/// `WM_NCHITTEST` in the WndProc.
///
/// Wrapped in [`SharedHitRegions`] for cross-thread access; the WndProc fires
/// from the OS message-pump thread, the titlebar updates from the render
/// thread (which on most setups is the same thread but the API guarantees
/// safety either way).
#[derive(Debug, Clone, Default)]
pub struct HitRegions {
    /// Total titlebar height (from window top) — defines where the resize
    /// edge zone ends and the drag/caption zone begins.
    pub titlebar_height: i32,
    /// Resize edge thickness in physical pixels (border zone for HTLEFT/...).
    pub resize_zone: i32,
    /// Caption (drag) area — clicking here returns HTCAPTION so the OS
    /// drives the drag natively (incl. Aero Snap, Aero Shake, drag to
    /// edge, snap to grid).
    pub caption: PixelRect,
    /// Minimize button hit area — returns HTMINBUTTON. OS handles the click
    /// (sends WM_SYSCOMMAND(SC_MINIMIZE)) and on Win11 shows the snap-layouts
    /// hover popup if applicable.
    pub min_btn: PixelRect,
    /// Maximize/Restore button — returns HTMAXBUTTON. **This is the key
    /// region that triggers Win11 Snap Layouts on hover** (no extra code
    /// needed; just reporting HTMAXBUTTON is enough).
    pub max_btn: PixelRect,
    /// Close button — returns HTCLOSE. OS sends WM_SYSCOMMAND(SC_CLOSE)
    /// which winit translates to `WindowEvent::CloseRequested`.
    pub close_btn: PixelRect,
    /// Optional icon click region — returns HTSYSMENU when set, opening
    /// the system menu (Win10/Win11 native). Empty rect = no system menu.
    pub icon_btn: PixelRect,
    /// Custom (extra) buttons drawn in the titlebar by the user — these
    /// must be HTCLIENT so winit's normal mouse pipeline gets the clicks
    /// and the user code sees them via the [`AppHandlerV2::on_extra_button`]
    /// callback.
    pub extras: Vec<PixelRect>,
    /// Current maximized state — used by the WndProc to disable resize
    /// edges (no resize is possible while maximized; reporting HTLEFT/...
    /// would let the user drag the edge of a maximized window).
    pub is_maximized: bool,
    /// When `true`, the WndProc returns HTCLIENT for everything — used to
    /// disable native non-client behavior (e.g. before subclass attach is
    /// complete).
    pub passthrough: bool,
    /// Which system button (if any) the cursor is currently over. Updated
    /// from `WM_NCMOUSEMOVE` / `WM_NCMOUSELEAVE` by the WndProc.
    pub hovered_button: HoveredNcButton,
    /// Which system button (if any) the user is currently pressing
    /// (mouse down on the button, not yet released). Updated from
    /// `WM_NCLBUTTONDOWN` / `WM_NCLBUTTONUP` by the WndProc — used so the
    /// titlebar can show a "pressed" highlight matching native button feel.
    pub pressed_button: HoveredNcButton,
}

/// Cross-thread handle for [`HitRegions`].
///
/// Cheap to clone (`Arc`); use [`Self::write`] for the per-frame update from
/// the titlebar, and [`Self::read`] from `WM_NCHITTEST`. The lock window is
/// short — copying ~10 rectangles — so contention is negligible even on
/// pathological message storms.
#[derive(Debug, Clone, Default)]
pub struct SharedHitRegions {
    inner: Arc<Mutex<HitRegions>>,
    /// Milliseconds since UNIX_EPOCH when the last WM_NCLBUTTONDOWN fired.
    /// 0 = never. Used by the app layer to debounce spurious Focused(false)
    /// events that Win11 fires during OS-driven HTCAPTION drags.
    last_nc_lbuttondown_ms: Arc<AtomicU64>,
}

impl SharedHitRegions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the layout-related region set in one atomic step.
    ///
    /// **Preserves** the WndProc-owned fields (`hovered_button`,
    /// `pressed_button`) so the next render frame's hover paint stays in
    /// sync with the latest `WM_NCMOUSEMOVE` / `WM_NCLBUTTONDOWN` event,
    /// even though those fields don't exist in the incoming layout struct.
    ///
    /// Called by the titlebar at the end of its layout pass.
    pub fn write(&self, regions: HitRegions) {
        if let Ok(mut g) = self.inner.lock() {
            let hovered = g.hovered_button;
            let pressed = g.pressed_button;
            *g = regions;
            g.hovered_button = hovered;
            g.pressed_button = pressed;
        }
    }

    /// Snapshot the current region set.
    ///
    /// Called by `WM_NCHITTEST`. Returns a clone so the lock is released
    /// before doing the actual point-in-rect math.
    pub fn read(&self) -> HitRegions {
        self.inner
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Update only the maximized flag — used after `WindowEvent::Resized`
    /// to keep the WndProc's resize-edge suppression in sync without
    /// rebuilding the entire region set.
    pub fn set_maximized(&self, v: bool) {
        if let Ok(mut g) = self.inner.lock() {
            g.is_maximized = v;
        }
    }

    /// Update the hovered system-button — called from the WndProc on
    /// `WM_NCMOUSEMOVE` / `WM_NCMOUSELEAVE`.
    pub fn set_hovered_button(&self, b: HoveredNcButton) {
        if let Ok(mut g) = self.inner.lock() {
            g.hovered_button = b;
        }
    }

    /// Update the currently-pressed system-button — called from the WndProc
    /// on `WM_NCLBUTTONDOWN` / `WM_NCLBUTTONUP`.
    pub fn set_pressed_button(&self, b: HoveredNcButton) {
        if let Ok(mut g) = self.inner.lock() {
            g.pressed_button = b;
        }
    }

    /// Record the current wall-clock time as the last `WM_NCLBUTTONDOWN`
    /// event. Called from the WndProc subclass on every NC left-button-down.
    pub fn mark_nc_lbuttondown(&self) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_nc_lbuttondown_ms.store(ms, Ordering::Relaxed);
    }

    /// Returns how many milliseconds have elapsed since the last
    /// `WM_NCLBUTTONDOWN`. Returns `u64::MAX` if no event has been recorded.
    pub fn nc_down_elapsed_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let stored = self.last_nc_lbuttondown_ms.load(Ordering::Relaxed);
        if stored == 0 {
            return u64::MAX;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now.saturating_sub(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_rect_contains_inclusive_left_top_exclusive_right_bottom() {
        let r = PixelRect::from_xywh(10, 20, 100, 50);
        assert!(r.contains(10, 20));
        assert!(r.contains(109, 69));
        assert!(!r.contains(110, 70));
        assert!(!r.contains(9, 20));
    }

    #[test]
    fn pixel_rect_empty() {
        let e = PixelRect::empty();
        assert!(e.is_empty());
        assert!(!e.contains(0, 0));
    }

    #[test]
    fn shared_regions_round_trip() {
        let s = SharedHitRegions::new();
        let mut r = HitRegions::default();
        r.titlebar_height = 28;
        r.max_btn = PixelRect::from_xywh(100, 0, 28, 28);
        s.write(r.clone());
        let got = s.read();
        assert_eq!(got.titlebar_height, 28);
        assert_eq!(got.max_btn.left, 100);
    }

    #[test]
    fn shared_regions_set_maximized() {
        let s = SharedHitRegions::new();
        assert!(!s.read().is_maximized);
        s.set_maximized(true);
        assert!(s.read().is_maximized);
    }
}
