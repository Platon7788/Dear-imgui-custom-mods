//! # app_window
//!
//! Borderless-window framework. Custom Dear ImGui titlebar; native OS
//! resize, Aero Snap, drop shadow, taskbar / Alt-Tab integration preserved.
//!
//! ## Architecture (Windows)
//!
//! Borderless via the **post-creation** route: the window is created
//! with normal `WS_OVERLAPPEDWINDOW` decorations, then after wgpu is
//! initialised [`startup`] calls `window.set_decorations(false)`.
//! winit's `MARKER_DECORATIONS` flag flips, `SetWindowPos(SWP_FRAMECHANGED)`
//! triggers a `WM_NCCALCSIZE` recalc, and from that point winit's own
//! handler returns `0` — the visual frame is gone, but DWM had a chance
//! to fully initialise composition with proper chrome metrics first.
//! This sequence avoids the phantom NC-frame artifacts that creating
//! frameless from the start can cause on high-DPI laptops, hybrid GPU
//! machines, and configurations with non-standard shell extensions.
//! Verified against the reference at
//! `D:\\GitHub\\Rust_Projects\\test-dear-imgui-rs`.
//!
//! Win32-side helpers (in [`win32::setup_window`]) are minimal:
//! - `DWMWA_USE_IMMERSIVE_DARK_MODE` for the Alt-Tab thumbnail.
//! - `DWMWA_WINDOW_CORNER_PREFERENCE` (Win11) or `SetWindowRgn`
//!   (Win10 fallback) for rounded corners.
//! - `WS_EX_TOOLWINDOW` for tool-window kinds.
//! - `set_opacity` toggles `WS_EX_LAYERED` on demand.
//!
//! Notably absent: no `WM_NCCALCSIZE` / `WM_NCHITTEST` / `WM_GETMINMAXINFO`
//! subclass. winit owns those handlers — its NCCALCSIZE already clamps
//! `rgrc[0]` to the monitor work area on maximise, so the taskbar stays
//! visible without us doing anything; native edge-resize works through
//! `WS_OVERLAPPEDWINDOW` + `DefWindowProc`.
//!
//! The titlebar itself is pure Dear ImGui — buttons, drag, double-click
//! are all drawn into the ImGui draw list and dispatched to the OS via
//! [`winit::window::Window::drag_window`] / `drag_resize_window`.
//!
//! Builder presets — [`AppConfig::splash`], [`AppConfig::tool`],
//! [`AppConfig::dialog`], [`AppConfig::main`] — give sensible defaults;
//! the builders ([`AppConfig::with_*`]) tune anything else.
//!
//! ## Module layout
//!
//! - [`config`]   — declarative config types: [`AppConfig`] + presets + builders.
//! - [`chrome`]   — custom titlebar render + resize-edge detection.
//! - [`gpu`]      — wgpu / Dear ImGui setup + per-frame render loop.
//! - [`state`]    — runtime state container ([`AppState`]).
//! - [`handler`]  — [`AppHandler`] application-logic trait.
//! - `win32`     — Windows-specific glue (subclass, DWM, opacity).
//! - [`startup`]  — GPU + ImGui init logic (`resumed` body).
//! - [`dispatch`] — Per-frame event dispatch and idle scheduling.
//!
//! ## Example: main window
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::app_window::{AppConfig, AppHandler, AppState, AppWindow};
//! use dear_imgui_rs::Ui;
//!
//! struct MyApp;
//! impl AppHandler for MyApp {
//!     fn render(&mut self, ui: &Ui, _state: &mut AppState) {
//!         ui.text("Hello, world!");
//!     }
//! }
//!
//! fn main() {
//!     AppWindow::new(AppConfig::main("My App", 1100.0, 680.0))
//!         .run(MyApp)
//!         .unwrap();
//! }
//! ```
//!
//! ## Example: splash with auto-close
//!
//! ```rust,no_run
//! use std::time::Duration;
//! use dear_imgui_custom_mod::app_window::{AppConfig, AppWindow};
//! # use dear_imgui_custom_mod::app_window::{AppHandler, AppState};
//! # use dear_imgui_rs::Ui;
//! # struct Splash; impl AppHandler for Splash {
//! #   fn render(&mut self, _: &Ui, _: &mut AppState) {} }
//!
//! AppWindow::new(
//!     AppConfig::splash("Loading…", 600.0, 400.0)
//!         .with_auto_close(Duration::from_secs(3))
//!         .with_corner_radius(16),
//! ).run(Splash).unwrap();
//! ```
//!
//! ## Example: tool window
//!
//! ```rust,no_run
//! # use dear_imgui_custom_mod::app_window::{AppConfig, AppWindow, AppHandler, AppState};
//! # use dear_imgui_rs::Ui;
//! # struct Props; impl AppHandler for Props {
//! #   fn render(&mut self, _: &Ui, _: &mut AppState) {} }
//! AppWindow::new(
//!     AppConfig::tool("Properties", 320.0, 480.0).stay_on_top()
//! ).run(Props).unwrap();
//! ```
//!
//! ## Example: dialog
//!
//! ```rust,no_run
//! # use dear_imgui_custom_mod::app_window::{AppConfig, AppWindow, AppHandler, AppState};
//! # use dear_imgui_rs::Ui;
//! # struct Confirm; impl AppHandler for Confirm {
//! #   fn render(&mut self, _: &Ui, _: &mut AppState) {} }
//! AppWindow::new(
//!     AppConfig::dialog("Confirm", 400.0, 150.0)
//! ).run(Confirm).unwrap();
//! ```

