//! # app_window
//!
//! Zero-boilerplate borderless window with wgpu + winit + Dear ImGui.
//!
//! ## Minimal Example
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::app_window::{AppConfig, AppHandler, AppState, AppWindow};
//! use dear_imgui_rs::Ui;
//!
//! struct MyApp;
//!
//! impl AppHandler for MyApp {
//!     fn render(&mut self, ui: &Ui, _state: &mut AppState) {
//!         ui.window("Hello").build(|| { ui.text("Hello from AppWindow!"); });
//!     }
//! }
//!
//! fn main() {
//!     AppWindow::new(AppConfig::new("My App", 1024.0, 768.0))
//!         .run(MyApp)
//!         .expect("run");
//! }
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
pub mod gpu;
pub mod state;
pub mod style;

pub use config::{AppConfig, FpsMode, PowerMode, StartPosition};
pub use state::AppState;
pub use style::apply_imgui_style_for_theme;

pub use crate::borderless_window::{
    BorderlessConfig, ButtonConfig, CloseMode, ExtraButton, TitleAlign,
};
pub use crate::theme::Theme;

use dear_imgui_rs::Ui;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

// ── Constants ────────────────────────────────────────────────────────────────

/// Window inside which a `WindowEvent::Focused(false)` arriving after a
/// `DragStart` / `ResizeStart` is treated as a Win11 WM_ACTIVATE artefact
/// and dropped instead of dimming the titlebar. 250 ms is comfortably above
/// the SendMessage roundtrip latency seen on Win11 24H2 while staying under
/// the perceptible Alt-Tab / click-out-of-window threshold.
const FOCUS_DEBOUNCE: Duration = Duration::from_millis(250);

// ── AppHandler trait ──────────────────────────────────────────────────────────

/// Implement this trait to provide your application's render logic.
///
/// All methods have default implementations so you only override what you need.
pub trait AppHandler {
    /// Called every frame inside the full-screen ImGui window,
    /// **after** the titlebar has been rendered.
    ///
    /// The cursor is already positioned below the titlebar — call
    /// `ui.content_region_avail()` for the available space.
    fn render(&mut self, ui: &Ui, state: &mut AppState);

    /// Called when a close is requested (via close button or OS).
    ///
    /// Default: confirm immediately (`state.exit()`).
    /// Override to show a custom confirmation dialog.
    fn on_close_requested(&mut self, state: &mut AppState) {
        state.exit();
    }

    /// Called when a custom extra-button in the titlebar is clicked.
    ///
    /// `id` is the [`ExtraButton::id`](crate::borderless_window::ExtraButton::id).
    fn on_extra_button(&mut self, _id: &'static str, _state: &mut AppState) {}

    /// Called when the window icon (if set) is clicked.
    ///
    /// Override to show a custom context menu or handle the click freely.
    fn on_icon_click(&mut self, _state: &mut AppState) {}

    /// Called after the theme changes (e.g., from an extra button).
    ///
    /// Default: no-op. Override to apply your own imgui style.
    fn on_theme_changed(&mut self, _theme: &Theme, _state: &mut AppState) {}
}

// ── AppWindow ─────────────────────────────────────────────────────────────────

/// A fully managed borderless application window.
///
/// Wraps wgpu, winit, and Dear ImGui setup so your application code only
/// needs to implement [`AppHandler`].
///
/// # Example
/// ```rust,no_run
/// use dear_imgui_custom_mod::app_window::{AppConfig, AppHandler, AppState, AppWindow};
/// use dear_imgui_rs::Ui;
///
/// struct MyApp;
/// impl AppHandler for MyApp {
///     fn render(&mut self, ui: &Ui, _state: &mut AppState) {
///         ui.window("Hello").build(|| { ui.text("world"); });
///     }
/// }
///
/// fn main() {
///     AppWindow::new(AppConfig::new("Hello", 800.0, 600.0))
///         .run(MyApp)
///         .expect("run");
/// }
/// ```
pub struct AppWindow {
    config: AppConfig,
}

impl AppWindow {
    /// Create a new `AppWindow` with the given configuration.
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    /// Run the application event loop, blocking until the window closes.
    pub fn run<H: AppHandler + 'static>(
        self,
        handler: H,
    ) -> Result<(), winit::error::EventLoopError> {
        let event_loop = EventLoop::new()?;
        // Initial ControlFlow is set per-frame inside about_to_wait based on fps_mode.
        let mut app = WinitApp::new(self.config, handler);
        event_loop.run_app(&mut app)
    }
}

