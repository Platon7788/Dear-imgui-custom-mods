//! Demo: DiffViewer — side-by-side / unified diff showcase.
//!
//! Demonstrates Myers diff algorithm, side-by-side & unified modes,
//! fold unchanged, hunk navigation, and configuration options.
//!
//! Run: cargo run --example demo_diff_viewer

use dear_imgui_custom_mod::diff_viewer::{DiffMode, DiffViewer};
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

// ─── Sample data ─────────────────────────────────────────────────────────────

const OLD_TEXT: &str = r#"use std::collections::HashMap;

fn main() {
    let mut map = HashMap::new();
    map.insert("name", "Alice");
    map.insert("age", "30");

    println!("Hello, {}!", map["name"]);

    for (key, value) in &map {
        println!("  {}: {}", key, value);
    }

    // Calculate something
    let x = 10;
    let y = 20;
    let result = x + y;
    println!("Result: {}", result);

    // Old logging
    eprintln!("DEBUG: map has {} entries", map.len());
}
"#;

const NEW_TEXT: &str = r#"use std::collections::HashMap;
use std::fmt;

fn main() {
    let mut map = HashMap::new();
    map.insert("name", "Bob");
    map.insert("age", "25");
    map.insert("role", "developer");

    println!("Hello, {}!", map["name"]);

    for (key, value) in &map {
        println!("  {}: {}", key, value);
    }

    // Calculate something
    let x = 10;
    let y = 20;
    let z = 5;
    let result = x + y + z;
    println!("Result: {}", result);

    // New structured logging
    log_info(&format!("map has {} entries", map.len()));
}

fn log_info(msg: &str) {
    println!("[INFO] {}", msg);
}
"#;

// ─── Demo state ──────────────────────────────────────────────────────────────

struct DemoState {
    viewer: DiffViewer,
    show_config: bool,
    mode_idx: usize,
    sample_idx: usize,
}

impl DemoState {
    fn new() -> Self {
        let mut viewer = DiffViewer::new("##diff_demo");
        viewer.old_label = "old.rs".into();
        viewer.new_label = "new.rs".into();
        viewer.set_texts(OLD_TEXT, NEW_TEXT);

        Self {
            viewer,
            show_config: true,
            mode_idx: 0,
            sample_idx: 0,
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("DiffViewer Demo")
            .size([1100.0, 650.0], Condition::FirstUseEver)
            .build(|| {
                self.render_toolbar(ui);
                ui.separator();

                let avail = ui.content_region_avail();
                let config_w = if self.show_config { 220.0 } else { 0.0 };
                let viewer_w = avail[0] - config_w - if self.show_config { 8.0 } else { 0.0 };

                ui.child_window("##viewer_col")
                    .size([viewer_w, avail[1]])
                    .build(ui, || {
                        let _events = self.viewer.render(ui);
                    });

                if self.show_config {
                    ui.same_line();
                    ui.child_window("##config_col")
                        .size([config_w, avail[1]])
                        .build(ui, || {
                            self.render_config(ui);
                        });
                }
            });
    }

    fn render_toolbar(&mut self, ui: &Ui) {
        // Sample selector
        let samples = ["Rust Code", "Short", "Identical", "All New"];
        ui.set_next_item_width(150.0);
        if ui.combo_simple_string("Sample##s", &mut self.sample_idx, &samples) {
            match self.sample_idx {
                0 => {
                    self.viewer.old_label = "old.rs".into();
                    self.viewer.new_label = "new.rs".into();
                    self.viewer.set_texts(OLD_TEXT, NEW_TEXT);
                }
                1 => {
                    self.viewer.old_label = "before.txt".into();
                    self.viewer.new_label = "after.txt".into();
                    self.viewer.set_texts("line 1\nline 2\nline 3", "line 1\nchanged\nline 3\nline 4");
                }
                2 => {
                    self.viewer.old_label = "same.txt".into();
                    self.viewer.new_label = "same.txt".into();
                    self.viewer.set_texts("identical\ncontent\nhere", "identical\ncontent\nhere");
                }
                3 => {
                    self.viewer.old_label = "empty.txt".into();
                    self.viewer.new_label = "new_file.txt".into();
                    self.viewer.set_texts("", "brand new\nfile content\nthree lines");
                }
                _ => {}
            }
        }

        ui.same_line();
        ui.text(format!("Hunks: {}", self.viewer.hunk_count()));

        ui.same_line_with_pos(ui.content_region_avail()[0] - 80.0);
        ui.checkbox("Config", &mut self.show_config);
    }

    fn render_config(&mut self, ui: &Ui) {
        ui.text("Configuration");
        ui.separator();

        // Mode
        let modes = ["Side-by-Side", "Unified"];
        ui.set_next_item_width(-1.0);
        if ui.combo_simple_string("Mode##dm", &mut self.mode_idx, &modes) {
            self.viewer.config.mode = match self.mode_idx {
                0 => DiffMode::SideBySide,
                _ => DiffMode::Unified,
            };
        }

        ui.spacing();
        ui.separator();
        ui.text("Display");

        ui.checkbox("Line Numbers", &mut self.viewer.config.show_line_numbers);
        ui.checkbox("Fold Unchanged", &mut self.viewer.config.fold_unchanged);
        ui.checkbox("Sync Scroll", &mut self.viewer.config.sync_scroll);
        ui.checkbox("Mini-map", &mut self.viewer.config.show_minimap);

        ui.spacing();
        ui.set_next_item_width(-1.0);
        let mut ctx = self.viewer.config.context_lines as i32;
        if ui.slider("Context", 0, 10, &mut ctx) {
            self.viewer.config.context_lines = ctx.max(0) as usize;
        }

        ui.spacing();
        ui.separator();
        ui.text("Navigation");
        if ui.button("Prev Hunk (Shift+F7)") {
            self.viewer.prev_hunk();
        }
        ui.same_line();
        if ui.button("Next Hunk (F7)") {
            self.viewer.next_hunk();
        }

        ui.spacing();
        ui.separator();
        ui.text_disabled("Side-by-side: two panels");
        ui.text_disabled("Unified: single panel +/-");
        ui.text_disabled("Green = added, Red = removed");
    }
}

// ─── wgpu + winit boilerplate ────────────────────────────────────────────────

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

struct App { gpu: Option<GpuState> }
impl App { fn new() -> Self { Self { gpu: None } } }

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() { return; }
        let window = Arc::new(event_loop.create_window(
            Window::default_attributes()
                .with_inner_size(LogicalSize::new(1100.0, 650.0))
                .with_title("DiffViewer Demo"),
        ).expect("window"));

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY, ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface), force_fallback_adapter: false,
        })).expect("adapter");
        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor::default(),
        )).expect("device");

        let phys = window.inner_size();
        let surface_cfg = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: phys.width.max(1), height: phys.height.max(1),
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

        use dear_imgui_custom_mod::code_editor::BuiltinFont;
        context.fonts().add_font_from_memory_ttf(
            BuiltinFont::Hack.data(), font_size,
            Some(&dear_imgui_rs::FontConfig::new().size_pixels(font_size).oversample_h(2).name("Hack")),
            None,
        );

        apply_dark_theme(context.style_mut());
        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_cfg.format),
            &mut context,
        ).expect("renderer");

        self.gpu = Some(GpuState {
            device, queue, window, surface_cfg, surface,
            context, platform, renderer, demo: DemoState::new(),
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: winit::window::WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        gpu.platform.handle_event::<()>(&mut gpu.context, &gpu.window,
            &Event::WindowEvent { window_id, event: event.clone() });
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(s) => {
                gpu.surface_cfg.width = s.width.max(1);
                gpu.surface_cfg.height = s.height.max(1);
                gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = match gpu.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Outdated) => { gpu.surface.configure(&gpu.device, &gpu.surface_cfg); return; }
                    Err(e) => { eprintln!("Surface error: {e:?}"); return; }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);
                let ui = gpu.context.frame();
                gpu.demo.render(ui);
                gpu.platform.prepare_render_with_ui(ui, &gpu.window);
                let draw_data = gpu.context.render();
                let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("imgui") });
                { let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("imgui_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view, resolve_target: None, depth_slice: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.06, g: 0.06, b: 0.08, a: 1.0 }), store: wgpu::StoreOp::Store },
                    })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
                });
                if draw_data.total_vtx_count > 0 { gpu.renderer.render_draw_data(draw_data, &mut pass).expect("render"); } }
                gpu.queue.submit(Some(encoder.finish()));
                frame.present();
                gpu.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(gpu) = self.gpu.as_ref() { gpu.window.request_redraw(); }
    }
}

