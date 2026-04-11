//! Demo: StatusBar + Toolbar — composable UI chrome showcase.
//!
//! Demonstrates toolbar with buttons, toggles, dropdowns, separators,
//! spacers; and status bar with indicators, progress, clickable items.
//!
//! Run: cargo run --example demo_status_toolbar

use dear_imgui_custom_mod::status_bar::{Indicator, StatusBar, StatusItem};
use dear_imgui_custom_mod::toolbar::{Toolbar, ToolbarEvent, ToolbarItem};
use dear_imgui_rs::{Condition, StyleColor, Ui};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

// ─── Demo state ──────────────────────────────────────────────────────────────

struct DemoState {
    toolbar: Toolbar,
    status: StatusBar,
    log: Vec<String>,
    line: u32,
    col: u32,
    progress: f32,
    connected: bool,
    encoding_idx: usize,
}

impl DemoState {
    fn new() -> Self {
        let mut toolbar = Toolbar::new("##toolbar_demo");

        // File operations
        toolbar.add(ToolbarItem::button("\u{E800} New", "Create new file (Ctrl+N)"));
        toolbar.add(ToolbarItem::button("\u{E801} Open", "Open file (Ctrl+O)"));
        toolbar.add(ToolbarItem::button("\u{E802} Save", "Save file (Ctrl+S)"));
        toolbar.add(ToolbarItem::separator());

        // Edit operations
        toolbar.add(ToolbarItem::button("Undo", "Undo (Ctrl+Z)"));
        toolbar.add(ToolbarItem::button("Redo", "Redo (Ctrl+Y)"));
        toolbar.add(ToolbarItem::separator());

        // Toggles
        toolbar.add(ToolbarItem::toggle("Bold", false, "Toggle bold (Ctrl+B)"));
        toolbar.add(ToolbarItem::toggle("Italic", false, "Toggle italic (Ctrl+I)"));
        toolbar.add(ToolbarItem::toggle("Wrap", true, "Toggle word wrap"));
        toolbar.add(ToolbarItem::separator());

        // Run
        toolbar.add(ToolbarItem::button("\u{25B6} Run", "Run project (F5)"));
        toolbar.add(ToolbarItem::button("\u{25A0} Stop", "Stop (Shift+F5)")
            .with_enabled(false));
        toolbar.add(ToolbarItem::separator());

        // Dropdown
        toolbar.add(ToolbarItem::dropdown(
            "Config",
            vec!["Debug".into(), "Release".into(), "Test".into()],
            0,
            "Build configuration",
        ));

        // Spacer pushes remaining items right
        toolbar.add(ToolbarItem::spacer());

        // Settings at the right
        toolbar.add(ToolbarItem::button("Settings", "Open settings"));

        Self {
            toolbar,
            status: StatusBar::new("##status_demo"),
            log: vec!["Toolbar + StatusBar demo loaded.".into()],
            line: 42,
            col: 15,
            progress: 0.0,
            connected: true,
            encoding_idx: 0,
        }
    }

    fn render(&mut self, ui: &Ui) {
        // Animate progress
        self.progress += ui.io().delta_time() * 0.05;
        if self.progress > 1.0 { self.progress = 0.0; }

        ui.window("Toolbar + StatusBar Demo")
            .size([900.0, 550.0], Condition::FirstUseEver)
            .build(|| {
                // ── Toolbar at top ──────────────────────────────
                let events = self.toolbar.render(ui);

                for ev in &events {
                    match ev {
                        ToolbarEvent::ButtonClicked { label, .. } => {
                            self.log.push(format!("Button: {}", label));
                        }
                        ToolbarEvent::Toggled { label, on, .. } => {
                            self.log.push(format!("Toggle: {} = {}", label, on));
                        }
                        ToolbarEvent::DropdownChanged { label, selected, .. } => {
                            self.log.push(format!("Dropdown: {} -> #{}", label, selected));
                        }
                    }
                }

                ui.separator();

                // ── Main content area ───────────────────────────
                let avail = ui.content_region_avail();
                let status_h = 26.0;
                let content_h = avail[1] - status_h - 4.0;

                ui.child_window("##content")
                    .size([avail[0], content_h])
                    .build(ui, || {
                        self.render_content(ui);
                    });

                ui.spacing();

                // ── Status bar at bottom ────────────────────────
                self.rebuild_status();
                let status_events = self.status.render(ui);

                for ev in &status_events {
                    self.log.push(format!("Status clicked: {}", ev.label));
                    if ev.label == "UTF-8" || ev.label == "UTF-16" {
                        self.encoding_idx = (self.encoding_idx + 1) % 2;
                    }
                    if ev.label.contains("Connected") || ev.label.contains("Disconnected") {
                        self.connected = !self.connected;
                    }
                }
            });
    }

