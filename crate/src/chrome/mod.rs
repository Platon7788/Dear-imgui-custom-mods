//! # chrome
//!
//! Borderless-window helpers — custom Dear ImGui titlebar, edge-resize
//! detection, Win32 setup. **Stateless / explicit-state design**: the
//! chrome doesn't own a runner; the host wires these helpers into its
//! own event loop (typically [`dear-app`]) via the `on_gpu_init`,
//! `on_event`, and `on_frame` callbacks.
//!
//! ## Architecture
//!
//! - [`TitlebarConfig`] / [`Buttons`] — declarative configuration
//!   (schema in `config.rs`, defaults in `config.ron`).
//! - [`render_titlebar`] — paint the titlebar inside an ImGui window
//!   and return a [`TitlebarAction`] for the host to dispatch.
//! - [`Chrome`] — convenience wrapper that bundles the per-frame state
//!   (cursor, hover edge, maximised tracking) and translates
//!   [`TitlebarAction`] into actual `winit::Window` calls.
//! - [`win32::setup_window`] / [`win32::sync_region`] / [`win32::set_opacity`]
//!   — Win32-only: DWM dark mode, rounded corners, Win10 region sync,
//!   opacity (`WS_EX_LAYERED`).
//! - [`clamp_size_to_monitor`] — pre-window-creation guard against
//!   Windows' borderless-fullscreen heuristic (clamp `RunnerConfig::window_size`
//!   strictly below monitor work-area).
//!
//! ## Wiring with `dear-app`
//!
//! ```ignore
//! use std::sync::{Arc, Mutex};
//! use dear_app::{AppBuilder, DockingConfig, RunnerConfig};
//! use dear_imgui_custom_mod::chrome::{Chrome, TitlebarConfig};
//! use dear_imgui_custom_mod::theme::Theme;
//!
//! let chrome = Arc::new(Mutex::new(
//!     Chrome::new(TitlebarConfig::default())
//!         .with_title("My App")
//!         .with_theme(Theme::Dark),
//! ));
//! let win_stash: Arc<Mutex<Option<Arc<winit::window::Window>>>> = Default::default();
//!
//! AppBuilder::new()
//!     .with_config(RunnerConfig {
//!         window_title: "My App".into(),
//!         window_size: (1100.0, 700.0),
//!         // CRITICAL: dear-app's auto-dockspace would absorb every
//!         // click before chrome / toolbars see it — disable.
//!         docking: DockingConfig {
//!             enable: false,
//!             auto_dockspace: false,
//!             ..Default::default()
//!         },
//!         ..Default::default()
//!     })
//!     .on_gpu_init({
//!         let c = chrome.clone();
//!         let w = win_stash.clone();
//!         move |window, _, _, _| {
//!             c.lock().unwrap().on_setup(window);
//!             *w.lock().unwrap() = Some(window.clone());
//!         }
//!     })
//!     .on_event({
//!         let c = chrome.clone();
//!         let w = win_stash.clone();
//!         move |event, _, ctx| {
//!             if let Some(window) = w.lock().unwrap().as_ref() {
//!                 c.lock().unwrap().on_event(event, window, ctx);
//!             }
//!         }
//!     })
//!     .on_frame({
//!         let c = chrome.clone();
//!         let w = win_stash.clone();
//!         move |ui, _| {
//!             let Some(window) = w.lock().unwrap().clone() else { return };
//!             c.lock().unwrap().render(ui, &window, |ui, _area| {
//!                 ui.text("Content goes inside the chrome content area.");
//!             });
//!             if c.lock().unwrap().take_close_request().is_some() {
//!                 std::process::exit(0);
//!             }
//!         }
//!     })
//!     .run().unwrap();
//! ```

mod config;
mod edge;
mod glyph;
// Split out of mod.rs (CLAUDE.md: keep files < 500 lines).
mod render;
mod state;

#[cfg(windows)]
pub mod win32;

pub use config::{Buttons, CloseMode, TitleAlign, TitlebarConfig};
pub use edge::{ResizeEdge, cursor_for_edge, edge_at, resize_direction};
// Keep `chrome::render_titlebar` / `whole_window_resize` paths stable.
pub use render::{render_titlebar, whole_window_resize};

use std::sync::Arc;

use dear_imgui_rs::{MouseButton, PopupFlags, Ui};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, Window};

/// Logical-pixel reserve we keep between the chrome window and the
/// monitor's logical edge. Wide enough to cover a taskbar across DPI
/// scales (100 % – 250 %). Single source of truth for both
/// [`clamp_size_to_monitor`] (pre-create) and
/// [`Chrome::shrink_to_monitor_after_create`] (post-create) — the two
/// previously held independent magic numbers and the post-create path
/// silently ignored DPI scaling.
const MONITOR_RESERVE_LOGICAL_PX: f64 = 80.0;

