//! # app_window
//!
//! Borderless-window framework. Custom Dear ImGui titlebar; native OS
//! resize, Aero Snap, drop shadow, taskbar / Alt-Tab integration preserved.
//!
//! ## Architecture (Windows)
//!
//! All windows are created with `decorations=false`, so winit produces a
//! `WS_POPUP + WS_THICKFRAME` window. That style has *no* caption, *no*
//! system menu, *no* DWM chrome — meaning DWM has nothing to draw or
//! tint when the window loses focus, and there is no inactive-window
//! dimming to fight. `WS_THICKFRAME` keeps native edge resize, Aero
//! Snap, and the DWM drop shadow.
//!
//! Win32-side helpers are minimal:
//! - `DWMWA_USE_IMMERSIVE_DARK_MODE` for the Alt-Tab thumbnail.
//! - `DWMWA_WINDOW_CORNER_PREFERENCE` (Win11) or `SetWindowRgn`
//!   (Win10 fallback) for rounded corners.
//! - `WS_EX_TOOLWINDOW` for tool-window kinds.
//! - A tiny `WM_GETMINMAXINFO` subclass that clamps the maximised rect
//!   to the monitor work area (so the taskbar stays visible).
//! - `set_opacity` toggles `WS_EX_LAYERED` on demand.
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

mod gpu;
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

