//! wgpu + winit + Dear ImGui initialisation and per-frame rendering helpers.

use crate::borderless_window::{
    WindowAction, actions::ResizeEdge, platform::{cursor_icon_for_edge, resize_direction_of},
    render_titlebar,
};
use dear_imgui_rs::{Condition, StyleVar, WindowFlags};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::sync::Arc;
use std::time::Duration;
use winit::{event_loop::ActiveEventLoop, window::Window};

use super::state::AppState;
use super::AppHandler;

/// All GPU + ImGui resources needed for one application window.
pub(super) struct GpuState {
    pub device:       wgpu::Device,
    pub queue:        wgpu::Queue,
    pub window:       Arc<Window>,
    pub surface_cfg:  wgpu::SurfaceConfiguration,
    pub surface:      wgpu::Surface<'static>,
    pub context:      dear_imgui_rs::Context,
    pub platform:     WinitPlatform,
    pub renderer:     WgpuRenderer,
    pub app_state:    AppState,
    pub titlebar_cfg: crate::borderless_window::BorderlessConfig,
    pub fps_interval: Duration,
}

// ── wgpu setup ────────────────────────────────────────────────────────────────

/// Create and configure a `wgpu` surface + device/queue for the given window.
pub(super) fn init_wgpu(window: &Arc<Window>) -> (
    wgpu::Device,
    wgpu::Queue,
    wgpu::Surface<'static>,
    wgpu::SurfaceConfiguration,
) {
    #[cfg(target_os = "windows")]
    let backends = wgpu::Backends::DX12 | wgpu::Backends::VULKAN | wgpu::Backends::GL;
    #[cfg(not(target_os = "windows"))]
    let backends = wgpu::Backends::PRIMARY;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let surface = instance
        .create_surface(window.clone())
        .expect("wgpu: create_surface failed");

    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .or_else(|_| {
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            compatible_surface: Some(&surface),
            force_fallback_adapter: true,
        }))
    })
    .expect("wgpu: no suitable adapter found");

    let (device, queue) = block_on(adapter.request_device(
        &wgpu::DeviceDescriptor::default(),
    ))
    .expect("wgpu: request_device failed");

    let phys = window.inner_size();
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|&&f| {
            f == wgpu::TextureFormat::Bgra8UnormSrgb
                || f == wgpu::TextureFormat::Rgba8UnormSrgb
        })
        .copied()
        .or_else(|| surface_caps.formats.first().copied())
        .expect("wgpu: adapter reports no supported surface formats");

    let surface_cfg = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: phys.width.max(1),
        height: phys.height.max(1),
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
    };
    surface.configure(&device, &surface_cfg);

    (device, queue, surface, surface_cfg)
}

// ── ImGui setup ───────────────────────────────────────────────────────────────

/// Build the Dear ImGui context + platform + renderer.
pub(super) fn init_imgui(
    window: &Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    font_size: f32,
    titlebar_cfg: &crate::borderless_window::BorderlessConfig,
) -> (dear_imgui_rs::Context, WinitPlatform, WgpuRenderer) {
    let mut context = dear_imgui_rs::Context::create();
    let _ = context.set_ini_filename(None::<std::path::PathBuf>);

    let mut platform = WinitPlatform::new(&mut context);
    platform.attach_window(window, HiDpiMode::Default, &mut context);

    let hidpi = (window.scale_factor() as f32).clamp(1.0, 3.0);
    let scaled_font = (font_size * hidpi).round();
    context.io_mut().set_font_global_scale(1.0 / hidpi);

    use crate::code_editor::BuiltinFont;
    context.fonts().add_font_from_memory_ttf(
        BuiltinFont::Hack.data(),
        scaled_font,
        Some(
            &dear_imgui_rs::FontConfig::new()
                .size_pixels(scaled_font)
                .oversample_h(2)
                .name("Hack"),
        ),
        None,
    );

    titlebar_cfg.theme.apply_imgui_style(context.style_mut());

    let renderer = WgpuRenderer::new(
        WgpuInitInfo::new(device, queue, surface_format),
        &mut context,
    )
    .expect("imgui-wgpu: renderer init failed");

    (context, platform, renderer)
}

// ── Frame rendering ───────────────────────────────────────────────────────────