use crate::theme::{Theme, TitlebarColors};
use crate::utils::color::rgba_f32;
use crate::utils::text::{calc_text_size, line_height};

// ── Public action / result types ─────────────────────────────────────────────

/// Action produced by the titlebar each frame. The host dispatches these
/// to `winit::Window` (or its own state). [`Chrome`] does this for you.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TitlebarAction {
    /// No interaction this frame.
    None,
    /// Minimise was clicked. The host should call `window.set_minimized(true)`.
    Minimize,
    /// Maximise / restore was clicked or the title was double-clicked.
    /// Host should toggle `window.is_maximized()`.
    Maximize,
    /// Close was clicked. Host should honour
    /// [`Chrome::take_close_request`] (or, for stateless callers,
    /// inspect [`TitlebarConfig::close_mode`]).
    Close,
    /// User initiated a window drag (clicked the title area). The host
    /// should call `window.drag_window()`.
    DragStart,
    /// User clicked an edge / corner. The host should call
    /// `window.drag_resize_window(resize_direction(edge))`.
    ResizeStart(ResizeEdge),
}

/// Frame-result returned by [`render_titlebar`].
#[derive(Debug, Clone, Copy)]
#[must_use = "titlebar actions must be dispatched"]
pub struct TitlebarResult {
    /// Action triggered this frame. Mostly `None`.
    pub action: TitlebarAction,
    /// `Some(edge)` while the cursor hovers a resize edge or corner;
    /// the host should set the matching cursor via [`cursor_for_edge`].
    pub hover_edge: Option<ResizeEdge>,
}

impl TitlebarResult {
    /// Empty result — no action, no hovered edge.
    pub fn none() -> Self {
        Self {
            action: TitlebarAction::None,
            hover_edge: None,
        }
    }
}

/// Where the host content should be rendered, in **logical pixels** from
/// the origin of the active ImGui window.
#[derive(Debug, Clone, Copy)]
pub struct ContentArea {
    /// Top-left of the content rect, relative to the chrome's root window.
    pub origin: [f32; 2],
    /// Available width / height in logical pixels.
    pub size: [f32; 2],
}

// ── Fullscreen-clamp helper ──────────────────────────────────────────────────

/// Clamp a logical-pixel `(width, height)` request so the window can never
/// open at exactly the monitor's logical size.
///
/// **Why:** Windows DWM treats a borderless window covering the full
/// monitor rect as **fullscreen** — the taskbar disappears, the window
/// is promoted to fullscreen Z-order, and the user thinks the app
/// crashed the desktop. Reserving an 80-logical-px buffer (covers a
/// taskbar across DPI scales) keeps us strictly windowed.
///
/// Call this **before** building [`dear_app::RunnerConfig::window_size`]
/// when an [`ActiveEventLoop`] is in scope (e.g. from a
/// `winit::event_loop::EventLoopBuilder` trait impl, or by spawning a
/// throw-away event loop just to read the primary monitor). For hosts
/// that can't get an `ActiveEventLoop` before window creation, see
/// [`Chrome::shrink_to_monitor_after_create`] which clamps after the
/// window opens.
///
/// `min_size` (when set) is honoured as a floor — if the monitor is
/// smaller than the configured minimum, the minimum wins and the host
/// gets to deal with the over-large request manually.
pub fn clamp_size_to_monitor(
    event_loop: &ActiveEventLoop,
    requested: (f64, f64),
    min_size: Option<(f64, f64)>,
) -> (f64, f64) {
    let Some(mon) = event_loop.primary_monitor() else {
        return requested;
    };
    let ms = mon.size();
    clamp_size_logic(
        ms.width as f64,
        ms.height as f64,
        mon.scale_factor(),
        requested,
        min_size,
    )
}

/// Pure math behind [`clamp_size_to_monitor`], factored out so the subtle
/// DPI / min-size clamping can be unit-tested without a live `winit`
/// monitor. `mon_phys_*` are the monitor's physical pixels; `scale` is its
/// DPI scale factor. A non-positive `scale` passes the request through.
fn clamp_size_logic(
    mon_phys_w: f64,
    mon_phys_h: f64,
    scale: f64,
    requested: (f64, f64),
    min_size: Option<(f64, f64)>,
) -> (f64, f64) {
    if scale <= 0.0 {
        return requested;
    }
    let max_w = (mon_phys_w / scale - MONITOR_RESERVE_LOGICAL_PX).max(0.0);
    let max_h = (mon_phys_h / scale - MONITOR_RESERVE_LOGICAL_PX).max(0.0);
    let (min_w, min_h) = min_size.unwrap_or((200.0, 150.0));
    let w = requested.0.min(max_w).max(min_w.min(requested.0));
    let h = requested.1.min(max_h).max(min_h.min(requested.1));
    (w, h)
}

