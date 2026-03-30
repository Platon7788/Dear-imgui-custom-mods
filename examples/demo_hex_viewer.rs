//! Demo: HexViewer — binary hex dump viewer showcase.
//!
//! Demonstrates color regions, data inspector, search, goto,
//! diff highlighting, editing mode, and all configuration options.
//!
//! Run: cargo run --example demo_hex_viewer

use dear_imgui_custom_mod::hex_viewer::{
    ByteGrouping, BytesPerRow, ColorRegion, Endianness, HexViewer,
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

// ─── Sample data ─────────────────────────────────────────────────────────────

/// Fake PE header for demonstration.
fn sample_pe_header() -> Vec<u8> {
    let mut data = Vec::with_capacity(512);

    // DOS header
    data.extend_from_slice(b"MZ");                              // 0x00: Magic
    data.extend_from_slice(&[0x90, 0x00]);                      // 0x02: Bytes on last page
    data.extend_from_slice(&[0x03, 0x00, 0x00, 0x00]);          // 0x04: Pages
    data.extend_from_slice(&[0x04, 0x00, 0x00, 0x00]);          // 0x08: Relocations
    data.extend_from_slice(&[0x00, 0x00, 0xFF, 0xFF]);          // 0x0C: Header paragraphs
    data.extend_from_slice(&[0x00, 0x00, 0xB8, 0x00]);          // 0x10
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);          // 0x14
    data.extend_from_slice(&[0x40, 0x00, 0x00, 0x00]);          // 0x18
    // Pad to 0x3C
    data.resize(0x3C, 0x00);
    data.extend_from_slice(&[0x80, 0x00, 0x00, 0x00]);          // 0x3C: PE offset

    // Pad to PE header at 0x80
    data.resize(0x80, 0x00);

    // PE signature
    data.extend_from_slice(b"PE\0\0");                           // 0x80: Signature
    data.extend_from_slice(&[0x4C, 0x01]);                       // 0x84: Machine (i386)
    data.extend_from_slice(&[0x06, 0x00]);                       // 0x86: Sections
    data.extend_from_slice(&[0xA2, 0xB3, 0xC4, 0xD5]);          // 0x88: Timestamp
    data.extend_from_slice(&[0x00; 8]);                          // 0x8C: Symbol table + count
    data.extend_from_slice(&[0xE0, 0x00]);                       // 0x94: Optional header size
    data.extend_from_slice(&[0x02, 0x01]);                       // 0x96: Characteristics

    // Fill rest with varied data for interesting hex view
    while data.len() < 512 {
        let i = data.len();
        data.push(((i * 7 + 13) % 256) as u8);
    }

    // Add some ASCII strings for the ASCII column
    let msg = b"Hello from HexViewer demo! This is sample binary data.";
    if data.len() >= 0x100 + msg.len() {
        data[0x100..0x100 + msg.len()].copy_from_slice(msg);
    }

    data
}

/// Color regions for PE header struct overlay.
fn pe_color_regions() -> Vec<ColorRegion> {
    vec![
        ColorRegion::new(0x00, 2,  [0.4, 0.8, 1.0, 1.0], "DOS Magic (MZ)"),
        ColorRegion::new(0x3C, 4,  [1.0, 0.8, 0.3, 1.0], "PE Offset"),
        ColorRegion::new(0x80, 4,  [0.3, 1.0, 0.5, 1.0], "PE Signature"),
        ColorRegion::new(0x84, 2,  [1.0, 0.5, 0.5, 1.0], "Machine"),
        ColorRegion::new(0x86, 2,  [0.8, 0.5, 1.0, 1.0], "Sections"),
        ColorRegion::new(0x88, 4,  [0.5, 0.8, 0.5, 1.0], "Timestamp"),
        ColorRegion::new(0x94, 2,  [1.0, 1.0, 0.5, 1.0], "Opt Header Size"),
        ColorRegion::new(0x96, 2,  [0.5, 1.0, 1.0, 1.0], "Characteristics"),
        ColorRegion::new(0x100, 54, [0.9, 0.7, 0.4, 1.0], "ASCII String"),
    ]
}

// ─── Demo state ──────────────────────────────────────────────────────────────

struct DemoState {
    viewer: HexViewer,
    show_config: bool,
    bpr_idx: usize,
    group_idx: usize,
    show_regions: bool,
    diff_mode: bool,
    /// Snapshot for diff highlighting.
    reference: Vec<u8>,
}

