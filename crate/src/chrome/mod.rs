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
//!         move |event, _, _| {
//!             if let Some(window) = w.lock().unwrap().as_ref() {
//!                 c.lock().unwrap().on_event(event, window);
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

#[cfg(windows)]
pub mod win32;

pub use config::{Buttons, CloseMode, TitleAlign, TitlebarConfig};
pub use edge::{cursor_for_edge, edge_at, resize_direction, ResizeEdge};

use std::sync::Arc;

use dear_imgui_rs::{MouseButton, Ui};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, Window};

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

// ── Stateless render functions ──────────────────────────────────────────────

/// Paint the titlebar into the current ImGui window's draw list, returning
/// the action and hovered resize edge for this frame.
///
/// Call as the **first** thing inside a full-screen, zero-padding ImGui
/// window. The caller is responsible for advancing the cursor below
/// `cfg.height` (`ui.set_cursor_pos([0.0, cfg.height])` + a zero-size
/// `dummy([0.0, 0.0])`) before rendering content beneath the titlebar.
///
/// Hover detection for the buttons uses the position-based check (no
/// `is_window_hovered`) so the chrome works correctly even when the
/// host wraps its content in a sibling child window inside the same root.
pub fn render_titlebar(
    ui: &Ui,
    cfg: &TitlebarConfig,
    title: &str,
    palette: &TitlebarColors,
    maximized: bool,
    resize_zone: f32,
    os_resizable: bool,
) -> TitlebarResult {
    let cursor = ui.cursor_screen_pos();
    let win_pos = ui.window_pos();
    let win_size = ui.window_size();
    let draw = ui.get_window_draw_list();

    let h = cfg.height;
    let sep_h = cfg.separator_height;
    let btn_w = cfg.buttons.width;
    let ir = cfg.buttons.icon_radius;
    let zoom = cfg.buttons.hover_zoom_scale;
    let [ww, wh] = win_size;
    let [mx, my] = ui.io().mouse_pos();

    let bg = c32(palette.bg);
    let separator = c32(palette.separator);
    let title_col = c32(palette.title);
    let icon_col = c32(palette.icon);
    let btn_min = c32(palette.btn_minimize);
    let btn_max = c32(palette.btn_maximize);
    let btn_close_col = c32(palette.btn_close);

    // Background.
    draw.add_rect(
        [cursor[0], cursor[1]],
        [cursor[0] + ww, cursor[1] + h],
        bg,
    )
    .filled(true)
    .build();

    // Separator.
    if cfg.separator_visible {
        draw.add_rect(
            [cursor[0], cursor[1] + h - sep_h],
            [cursor[0] + ww, cursor[1] + h],
            separator,
        )
        .filled(true)
        .build();
    }

    // Layout — buttons drawn right-to-left.
    let std_count =
        cfg.buttons.minimize as usize + cfg.buttons.maximize as usize + cfg.buttons.close as usize;
    let btn_total_w = std_count as f32 * btn_w;
    let btn_area_x = cursor[0] + ww - btn_total_w;
    let text_h = line_height(ui);
    let text_y = cursor[1] + (h - text_h) * 0.5;
    let cy_btn = cursor[1] + h * 0.5;

    // Hover detection: titlebar y-strip (cursor[1] .. cursor[1]+h). We
    // intentionally do NOT use `ui.is_window_hovered()` here — the host's
    // content child window can be a sibling of the chrome root in some
    // wiring patterns, and `is_window_hovered` returns false at hovered
    // child positions. Position-only is robust across all layouts; the
    // titlebar y-band is exclusive to chrome by construction.
    let in_row = my >= cursor[1] && my < cursor[1] + h;
    let clicked = ui.is_mouse_clicked(MouseButton::Left);
    let mut action = TitlebarAction::None;

    // Icon + title.
    let mut title_x = cursor[0] + cfg.title_padding_left;
    if let Some(ref icon) = cfg.icon {
        draw.add_text([title_x, text_y], icon_col, icon.as_str());
        title_x += calc_text_size(icon.as_str())[0] + 6.0;
    }

    if cfg.title_visible {
        match cfg.title_align {
            TitleAlign::Left => {
                draw.add_text([title_x, text_y], title_col, title);
            }
            TitleAlign::Center => {
                let tw = calc_text_size(title)[0];
                let cx = cursor[0] + (ww - btn_total_w - tw) * 0.5;
                draw.add_text([cx.max(title_x), text_y], title_col, title);
            }
        }
    }

    let mut bx = cursor[0] + ww;

    if cfg.buttons.close {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov && clicked && action == TitlebarAction::None {
            action = TitlebarAction::Close;
        }
        let r = if hov { ir * zoom } else { ir };
        glyph::draw_close(&draw, cx_btn, cy_btn, r, btn_close_col);
    }

    if cfg.buttons.maximize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov && clicked && action == TitlebarAction::None {
            action = TitlebarAction::Maximize;
        }
        let r = if hov { ir * zoom } else { ir };
        if maximized {
            glyph::draw_restore(&draw, cx_btn, cy_btn, r, btn_max);
        } else {
            glyph::draw_maximize(&draw, cx_btn, cy_btn, r, btn_max);
        }
    }

    if cfg.buttons.minimize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov && clicked && action == TitlebarAction::None {
            action = TitlebarAction::Minimize;
        }
        let r = if hov { ir * zoom } else { ir };
        glyph::draw_minimize(&draw, cx_btn, cy_btn, r, btn_min);
    }

    // Resize hover (only when OS-resizable and not maximized).
    let lx = mx - win_pos[0];
    let ly = my - win_pos[1];
    let over_buttons = in_row && mx >= btn_area_x;
    let hover_edge = if os_resizable && !over_buttons && !maximized {
        edge_at(lx, ly, ww, wh, resize_zone)
    } else {
        None
    };

    if action == TitlebarAction::None
        && clicked
        && let Some(edge) = hover_edge
    {
        action = TitlebarAction::ResizeStart(edge);
    }

    if action == TitlebarAction::None && in_row && mx < btn_area_x && hover_edge.is_none() {
        if cfg.double_click_maximize && ui.is_mouse_double_clicked(MouseButton::Left) {
            action = TitlebarAction::Maximize;
        } else if clicked {
            action = TitlebarAction::DragStart;
        }
    }

    TitlebarResult { action, hover_edge }
}

