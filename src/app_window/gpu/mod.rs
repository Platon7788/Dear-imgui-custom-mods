//! wgpu / Dear ImGui setup and per-frame render loop.

mod imgui;
mod position;
mod setup;

pub(super) use imgui::{init_imgui, rebuild_fonts_for_scale};
pub(super) use position::position_window;
pub(super) use setup::init_wgpu;

use std::sync::Arc;
use std::time::{Duration, Instant};

use dear_imgui_rs::{Condition, StyleVar, WindowFlags};
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use winit::{event_loop::ActiveEventLoop, window::Window};

use super::chrome::{
    ResizeEdge, TitlebarAction, TitlebarResult, cursor_for_edge, render_titlebar,
    resize_direction, whole_window_resize,
};
use super::config::{AppConfig, Chrome};
use super::handler::AppHandler;
use super::state::AppState;

// ── GpuState ──────────────────────────────────────────────────────────────────

pub(super) struct GpuState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub surface_cfg: wgpu::SurfaceConfiguration,
    pub context: dear_imgui_rs::Context,
    pub platform: WinitPlatform,
    pub renderer: WgpuRenderer,
    pub app_state: AppState,
    /// `true` while the OS reports the window as the active foreground window.
    pub focused: bool,
    /// Event-driven mode flag — mirrors
    /// `matches!(cfg.render_mode, RenderMode::EventDriven { .. })`.
    /// Hot-path scheduling decisions read this once per loop iteration.
    pub event_driven: bool,
    /// Foreground idle pulse (event-driven mode only).
    pub idle_pulse: Option<Duration>,
    /// Background idle pulse, applied while the window is unfocused
    /// (event-driven mode only).
    pub unfocused_idle_pulse: Option<Duration>,
    /// Foreground frame-cap interval (continuous mode only).
    /// `Duration::ZERO` means "no cap — use `Poll`".
    pub fps_interval: Duration,
    /// Background frame-cap interval (continuous mode only).
    /// `Duration::ZERO` means "no separate cap; fall back to `fps_interval`".
    pub unfocused_fps_interval: Duration,
    /// "Render at least N more frames" budget. Bumped by input events,
    /// animation widgets ([`crate::frame_demand`]) and explicit
    /// [`AppState::keep_alive`] calls. Decremented per frame.
    pub pending_frames: u8,
    /// Wall-clock of the last completed render. Used to schedule the next
    /// idle-pulse via [`ControlFlow::WaitUntil`].
    pub last_redraw: Instant,
    pub last_hover_edge: Option<ResizeEdge>,
    pub cursor_set: bool,
    pub pending_remax: bool,
    pub was_minimized: bool,
    pub started_at: Instant,
    /// Cached `theme.titlebar().bg` so the per-frame clear-pass doesn't
    /// build a fresh `TitlebarColors` (~200 B on stack) every redraw.
    /// Refreshed when [`AppState::pending_theme`] is applied.
    pub clear_color: wgpu::Color,
}

impl GpuState {
    pub(super) fn refresh_clear_color(&mut self, cfg: &AppConfig) {
        let bg = cfg.theme.titlebar().bg;
        self.clear_color = wgpu::Color {
            r: bg[0] as f64,
            g: bg[1] as f64,
            b: bg[2] as f64,
            a: 1.0,
        };
    }
}

// ── Per-frame render ──────────────────────────────────────────────────────────