impl DemoState {
    fn new() -> Self {
        let data = sample_pe_header();
        let reference = data.clone();

        let mut viewer = HexViewer::new("##hex_demo");
        viewer.set_data(&data);
        viewer.set_regions(pe_color_regions());

        Self {
            viewer,
            show_config: true,
            bpr_idx: 1, // 16
            group_idx: 2, // DWord
            show_regions: true,
            diff_mode: false,
            reference,
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("HexViewer Demo")
            .size([1100.0, 700.0], Condition::FirstUseEver)
            .build(|| {
                // ── Toolbar ──────────────────────────────────────────
                self.render_toolbar(ui);
                ui.separator();

                // ── Layout: viewer + config ──────────────────────────
                let avail = ui.content_region_avail();
                let config_w = if self.show_config { 240.0 } else { 0.0 };
                let viewer_w = avail[0] - config_w - if self.show_config { 8.0 } else { 0.0 };

                ui.child_window("##viewer_col")
                    .size([viewer_w, avail[1]])
                    .build(ui, || {
                        self.viewer.render(ui);
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
        let cursor = self.viewer.cursor();
        let sel = self.viewer.selection();
        let data_len = self.viewer.data().len();

        ui.text(format!(
            "Offset: 0x{:08X} ({})  |  {} bytes",
            cursor, cursor, data_len,
        ));

        if !sel.is_empty() {
            ui.same_line();
            let (lo, hi) = sel.ordered();
            ui.text_colored(
                [0.5, 0.8, 1.0, 1.0],
                format!("  Sel: 0x{:X}..0x{:X} ({} bytes)", lo, hi, sel.len()),
            );
        }

        ui.same_line_with_pos(ui.content_region_avail()[0] - 120.0);
        ui.checkbox("Config", &mut self.show_config);

        // Action buttons
        if ui.button("Goto (Ctrl+G)") {
            // Trigger goto popup via keyboard sim — or just set cursor directly
            self.viewer.goto(0x80); // jump to PE signature
        }
        ui.same_line();
        if ui.button("Top") {
            self.viewer.goto(0);
        }
        ui.same_line();
        if ui.button("PE Header") {
            self.viewer.goto(0x80);
        }
        ui.same_line();
        if ui.button("ASCII String") {
            self.viewer.goto(0x100);
        }
        ui.same_line();
        if ui.button("Randomize 8 bytes") {
            // Modify some bytes to demo diff highlighting.
            let data = self.viewer.data_mut();
            let offset = 0x90;
            for i in 0..8 {
                if offset + i < data.len() {
                    data[offset + i] = ((offset + i) * 37 % 256) as u8;
                }
            }
        }
    }

    fn render_config(&mut self, ui: &Ui) {
        ui.text("Configuration");
        ui.separator();

        // Bytes per row
        let bpr_names = ["8", "16", "32"];
        ui.set_next_item_width(-1.0);
        if ui.combo_simple_string("Bytes/Row", &mut self.bpr_idx, &bpr_names) {
            self.viewer.config_mut().bytes_per_row = BytesPerRow::ALL[self.bpr_idx];
        }

        // Grouping
        let group_names = ["None", "Word (2)", "DWord (4)", "QWord (8)"];
        ui.set_next_item_width(-1.0);
        if ui.combo_simple_string("Grouping", &mut self.group_idx, &group_names) {
            self.viewer.config_mut().grouping = match self.group_idx {
                0 => ByteGrouping::None,
                1 => ByteGrouping::Word,
                2 => ByteGrouping::DWord,
                3 => ByteGrouping::QWord,
                _ => ByteGrouping::DWord,
            };
        }

        // Endianness
        let mut le = matches!(self.viewer.config().endianness, Endianness::Little);
        if ui.checkbox("Little Endian", &mut le) {
            self.viewer.config_mut().endianness = if le {
                Endianness::Little
            } else {
                Endianness::Big
            };
        }

        ui.spacing();
        ui.separator();
        ui.text("Display");

        ui.checkbox("Show Offsets", &mut self.viewer.config.show_offsets);
        ui.checkbox("Show ASCII", &mut self.viewer.config.show_ascii);
        ui.checkbox("Show Inspector", &mut self.viewer.config.show_inspector);
        ui.checkbox("Column Headers", &mut self.viewer.config.show_column_headers);
        ui.checkbox("Uppercase Hex", &mut self.viewer.config.uppercase);
        ui.checkbox("Dim Zeros", &mut self.viewer.config.dim_zeros);

        ui.spacing();
        ui.separator();
        ui.text("Mode");

        ui.checkbox("Editable", &mut self.viewer.config.editable);

        if ui.checkbox("Show Regions", &mut self.show_regions) {
            if self.show_regions {
                self.viewer.set_regions(pe_color_regions());
            } else {
                self.viewer.clear_regions();
            }
        }

        if ui.checkbox("Diff Highlight", &mut self.diff_mode) {
            if self.diff_mode {
                self.viewer.set_reference(&self.reference);
                self.viewer.config.highlight_changes = true;
            } else {
                self.viewer.clear_reference();
                self.viewer.config.highlight_changes = false;
            }
        }

        ui.spacing();
        ui.separator();
        ui.text("Base Address");
        let mut addr = self.viewer.config.base_address as i64;
        ui.set_next_item_width(-1.0);
        if ui.input_scalar("##base_addr", &mut addr).build() {
            self.viewer.config.base_address = addr.max(0) as u64;
        }

        ui.spacing();
        ui.separator();

        // Region legend
        if self.show_regions {
            ui.text("Regions");
            for region in &pe_color_regions() {
                ui.text_colored(region.color, format!(
                    "0x{:04X}..{:04X} {}",
                    region.offset,
                    region.offset + region.len,
                    region.label,
                ));
            }
        }

        ui.spacing();
        ui.separator();
        ui.text_disabled("Ctrl+G: Goto  Ctrl+F: Search");
        ui.text_disabled("Ctrl+C: Copy  F3: Next match");
        ui.text_disabled("Arrows: Navigate  Shift: Select");
    }
}

// ─── wgpu + winit + imgui boilerplate ────────────────────────────────────────

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
    fn new() -> Self { Self { gpu: None } }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() { return; }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_inner_size(LogicalSize::new(1100.0, 700.0))
                        .with_title("HexViewer Demo"),
                )
                .expect("window"),
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("adapter");
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("device");

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

        // Load a monospace font for the hex viewer.
        use dear_imgui_custom_mod::code_editor::BuiltinFont;
        let cfg = dear_imgui_rs::FontConfig::new()
            .size_pixels(font_size)
            .oversample_h(2)
            .name("Hack");
        context.fonts().add_font_from_memory_ttf(
            BuiltinFont::Hack.data(),
            font_size,
            Some(&cfg),
            None,
        );

        apply_dark_theme(context.style_mut());

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
        let Some(gpu) = self.gpu.as_mut() else { return };

        gpu.platform.handle_event::<()>(
            &mut gpu.context,
            &gpu.window,
            &Event::WindowEvent { window_id, event: event.clone() },
        );

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                gpu.surface_cfg.width = new_size.width.max(1);
                gpu.surface_cfg.height = new_size.height.max(1);
                gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = match gpu.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Outdated) => {
                        gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                        return;
                    }
                    Err(e) => {
                        eprintln!("Surface error: {e:?}");
                        return;
                    }
                };

                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);