/// For chrome-less windows (splash / kiosk): detect resize edges over the
/// full window area without rendering a titlebar.
///
/// ```ignore
/// // Splash with chrome-less resize support
/// chrome.lock().unwrap().render_splash(ui, &window, |ui, area| {
///     ui.image(splash_logo).size(area.size).build();
/// });
/// ```
pub fn whole_window_resize(
    ui: &Ui,
    resize_zone: f32,
    os_resizable: bool,
    maximized: bool,
) -> TitlebarResult {
    if !os_resizable || maximized {
        return TitlebarResult::none();
    }
    let win_pos = ui.window_pos();
    let win_size = ui.window_size();
    let [mx, my] = ui.io().mouse_pos();
    let lx = mx - win_pos[0];
    let ly = my - win_pos[1];
    let edge = edge_at(lx, ly, win_size[0], win_size[1], resize_zone);
    let action = if let Some(e) = edge {
        if ui.is_mouse_clicked(MouseButton::Left) {
            TitlebarAction::ResizeStart(e)
        } else {
            TitlebarAction::None
        }
    } else {
        TitlebarAction::None
    };
    TitlebarResult {
        action,
        hover_edge: edge,
    }
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
    let scale = mon.scale_factor();
    if scale <= 0.0 {
        return requested;
    }
    let ms = mon.size();
    const RESERVE_PX: f64 = 80.0;
    let max_w = (ms.width as f64 / scale - RESERVE_PX).max(0.0);
    let max_h = (ms.height as f64 / scale - RESERVE_PX).max(0.0);
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

impl Chrome {
    /// Create a new chrome with the given titlebar configuration.
    pub fn new(config: TitlebarConfig) -> Self {
        let theme = Theme::Dark;
        let palette = theme.titlebar();
        Self {
            config,
            title: String::new(),
            theme,
            palette,
            corner_radius: 8,
            resize_zone: 6.0,
            last_cursor: CursorIcon::Default,
            last_size: (0, 0),
            last_maximized: false,
            pending_remax: false,
            pending_close: false,
        }
    }

    /// Builder: set the window title (used by the titlebar text).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Builder: set the chrome theme (palette source). Default `Theme::Dark`.
    /// Cheaper than `set_theme` since no replacement happens — caches
    /// the palette on construction.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.palette = theme.titlebar();
        self
    }

    /// Builder: set the rounded-corner radius (Win10 only — Win11 DWM
    /// owns the corners). Default `8`.
    pub fn with_corner_radius(mut self, r: i32) -> Self {
        self.corner_radius = r;
        self
    }

    /// Builder: set the edge-resize hit zone width (logical pixels).
    /// Default `6.0`.
    pub fn with_resize_zone(mut self, px: f32) -> Self {
        self.resize_zone = px.max(1.0);
        self
    }

    /// Update the title at runtime.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Update the theme at runtime — refreshes the cached palette.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.palette = theme.titlebar();
    }

    /// Read-only access to the underlying [`TitlebarConfig`].
    pub fn config(&self) -> &TitlebarConfig {
        &self.config
    }

    /// Mutable access to the underlying [`TitlebarConfig`] — use to
    /// flip button visibility, switch close mode, change height, etc.
    /// at runtime. The change applies on the next frame.
    pub fn config_mut(&mut self) -> &mut TitlebarConfig {
        &mut self.config
    }

    /// Read & clear the close-request flag. Returns `Some(close_mode)`
    /// once after the user clicked close, then `None` until the next
    /// click. The embedded [`CloseMode`] tells the host whether to
    /// exit immediately ([`CloseMode::Immediate`]) or surface a
    /// confirmation flow ([`CloseMode::Confirm`]).
    ///
    /// ```ignore
    /// match chrome.lock().unwrap().take_close_request() {
    ///     Some(CloseMode::Immediate) => std::process::exit(0),
    ///     Some(CloseMode::Confirm)   => self.show_confirm_dialog(),
    ///     None => {}
    /// }
    /// ```
    pub fn take_close_request(&mut self) -> Option<CloseMode> {
        if std::mem::replace(&mut self.pending_close, false) {
            Some(self.config.close_mode)
        } else {
            None
        }
    }

    /// One-shot setup — call from `on_gpu_init`. Strips OS chrome,
    /// applies Win32 dark mode + rounded corners, and shrinks the window
    /// if it came up at a fullscreen-equivalent size (regression guard
    /// against Windows' borderless-fullscreen heuristic on small / hi-DPI
    /// monitors).
    pub fn on_setup(&mut self, window: &Arc<Window>) {
        // Strip decorations FIRST so the rounded-region / DWM corner
        // preference is computed against the borderless geometry.
        window.set_decorations(false);

        #[cfg(windows)]
        win32::setup_window(window, self.corner_radius);

        // Defensive shrink: if the dear-app `RunnerConfig::window_size`
        // matched (or exceeded) the monitor's logical size — common
        // when the developer's machine has a 1920×1080 monitor and the
        // user is on a 1366×768 laptop — Windows treats the borderless
        // window as fullscreen, hides the taskbar, and the chrome is
        // unreachable. Resize down before showing.
        Self::shrink_to_monitor_after_create(window);

        let sz = window.inner_size();
        self.last_size = (sz.width, sz.height);
        self.last_maximized = window.is_maximized();
    }

    /// Hosts that can't call [`clamp_size_to_monitor`] before window
    /// creation (most `dear-app` users — `RunnerConfig` is built before
    /// the `EventLoop` exists) can rely on this post-create fallback.
    /// Called automatically by [`Chrome::on_setup`].
    pub fn shrink_to_monitor_after_create(window: &Arc<Window>) {
        let Some(mon) = window.current_monitor() else { return };
        let ms = mon.size();
        let inner = window.inner_size();
        const RESERVE_PX: u32 = 80;
        let max_w = ms.width.saturating_sub(RESERVE_PX).max(1);
        let max_h = ms.height.saturating_sub(RESERVE_PX).max(1);
        if inner.width >= max_w || inner.height >= max_h {
            let new_w = inner.width.min(max_w);
            let new_h = inner.height.min(max_h);
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(new_w, new_h));
        }
    }

    /// Per-event update — call from `on_event`. Tracks resize / maximise
    /// transitions so:
    ///
    /// - The Win10 clip region stays in sync with maximise / restore
    ///   (no-op on Win11 — DWM owns the corners there).
    /// - The cached `last_maximized` flag matches OS state, so per-frame
    ///   render reads it without an extra `window.is_maximized()` call.
    /// - The Win11 `pending_remax` workaround triggers when a window
    ///   that minimised from maximised state restores.
    ///
    /// Events handled: `WindowEvent::Resized`, `WindowEvent::Focused`.
    /// Everything else is forwarded to the platform handler unchanged
    /// (chrome makes no input claims). Hosts that need full event
    /// access keep their own `on_event` callback chain — chrome doesn't
    /// consume the event.
    ///
    /// **Note:** chrome does NOT inject the non-Latin keyboard fix
    /// ([`crate::input::keyboard`]). That fix requires the runner to
    /// suppress `platform.handle_event` for the same event chrome saw,
    /// which `dear-app::on_event` does not currently expose.
    pub fn on_event(&mut self, event: &winit::event::Event<()>, window: &Arc<Window>) {
        let winit::event::Event::WindowEvent { event: we, .. } = event else {
            return;
        };
        match we {
            WindowEvent::Resized(s) => {
                if s.width == 0 || s.height == 0 {
                    return;
                }
                let new_size = (s.width, s.height);
                let new_max = window.is_maximized();

                // Win11 remax workaround: if a minimised-from-maximised
                // window restores back to non-maximised, the OS doesn't
                // restore the maximised flag automatically. Re-set it.
                if self.pending_remax && !new_max {
                    self.pending_remax = false;
                    window.set_maximized(true);
                    self.last_maximized = true;
                    return;
                }

                if new_size != self.last_size || new_max != self.last_maximized {
                    self.last_size = new_size;
                    self.last_maximized = new_max;
                    #[cfg(windows)]
                    win32::sync_region(window, self.corner_radius, new_max);
                }
            }
            WindowEvent::Focused(_) => {
                // A focus change can race with maximise / minimise — re-poll.
                self.last_maximized = window.is_maximized();
            }
            _ => {}
        }
    }

    /// Per-frame render — call from `on_frame`. Wraps the host's content
    /// in a single full-display ImGui root window so:
    ///
    /// 1. The titlebar draws into the same window as the content (one
    ///    Z-layer; foreground / background draw-list overlays paint
    ///    relative to it correctly).
    /// 2. Resize-edge detection covers the **full window**, not just
    ///    the titlebar strip — drag-resize works on every edge / corner.
    /// 3. There's a single hit-test surface, so dockspace-less hosts
    ///    don't have two stacked root windows competing for click input.
    ///
    /// The host renders inside the `content` closure, which receives a
    /// [`ContentArea`] (origin + size in logical pixels, relative to the
    /// root window). The cursor is already positioned at `area.origin`
    /// when the closure fires.
    ///
    /// Drag / resize / minimise / maximise / close are dispatched to
    /// `window` automatically. Close requests surface via
    /// [`Self::take_close_request`] for the host to honour.
    pub fn render<F: FnOnce(&Ui, ContentArea)>(
        &mut self,
        ui: &Ui,
        window: &Arc<Window>,
        content: F,
    ) {
        use dear_imgui_rs::{Condition, StyleVar, WindowFlags};

        let display = ui.io().display_size();
        let _np = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _ns = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));
        let _bs = ui.push_style_var(StyleVar::WindowBorderSize(0.0));

        let h = self.config.height;
        let mut tb_result = TitlebarResult::none();
        let maximized = self.last_maximized;
        let os_resizable = !maximized;

        let root_flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_SCROLLBAR
            | WindowFlags::NO_SCROLL_WITH_MOUSE
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS;

        let area = ContentArea {
            origin: [0.0, h],
            size: [display[0], (display[1] - h).max(0.0)],
        };

        ui.window("##chrome_root")
            .position([0.0, 0.0], Condition::Always)
            .size(display, Condition::Always)
            .flags(root_flags)
            .build(|| {
                tb_result = render_titlebar(
                    ui,
                    &self.config,
                    &self.title,
                    &self.palette,
                    maximized,
                    self.resize_zone,
                    os_resizable,
                );

                ui.set_cursor_pos([0.0, h]);
                ui.dummy([0.0, 0.0]);

                content(ui, area);
            });

        // Cursor — only update when changed (avoids Win32 flicker).
        let want_cursor = cursor_for_edge(tb_result.hover_edge);
        if want_cursor != self.last_cursor {
            window.set_cursor(want_cursor);
            self.last_cursor = want_cursor;
        }

        // Dispatch actions.
        match tb_result.action {
            TitlebarAction::None => {}
            TitlebarAction::Minimize => {
                // Win11 quirk: minimising a maximised borderless window
                // can leave it in a fullscreen-like state on restore.
                // Drop maximise BEFORE minimise and re-set it via
                // `pending_remax` once the OS sends the next Resized.
                #[cfg(windows)]
                if win32::is_win11() && self.last_maximized {
                    window.set_maximized(false);
                    self.pending_remax = true;
                }
                window.set_minimized(true);
            }
            TitlebarAction::Maximize => {
                // Use cached state — avoids an extra Win32 round-trip.
                let next = !self.last_maximized;
                window.set_maximized(next);
                self.last_maximized = next;
            }
            TitlebarAction::Close => {
                self.pending_close = true;
            }
            TitlebarAction::DragStart => {
                // `clicked` is one-frame; no need to guard against
                // re-firing across frames. The OS owns the mouse during
                // the modal drag — winit won't deliver further click
                // events until the drag ends.
                let _ = window.drag_window();
            }
            TitlebarAction::ResizeStart(edge) => {
                let _ = window.drag_resize_window(resize_direction(edge));
            }
        }
    }

    /// Read the configured titlebar height (logical px). Useful when the
    /// host wants to position content manually rather than using the
    /// [`ContentArea`] from [`Self::render`].
    pub fn titlebar_height(&self) -> f32 {
        self.config.height
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#[inline]
fn c32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
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
}
