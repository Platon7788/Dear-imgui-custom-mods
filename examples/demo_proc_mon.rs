//! Demo: Process Monitor — NT syscall enumeration with VirtualTable display.
//!
//! Run: cargo run --example demo_proc_mon
//!
//! Note: Windows-only. Uses direct NT syscalls for process enumeration.

use dear_imgui_custom_mod::proc_mon::{MonitorEvent, MonitorConfig, ProcessEnumerator, ProcessMonitor};
use dear_imgui_custom_mod::theme::Theme;
use dear_imgui_rs::{Condition, StyleColor, Ui};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

// ─── Demo state ─────────────────────────────────────────────────────────────

struct DemoState {
    /// Process enumerator (syscall-based).
    enumerator: ProcessEnumerator,
    /// UI monitor.
    monitor: ProcessMonitor,
    /// Configuration.
    config: MonitorConfig,
    /// Last enumeration time.
    last_tick: Instant,
    /// Show monitor window.
    show_monitor: bool,
    /// Selected PID for context menu demo.
    context_pid: Option<u32>,
    /// Show context menu popup.
    show_context_menu: bool,
    /// Status message.
    status: String,
}

impl DemoState {
    fn new() -> Self {
        let config = MonitorConfig::default();
        let mut enumerator = ProcessEnumerator::new();
        // Only pay the CPU% delta cost when the column is actually shown.
        enumerator.set_cpu_tracking(config.columns.cpu_percent);
        Self {
            enumerator,
            monitor: ProcessMonitor::new(config.clone()),
            config,
            last_tick: Instant::now(),
            show_monitor: true,
            context_pid: None,
            show_context_menu: false,
            status: "Ready".to_string(),
        }
    }

    fn render(&mut self, ui: &Ui) {
        // Enumerate processes at configured interval (1-5000ms)
        let now = Instant::now();
        if now.duration_since(self.last_tick) >= self.config.interval() {
            self.last_tick = now;

            match self.enumerator.enumerate_delta() {
                Ok(delta) => {
                    self.monitor.apply_delta(&delta);
                    self.status = format!("{} processes", delta.total);
                }
                Err(e) => {
                    self.status = format!("Error: {}", e);
                }
            }
        }

        // Main window
        ui.window("Process Monitor Demo")
            .size([800.0, 600.0], Condition::FirstUseEver)
            .build(|| {
                // Toolbar
                ui.text(&self.status);
                ui.same_line();
                if ui.button("Refresh") {
                    self.enumerator.clear_cache();
                    self.last_tick = Instant::now() - Duration::from_secs(1); // Force immediate refresh
                }
                ui.same_line();
                ui.checkbox("Show Monitor", &mut self.show_monitor);

                ui.separator();

                // Instructions
                ui.text_wrapped("Right-click a process to see the context menu (handled by caller).");
                ui.text_wrapped("This demo shows how to integrate proc_mon with your own actions.");

                ui.spacing();

                // Render monitor
                if self.show_monitor
                    && let Some(event) = self.monitor.render(ui, &mut self.show_monitor)
                {
                    match event {
                        MonitorEvent::RowSelected(pid) => {
                            self.status = format!("Selected PID: {}", pid);
                        }
                        MonitorEvent::RowDoubleClicked(pid) => {
                            self.status = format!("Double-clicked PID: {}", pid);
                        }
                        MonitorEvent::ContextMenuRequested(pid) => {
                            self.context_pid = Some(pid);
                            self.show_context_menu = true;
                            ui.open_popup("##proc_ctx");
                        }
                    }
                }

                // Context menu (rendered by caller)
                if self.show_context_menu {
                    let _popup = ui
                        .begin_modal_popup_config("##proc_ctx")
                        .flags(dear_imgui_rs::WindowFlags::NO_TITLE_BAR | dear_imgui_rs::WindowFlags::ALWAYS_AUTO_RESIZE)
                        .begin();

                    if let Some(_popup) = _popup {
                        if let Some(pid) = self.context_pid {
                            ui.text(format!("PID: {}", pid));
                            ui.separator();

                            // Example actions - caller decides what to show
                            if ui.button("Copy PID") {
                                // In real app, copy to clipboard
                                self.status = format!("Copied PID: {}", pid);
                                ui.close_current_popup();
                            }

                            ui.same_line();

                            if ui.button("Details...") {
                                self.status = format!("Show details for PID: {}", pid);
                                ui.close_current_popup();
                            }

                            ui.separator();

                            // Destructive action with confirmation style
                            let _red = ui.push_style_color(StyleColor::Button, [0.6, 0.2, 0.2, 1.0]);
                            let _red_hover = ui.push_style_color(StyleColor::ButtonHovered, [0.8, 0.25, 0.25, 1.0]);
                            if ui.button("Kill Process") {
                                self.status = format!("Kill PID: {} (not implemented in demo)", pid);
                                ui.close_current_popup();
                            }
                        }

                        // Close on click outside
                        if !ui.is_item_hovered() && ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Left) {
                            ui.close_current_popup();
                        }
                    } else {
                        self.show_context_menu = false;
                    }
                }
            });
    }
}

