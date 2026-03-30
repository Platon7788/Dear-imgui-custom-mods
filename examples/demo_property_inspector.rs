//! Demo: PropertyInspector — hierarchical property editor showcase.
//!
//! Demonstrates typed properties, categories, search/filter,
//! nested objects, read-only fields, diff highlighting.
//!
//! Run: cargo run --example demo_property_inspector

use dear_imgui_custom_mod::property_inspector::{
    PropertyInspector, PropertyNode, PropertyValue,
};
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
    inspector: PropertyInspector,
    show_config: bool,
    frame_count: u64,
}

impl DemoState {
    fn new() -> Self {
        let mut inspector = PropertyInspector::new("##props_demo");

        // Transform category
        inspector.add_category("Transform");
        inspector.add("position", PropertyValue::Vec3([120.0, 45.0, 0.0]));
        inspector.add("rotation", PropertyValue::F32(15.0));
        inspector.add("scale", PropertyValue::Vec2([1.0, 1.0]));

        // Material category
        inspector.add_category("Material");
        inspector.add("color", PropertyValue::Color4([1.0, 0.42, 0.21, 1.0]));
        inspector.add("opacity", PropertyValue::F32(0.80));
        inspector.add("shader", PropertyValue::Enum(
            0, vec!["PBR Standard".into(), "Unlit".into(), "Toon".into()],
        ));
        inspector.add("double_sided", PropertyValue::Bool(true));
        inspector.add("emission", PropertyValue::Color3([0.0, 0.0, 0.0]));

        // Physics category
        inspector.add_category("Physics");
        inspector.add("mass", PropertyValue::F32(1.5));
        inspector.add("velocity", PropertyValue::Vec3([0.0, -9.81, 0.0]));
        inspector.add("is_static", PropertyValue::Bool(false));
        inspector.add("friction", PropertyValue::F32(0.6));
        inspector.add("restitution", PropertyValue::F32(0.3));
        inspector.add("collision_layer", PropertyValue::Flags(
            0x03, vec!["Default".into(), "Player".into(), "Trigger".into(), "Static".into()],
        ));

        // Debug category
        inspector.add_category("Debug");
        inspector.add_node(
            PropertyNode::new("frame_time", PropertyValue::String("16.2ms".into()))
                .with_readonly(true),
        );
        inspector.add_node(
            PropertyNode::new("draw_calls", PropertyValue::I32(142))
                .with_readonly(true)
                .with_changed(true),
        );
        inspector.add_node(
            PropertyNode::new("triangles", PropertyValue::I64(1_250_000))
                .with_readonly(true),
        );
        inspector.add_node(
            PropertyNode::new("fps", PropertyValue::F64(61.5))
                .with_readonly(true),
        );

        // Metadata category
        inspector.add_category("Metadata");
        inspector.add("name", PropertyValue::String("Player Character".into()));
        inspector.add("tag", PropertyValue::String("hero".into()));
        inspector.add("layer", PropertyValue::Enum(
            1, vec!["Default".into(), "Player".into(), "Enemy".into(), "UI".into()],
        ));
        inspector.add("active", PropertyValue::Bool(true));
        inspector.add("id", PropertyValue::I64(42));

        Self {
            inspector,
            show_config: true,
            frame_count: 0,
        }
    }

    fn render(&mut self, ui: &Ui) {
        self.frame_count += 1;

        ui.window("PropertyInspector Demo")
            .size([700.0, 650.0], Condition::FirstUseEver)
            .build(|| {
                // Toolbar
                ui.text("PropertyInspector — typed key-value editor");
                ui.same_line_with_pos(ui.content_region_avail()[0] - 80.0);
                ui.checkbox("Config", &mut self.show_config);
                ui.separator();

                let avail = ui.content_region_avail();
                let config_w = if self.show_config { 200.0 } else { 0.0 };
                let insp_w = avail[0] - config_w - if self.show_config { 8.0 } else { 0.0 };

                ui.child_window("##insp_col")
                    .size([insp_w, avail[1]])
                    .build(ui, || {
                        let _events = self.inspector.render(ui);
                    });

                if self.show_config {
                    ui.same_line();
                    ui.child_window("##cfg_col")
                        .size([config_w, avail[1]])
                        .build(ui, || {
                            self.render_config(ui);
                        });
                }
            });
    }

    fn render_config(&mut self, ui: &Ui) {
        ui.text("Configuration");
        ui.separator();

        let mut ratio = self.inspector.config.key_width_ratio;
        ui.set_next_item_width(-1.0);
        if ui.slider("Key Width", 0.2, 0.7, &mut ratio) {
            self.inspector.config.key_width_ratio = ratio;
        }

        let mut row_h = self.inspector.config.row_height;
        ui.set_next_item_width(-1.0);
        if ui.slider("Row Height", 16.0, 36.0, &mut row_h) {
            self.inspector.config.row_height = row_h;
        }

        let mut indent = self.inspector.config.indent;
        ui.set_next_item_width(-1.0);
        if ui.slider("Indent", 8.0, 32.0, &mut indent) {
            self.inspector.config.indent = indent;
        }

        ui.spacing();
        ui.separator();
        ui.text("Display");

        ui.checkbox("Show Filter", &mut self.inspector.config.show_filter);
        ui.checkbox("Show Categories", &mut self.inspector.config.show_categories);
        ui.checkbox("Highlight Changes", &mut self.inspector.config.highlight_changes);

        ui.spacing();
        ui.separator();
        ui.text("Stats");
        ui.text_disabled(format!("Properties: {}", self.inspector.property_count()));
        ui.text_disabled(format!("Frame: {}", self.frame_count));

        ui.spacing();
        ui.separator();
        ui.text_disabled("Type filter text to search");
        ui.text_disabled("15+ value types supported");
        ui.text_disabled("Categories collapsible");
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
                .with_inner_size(LogicalSize::new(700.0, 650.0))
                .with_title("PropertyInspector Demo"),
        ).expect("window"));
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY, ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface), force_fallback_adapter: false,
        })).expect("adapter");
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
        let phys = window.inner_size();
        let surface_cfg = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: phys.width.max(1), height: phys.height.max(1),
            present_mode: wgpu::PresentMode::Fifo, desired_maximum_frame_latency: 2,
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
                let frame = match gpu.surface.get_current_texture() { Ok(f) => f, Err(wgpu::SurfaceError::Outdated) => { gpu.surface.configure(&gpu.device, &gpu.surface_cfg); return; } Err(e) => { eprintln!("{e:?}"); return; } };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);
                let ui = gpu.context.frame(); gpu.demo.render(ui);
                gpu.platform.prepare_render_with_ui(ui, &gpu.window);
                let draw_data = gpu.context.render();
                let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("imgui") });
                { let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("p"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, depth_slice: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.06, g: 0.06, b: 0.08, a: 1.0 }), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None });
                if draw_data.total_vtx_count > 0 { gpu.renderer.render_draw_data(draw_data, &mut pass).expect("render"); } }
                gpu.queue.submit(Some(encoder.finish())); frame.present(); gpu.window.request_redraw();
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