pub(super) fn render_frame<H: AppHandler>(
    gpu: &mut GpuState,
    cfg: &mut AppConfig,
    handler: &mut H,
    event_loop: &ActiveEventLoop,
) {
    // We are servicing a `RedrawRequested` — consume one frame from the
    // budget. New work added during this render (input, animation widgets)
    // will bump the counter back up before `about_to_wait` runs again.
    gpu.pending_frames = gpu.pending_frames.saturating_sub(1);

    // Splash auto-close.
    if let Some(d) = cfg.auto_close_after
        && gpu.started_at.elapsed() >= d
    {
        gpu.app_state.should_exit = true;
    }

    let frame = match gpu.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) => f,
        wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
            gpu.window.request_redraw();
            f
        }
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
            gpu.window.request_redraw();
            return;
        }
        other => {
            let line = format!("app_window: surface error: {other:?}");
            eprintln!("{line}");
            super::win32_debug_log(&line);
            return;
        }
    };

    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);
    let ui = gpu.context.frame();
    let mut tb_result = TitlebarResult::none();

    {
        let display = ui.io().display_size();
        let _np = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _ns = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));

        ui.window("##app_root")
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
                let mut content_top = 0.0;
                match &cfg.chrome {
                    Chrome::None => {
                        // Splash / chrome-less: no titlebar at all.
                        let (edge, action) = whole_window_resize(
                            ui,
                            6.0,
                            cfg.os_resizable(),
                            gpu.app_state.titlebar.maximized,
                        );
                        tb_result = TitlebarResult {
                            action,
                            hover_edge: edge,
                        };
                    }
                    Chrome::Custom(t) => {
                        let colors = cfg.theme.titlebar();
                        tb_result = render_titlebar(
                            ui,
                            t,
                            &cfg.title,
                            &colors,
                            &gpu.app_state.titlebar,
                            6.0,
                            cfg.os_resizable(),
                        );
                        content_top = t.height;
                    }
                }

                let avail_h = (display[1] - content_top).max(0.0);
                ui.set_cursor_pos([0.0, content_top]);
                // `set_cursor_pos` is a directive that takes effect on
                // the *next* item. Without an item committed at this
                // position, the first thing the handler does (a
                // `child_window`, a `draw_list` op, another
                // `set_cursor_pos`) can leave ImGui's layout state
                // inconsistent — items dispatched later use a stale
                // anchor and end up shifted. A zero-size `dummy` is
                // the canonical fix: it registers an item at the
                // current cursor without advancing it, so every
                // subsequent path (`raw_content` or padded) starts
                // from a fully committed layout.
                ui.dummy([0.0, 0.0]);

                if cfg.raw_content {
                    // Full-bleed mode: the handler runs directly inside
                    // the root window with no padding, no item-spacing,
                    // no child wrapper. The handler owns every pixel of
                    // the content rect — useful for chart viewers / 3D
                    // viewports / pixel-perfect editors.
                    handler.render(ui, &mut gpu.app_state);
                } else {
                    // Default: host the user's UI in a child window so
                    // `WindowPadding` / `ItemSpacing` actually apply —
                    // pushing them on the root `##app_root` is a no-op
                    // because ImGui locks `WindowPadding` at window-
                    // creation time.
                    let _ip = ui.push_style_var(StyleVar::WindowPadding([8.0, 8.0]));
                    let _is = ui.push_style_var(StyleVar::ItemSpacing([6.0, 4.0]));
                    ui.child_window("##app_content")
                        .size([0.0, avail_h])
                        .border(false)
                        .build(ui, || {
                            handler.render(ui, &mut gpu.app_state);
                        });
                }
            });
    }

    // Cursor — only update on edge changes (or after a drag, where we reset).
    if !gpu.cursor_set || tb_result.hover_edge != gpu.last_hover_edge {
        gpu.window.set_cursor(cursor_for_edge(tb_result.hover_edge));
        gpu.last_hover_edge = tb_result.hover_edge;
        gpu.cursor_set = true;
    }

    // ── Collect "keep rendering" signals from this frame ──────────────
    // Built-in animation widgets and user code call `frame_demand::request`
    // / `AppState::keep_alive` from inside `handler.render()`.
    // ImGui flags `want_text_input` whenever an InputText is active —
    // we need at least one more frame for the cursor blink to advance.
    let demanded = crate::frame_demand::take();
    let want_text = ui.io().want_text_input();
    let mut keep_alive: u8 = demanded;
    if want_text {
        keep_alive = keep_alive.max(1);
    }
    if keep_alive > 0 {
        // +1 because the next frame is "frame 1" — the budget needs to
        // outlive *this* render so `about_to_wait` schedules another one.
        gpu.pending_frames = gpu.pending_frames.max(keep_alive);
    }

    gpu.platform.prepare_render_with_ui(ui, &gpu.window);
    let draw_data = gpu.context.render();

    let mut enc = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("app_window"),
        });
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(gpu.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if draw_data.total_vtx_count > 0
            && let Err(e) = gpu.renderer.render_draw_data(draw_data, &mut pass)
        {
            let line = format!("app_window: imgui render error: {e:?}");
            eprintln!("{line}");
            super::win32_debug_log(&line);
        }
    }
    gpu.queue.submit(Some(enc.finish()));
    frame.present();

    gpu.last_redraw = Instant::now();

    dispatch_actions(gpu, cfg, handler, tb_result, event_loop);
}