                let ui = gpu.context.frame();
                gpu.demo.render(ui);
                gpu.platform.prepare_render_with_ui(ui, &gpu.window);

                let draw_data = gpu.context.render();

                let mut encoder = gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("imgui") },
                );

                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("imgui_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.06, g: 0.06, b: 0.08, a: 1.0,
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
                        gpu.renderer.render_draw_data(draw_data, &mut pass).expect("render");
                    }
                }

                gpu.queue.submit(Some(encoder.finish()));
                frame.present();
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

fn apply_dark_theme(style: &mut dear_imgui_rs::Style) {
    style.set_window_rounding(6.0);
    style.set_frame_rounding(4.0);
    style.set_grab_rounding(4.0);
    style.set_tab_rounding(4.0);
    style.set_scrollbar_rounding(6.0);
    style.set_window_border_size(1.0);
    style.set_frame_border_size(0.0);
    style.set_popup_rounding(4.0);
    style.set_cell_padding([6.0, 2.0]);
    style.set_frame_padding([3.0, 2.0]);
    style.set_item_spacing([8.0, 4.0]);
    style.set_item_inner_spacing([6.0, 3.0]);

    let accent = [0.40, 0.63, 0.88, 1.0];
    let accent_dim = [0.30, 0.50, 0.75, 1.0];
    let accent_hi = [0.50, 0.73, 0.95, 1.0];

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
    style.set_color(StyleColor::Tab, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::TabHovered, accent_dim);
    style.set_color(StyleColor::TabSelected, [0.22, 0.24, 0.30, 1.0]);
    style.set_color(StyleColor::TextSelectedBg, [accent[0], accent[1], accent[2], 0.30]);
    style.set_color(StyleColor::Text, [0.92, 0.93, 0.95, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.42, 0.45, 0.52, 1.0]);
    style.set_color(StyleColor::PlotHistogram, accent_hi);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run");
}
