//! Main application loop — winit `ApplicationHandler` impl that drives the v2 window.
//!
//! ## Win32 window strategy
//!
//! `with_decorations(false)` → winit creates **`WS_POPUP | WS_THICKFRAME`**.
//!
//! `WS_POPUP` is the correct base style for a custom-drawn borderless window:
//!   - No DWM caption layer is inserted over the client area (WS_CAPTION causes
//!     DWM to render a dark caption strip over the top of the window on Win11).
//!   - `inner_size()` always equals the full window rect — no NC area to subtract.
//!
//! `WS_THICKFRAME` (set by winit when `with_resizable(true)`) enables:
//!   - Native edge / corner resize
//!   - Aero Snap (drag to edge)
//!   - Win11 Snap Layouts popup (hover the Maximize button)
//!
//! `WM_NCCALCSIZE` is **not** intercepted. With `WS_POPUP` the default
//! handler already makes client ≈ window rect. Intercepting and returning 0
//! causes Win11 DWM to composite a permanent ~30 px dark caption strip at
//! the top — it treats the explicit override as "app manages its own NC area".
//!
//! `DwmExtendFrameIntoClientArea` is **not** called. On Win11 22H2+, any
//! extension call (including full-glass `{-1,-1,-1,-1}`) causes DWM to
//! composite native caption chrome (title bar + min/max/close buttons) over
//! the client area even with `CompositeAlphaMode::Opaque`. Drop shadow and
//! rounded corners are provided by `enable_dwm_rounded_corners`.
//!
//! Caption drag is driven by **ImGui detection + `drag_window()`**: the
//! titlebar reports `HTCLIENT` for the drag area (not `HTCAPTION`), detects
//! clicks via ImGui, and signals the app layer to call `window.drag_window()`.
//! Only `HTMAXBUTTON` (Win11 Snap Layouts) and `HTCLOSE` (OS close) are
//! returned from `WM_NCHITTEST`; everything else is `HTCLIENT`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dear_imgui_rs::{Condition, StyleVar, WindowFlags};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::Window;

use super::config::{AppConfigV2, CloseMode, FpsMode, StartPosition};
use super::gpu;
use super::handler::AppHandlerV2;
use super::hit_test::SharedHitRegions;
use super::state::AppStateV2;
use super::titlebar::render_titlebar_v2;

pub(super) struct WinitAppV2<H: AppHandlerV2> {
    cfg:     AppConfigV2,
    handler: Option<H>,
    gpu:     Option<GpuState>,
    regions: SharedHitRegions,
}

/// Window within which `WindowEvent::Focused(false)` is dropped after an
/// OS-driven NC drag/resize — Win11 can fire a spurious WM_ACTIVATE(WA_INACTIVE)
/// during caption drag. 250 ms is above the message-loop roundtrip but below
/// the perceptible Alt-Tab threshold.
const FOCUS_DEBOUNCE: Duration = Duration::from_millis(250);

struct GpuState {
    device:       wgpu::Device,
    queue:        wgpu::Queue,
    window:       Arc<Window>,
    surface_cfg:  wgpu::SurfaceConfiguration,
    surface:      wgpu::Surface<'static>,
    context:      dear_imgui_rs::Context,
    platform:     dear_imgui_winit::WinitPlatform,
    renderer:     dear_imgui_wgpu::WgpuRenderer,
    app_state:    AppStateV2,
    fps_interval: Duration,
    /// Win11: set when the user minimizes from a maximized state. Consumed in
    /// `WindowEvent::Resized` when `was_minimized` transitions `true→false` to
    /// re-apply maximized state, because SC_MINIMIZE from a maximized WS_POPUP
    /// drops the follow-up SC_RESTORE from the taskbar on Win11.
    pending_remax: bool,
    /// Whether the window was minimized on the previous Resized event. Used to
    /// detect the restore-from-minimize transition that triggers `pending_remax`.
    was_minimized: bool,
    /// Instant the last caption-area drag or resize was initiated by ImGui.
    /// Used to debounce the spurious `Focused(false)` Win11 fires after
    /// `drag_window()` (which internally sends `WM_NCLBUTTONDOWN`).
    last_drag_at: Option<Instant>,
}