#![allow(missing_docs)]

pub mod chrome;
pub mod config;
pub mod handler;
pub mod proxy;
pub mod state;

mod dispatch;
mod gpu;
mod startup;
#[cfg(windows)]
mod win32;

pub use crate::theme::Theme;
pub use chrome::{ResizeEdge, TitlebarAction, TitlebarResult};
pub use config::{
    AppConfig, BorderStyle, Buttons, Chrome, CloseMode, ExtraButton, FontChoice, FontLayer,
    FormStyle, FpsMode, GlyphRanges, Position, PowerMode, RenderMode, TitleAlign, TitlebarConfig,
    WindowIcon, WindowKind,
};
pub use handler::AppHandler;
pub use proxy::AppProxy;
pub use state::{AppState, TitlebarState};

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
};

// ── win32 cross-module debug log ──────────────────────────────────────────────

/// Cross-module debug log. On Windows this routes through `OutputDebugStringW`
/// so messages survive `windows_subsystem = "windows"` (where stderr is gone).
/// On other platforms it's a no-op — `eprintln!` covers them.
#[cfg(windows)]
pub(crate) fn win32_debug_log(msg: &str) {
    win32::debug_log(msg);
}

#[cfg(not(windows))]
pub(crate) fn win32_debug_log(_msg: &str) {}

// ── AppWindow ─────────────────────────────────────────────────────────────────

/// A managed application window. Wraps wgpu, winit and Dear ImGui.
pub struct AppWindow {
    config: AppConfig,
}

impl AppWindow {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    /// Run the event loop, blocking until the window closes.
    pub fn run<H: AppHandler + 'static>(
        self,
        handler: H,
    ) -> Result<(), winit::error::EventLoopError> {
        // Use the typed-event variant so background threads can wake the
        // loop via `EventLoopProxy::send_event(())`. The carrier type is
        // unit — payload data flows through user-owned mpsc/oneshot channels.
        let event_loop = EventLoop::<()>::with_user_event().build()?;
        let proxy = AppProxy::new(event_loop.create_proxy());
        let mut app = WinitApp::new(self.config, handler, proxy);
        event_loop.run_app(&mut app)
    }
}

// ── Internal winit application ────────────────────────────────────────────────

struct WinitApp<H: AppHandler> {
    config: AppConfig,
    handler: Option<H>,
    gpu: Option<gpu::GpuState>,
    proxy: AppProxy,
    on_ready_fired: bool,
}

impl<H: AppHandler> WinitApp<H> {
    fn new(config: AppConfig, handler: H, proxy: AppProxy) -> Self {
        Self {
            config,
            handler: Some(handler),
            gpu: None,
            proxy,
            on_ready_fired: false,
        }
    }
}

impl<H: AppHandler + 'static> ApplicationHandler<()> for WinitApp<H> {
    /// Cross-thread wake-up via [`AppProxy::wake`]. Bumps the redraw
    /// budget but **does NOT issue `request_redraw()` directly** —
    /// that would bypass the FPS cap. The `about_to_wait` scheduler
    /// (see `dispatch::schedule`) honours `g.fps_interval` /
    /// `g.idle_pulse` and only emits a redraw request once the cap
    /// allows it. Two frames so any background-mutated state still
    /// propagates to the next stable hover frame.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
        if let Some(g) = self.gpu.as_mut() {
            g.pending_frames = g.pending_frames.max(2);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        startup::init(self, event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        dispatch::handle_window_event(self, event_loop, window_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        dispatch::schedule(self, event_loop);
    }
}