    fn rebuild_status(&mut self) {
        self.status.clear();

        // Left: connection status + position
        let (ind, label) = if self.connected {
            (Indicator::Success, "Connected")
        } else {
            (Indicator::Error, "Disconnected")
        };
        self.status.left(StatusItem::indicator(label, ind)
            .with_tooltip("Click to toggle connection"));
        self.status.left(StatusItem::text(format!("Ln {}, Col {}", self.line, self.col)));
        self.status.left(StatusItem::progress("Build", self.progress)
            .with_tooltip(format!("{:.0}% complete", self.progress * 100.0)));

        // Center: mode
        self.status.center(StatusItem::text("NORMAL")
            .with_color([0.5, 0.8, 0.5, 1.0]));

        // Right: encoding, language, eol
        let enc = if self.encoding_idx == 0 { "UTF-8" } else { "UTF-16" };
        self.status.right(StatusItem::clickable(enc)
            .with_tooltip("Click to change encoding"));
        self.status.right(StatusItem::text("Rust")
            .with_color([0.85, 0.55, 0.25, 1.0]));
        self.status.right(StatusItem::text("LF"));
        self.status.right(StatusItem::text("4 spaces"));

        // Warning indicator
        if self.log.len() > 5 {
            self.status.right(StatusItem::indicator(
                format!("{} msgs", self.log.len()),
                Indicator::Warning,
            ).with_tooltip("Event log is growing"));
        }
    }

    fn render_content(&mut self, ui: &Ui) {
        // Left: config panel
        let avail = ui.content_region_avail();
        let panel_w = 280.0;

        ui.child_window("##left_panel")
            .size([panel_w, avail[1]])
            .build(ui, || {
                ui.text("Toolbar Configuration");
                ui.separator();

                let items = self.toolbar.items_mut();
                for (i, item) in items.iter_mut().enumerate() {
                    match &item.kind {
                        dear_imgui_custom_mod::toolbar::ToolbarItemKind::Button => {
                            ui.text(format!("[{}] Button: {}", i, item.label));
                            ui.same_line();
                            let mut en = item.enabled;
                            if ui.checkbox(format!("##en{}", i), &mut en) {
                                item.enabled = en;
                            }
                        }
                        dear_imgui_custom_mod::toolbar::ToolbarItemKind::Toggle { on } => {
                            ui.text(format!("[{}] Toggle: {} = {}", i, item.label, on));
                        }
                        dear_imgui_custom_mod::toolbar::ToolbarItemKind::Dropdown { selected, .. } => {
                            ui.text(format!("[{}] Dropdown: {} = #{}", i, item.label, selected));
                        }
                        dear_imgui_custom_mod::toolbar::ToolbarItemKind::Separator => {
                            ui.text_disabled(format!("[{}] ──separator──", i));
                        }
                        dear_imgui_custom_mod::toolbar::ToolbarItemKind::Spacer => {
                            ui.text_disabled(format!("[{}] ←spacer→", i));
                        }
                    }
                }

                ui.spacing();
                ui.separator();
                ui.text("Toolbar Style");

                ui.set_next_item_width(-1.0);
                ui.slider("Height##tb", 20.0, 50.0, &mut self.toolbar.config.height);
                ui.set_next_item_width(-1.0);
                ui.slider("Btn Rounding", 0.0, 10.0, &mut self.toolbar.config.button_rounding);
                ui.set_next_item_width(-1.0);
                ui.slider("Spacing##tb", 0.0, 10.0, &mut self.toolbar.config.item_spacing);

                ui.spacing();
                ui.separator();
                ui.text("Status Bar Style");
                ui.set_next_item_width(-1.0);
                ui.slider("Height##sb", 16.0, 36.0, &mut self.status.config.height);
                ui.checkbox("Separators", &mut self.status.config.show_separators);

                ui.spacing();
                ui.separator();
                ui.text("Simulate");
                if ui.button("Move Cursor") {
                    self.line += 1;
                    self.col = (self.col + 3) % 80;
                }
                ui.same_line();
                if ui.button("Toggle Connection") {
                    self.connected = !self.connected;
                }
            });

        ui.same_line();

        // Right: event log
        ui.child_window("##right_panel")
            .size([avail[0] - panel_w - 8.0, avail[1]])
            .build(ui, || {
                ui.text("Event Log");
                ui.separator();

                for (i, msg) in self.log.iter().enumerate().rev().take(30) {
                    ui.text_disabled(format!("[{}]", i));
                    ui.same_line();
                    ui.text(msg);
                }

                if self.log.len() > 30 {
                    ui.text_disabled(format!("... and {} more", self.log.len() - 30));
                }
            });
    }
}

// ─── wgpu + winit boilerplate ────────────────────────────────────────────────