// ── Action dispatcher ─────────────────────────────────────────────────────────

fn dispatch_actions<H: AppHandler>(
    gpu: &mut GpuState,
    cfg: &mut AppConfig,
    handler: &mut H,
    tb: TitlebarResult,
    event_loop: &ActiveEventLoop,
) {
    // Titlebar actions first.
    match tb.action {
        TitlebarAction::None => {}
        TitlebarAction::Close => {
            gpu.app_state.should_exit = true;
        }
        TitlebarAction::CloseRequested => {
            handler.on_close_requested(&mut gpu.app_state);
        }
        TitlebarAction::Extra(id) => {
            handler.on_extra_button(id, &mut gpu.app_state);
        }
        TitlebarAction::IconClick => {
            handler.on_icon_click(&mut gpu.app_state);
        }
        TitlebarAction::DragStart => {
            gpu.cursor_set = false;
            gpu.window.drag_window().ok();
        }
        TitlebarAction::ResizeStart(edge) => {
            gpu.cursor_set = false;
            gpu.window.drag_resize_window(resize_direction(edge)).ok();
        }
        TitlebarAction::Minimize => {
            #[cfg(windows)]
            if super::win32::is_win11() && gpu.window.is_maximized() {
                gpu.window.set_maximized(false);
                gpu.pending_remax = true;
            }
            gpu.window.set_minimized(true);
        }
        TitlebarAction::Maximize => {
            let next = !gpu.window.is_maximized();
            gpu.window.set_maximized(next);
            // Flip the titlebar state immediately so the icon updates in the
            // same frame, instead of waiting for the OS to deliver `WM_SIZE`.
            gpu.app_state.titlebar.set_maximized(next);
            gpu.app_state.request_maximize = None;
        }
    }

    // AppState-requested actions (set inside `render()` callbacks).
    if let Some(v) = gpu.app_state.request_maximize.take() {
        gpu.window.set_maximized(v);
    }
    if gpu.app_state.request_minimize {
        gpu.app_state.request_minimize = false;
        #[cfg(windows)]
        if super::win32::is_win11() && gpu.window.is_maximized() {
            gpu.window.set_maximized(false);
            gpu.pending_remax = true;
        }
        gpu.window.set_minimized(true);
    }
    if let Some(theme) = gpu.app_state.pending_theme.take() {
        theme.apply_imgui_style(gpu.context.style_mut());
        cfg.theme = theme;
        gpu.refresh_clear_color(cfg);
        handler.on_theme_changed(&theme, &mut gpu.app_state);
    }
    if let Some(title) = gpu.app_state.pending_title.take() {
        gpu.window.set_title(&title);
        cfg.title = title;
    }
    #[cfg(windows)]
    if let Some(alpha) = gpu.app_state.pending_opacity.take()
        && let Some(hwnd) = super::win32::hwnd_of(&gpu.window)
    {
        super::win32::set_opacity(hwnd, alpha);
    }
    if let Some(v) = gpu.app_state.request_visible.take() {
        gpu.window.set_visible(v);
    }
    if gpu.app_state.should_exit {
        event_loop.exit();
    }
}