use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
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
    /// Cross-thread wake-up via [`AppProxy::wake`]. Triggers two redraws
    /// so any in-flight state mutated from the background propagates to
    /// the next stable hover frame as well.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
        if let Some(g) = self.gpu.as_mut() {
            g.pending_frames = g.pending_frames.max(2);
            g.window.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let cfg = &self.config;

        // ALWAYS `decorations=false`. This makes winit create a `WS_POPUP +
        // WS_THICKFRAME`-style window: no caption, no system menu, no DWM
        // chrome of any kind — therefore nothing for DWM to dim or tint
        // when the window loses focus. Resize, Aero Snap, drop shadow,
        // taskbar / Alt-Tab integration all keep working through
        // `WS_THICKFRAME`. This matches the v1 `app_window` approach.
        let mut attrs = Window::default_attributes()
            .with_title(cfg.title.clone())
            .with_inner_size(LogicalSize::new(cfg.size[0], cfg.size[1]))
            .with_decorations(false)
            .with_resizable(cfg.os_resizable())
            .with_visible(false);

        if let Some([w, h]) = cfg.min_size {
            attrs = attrs.with_min_inner_size(LogicalSize::new(w, h));
        }
        if let Some([w, h]) = cfg.max_size {
            attrs = attrs.with_max_inner_size(LogicalSize::new(w, h));
        }
        if matches!(cfg.form_style, FormStyle::StayOnTop) {
            attrs = attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }
        if let Some(ref icon) = cfg.window_icon
            && let Ok(winit_icon) =
                winit::window::Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height)
        {
            attrs = attrs.with_window_icon(Some(winit_icon));
        }

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("window creation failed"),
        );

        // Win32 setup before set_visible to avoid any flash.
        #[cfg(windows)]
        if let Some(hwnd) = win32::hwnd_of(&window) {
            let opts = win32::SetupOptions {
                tool_window: matches!(
                    cfg.border,
                    BorderStyle::ToolWindow | BorderStyle::SizeToolWin,
                ) || matches!(cfg.kind, WindowKind::Tool),
                corner_radius: cfg.corner_radius,
            };
            win32::setup_window(hwnd, opts);
        }

        gpu::position_window(&window, &cfg.position, event_loop);
        #[cfg(windows)]
        if cfg.opacity < 1.0
            && let Some(hwnd) = win32::hwnd_of(&window)
        {
            win32::set_opacity(hwnd, cfg.opacity);
        }
        let (instance, adapter, device, queue, surface, surface_cfg) = gpu::init_wgpu(&window, cfg);
        let surface_format = surface_cfg.format;

        // Prime the swapchain with a single clear-pass to the theme background
        // colour BEFORE the OS shows the window. Without this priming the
        // first frame the user sees is whatever garbage the OS happened to
        // initialise the surface with — usually pure black, which flashes
        // visibly on every cold start. ImGui content takes another frame or
        // two to render after `set_visible`, but at least the background is
        // already correct, so there's no jarring transition.
        {
            let bg = cfg.theme.titlebar().bg;
            if let wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) = surface.get_current_texture()
            {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut enc =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                {
                    let _rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("app_window: priming clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: bg[0] as f64,
                                    g: bg[1] as f64,
                                    b: bg[2] as f64,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                }
                queue.submit(Some(enc.finish()));
                frame.present();
            }
        }

        // Now reveal the window. The pixel buffer in the swapchain is already
        // a flat tinted rectangle, so users no longer see a black flash.
        if cfg.visible {
            window.set_visible(true);
        }

        let (context, platform, renderer) = gpu::init_imgui(
            &window,
            instance,
            adapter,
            device.clone(),
            queue.clone(),
            surface_format,
            cfg,
        );

        // Resolve the render-mode enum into the flat fields the hot-path
        // event loop reads. Computed once at init — `about_to_wait` then
        // makes scheduling decisions with simple field reads.
        let (event_driven, idle_pulse, unfocused_idle_pulse, fps_interval, unfocused_fps_interval) =
            match &cfg.render_mode {
                RenderMode::EventDriven {
                    idle_pulse,
                    unfocused_idle_pulse,
                } => (
                    true,
                    *idle_pulse,
                    *unfocused_idle_pulse,
                    Duration::ZERO,
                    Duration::ZERO,
                ),
                RenderMode::Continuous {
                    fps_mode,
                    unfocused_fps,
                } => {
                    let fi = match fps_mode {
                        FpsMode::Fixed(n) if *n > 0 => Duration::from_secs_f64(1.0 / *n as f64),
                        _ => Duration::ZERO,
                    };
                    let ui = if *unfocused_fps > 0 {
                        Duration::from_secs_f64(1.0 / *unfocused_fps as f64)
                    } else {
                        Duration::ZERO
                    };
                    (false, None, None, fi, ui)
                }
            };

        // Match `refresh_clear_color`: clear to the theme's window
        // background, not the titlebar surface. That keeps the
        // visible page in sync with `StyleColor::WindowBg` for both
        // raw_content (transparent root) and padded mode.
        // `wgpu_clear_color` performs the sRGB → linear conversion
        // when (and only when) the swap chain format requires it —
        // see the helper's doc-comment for the full rationale.
        let clear_color =
            crate::utils::color::wgpu_clear_color(cfg.theme.window_bg(), surface_cfg.format);

        self.gpu = Some(gpu::GpuState {
            device,
            queue,
            window,
            surface,
            surface_cfg,
            context,
            platform,
            renderer,
            app_state: state::AppState::new(self.proxy.clone()),
            focused: true,
            event_driven,
            idle_pulse,
            unfocused_idle_pulse,
            fps_interval,
            unfocused_fps_interval,
            // Two frames so the first paint actually happens before any
            // idle gating kicks in (one to acquire surface, one to settle).
            pending_frames: 2,
            last_redraw: Instant::now(),
            last_hover_edge: None,
            cursor_set: false,
            pending_remax: false,
            was_minimized: false,
            started_at: Instant::now(),
            clear_color,
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

        // First-frame hook: AppHandler::on_ready.
        if !self.on_ready_fired {
            self.on_ready_fired = true;
            handler.on_ready(&mut g.app_state);
        }

        // ── Raw event hook ──────────────────────────────────────────
        // Hand the event to the user **before** Dear ImGui's platform
        // layer sees it. The handler can read drag-drop file paths, run
        // layout-independent hotkeys, intercept gestures — and consume
        // the event by returning `true` so Dear ImGui does not see it.
        //
        // **`consumed` only suppresses the ImGui platform handler** —
        // the framework's own routing (Resized → surface reconfigure,
        // CloseRequested → exit, Focused → titlebar tint, Redraw) still
        // runs. Consuming a structural event is normally pointless;
        // doing so previously skipped these handlers and produced
        // black-rect / stuck-window bugs.
        let consumed = handler.on_window_event(&event, &mut g.app_state);

        // ── Layout-independent keyboard / IME fixes ─────────────────
        // `dear-imgui-winit` derives Dear ImGui keys from the *logical*
        // key (post-keyboard-layout). On Cyrillic / Greek / CJK layouts
        // the physical `C` key arrives as Cyrillic 'с', which neither
        // maps to `Key::C` nor reaches `InputText` as a shortcut — the
        // user has to switch to English to use `Ctrl+C` / `Ctrl+V`,
        // which is unacceptable UX. We inject the right Dear ImGui key
        // **based on the physical scan code** before the platform layer
        // sees the event, then skip the forward so the Cyrillic
        // character isn't typed into the focused field.
        //
        // Numpad digits (`Keypad0..9`) need similar handling: ImGui
        // treats them as navigation, not text — without injection,
        // typing `1` on the numpad never appears in `InputText`.
        //
        // IME commits (CJK composition) are ignored by
        // `dear-imgui-winit` entirely; we forward the committed string
        // as input characters directly.
        let mut kbd_handled = false;
        let mut kbd_event: Option<winit::event::KeyEvent> = None;
        if !consumed {
            match &event {
                WindowEvent::KeyboardInput { event: ke, .. } => {
                    let io = g.context.io_mut();
                    if crate::input::keyboard::try_inject_numpad_text(io, ke)
                        || crate::input::keyboard::try_inject_ctrl_alt_shortcut(io, ke)
                    {
                        kbd_handled = true;
                        // Bump the redraw budget — modifiers + key press
                        // is genuine input, the user expects an immediate
                        // visual response (selection change, cursor move).
                        g.pending_frames = g.pending_frames.max(2);
                        g.window.request_redraw();
                    } else {
                        // Save a clone for the post-forward reinforce pass
                        // below — fixes "stuck Key::C" when Ctrl is
                        // released *before* the letter on non-Latin
                        // layouts.
                        kbd_event = Some(ke.clone());
                    }
                }
                WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                    crate::input::keyboard::inject_ime_commit(g.context.io_mut(), text);
                    kbd_handled = true;
                    g.pending_frames = g.pending_frames.max(2);
                    g.window.request_redraw();
                }
                _ => {}
            }
        }

        if !consumed && !kbd_handled {
            g.platform.handle_event::<()>(
                &mut g.context,
                &g.window,
                &Event::WindowEvent {
                    window_id,
                    event: event.clone(),
                },
            );
            // Reinforce physical-key state so the eventual release
            // matches the press we recorded — see
            // `reinforce_physical_key_state` doc-comment.
            if let Some(ref ke) = kbd_event {
                crate::input::keyboard::reinforce_physical_key_state(g.context.io_mut(), ke);
            }
        }

        // Classify any event that should kick the renderer out of idle.
        // Two frames is the minimum ImGui needs for hover-state to settle
        // (one to detect, one to draw the resulting style change).
        //
        // Coverage: pointer / cursor / mouse / wheel, keyboard / modifiers /
        // IME, scale / theme system changes, touch + touchpad gestures,
        // drag-and-drop hover/cancel/drop. Excluded: `Occluded` (just OS
        // bookkeeping), `RedrawRequested` (handled separately), focus and
        // resize (handled in their own match arms).
        let needs_redraw = matches!(
            &event,
            WindowEvent::CursorMoved { .. }
                | WindowEvent::CursorEntered { .. }
                | WindowEvent::CursorLeft { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::KeyboardInput { .. }
                | WindowEvent::ModifiersChanged(..)
                | WindowEvent::Ime(..)
                | WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::ThemeChanged(..)
                | WindowEvent::Touch(..)
                | WindowEvent::TouchpadPressure { .. }
                | WindowEvent::PinchGesture { .. }
                | WindowEvent::PanGesture { .. }
                | WindowEvent::RotationGesture { .. }
                | WindowEvent::DoubleTapGesture { .. }
                | WindowEvent::DroppedFile(..)
                | WindowEvent::HoveredFile(..)
                | WindowEvent::HoveredFileCancelled
        );
        if needs_redraw {
            g.pending_frames = g.pending_frames.max(2);
            g.window.request_redraw();
        }

        match event {
            WindowEvent::CloseRequested => {
                handler.on_close_requested(&mut g.app_state);
                if g.app_state.should_exit {
                    event_loop.exit();
                }
            }

            WindowEvent::Focused(focused) => {
                g.focused = focused;
                g.app_state.titlebar.set_focused(focused);
                // Force a paint so the inactive titlebar tint applies
                // before any unfocused throttle silences the next frames.
                g.pending_frames = g.pending_frames.max(2);
                g.window.request_redraw();
            }

            WindowEvent::Resized(s) => {
                let is_min = g.window.is_minimized().unwrap_or(false);
                let restored = g.was_minimized && !is_min;
                g.was_minimized = is_min;

                if s.width == 0 || s.height == 0 {
                    return;
                }

                g.surface_cfg.width = s.width.max(1);
                g.surface_cfg.height = s.height.max(1);
                g.surface.configure(&g.device, &g.surface_cfg);

                let is_max = g.window.is_maximized();
                if restored && g.pending_remax && !is_max {
                    g.pending_remax = false;
                    g.window.set_maximized(true);
                    g.pending_frames = g.pending_frames.max(2);
                    g.window.request_redraw();
                    return;
                }
                if g.app_state.titlebar.maximized != is_max {
                    g.app_state.titlebar.set_maximized(is_max);
                }

                #[cfg(windows)]
                if let Some(hwnd) = win32::hwnd_of(&g.window) {
                    win32::update_rounded_region(hwnd, self.config.corner_radius);
                }
                g.pending_frames = g.pending_frames.max(2);
                g.window.request_redraw();
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                // The user dragged the window between monitors of
                // differing DPI. Rebuild the font atlas at the new
                // scale so text doesn't render blurry through a 2× /
                // 0.5× upscale.
                gpu::rebuild_fonts_for_scale(
                    &mut g.context,
                    &mut g.renderer,
                    &self.config,
                    scale_factor as f32,
                );
                g.pending_frames = g.pending_frames.max(2);
                g.window.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                gpu::render_frame(g, &mut self.config, handler, event_loop);
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(g) = self.gpu.as_ref() else { return };

        // ── 1. Minimized: park the loop entirely ─────────────────────
        // The OS will wake us on restore / close / focus / shell event.
        // Until then CPU and GPU draw zero work.
        if g.was_minimized {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        // ── 2. Continuous-render mode ────────────────────────────────
        // Game-style: every loop iteration repaints, gated by `fps_mode`
        // and `unfocused_fps`.
        if !g.event_driven {
            g.window.request_redraw();
            let interval = if !g.focused && g.unfocused_fps_interval > Duration::ZERO {
                g.unfocused_fps_interval
            } else {
                g.fps_interval
            };
            if interval > Duration::ZERO {
                event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + interval));
            } else {
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            return;
        }

        // ── 3. Event-driven mode (default) ───────────────────────────
        //
        // Pending frames in flight. Re-arm the redraw — winit only
        // buffers a single paint event per call to `request_redraw`, so
        // any `pending_frames > 1` budget needs to be reissued each loop
        // iteration. The next iteration's `RedrawRequested` decrements
        // it by exactly one.
        if g.pending_frames > 0 {
            g.window.request_redraw();
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        // Pick the focused / unfocused idle pulse. Either (or both) can
        // be `None` — `event_driven_minimal()` disables both, giving
        // strictly-zero-idle behaviour.
        let pulse = if g.focused {
            g.idle_pulse
        } else {
            g.unfocused_idle_pulse
        };

        match pulse {
            Some(dt) => {
                let next = g.last_redraw + dt;
                let now = Instant::now();
                if next <= now {
                    g.window.request_redraw();
                    event_loop.set_control_flow(ControlFlow::Wait);
                } else {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(next));
                }
            }
            None => {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
    }
}
