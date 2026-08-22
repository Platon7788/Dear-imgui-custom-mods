//! Demo: `chrome::Chrome` + `dear-app` — canonical borderless-window pattern.
//!
//! Replaces the deleted `demo_app_window`. Shows the recommended wiring:
//! single full-display root window where the chrome titlebar + host content
//! coexist, dispatched via `dear-app`'s three callbacks.
//!
//! Run with:
//!   cargo run --example demo_chrome --release
//!
//! Highlights:
//! - **Borderless titlebar** with min / max / close + drag + double-click
//!   maximise + 8-direction edge resize + DPI-scaled rounded corners.
//! - **Runtime theme switch** (Dark / Light / Midnight / Solarized / Monokai
//!   / Catppuccin / Nord) — `Chrome::set_theme` refreshes the cached palette.
//! - **CloseMode::Confirm** flow — close button surfaces an in-app
//!   confirmation popup instead of exiting; the host calls `process::exit`
//!   only after explicit user confirmation.
//! - **Runtime config mutation** — `Chrome::config_mut()` flips button
//!   visibility on the fly without reconstructing the chrome.
//! - **Critical** `docking: { enable: false, auto_dockspace: false }` — without
//!   this dear-app's auto dockspace would absorb every click before chrome
//!   sees it.

use std::sync::Arc;

use dear_imgui_custom_mod::chrome::{Chrome, CloseMode, TitlebarConfig};
use dear_imgui_custom_mod::theme::Theme;
use dear_imgui_rs::{StyleVar, Ui, WindowFlags};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

/// Per-app state owned by the `on_frame` closure.
struct DemoState {
    /// Active theme. `last_applied_theme` lets us call `Chrome::set_theme`
    /// only on transitions instead of every frame.
    theme: Theme,
    last_applied_theme: Theme,
    /// `true` while the close-confirm popup is open. Set by
    /// `take_close_request` when the user clicks × in `CloseMode::Confirm`.
    confirm_close_open: bool,
    /// Counter incremented by the demo button — proves clicks reach widgets.
    counter: u32,
    /// Toggles for the `Chrome::config_mut` runtime mutation demo.
    show_minimize: bool,
    show_maximize: bool,
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            last_applied_theme: Theme::Dark,
            confirm_close_open: false,
            counter: 0,
            show_minimize: true,
            show_maximize: true,
        }
    }
}

const THEMES: &[(Theme, &str)] = &[(Theme::Dark, "Dark"), (Theme::Light, "Light")];

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run app");
}

// ─── wgpu + winit + imgui + chrome boilerplate ──────────────────────────────
//
// dear-app 0.17 owns the window and its `Application`/frame contexts lend only
// `&Window` (and none in `frame()`), but `chrome` needs `&Arc<Window>` plus the
// window every frame for `render`. So this demo drives its own winit + wgpu loop
// (see demo_table for the base harness) and wires chrome into it directly.

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_cfg: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    context: dear_imgui_rs::Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    chrome: Chrome,
    state: DemoState,
}

struct App {
    gpu: Option<GpuState>,
}

impl App {
    fn new() -> Self {
        Self { gpu: None }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_inner_size(LogicalSize::new(1100.0, 720.0))
                        .with_title("Chrome demo")
                        .with_decorations(false),
                )
                .expect("window"),
        );

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("adapter");
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");

        let phys = window.inner_size();
        let surface_cfg = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: phys.width.max(1),
            height: phys.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };
        surface.configure(&device, &surface_cfg);

        let mut context = dear_imgui_rs::Context::create();
        let _ = context.set_ini_filename(None::<std::path::PathBuf>);

        let mut platform = WinitPlatform::new(&mut context).expect("winit platform");
        platform
            .attach_window(window.clone(), HiDpiMode::Default, &mut context)
            .expect("attach window");

        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_cfg.format),
            &mut context,
        )
        .expect("renderer");

        // Borderless chrome: strip OS decorations, apply DWM dark mode +
        // rounded corners. Runs once, now that the window exists.
        let mut chrome = Chrome::new(TitlebarConfig::default().with_close_confirm())
            .with_title("Chrome demo")
            .with_theme(Theme::Dark)
            .with_corner_radius(8);
        chrome.on_setup(&window);

        self.gpu = Some(GpuState {
            device,
            queue,
            window,
            surface_cfg,
            surface,
            context,
            platform,
            renderer,
            chrome,
            state: DemoState::default(),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };

        // Chrome first — borderless drag / resize / maximise tracking plus its
        // Ctrl/Alt shortcut normalisation. It expects a full winit `Event`, so
        // wrap the `WindowEvent`; then forward to the imgui platform. This is
        // the exact ordering dear-app applied internally.
        let wrapped = winit::event::Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        gpu.chrome.on_event(&wrapped, &gpu.window, &mut gpu.context);
        let _ = gpu
            .platform
            .handle_window_event(&mut gpu.context, &gpu.window, &event);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                gpu.surface_cfg.width = new_size.width.max(1);
                gpu.surface_cfg.height = new_size.height.max(1);
                gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                        return;
                    }
                    other => {
                        eprintln!("Surface unavailable: {other:?}");
                        return;
                    }
                };
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                gpu.platform
                    .prepare_frame(&mut gpu.context, &gpu.window)
                    .expect("prepare_frame");

                let ui = gpu.context.frame();

                // Borderless titlebar + host content in one full-display root.
                gpu.chrome.render(ui, &gpu.window, |ui, _area| {
                    let _pad = ui.push_style_var(StyleVar::WindowPadding([12.0, 12.0]));
                    let _spc = ui.push_style_var(StyleVar::ItemSpacing([8.0, 6.0]));
                    ui.child_window("##content")
                        .size([0.0, 0.0])
                        .border(false)
                        .build(ui, || {
                            render_content(ui, &mut gpu.state);
                        });
                });

                // Bridge demo state → chrome config / palette (theme switch is
                // gated so `set_theme`'s palette rebuild only fires on change).
                if gpu.state.theme != gpu.state.last_applied_theme {
                    gpu.chrome.set_theme(gpu.state.theme);
                    gpu.state.last_applied_theme = gpu.state.theme;
                }
                gpu.chrome.config_mut().buttons.minimize = gpu.state.show_minimize;
                gpu.chrome.config_mut().buttons.maximize = gpu.state.show_maximize;

                // CloseMode::Confirm — surface an in-app popup instead of exiting.
                if let Some(mode) = gpu.chrome.take_close_request() {
                    match mode {
                        CloseMode::Immediate => std::process::exit(0),
                        CloseMode::Confirm => {
                            gpu.state.confirm_close_open = true;
                        }
                    }
                }
                if gpu.state.confirm_close_open {
                    ui.open_popup("##confirm_close");
                }
                draw_confirm_close_popup(ui, &mut gpu.state);

                gpu.platform
                    .prepare_render(ui, &gpu.window)
                    .expect("prepare_render");

                let consumer = gpu.renderer.renderer_consumer().expect("renderer consumer");
                let pending = gpu.context.render(consumer);
                let framebuffer_extent =
                    dear_imgui_wgpu::FramebufferExtent::from_texture(&frame.texture);

                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("imgui"),
                        });
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("imgui_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.08,
                                    g: 0.09,
                                    b: 0.11,
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
                    gpu.renderer
                        .render(pending, &mut pass, framebuffer_extent)
                        .expect("render");
                }
                gpu.queue.submit(Some(encoder.finish()));
                gpu.queue.present(frame);
                gpu.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(gpu) = self.gpu.as_ref() {
            gpu.window.request_redraw();
        }
    }
}