fn apply_dark_theme(style: &mut dear_imgui_rs::Style) {
    style.set_window_rounding(6.0); style.set_frame_rounding(4.0);
    style.set_grab_rounding(4.0); style.set_scrollbar_rounding(6.0);
    style.set_window_border_size(1.0); style.set_popup_rounding(4.0);
    let accent = [0.40, 0.63, 0.88, 1.0];
    let accent_dim = [0.30, 0.50, 0.75, 1.0];
    style.set_color(StyleColor::WindowBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::ChildBg, [0.10, 0.10, 0.13, 1.0]);
    style.set_color(StyleColor::PopupBg, [0.11, 0.12, 0.15, 0.96]);
    style.set_color(StyleColor::Border, [0.20, 0.22, 0.27, 0.70]);
    style.set_color(StyleColor::FrameBg, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::FrameBgHovered, [0.19, 0.20, 0.26, 1.0]);
    style.set_color(StyleColor::FrameBgActive, [0.24, 0.26, 0.33, 1.0]);
    style.set_color(StyleColor::TitleBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::TitleBgActive, [0.12, 0.13, 0.17, 1.0]);
    style.set_color(StyleColor::ScrollbarBg, [0.08, 0.08, 0.10, 0.60]);
    style.set_color(StyleColor::ScrollbarGrab, [0.22, 0.24, 0.30, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabHovered, [0.30, 0.33, 0.40, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabActive, accent_dim);
    style.set_color(StyleColor::CheckMark, accent);
    style.set_color(StyleColor::SliderGrab, accent_dim);
    style.set_color(StyleColor::SliderGrabActive, accent);
    style.set_color(StyleColor::Button, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::ButtonHovered, [0.26, 0.29, 0.36, 1.0]);
    style.set_color(StyleColor::ButtonActive, accent_dim);
    style.set_color(StyleColor::Header, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::HeaderHovered, [0.24, 0.27, 0.34, 1.0]);
    style.set_color(StyleColor::HeaderActive, accent_dim);
    style.set_color(StyleColor::Separator, [0.20, 0.22, 0.27, 0.60]);
    style.set_color(StyleColor::Text, [0.92, 0.93, 0.95, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.42, 0.45, 0.52, 1.0]);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new()).expect("run");
}