impl<H: AppHandlerV2> WinitAppV2<H> {
    pub(super) fn new(cfg: AppConfigV2, handler: H) -> Self {
        Self { cfg, handler: Some(handler), gpu: None, regions: SharedHitRegions::new() }
    }
}

impl<H: AppHandlerV2 + 'static> ApplicationHandler for WinitAppV2<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let cfg = &self.cfg;

        // with_decorations(false) → WS_POPUP | WS_THICKFRAME (winit adds
        // WS_THICKFRAME automatically when resizable=true). WS_POPUP avoids
        // the DWM caption layer that causes black strips on Win11.
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(cfg.titlebar.title.clone())
                        .with_inner_size(LogicalSize::new(cfg.size[0], cfg.size[1]))
                        .with_min_inner_size(LogicalSize::new(cfg.min_size[0], cfg.min_size[1]))
                        .with_decorations(false)
                        .with_resizable(true)
                        .with_visible(false),
                )
                .expect("failed to create window"),
        );

        #[cfg(windows)]
        {
            if let Some(hwnd) = hwnd_of(&window) {
                let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;

                // Install subclass before any DWM calls so WM_NCHITTEST,
                // WM_GETMINMAXINFO, and WM_NCACTIVATE are handled correctly
                // from the very first message pump.
                // SAFETY: hwnd is a valid Win32 HWND we just created.
                unsafe {
                    let _ = super::win32::subclass::install(hwnd, self.regions.clone());
                }

                // DWM attributes applied before show to avoid colour flash.
                super::win32::dwm::set_immersive_dark_mode(hwnd, true);
                super::win32::dwm::enable_dwm_rounded_corners(hwnd);

                // Win11-only extras: suppress caption tint and Mica/Acrylic.
                if super::win32::dwm::is_win11_dwm_corners() {
                    super::win32::dwm::suppress_caption_color(hwnd);
                    super::win32::dwm::suppress_system_backdrop(hwnd);
                }
            }
        }

        position_window(&window, &cfg.start_position, event_loop);
        window.set_visible(true);

        let (device, queue, surface, surface_cfg) = gpu::init_wgpu(&window, cfg.power_mode);
        let surface_format = surface_cfg.format;

        let (context, platform, renderer) = gpu::init_imgui(
            &window,
            device.clone(),
            queue.clone(),
            surface_format,
            cfg.font_size,
            cfg.titlebar.theme,
            cfg.merge_mdi_icons,
        );

        let fps_interval = match cfg.fps_mode {
            FpsMode::Auto | FpsMode::Unlimited => Duration::ZERO,
            FpsMode::Fixed(n) if n > 0 => Duration::from_secs_f64(1.0 / n as f64),
            FpsMode::Fixed(_) => Duration::ZERO,
        };

        let mut app_state = AppStateV2::new();
        app_state.titlebar.maximized = window.is_maximized();
        app_state.titlebar.focused   = true;
        self.regions.set_maximized(app_state.titlebar.maximized);

        self.gpu = Some(GpuState {
            device,
            queue,
            window,
            surface_cfg,
            surface,
            context,
            platform,
            renderer,
            app_state,
            fps_interval,
            pending_remax: false,
            was_minimized: false,
            last_drag_at: None,
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

        g.platform.handle_event::<()>(
            &mut g.context,
            &g.window,
            &Event::WindowEvent { window_id, event: event.clone() },
        );

        match event {
            WindowEvent::CloseRequested => {
                match self.cfg.titlebar.close_mode {
                    CloseMode::Immediate => g.app_state.exit(),
                    CloseMode::Confirm   => handler.on_close_requested(&mut g.app_state),
                }
                if g.app_state.should_exit {
                    event_loop.exit();
                }
            }
            WindowEvent::Focused(focused) => {
                // Win11 fires a spurious WM_ACTIVATE(WA_INACTIVE) after a
                // caption drag (drag_window()) or a resize-edge drag. Debounce:
                // ignore Focused(false) that arrives within FOCUS_DEBOUNCE of
                // either the last ImGui-detected drag start OR the last
                // WM_NCLBUTTONDOWN (covers native resize-edge drags).
                if !focused {
                    let drag_recent = g
                        .last_drag_at
                        .is_some_and(|t| t.elapsed() < FOCUS_DEBOUNCE);
                    let nc_recent = self.regions.nc_down_elapsed_ms()
                        < FOCUS_DEBOUNCE.as_millis() as u64;
                    if drag_recent || nc_recent {
                        return;
                    }
                }
                g.app_state.titlebar.focused = focused;
            }
            WindowEvent::Resized(s) => {
                // Track minimize transition before skipping on zero-size.
                let is_minimized = g.window.is_minimized().unwrap_or(false);
                let restored_from_min = g.was_minimized && !is_minimized;
                g.was_minimized = is_minimized;

                if s.width == 0 || s.height == 0 {
                    return;
                }

                g.surface_cfg.width  = s.width.max(1);
                g.surface_cfg.height = s.height.max(1);
                g.surface.configure(&g.device, &g.surface_cfg);

                // Win11 tray-restore: if we restored from maximized→minimized
                // and the user brings the window back, re-apply maximized state.
                let is_max = g.window.is_maximized();
                if restored_from_min && g.pending_remax && !is_max {
                    g.pending_remax = false;
                    g.window.set_maximized(true);
                    g.window.request_redraw();
                    return;
                }

                // Source-of-truth state sync (Aero Snap, Win+Up/Down, snap layouts).
                if g.app_state.titlebar.maximized != is_max {
                    g.app_state.titlebar.maximized = is_max;
                }
                self.regions.set_maximized(is_max);
                g.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                g.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                render_frame(g, &mut self.cfg, &self.regions, handler, event_loop);
                if g.app_state.should_exit {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(g) = self.gpu.as_ref() {
            g.window.request_redraw();
            if g.fps_interval > Duration::ZERO {
                event_loop
                    .set_control_flow(ControlFlow::WaitUntil(Instant::now() + g.fps_interval));
            } else {
                event_loop.set_control_flow(ControlFlow::Poll);
            }
        }
    }
}

fn render_frame<H: AppHandlerV2>(
    g:           &mut GpuState,
    cfg:         &mut AppConfigV2,
    regions:     &SharedHitRegions,
    handler:     &mut H,
    _event_loop: &ActiveEventLoop,
) {
    let frame = match g.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f)    => f,
        wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
            g.window.request_redraw();
            f
        }
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            g.surface.configure(&g.device, &g.surface_cfg);
            g.window.request_redraw();
            return;
        }
        other => { let _ = other; return; }
    };

    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

    g.platform.prepare_frame(&g.window, &mut g.context);
    let dpi_scale = g.window.scale_factor() as f32;
    let ui = g.context.frame();

    let mut titlebar_frame = super::titlebar::TitlebarFrame::default();

    {
        let display = ui.io().display_size();
        let _no_pad = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _no_sp  = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));

        ui.window("##app_root_v2")
            .size(display, Condition::Always)
            .position([0.0, 0.0], Condition::Always)
            .flags(
                WindowFlags::NO_TITLE_BAR
                    | WindowFlags::NO_RESIZE
                    | WindowFlags::NO_MOVE
                    | WindowFlags::NO_SCROLLBAR
                    | WindowFlags::NO_SCROLL_WITH_MOUSE
                    | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
                    | WindowFlags::NO_NAV_FOCUS,
            )
            .build(|| {
                titlebar_frame = render_titlebar_v2(
                    ui,
                    &cfg.titlebar,
                    &g.app_state.titlebar,
                    regions,
                    dpi_scale,
                );

                ui.set_cursor_pos([0.0, cfg.titlebar.titlebar_height]);
                ui.dummy([0.0, 0.0]); // extend content boundary to titlebar_height
                let _ip = ui.push_style_var(StyleVar::WindowPadding([8.0, 8.0]));
                let _is = ui.push_style_var(StyleVar::ItemSpacing([6.0, 4.0]));
                handler.render(ui, &mut g.app_state);
            });
    }

    g.platform.prepare_render_with_ui(ui, &g.window);
    let draw_data = g.context.render();

    let bg = cfg.titlebar.theme.titlebar().bg;
    let clear = wgpu::Color {
        r: bg[0] as f64,
        g: bg[1] as f64,
        b: bg[2] as f64,
        a: 1.0,
    };

    let mut enc = g.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("app_window_v2"),
    });
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view:           &view,
                resolve_target: None,
                depth_slice:    None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes:         None,
            occlusion_query_set:      None,
            multiview_mask:           None,
        });
        if draw_data.total_vtx_count > 0
            && let Err(e) = g.renderer.render_draw_data(draw_data, &mut pass)
        {
            let _ = e;
        }
    }
    g.queue.submit(Some(enc.finish()));
    frame.present();

    // ── Per-frame requests ───────────────────────────────────────────────────

    if let Some(id) = titlebar_frame.extra_clicked {
        handler.on_extra_button(id, &mut g.app_state);
    }

    // Caption drag — HTCLIENT in WM_NCHITTEST so the OS won't start a native
    // drag; we initiate it explicitly. Record the time first so the Focused(false)
    // debounce is set before drag_window() sends WM_NCLBUTTONDOWN internally.
    if titlebar_frame.drag_started {
        g.last_drag_at = Some(Instant::now());
        g.window.drag_window().ok();
    }

    if titlebar_frame.maximize_toggled {
        g.window.set_maximized(!g.window.is_maximized());
    }

    // Minimize button is HTCLIENT so ImGui owns the click. Apply the Win11
    // restore-before-minimize workaround so SC_RESTORE from the taskbar is
    // not dropped after a maximize→minimize→taskbar-click sequence.
    let want_minimize = titlebar_frame.minimize_clicked || g.app_state.request_minimize;
    if want_minimize {
        g.app_state.request_minimize = false;
        #[cfg(windows)]
        if super::win32::dwm::is_win11_dwm_corners() && g.window.is_maximized() {
            g.window.set_maximized(false);
            g.pending_remax = true;
        }
        g.window.set_minimized(true);
    }

    if let Some(v) = g.app_state.request_maximize.take() {
        g.window.set_maximized(v);
    }
    if let Some(theme) = g.app_state.pending_theme.take() {
        theme.apply_imgui_style(g.context.style_mut());
        cfg.titlebar.theme = theme;
        handler.on_theme_changed(&cfg.titlebar.theme, &mut g.app_state);
    }
    if g.app_state.confirmed_close {
        g.app_state.confirmed_close = false;
        g.app_state.should_exit     = true;
    }
}

// ── Window positioning ────────────────────────────────────────────────────────

fn position_window(window: &Window, pos: &StartPosition, event_loop: &ActiveEventLoop) {
    match pos {
        StartPosition::CenterScreen => {
            if let Some(mon) = event_loop.primary_monitor() {
                let mp = mon.position();
                let ms = mon.size();
                let ws = window.inner_size();
                window.set_outer_position(winit::dpi::PhysicalPosition::new(
                    mp.x + (ms.width  as i32 - ws.width  as i32) / 2,
                    mp.y + (ms.height as i32 - ws.height as i32) / 2,
                ));
            }
        }
        StartPosition::TopLeft => {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(0, 0));
        }
        StartPosition::Custom(x, y) => {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(*x, *y));
        }
    }
}

// ── HWND extraction ───────────────────────────────────────────────────────────

#[cfg(windows)]
fn hwnd_of(window: &Window) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(h) = window.window_handle()
        && let RawWindowHandle::Win32(w) = h.as_raw()
    {
        return Some(w.hwnd.get());
    }
    None
}