// ── Stateful Chrome wrapper ──────────────────────────────────────────────────

/// Convenience wrapper that holds the per-frame chrome state and dispatches
/// titlebar actions to a `winit::Window`. Use this when you don't want to
/// hand-roll the state tracking yourself.
///
/// Wires into a `dear-app`-style runner with three callbacks:
///
/// - [`Chrome::on_setup`] — called from `on_gpu_init`.
/// - [`Chrome::on_event`] — called from `on_event`.
/// - [`Chrome::render`]   — called from `on_frame`.
pub struct Chrome {
    config: TitlebarConfig,
    title: String,
    /// Theme used for `Chrome::render`'s palette. Hosts that need
    /// runtime-mutable themes should call [`render_titlebar`] directly
    /// with their own palette and manage state.
    theme: Theme,
    /// Cached titlebar palette derived from `theme` — refreshed by
    /// [`Chrome::set_theme`]. Avoids rebuilding the 8-colour palette
    /// every frame (Theme::titlebar() is `match` over 7 variants and
    /// 8 RGBA literals — tiny but noisy in profile traces).
    palette: TitlebarColors,
    corner_radius: i32,
    resize_zone: f32,
    last_cursor: CursorIcon,
    last_size: (u32, u32),
    last_maximized: bool,
    /// Win11-only. When `true`, the next `Resized` after a minimise from
    /// maximised state will re-set the maximised flag — workaround for
    /// the "minimise from maximised leaves window in fullscreen-like
    /// state" Win11 quirk that lived in the deleted `app_window` runner.
    pending_remax: bool,
    pending_close: bool,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#[inline]
fn c32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// Pure math behind [`Chrome::shrink_to_monitor_after_create`], factored out
/// for unit testing. Returns the clamped `(width, height)` in physical pixels
/// when the window is at/over the monitor-minus-reserve cap, else `None` (no
/// resize needed). The LOGICAL reserve is scaled to physical via `scale` so a
/// 200 % display reserves 160 px (not the DPI-blind 80).
fn shrink_size_logic(
    mon_phys_w: u32,
    mon_phys_h: u32,
    scale: f64,
    inner: (u32, u32),
) -> Option<(u32, u32)> {
    let reserve_phys = if scale > 0.0 {
        (MONITOR_RESERVE_LOGICAL_PX * scale).round() as u32
    } else {
        MONITOR_RESERVE_LOGICAL_PX as u32
    };
    let max_w = mon_phys_w.saturating_sub(reserve_phys).max(1);
    let max_h = mon_phys_h.saturating_sub(reserve_phys).max(1);
    if inner.0 >= max_w || inner.1 >= max_h {
        Some((inner.0.min(max_w), inner.1.min(max_h)))
    } else {
        None
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titlebar_result_none() {
        let r = TitlebarResult::none();
        assert_eq!(r.action, TitlebarAction::None);
        assert!(r.hover_edge.is_none());
    }

    #[test]
    fn content_area_dimensions_match_display() {
        // Pure construction test — no ImGui context needed.
        let area = ContentArea {
            origin: [0.0, 28.0],
            size: [1280.0, 692.0],
        };
        assert_eq!(area.origin[1], 28.0);
        assert_eq!(area.origin[1] + area.size[1], 720.0);
    }

    // ── Chrome state machine ─────────────────────────────────────────────

    fn fresh_chrome() -> Chrome {
        Chrome::new(TitlebarConfig::default()).with_title("test")
    }

    #[test]
    fn chrome_take_close_request_starts_empty() {
        let mut c = fresh_chrome();
        assert!(c.take_close_request().is_none());
    }

    #[test]
    fn chrome_take_close_request_one_shot_immediate() {
        let mut c = fresh_chrome();
        c.pending_close = true;
        assert_eq!(c.take_close_request(), Some(CloseMode::Immediate));
        // Second call returns None — flag was consumed.
        assert!(c.take_close_request().is_none());
    }

    #[test]
    fn chrome_take_close_request_one_shot_confirm() {
        let mut c = fresh_chrome();
        c.config_mut().close_mode = CloseMode::Confirm;
        c.pending_close = true;
        assert_eq!(c.take_close_request(), Some(CloseMode::Confirm));
        assert!(c.take_close_request().is_none());
    }

    #[test]
    fn chrome_set_theme_refreshes_palette() {
        let mut c = fresh_chrome();
        let dark_bg = c.palette.bg;
        c.set_theme(Theme::Light);
        assert_ne!(c.palette.bg, dark_bg, "palette must change on theme switch");
    }

    #[test]
    fn chrome_with_theme_is_eager() {
        let c = Chrome::new(TitlebarConfig::default()).with_theme(Theme::Light);
        let expected = Theme::Light.titlebar();
        assert_eq!(c.palette.bg, expected.bg);
    }

    #[test]
    fn chrome_config_mut_updates_apply() {
        let mut c = fresh_chrome();
        assert!(c.config().buttons.minimize);
        c.config_mut().buttons.minimize = false;
        assert!(!c.config().buttons.minimize);
    }

    #[test]
    fn chrome_resize_zone_clamped() {
        let c = Chrome::new(TitlebarConfig::default()).with_resize_zone(0.0);
        assert_eq!(c.resize_zone, 1.0, "zone clamps to a minimum 1 logical px");
    }

    #[test]
    fn chrome_titlebar_height_reads_config() {
        let c = Chrome::new(TitlebarConfig::tool());
        assert_eq!(c.titlebar_height(), 22.0);
    }

    #[test]
    fn chrome_theme_getter_matches_set() {
        let mut c = fresh_chrome();
        assert_eq!(c.theme(), Theme::Dark, "default theme");
        c.set_theme(Theme::Light);
        assert_eq!(c.theme(), Theme::Light, "getter follows set_theme");
    }

    // ── clamp_size_logic ─────────────────────────────────────────────────

    #[test]
    fn clamp_passthrough_on_invalid_scale() {
        let req = (1920.0, 1080.0);
        assert_eq!(clamp_size_logic(2560.0, 1440.0, 0.0, req, None), req);
        assert_eq!(clamp_size_logic(2560.0, 1440.0, -1.0, req, None), req);
    }

    #[test]
    fn clamp_unchanged_when_request_fits() {
        // 1920×1080 monitor @100%, request 1100×700 → fits under the
        // (1920-80, 1080-80) cap unchanged.
        let (w, h) = clamp_size_logic(1920.0, 1080.0, 1.0, (1100.0, 700.0), None);
        assert_eq!((w, h), (1100.0, 700.0));
    }

    #[test]
    fn clamp_caps_oversized_request_below_monitor() {
        // Request equals the monitor → must shrink by the 80 px reserve.
        let (w, h) = clamp_size_logic(1920.0, 1080.0, 1.0, (1920.0, 1080.0), None);
        assert_eq!((w, h), (1840.0, 1000.0));
    }

    #[test]
    fn clamp_min_size_wins_on_tiny_monitor() {
        // Monitor smaller than the min → the configured minimum wins, even
        // though it exceeds the monitor-minus-reserve cap.
        let (w, h) = clamp_size_logic(240.0, 200.0, 1.0, (1000.0, 800.0), Some((400.0, 300.0)));
        assert_eq!((w, h), (400.0, 300.0));
    }

    #[test]
    fn clamp_respects_dpi_scale() {
        // 3840×2160 physical @200% = 1920×1080 logical; reserve 80 logical →
        // cap 1840×1000.
        let (w, h) = clamp_size_logic(3840.0, 2160.0, 2.0, (1920.0, 1080.0), None);
        assert_eq!((w, h), (1840.0, 1000.0));
    }

    // ── shrink_size_logic ────────────────────────────────────────────────

    #[test]
    fn shrink_none_when_within_cap() {
        // 1280×720 window on a 1920×1080 monitor @100% → well under cap.
        assert_eq!(shrink_size_logic(1920, 1080, 1.0, (1280, 720)), None);
    }

    #[test]
    fn shrink_clamps_fullscreen_sized_window() {
        // Window == monitor → must shrink to monitor-minus-(80 logical) reserve.
        assert_eq!(
            shrink_size_logic(1920, 1080, 1.0, (1920, 1080)),
            Some((1840, 1000))
        );
    }

    #[test]
    fn shrink_reserve_scales_with_dpi() {
        // @200% the 80-logical reserve becomes 160 physical px.
        assert_eq!(
            shrink_size_logic(3840, 2160, 2.0, (3840, 2160)),
            Some((3680, 2000))
        );
    }

    #[test]
    fn shrink_saturates_on_tiny_monitor() {
        // Monitor smaller than the reserve → cap saturates to 1, window clamps.
        assert_eq!(shrink_size_logic(40, 40, 1.0, (100, 100)), Some((1, 1)));
    }
}