/// Render one frame: acquire surface texture, build UI, submit GPU commands.
pub(super) fn render_frame<H: AppHandler>(
    gpu: &mut GpuState,
    handler: &mut H,
    event_loop: &ActiveEventLoop,
) {
    let frame = match gpu.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) => f,
        wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
            gpu.window.request_redraw();
            f
        }
        wgpu::CurrentSurfaceTexture::Outdated
        | wgpu::CurrentSurfaceTexture::Lost => {
            gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
            gpu.window.request_redraw();
            return;
        }
        other => {
            eprintln!("app_window: surface error: {other:?}");
            return;
        }
    };

    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);
    let ui = gpu.context.frame();

    let mut winit_action = WindowAction::None;
    let mut hover_edge: Option<ResizeEdge> = None;

    // Style tokens borrow `ui`; wrap in a block so they drop before
    // `prepare_render_with_ui` consumes `ui`.
    {
        let display = ui.io().display_size();
        let _no_pad = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _no_sp  = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));

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
                let res = render_titlebar(
                    ui,
                    &gpu.titlebar_cfg,
                    &mut gpu.app_state.titlebar,
                );
                hover_edge = res.hover_edge;

                match res.action {
                    WindowAction::Close => {
                        gpu.app_state.should_exit = true;
                    }
                    WindowAction::CloseRequested => {
                        handler.on_close_requested(&mut gpu.app_state);
                    }
                    WindowAction::Extra(id) => {
                        handler.on_extra_button(id, &mut gpu.app_state);
                    }
                    WindowAction::IconClick => {
                        handler.on_icon_click(&mut gpu.app_state);
                    }
                    other => winit_action = other,
                }

                // Restore content-area padding for user widgets.
                let _ip = ui.push_style_var(StyleVar::WindowPadding([8.0, 8.0]));
                let _is = ui.push_style_var(StyleVar::ItemSpacing([6.0, 4.0]));
                handler.render(ui, &mut gpu.app_state);
            });
    } // _no_pad, _no_sp dropped here

    gpu.window.set_cursor(cursor_icon_for_edge(hover_edge));
    gpu.platform.prepare_render_with_ui(ui, &gpu.window);
    let draw_data = gpu.context.render();

    // Background clear colour derived from theme.
    let bg = gpu.titlebar_cfg.resolved_colors().bg;
    let clear = wgpu::Color {
        r: bg[0] as f64,
        g: bg[1] as f64,
        b: bg[2] as f64,
        a: 1.0,
    };

    let mut enc = gpu.device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("app_window") },
    );
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
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
            eprintln!("app_window: imgui render error: {e:?}");
        }
    }
    gpu.queue.submit(Some(enc.finish()));
    frame.present();

    // Dispatch OS window actions from titlebar.
    match winit_action {
        WindowAction::Minimize => {
            gpu.window.set_minimized(true);
        }
        WindowAction::Maximize => {
            let next = !gpu.app_state.titlebar.maximized;
            gpu.window.set_maximized(next);
            gpu.app_state.titlebar.set_maximized(next);
            // Clear any same-frame AppState request to prevent double-toggle.
            gpu.app_state.maximize_toggle = None;
        }
        WindowAction::DragStart => {
            gpu.window.drag_window().ok();
        }
        WindowAction::ResizeStart(e) => {
            gpu.window.drag_resize_window(resize_direction_of(e)).ok();
        }
        _ => {}
    }

    // AppState-requested maximize toggle.
    if let Some(v) = gpu.app_state.maximize_toggle.take() {
        gpu.window.set_maximized(v);
    }

    // Theme change requested from within render().
    if let Some(theme) = gpu.app_state.pending_theme.take() {
        theme.apply_imgui_style(gpu.context.style_mut());
        gpu.titlebar_cfg.theme = theme;
        handler.on_theme_changed(&theme, &mut gpu.app_state);
    }

    if gpu.app_state.should_exit {
        event_loop.exit();
    }
}

// ── Window positioning ────────────────────────────────────────────────────────

/// Position the window on startup according to [`StartPosition`](super::StartPosition).
pub(super) fn position_window(
    window: &Window,
    pos: &super::StartPosition,
    event_loop: &ActiveEventLoop,
) {
    match pos {
        super::StartPosition::CenterScreen => {
            if let Some(mon) = event_loop.primary_monitor() {
                let mp = mon.position();
                let ms = mon.size();
                let ws = window.inner_size();
                window.set_outer_position(winit::dpi::PhysicalPosition::new(
                    mp.x + (ms.width as i32 - ws.width as i32) / 2,
                    mp.y + (ms.height as i32 - ws.height as i32) / 2,
                ));
            }
        }
        super::StartPosition::TopLeft => {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(0, 0));
        }
        super::StartPosition::Custom(x, y) => {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(*x, *y));
        }
    }
}
