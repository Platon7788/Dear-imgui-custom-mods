//! GPU + ImGui init invoked on the first `resumed` event.

use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::window::Window;

use super::{
    gpu, state,
    AppHandler, BorderStyle, FpsMode, FormStyle, RenderMode, WindowKind,
};
#[cfg(windows)]
use super::win32;

pub(super) fn init<H: AppHandler + 'static>(
    app: &mut super::WinitApp<H>,
    event_loop: &winit::event_loop::ActiveEventLoop,
) {
    let cfg = &app.config;

    // Borderless window via the **post-creation** `set_decorations(false)`
    // route. The window is created WITH normal decorations
    // (`WS_OVERLAPPEDWINDOW`, full caption-metrics, default DWM
    // composition); after wgpu init we call `window.set_decorations(false)`
    // which flips winit's `MARKER_DECORATIONS` flag and triggers
    // `SetWindowPos(SWP_FRAMECHANGED)`. From that point on winit's own
    // `WM_NCCALCSIZE` handler returns `0` (kills NC area).
    //
    // Why post-creation, not at attribute time:
    // creating with `with_decorations(false)` from the start works on
    // most machines but causes phantom NC-frame artifacts on a subset
    // of configurations (high-DPI laptops, hybrid GPUs, certain shell
    // extensions) — DWM has no chance to fully initialise composition
    // for the chrome before we strip it, and on those configs renders
    // a default chrome strip / left border as a fallback. Letting DWM
    // see proper chrome first, *then* stripping it, sidesteps this
    // entirely. Verified against the working reference at
    // `D:\\GitHub\\Rust_Projects\\test-dear-imgui-rs` which uses the
    // exact same sequence.
    let mut attrs = Window::default_attributes()
        .with_title(cfg.title.clone())
        .with_inner_size(LogicalSize::new(cfg.size[0], cfg.size[1]))
        .with_decorations(true)
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

    // Strip the OS chrome AFTER swapchain priming but BEFORE making the
    // window visible. winit flips MARKER_DECORATIONS off and triggers
    // SetWindowPos(SWP_FRAMECHANGED) — its own WM_NCCALCSIZE handler
    // now returns 0 for every NC layout pass, so the window is shown
    // already borderless. The window is hidden at this point so there
    // is no visible chrome flash.
    window.set_decorations(false);

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
                    // `Auto` — pacing handled by `PresentMode::Fifo`
                    // inside `render_frame::get_current_texture`, which
                    // blocks until the next vblank. No software interval
                    // needed; the vsync block IS the cap and naturally
                    // matches the monitor's real refresh rate.
                    FpsMode::Auto => Duration::ZERO,
                    // `Fixed(0)` (degenerate) and `Unlimited` — no cap.
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
    let titlebar_palette = cfg.theme.titlebar();
    let titlebar_u32 = super::chrome::TitlebarColorsU32::from_palette(&titlebar_palette);

    app.gpu = Some(gpu::GpuState {
        device,
        queue,
        window,
        surface,
        surface_cfg,
        context,
        platform,
        renderer,
        app_state: state::AppState::new(app.proxy.clone()),
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
        cached_titlebar: titlebar_palette,
        cached_titlebar_u32: titlebar_u32,
    });
}