// ─── wgpu + winit + imgui boilerplate ───────────────────────────────────────

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_cfg: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    context: dear_imgui_rs::Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    demo: DemoState,
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
                        .with_inner_size(LogicalSize::new(800.0, 600.0))
                        .with_title("Process Monitor Demo"),
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
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };
        surface.configure(&device, &surface_cfg);

        let mut context = dear_imgui_rs::Context::create();
        let _ = context.set_ini_filename(None::<std::path::PathBuf>);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, HiDpiMode::Default, &mut context);

        let hidpi = window.scale_factor() as f32;
        let font_size = 15.0 * hidpi;
        context.io_mut().set_font_global_scale(1.0 / hidpi);

        // Load font
        let segoe_path = "C:\\Windows\\Fonts\\segoeui.ttf";
        if std::path::Path::new(segoe_path).exists() {
            let font_data = std::fs::read(segoe_path).expect("read font");
            let font_data: &'static [u8] = Box::leak(font_data.into_boxed_slice());
            context
                .fonts()
                .add_font(&[dear_imgui_rs::FontSource::TtfData {
                    data: font_data,
                    size_pixels: Some(font_size),
                    config: Some(
                        dear_imgui_rs::FontConfig::new()
                            .size_pixels(font_size)
                            .oversample_h(2),
                    ),
                }]);
        } else {
            context
                .fonts()
                .add_font(&[dear_imgui_rs::FontSource::DefaultFontData {
                    config: Some(
                        dear_imgui_rs::FontConfig::new()
                            .size_pixels(font_size)
                            .oversample_h(2),
                    ),
                    size_pixels: Some(font_size),
                }]);
        }

        // Apply theme
        Theme::Midnight.apply_imgui_style(context.style_mut());

        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_cfg.format),
            &mut context,
        )
        .expect("renderer");

        self.gpu = Some(GpuState {
            device,
            queue,
            window,
            surface_cfg,
            surface,
            context,
            platform,
            renderer,
            demo: DemoState::new(),
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

        gpu.platform.handle_event::<()>(
            &mut gpu.context,
            &gpu.window,
            &Event::WindowEvent {
                window_id,
                event: event.clone(),
            },
        );

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

                gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);

                let ui = gpu.context.frame();
                gpu.demo.render(ui);
                gpu.platform.prepare_render_with_ui(ui, &gpu.window);

                let draw_data = gpu.context.render();

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

                    if draw_data.total_vtx_count > 0 {
                        gpu.renderer
                            .render_draw_data(draw_data, &mut pass)
                            .expect("render");
                    }
                }

                gpu.queue.submit(Some(encoder.finish()));
                frame.present();

                gpu.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Cap render loop to ~30 FPS (33ms period). Monitoring UIs don't need
        // 60 Hz and this halves CPU cost vs proactive `request_redraw` on every
        // Poll cycle. `WaitUntil` lets the OS park the thread between frames.
        let next = Instant::now() + Duration::from_millis(33);
        event_loop.set_control_flow(ControlFlow::WaitUntil(next));
        if let Some(gpu) = self.gpu.as_ref() {
            gpu.window.request_redraw();
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run");
}