struct GpuState {
    device: wgpu::Device, queue: wgpu::Queue, window: Arc<Window>,
    surface_cfg: wgpu::SurfaceConfiguration, surface: wgpu::Surface<'static>,
    context: dear_imgui_rs::Context, platform: WinitPlatform,
    renderer: WgpuRenderer, demo: DemoState,
}
struct App { gpu: Option<GpuState> }
impl App { fn new() -> Self { Self { gpu: None } } }

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() { return; }
        let window = Arc::new(event_loop.create_window(
            Window::default_attributes()
                .with_inner_size(LogicalSize::new(900.0, 550.0))
                .with_title("Toolbar + StatusBar Demo"),
        ).expect("window"));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface), force_fallback_adapter: false,
        })).expect("adapter");
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
        let phys = window.inner_size();
        let surface_cfg = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: phys.width.max(1), height: phys.height.max(1),
            present_mode: wgpu::PresentMode::Fifo, desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto, view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };
        surface.configure(&device, &surface_cfg);
        let mut context = dear_imgui_rs::Context::create();
        let _ = context.set_ini_filename(None::<std::path::PathBuf>);
        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, HiDpiMode::Default, &mut context);
        let hidpi = window.scale_factor() as f32;
        let font_size = 15.0 * hidpi;
        context.io_mut().set_font_global_scale(1.0 / hidpi);
        use dear_imgui_custom_mod::code_editor::BuiltinFont;
        context.fonts().add_font_from_memory_ttf(BuiltinFont::Hack.data(), font_size,
            Some(&dear_imgui_rs::FontConfig::new().size_pixels(font_size).oversample_h(2).name("Hack")), None);
        apply_dark_theme(context.style_mut());
        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_cfg.format), &mut context,
        ).expect("renderer");
        self.gpu = Some(GpuState { device, queue, window, surface_cfg, surface, context, platform, renderer, demo: DemoState::new() });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: winit::window::WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        gpu.platform.handle_event::<()>(&mut gpu.context, &gpu.window, &Event::WindowEvent { window_id, event: event.clone() });
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(s) => { gpu.surface_cfg.width = s.width.max(1); gpu.surface_cfg.height = s.height.max(1); gpu.surface.configure(&gpu.device, &gpu.surface_cfg); gpu.window.request_redraw(); }
            WindowEvent::RedrawRequested => {
                let frame = match gpu.surface.get_current_texture() { wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f, wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => { gpu.surface.configure(&gpu.device, &gpu.surface_cfg); return; } other => { eprintln!("{other:?}"); return; } };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);
                let ui = gpu.context.frame(); gpu.demo.render(ui);
                gpu.platform.prepare_render_with_ui(ui, &gpu.window);
                let draw_data = gpu.context.render();
                let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("imgui") });
                { let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("p"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, depth_slice: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.06, g: 0.06, b: 0.08, a: 1.0 }), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None });
                if draw_data.total_vtx_count > 0 { gpu.renderer.render_draw_data(draw_data, &mut pass).expect("render"); } }
                gpu.queue.submit(Some(enc.finish())); frame.present(); gpu.window.request_redraw();
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) { if let Some(gpu) = self.gpu.as_ref() { gpu.window.request_redraw(); } }
}

fn apply_dark_theme(style: &mut dear_imgui_rs::Style) {
    style.set_window_rounding(6.0); style.set_frame_rounding(4.0); style.set_grab_rounding(4.0);
    style.set_scrollbar_rounding(6.0); style.set_window_border_size(1.0); style.set_popup_rounding(4.0);
    let a = [0.40, 0.63, 0.88, 1.0]; let ad = [0.30, 0.50, 0.75, 1.0];
    style.set_color(StyleColor::WindowBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::ChildBg, [0.10, 0.10, 0.13, 1.0]);
    style.set_color(StyleColor::Border, [0.20, 0.22, 0.27, 0.70]);
    style.set_color(StyleColor::FrameBg, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::FrameBgHovered, [0.19, 0.20, 0.26, 1.0]);
    style.set_color(StyleColor::FrameBgActive, [0.24, 0.26, 0.33, 1.0]);
    style.set_color(StyleColor::TitleBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::TitleBgActive, [0.12, 0.13, 0.17, 1.0]);
    style.set_color(StyleColor::ScrollbarBg, [0.08, 0.08, 0.10, 0.60]);
    style.set_color(StyleColor::ScrollbarGrab, [0.22, 0.24, 0.30, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabHovered, [0.30, 0.33, 0.40, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabActive, ad);
    style.set_color(StyleColor::CheckMark, a); style.set_color(StyleColor::SliderGrab, ad);
    style.set_color(StyleColor::SliderGrabActive, a);
    style.set_color(StyleColor::Button, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::ButtonHovered, [0.26, 0.29, 0.36, 1.0]);
    style.set_color(StyleColor::ButtonActive, ad);
    style.set_color(StyleColor::Header, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::HeaderHovered, [0.24, 0.27, 0.34, 1.0]);
    style.set_color(StyleColor::HeaderActive, ad);
    style.set_color(StyleColor::Separator, [0.20, 0.22, 0.27, 0.60]);
    style.set_color(StyleColor::Text, [0.92, 0.93, 0.95, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.42, 0.45, 0.52, 1.0]);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new()).expect("run");
}