/// Demo body — a single child window's worth of content.
fn render_content(ui: &Ui, state: &mut DemoState) {
    ui.text_colored(
        [1.0, 0.84, 0.0, 1.0],
        "Chrome demo — borderless titlebar + dear-app",
    );
    ui.separator();
    ui.spacing();

    ui.text_wrapped(
        "Drag the title text to move the window. Drag any edge / corner to \
         resize. Double-click the title to toggle maximise. The titlebar \
         buttons (− □ ×) work as expected.",
    );
    ui.spacing();

    // ── Theme picker ─────────────────────────────────────────────────────
    if ui.collapsing_header("Theme switch", dear_imgui_rs::TreeNodeFlags::DEFAULT_OPEN) {
        ui.text("Click a button to switch the chrome titlebar palette in real time:");
        let mut switched_to: Option<Theme> = None;
        for &(theme, label) in THEMES {
            if state.theme == theme {
                ui.text_colored([0.4, 0.85, 0.4, 1.0], format!("• {label} (active)"));
            } else if ui.button(label) {
                switched_to = Some(theme);
            }
            ui.same_line();
        }
        ui.new_line();
        if let Some(theme) = switched_to {
            state.theme = theme;
            // The chrome's palette is bridged via the demo's main loop —
            // see the `state` mutation block in `on_frame`.
        }
    }

    ui.spacing();

    // ── Runtime config mutation ──────────────────────────────────────────
    if ui.collapsing_header(
        "Runtime config mutation",
        dear_imgui_rs::TreeNodeFlags::DEFAULT_OPEN,
    ) {
        ui.text("Toggles flow into Chrome::config_mut() on the next frame:");
        ui.checkbox("Show minimize button", &mut state.show_minimize);
        ui.checkbox("Show maximize button", &mut state.show_maximize);
    }

    ui.spacing();

    // ── Click-through proof ──────────────────────────────────────────────
    if ui.collapsing_header(
        "Click-through proof",
        dear_imgui_rs::TreeNodeFlags::DEFAULT_OPEN,
    ) {
        ui.text("Counter increments on each click — confirms clicks reach widgets");
        ui.text(
            "(broken when `docking.auto_dockspace = true` — that dockspace \
             absorbs the click before any non-docked window sees it).",
        );
        if ui.button("Click me!") {
            state.counter += 1;
        }
        ui.same_line();
        ui.text(format!("Clicks: {}", state.counter));
    }

    ui.spacing();

    // ── Close-mode notice ────────────────────────────────────────────────
    if ui.collapsing_header(
        "CloseMode::Confirm",
        dear_imgui_rs::TreeNodeFlags::DEFAULT_OPEN,
    ) {
        ui.text_wrapped(
            "This chrome was built with `.with_close_confirm()`. Clicking × \
             surfaces the popup below (the host owns the dialog flow); the \
             window only exits after explicit confirmation.",
        );
    }
}

/// Modal confirmation rendered when the user clicks the close button while
/// the chrome's `close_mode == Confirm`. Two buttons: cancel resets the
/// demo state; confirm exits the process.
fn draw_confirm_close_popup(ui: &Ui, state: &mut DemoState) {
    let _pad = ui.push_style_var(StyleVar::WindowPadding([16.0, 16.0]));
    if let Some(_tok) = ui
        .begin_modal_popup_config("##confirm_close")
        .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
        .begin()
    {
        ui.text("Close the demo?");
        ui.spacing();
        ui.separator();
        ui.spacing();
        if ui.button_with_size("Cancel", [120.0, 0.0]) {
            state.confirm_close_open = false;
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button_with_size("Quit", [120.0, 0.0]) {
            std::process::exit(0);
        }
    }
}