// ── Internal winit application ────────────────────────────────────────────────

struct WinitApp<H: AppHandler> {
    config: AppConfig,
    handler: Option<H>,
    gpu: Option<gpu::GpuState>,
}

impl<H: AppHandler> WinitApp<H> {
    fn new(config: AppConfig, handler: H) -> Self {
        Self {
            config,
            handler: Some(handler),
            gpu: None,
        }
    }
}

impl<H: AppHandler + 'static> ApplicationHandler for WinitApp<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }

        let cfg = &self.config;
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(cfg.title.clone())
                        .with_inner_size(LogicalSize::new(cfg.size[0], cfg.size[1]))
                        .with_min_inner_size(LogicalSize::new(cfg.min_size[0], cfg.min_size[1]))
                        .with_decorations(false)
                        .with_resizable(true)
                        .with_visible(false),
                )
                .expect("failed to create window"),
        );

        // Apply DWM dark mode + rounded corners on Windows before showing the window.
        // Win11 handles the rounding itself; on Win10 the SetWindowRgn fallback is
        // re-applied after each resize (see WindowEvent::Resized below).
        #[cfg(windows)]
        if let Some(hwnd) = crate::borderless_window::platform::hwnd_of(&window) {
            crate::borderless_window::platform::set_titlebar_dark_mode(hwnd, true);
            crate::borderless_window::platform::set_rounded_corners(hwnd, cfg.corner_radius);
        }

        // Position the window before showing it.
        gpu::position_window(&window, &cfg.start_position, event_loop);
        window.set_visible(true);

        // Initialise wgpu with the configured power preference.
        let (device, queue, surface, surface_cfg) = gpu::init_wgpu(&window, cfg.power_mode);
        let surface_format = surface_cfg.format;

        // Initialise Dear ImGui.
        let (context, platform, renderer) = gpu::init_imgui(
            &window,
            device.clone(),
            queue.clone(),
            surface_format,
            cfg.font_size,
            &cfg.titlebar,
            cfg.merge_mdi_icons,
        );

        let fps_interval = match cfg.fps_mode {
            // Auto: Poll + wgpu Fifo vsync.
            // get_current_texture() blocks until the GPU is ready for the next
            // vsync, so frame timing is controlled by display hardware rather
            // than the OS timer (which on Windows has ~15.6 ms granularity and
            // causes the stutter visible in progress-bar animations).
            FpsMode::Auto | FpsMode::Unlimited => Duration::ZERO,
            FpsMode::Fixed(n) if n > 0 => Duration::from_secs_f64(1.0 / n as f64),
            FpsMode::Fixed(_) => Duration::ZERO,
        };

        self.gpu = Some(gpu::GpuState {
            device,
            queue,
            window,
            surface_cfg,
            surface,
            context,
            platform,
            renderer,
            app_state: AppState::new(),
            titlebar_cfg: cfg.titlebar.clone(),
            fps_interval,
            last_hover_edge_set: None,
            cursor_set: false,
            last_drag_at: None,
            pending_remax: false,
            was_minimized: false,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let (Some(g), Some(handler)) = (self.gpu.as_mut(), self.handler.as_mut()) else {
            return;
        };

        // Layout-independent keyboard / IME fixes — see
        // `app_window_v2::window_event` for the full rationale.
        // Russian / Greek / CJK layouts otherwise break `Ctrl+C`,
        // `Ctrl+V`, numpad digit input, and IME commits.
        let mut kbd_handled = false;
        let mut kbd_event: Option<winit::event::KeyEvent> = None;
        match &event {
            WindowEvent::KeyboardInput { event: ke, .. } => {
                let io = g.context.io_mut();
                if crate::input::keyboard::try_inject_numpad_text(io, ke)
                    || crate::input::keyboard::try_inject_ctrl_alt_shortcut(io, ke)
                {
                    kbd_handled = true;
                } else {
                    kbd_event = Some(ke.clone());
                }
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                crate::input::keyboard::inject_ime_commit(g.context.io_mut(), text);
                kbd_handled = true;
            }
            _ => {}
        }

        if !kbd_handled {
            // Forward events to the winit platform for Dear ImGui input.
            g.platform.handle_event::<()>(
                &mut g.context,
                &g.window,
                &Event::WindowEvent {
                    window_id,
                    event: event.clone(),
                },
            );
            // Reinforce physical-key state after the platform forward —
            // ensures the eventual release matches the press we recorded
            // (otherwise `Key::C` stays "stuck" when Ctrl is released
            // before the letter on non-Latin layouts).
            if let Some(ref ke) = kbd_event {
                crate::input::keyboard::reinforce_physical_key_state(g.context.io_mut(), ke);
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                handler.on_close_requested(&mut g.app_state);
                if g.app_state.should_exit {
                    event_loop.exit();
                }
            }
            WindowEvent::Focused(focused) => {
                // Win11 fires a spurious `WM_ACTIVATE(WA_INACTIVE)` during the
                // `ReleaseCapture + SendMessage(WM_NCLBUTTONDOWN)` roundtrip
                // that begins an OS-driven titlebar drag or border resize.
                // Without debouncing, the titlebar dim/undim flickers visibly
                // when `focus_dim = true`. We ignore Focused(false) for a
                // short window after the most recent drag/resize dispatch.
                if !focused
                    && let Some(t) = g.last_drag_at
                    && t.elapsed() < FOCUS_DEBOUNCE
                {
                    return;
                }
                g.app_state.titlebar.set_focused(focused);
            }
            WindowEvent::Resized(s) => {
                // Track minimize transition (used by the Win11 minimize-from-max
                // workaround to detect when the user brings the window back from
                // the taskbar so we can re-apply maximize).
                let is_minimized = g.window.is_minimized().unwrap_or(false);
                let restored_from_min = g.was_minimized && !is_minimized;
                g.was_minimized = is_minimized;

                // Skip surface reconfigure for minimize (zero-size) — wgpu
                // rejects 0x0 surfaces and the next non-zero Resized will
                // configure correctly anyway.
                if s.width == 0 || s.height == 0 {
                    return;
                }

                g.surface_cfg.width = s.width.max(1);
                g.surface_cfg.height = s.height.max(1);
                g.surface.configure(&g.device, &g.surface_cfg);

                // Win11 recovery: if we were maximized before a minimize and
                // worked around it via restore-then-minimize, re-apply maximize
                // now that the window is visible again. Done before state sync
                // so the next Resized (after this set_maximized call) writes
                // the correct flag.
                let is_max = g.window.is_maximized();
                if restored_from_min && g.pending_remax && !is_max {
                    g.pending_remax = false;
                    g.window.set_maximized(true);
                    g.window.request_redraw();
                    return;
                }

                // Source-of-truth state sync from OS — handles Aero Snap,
                // Win+Up/Down, snap layouts, double-click on titlebar, and any
                // other path that changes maximized state without going
                // through our titlebar action dispatcher.
                if g.app_state.titlebar.maximized != is_max {
                    g.app_state.titlebar.set_maximized(is_max);
                }

                #[cfg(windows)]
                if let Some(hwnd) = crate::borderless_window::platform::hwnd_of(&g.window) {
                    crate::borderless_window::platform::update_rounded_region(
                        hwnd,
                        self.config.corner_radius,
                    );
                }
                g.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                gpu::render_frame(g, handler, event_loop);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(g) = self.gpu.as_ref() {
            g.window.request_redraw();
            if g.fps_interval > Duration::ZERO {
                // Cap frame rate: sleep until next frame deadline.
                event_loop
                    .set_control_flow(ControlFlow::WaitUntil(Instant::now() + g.fps_interval));
            } else {
                // FpsMode::Unlimited / Fixed(0): render as fast as possible (Poll mode).
                event_loop.set_control_flow(ControlFlow::Poll);
            }
        }
    }
}
