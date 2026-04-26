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
//! `WM_NCCALCSIZE → 0` (installed by our WndProc subclass) removes all NC area
//! so the client rect covers the entire window rect.
//!
//! **`DwmExtendFrameIntoClientArea({1,1,1,1})`** is called unconditionally:
//!   - Win11: extends the DWM compositing frame 1 px into the client on every
//!     side, which covers the phantom DWM resize-border pixels (the dark strip
//!     that appears at the window edges when the NC area is zeroed).
//!   - Win10: enables the native drop-shadow. The swap chain is configured with
//!     **`CompositeAlphaMode::Opaque`** (see `gpu.rs`) so DWM composites the
//!     wgpu pixels as fully opaque — no "glass transparency" visible.
//!
//! This is the same approach used by VS Code (Electron), Tauri, and
//! Flutter/Windows for their custom-chrome borderless windows.

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

                // Add WS_CLIPCHILDREN so child HWNDs don't paint over our
                // wgpu surface during DWM compositing.
                // Read-modify-write: preserves WS_POPUP, WS_THICKFRAME, and
                // any other flags winit already set.
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_CLIPCHILDREN,
                };
                // SAFETY: documented Win32 API.
                unsafe {
                    let cur = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                    SetWindowLongPtrW(hwnd, GWL_STYLE, (cur | WS_CLIPCHILDREN) as isize);
                }

                // Subclass BEFORE SWP_FRAMECHANGED so the resulting
                // WM_NCCALCSIZE is intercepted immediately (→ 0, client = full
                // window rect).
                // SAFETY: hwnd is a valid Win32 HWND we just created.
                unsafe {
                    let _ = super::win32::subclass::install(hwnd, self.regions.clone());
                }

                // DWM attributes applied before show to avoid colour flash.
                super::win32::dwm::set_immersive_dark_mode(hwnd, true);
                super::win32::dwm::enable_dwm_rounded_corners(hwnd);

                // NOTE: DwmExtendFrameIntoClientArea is intentionally NOT
                // called here. On Win11, any call to that API (with positive
                // OR full-negative margins) causes DWM to internally impose a
                // ~30px "caption region" NC area that overrides our
                // WM_NCCALCSIZE→0 handler, producing a permanent black strip
                // at the top of the window. Drop shadow and rounded corners
                // are provided by enable_dwm_rounded_corners + DWM defaults
                // for WS_POPUP windows — no frame extension needed.

                // Win11-only extras: suppress caption tint and Mica/Acrylic.
                if super::win32::dwm::is_win11_dwm_corners() {
                    super::win32::dwm::suppress_caption_color(hwnd);
                    super::win32::dwm::suppress_system_backdrop(hwnd);
                }

                // SWP_FRAMECHANGED triggers WM_NCCALCSIZE so the subclass
                // handler fires and the DWM attributes take visual effect
                // before the first paint.
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                    SWP_NOZORDER, SetWindowPos,
                };
                // SAFETY: documented Win32 API.
                unsafe {
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        0, 0, 0, 0,
                        SWP_FRAMECHANGED
                            | SWP_NOMOVE
                            | SWP_NOSIZE
                            | SWP_NOZORDER
                            | SWP_NOACTIVATE,
                    );
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
                // Win11 fires a spurious WM_ACTIVATE(WA_INACTIVE) during
                // OS-driven HTCAPTION drags. Debounce by ignoring Focused(false)
                // if it arrives within FOCUS_DEBOUNCE of the last WM_NCLBUTTONDOWN.
                if !focused
                    && self.regions.nc_down_elapsed_ms()
                        < FOCUS_DEBOUNCE.as_millis() as u64
                {
                    return;
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
