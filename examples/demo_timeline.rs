//! Demo: Timeline — profiler timeline / flame graph showcase.
//!
//! Demonstrates multi-track spans, nested depth, markers,
//! pan/zoom, color modes, tooltips, and all configuration options.
//!
//! Run: cargo run --example demo_timeline

use dear_imgui_custom_mod::timeline::{
    ColorMode, Marker, Span, Timeline, TimelineMode, Track,
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

fn build_sample_data() -> (Vec<Track>, Vec<Marker>) {
    let mut tracks = Vec::new();

    // ── Main Thread ─────────────────────────────────────────────
    let mut main_track = Track::new("Main Thread");
    let mut id = 0u64;

    // Frame 1: 0..50ms
    main_track.add_span(Span::new(id, 0.0, 0.050, 0, "frame()")); id += 1;
    main_track.add_span(Span::new(id, 0.0, 0.018, 1, "update()")); id += 1;
    main_track.add_span(Span::new(id, 0.0, 0.008, 2, "physics()")); id += 1;
    main_track.add_span(Span::new(id, 0.008, 0.015, 2, "ai_tick()")); id += 1;
    main_track.add_span(Span::new(id, 0.015, 0.018, 2, "animation()")); id += 1;
    main_track.add_span(Span::new(id, 0.018, 0.048, 1, "render()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.018, 0.030, 2, "draw_nodes()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.018, 0.024, 3, "batch()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.024, 0.030, 3, "submit()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.030, 0.042, 2, "draw_ui()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.042, 0.048, 2, "swap()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.048, 0.050, 1, "present()")); id += 1;

    // Frame 2: 50..100ms
    main_track.add_span(Span::new(id, 0.050, 0.100, 0, "frame()")); id += 1;
    main_track.add_span(Span::new(id, 0.050, 0.068, 1, "update()")); id += 1;
    main_track.add_span(Span::new(id, 0.050, 0.060, 2, "physics()")); id += 1;
    main_track.add_span(Span::new(id, 0.060, 0.068, 2, "ai_tick()")); id += 1;
    main_track.add_span(Span::new(id, 0.068, 0.096, 1, "render()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.068, 0.082, 2, "draw_nodes()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.082, 0.092, 2, "draw_ui()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.092, 0.096, 2, "swap()").with_category("render")); id += 1;
    main_track.add_span(Span::new(id, 0.096, 0.100, 1, "present()")); id += 1;

    // Frame 3: 100..160ms (slow frame — GC spike)
    main_track.add_span(Span::new(id, 0.100, 0.160, 0, "frame()")); id += 1;
    main_track.add_span(Span::new(id, 0.100, 0.115, 1, "update()")); id += 1;
    main_track.add_span(Span::new(id, 0.100, 0.108, 2, "physics()")); id += 1;
    main_track.add_span(Span::new(id, 0.108, 0.115, 2, "ai_tick()")); id += 1;
    main_track.add_span(Span::new(id, 0.115, 0.145, 1, "render()").with_category("render")); id += 1;
    main_track.add_span(
        Span::new(id, 0.120, 0.140, 2, "GC_PAUSE")
            .with_color([1.0, 0.2, 0.2, 0.95])
            .with_source("runtime/gc.rs:304"),
    ); id += 1;
    main_track.add_span(Span::new(id, 0.145, 0.158, 1, "present()")); id += 1;

    tracks.push(main_track);

    // ── Render Thread ───────────────────────────────────────────
    let mut render_track = Track::new("Render Thread");
    render_track.add_span(Span::new(id, 0.020, 0.045, 0, "gpu_submit")); id += 1;
    render_track.add_span(Span::new(id, 0.020, 0.035, 1, "command_buffer")); id += 1;
    render_track.add_span(Span::new(id, 0.035, 0.045, 1, "present")); id += 1;
    render_track.add_span(Span::new(id, 0.070, 0.094, 0, "gpu_submit")); id += 1;
    render_track.add_span(Span::new(id, 0.070, 0.085, 1, "command_buffer")); id += 1;
    render_track.add_span(Span::new(id, 0.085, 0.094, 1, "present")); id += 1;
    render_track.add_span(Span::new(id, 0.118, 0.155, 0, "gpu_submit")); id += 1;
    render_track.add_span(Span::new(id, 0.118, 0.148, 1, "command_buffer")); id += 1;
    render_track.add_span(Span::new(id, 0.148, 0.155, 1, "present")); id += 1;
    tracks.push(render_track);

    // ── Audio Thread ────────────────────────────────────────────
    let mut audio_track = Track::new("Audio Thread");
    for i in 0..16 {
        let start = i as f64 * 0.010;
        let end = start + 0.003;
        audio_track.add_span(
            Span::new(id, start, end, 0, "mix_buffer").with_category("audio"),
        );
        id += 1;
    }
    tracks.push(audio_track);

    // ── IO Thread ───────────────────────────────────────────────
    let mut io_track = Track::new("IO Thread");
    io_track.add_span(
        Span::new(id, 0.005, 0.035, 0, "load_texture")
            .with_source("assets/loader.rs:89"),
    ); id += 1;
    io_track.add_span(Span::new(id, 0.040, 0.060, 0, "load_mesh")); id += 1;
    io_track.add_span(Span::new(id, 0.080, 0.130, 0, "stream_level")); id += 1;
    io_track.add_span(Span::new(id, 0.080, 0.095, 1, "decompress")); id += 1;
    io_track.add_span(Span::new(id, 0.095, 0.130, 1, "upload"));
    tracks.push(io_track);

    // ── Markers ─────────────────────────────────────────────────
    let markers = vec![
        Marker::new(0.0, "Frame 1"),
        Marker::new(0.050, "Frame 2"),
        Marker::new(0.100, "Frame 3 (slow)")
            .with_color([1.0, 0.4, 0.3, 0.8]),
    ];

    (tracks, markers)
}

// ─── Demo state ──────────────────────────────────────────────────────────────

struct DemoState {
    timeline: Timeline,
    show_config: bool,
    color_mode_idx: usize,
    mode_idx: usize,
    last_event: String,
}

impl DemoState {
    fn new() -> Self {
        let (tracks, markers) = build_sample_data();

        let mut timeline = Timeline::new("##profiler_demo");
        for t in tracks {
            timeline.add_track(t);
        }
        for m in markers {
            timeline.add_marker(m);
        }

        Self {
            timeline,
            show_config: true,
            color_mode_idx: 0,
            mode_idx: 0,
            last_event: String::new(),
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("Timeline Demo")
            .size([1200.0, 600.0], Condition::FirstUseEver)
            .build(|| {
                // ── Toolbar ──────────────────────────────────────────
                self.render_toolbar(ui);
                ui.separator();

                // ── Layout: timeline + config ────────────────────────
                let avail = ui.content_region_avail();
                let config_w = if self.show_config { 220.0 } else { 0.0 };
                let tl_w = avail[0] - config_w
                    - if self.show_config { 8.0 } else { 0.0 };

                ui.child_window("##tl_col")
                    .size([tl_w, avail[1]])
                    .build(ui, || {
                        let events = self.timeline.render(ui);
                        for ev in &events {
                            match ev {
                                dear_imgui_custom_mod::timeline::TimelineEvent::SpanClicked { span_id } => {
                                    self.last_event = format!("Clicked span {}", span_id);
                                }
                                dear_imgui_custom_mod::timeline::TimelineEvent::SpanDoubleClicked { span_id } => {
                                    self.last_event = format!("Double-clicked span {}", span_id);
                                }
                                dear_imgui_custom_mod::timeline::TimelineEvent::MarkerClicked { index } => {
                                    self.last_event = format!("Clicked marker {}", index);
                                }
                                dear_imgui_custom_mod::timeline::TimelineEvent::ViewChanged { .. } => {}
                            }
                        }
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

    fn render_toolbar(&mut self, ui: &Ui) {
        if ui.button("Fit") {
            let w = ui.content_region_avail()[0];
            self.timeline.fit_to_content(w);
        }
        ui.same_line();

        let (lo, hi) = self.timeline.data_time_range();
        ui.text(format!(
            "Data: {:.1}ms .. {:.1}ms  |  {} tracks",
            lo * 1000.0,
            hi * 1000.0,
            self.timeline.tracks().len(),
        ));

        if let Some(sel) = self.timeline.selected_span() {
            ui.same_line();
            ui.text_colored([0.5, 0.8, 1.0, 1.0], format!("  Selected: #{}", sel));
        }

        if !self.last_event.is_empty() {
            ui.same_line();
            ui.text_colored([0.8, 0.7, 0.4, 1.0], format!("  [{}]", self.last_event));
        }

        ui.same_line_with_pos(ui.content_region_avail()[0] - 80.0);
        ui.checkbox("Config", &mut self.show_config);

        ui.text_disabled("Scroll: zoom | Right-drag/Mid-drag: pan | Shift+scroll: vertical scroll");
    }

    fn render_config(&mut self, ui: &Ui) {
        ui.text("Configuration");
        ui.separator();

        // Color mode
        let color_modes = ["By Name", "By Duration", "By Depth", "Explicit"];
        ui.set_next_item_width(-1.0);
        if ui.combo_simple_string("Color##cm", &mut self.color_mode_idx, &color_modes) {
            self.timeline.config.color_mode = match self.color_mode_idx {
                0 => ColorMode::ByName,
                1 => ColorMode::ByDuration,
                2 => ColorMode::ByDepth,
                _ => ColorMode::Explicit,
            };
        }

        // Mode
        let modes = ["Top-Down", "Flame (Bottom-Up)"];
        ui.set_next_item_width(-1.0);
        if ui.combo_simple_string("Mode##md", &mut self.mode_idx, &modes) {
            self.timeline.config.mode = match self.mode_idx {
                0 => TimelineMode::TopDown,
                _ => TimelineMode::BottomUp,
            };
        }

        ui.spacing();
        ui.separator();
        ui.text("Display");

        ui.checkbox("Show Ruler", &mut self.timeline.config.show_ruler);
        ui.checkbox("Show Labels", &mut self.timeline.config.show_track_labels);
        ui.checkbox("Show Tooltip", &mut self.timeline.config.show_tooltip);
        ui.checkbox("Show Markers", &mut self.timeline.config.show_markers);
        ui.checkbox("Smooth Zoom", &mut self.timeline.config.smooth_zoom);

        ui.spacing();
        ui.separator();
        ui.text("Layout");

        ui.set_next_item_width(-1.0);
        ui.slider("Row Height", 10.0, 40.0, &mut self.timeline.config.row_height);
        ui.set_next_item_width(-1.0);
        ui.slider("Row Gap", 0.0, 4.0, &mut self.timeline.config.row_gap);
        ui.set_next_item_width(-1.0);
        ui.slider("Ruler H", 16.0, 40.0, &mut self.timeline.config.ruler_height);
        ui.set_next_item_width(-1.0);
        ui.slider("Label W", 0.0, 200.0, &mut self.timeline.config.track_label_width);
        ui.set_next_item_width(-1.0);
        ui.slider("Min Span W", 1.0, 10.0, &mut self.timeline.config.min_span_width);

        ui.spacing();
        ui.separator();
        ui.text("Tracks");

        for (i, track) in self.timeline.tracks().iter().enumerate() {
            let label = format!("{}: {} spans, depth {}",
                track.name, track.spans.len(), track.max_depth());
            ui.text_disabled(&label);
            let _ = i;
        }

        ui.spacing();
        ui.separator();
        ui.text_disabled("Pan: right/middle drag");
        ui.text_disabled("Zoom: scroll wheel");
        ui.text_disabled("Click span: select");
        ui.text_disabled("Double-click: detail");
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
                        .with_inner_size(LogicalSize::new(1200.0, 600.0))
                        .with_title("Timeline Demo"),
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

        let mut demo = DemoState::new();
        // Fit after creation so the timeline shows all data
        demo.timeline.fit_to_content(1200.0);

        self.gpu = Some(GpuState {
            device,
            queue,
            window,
            surface_cfg,
            surface,
            context,
            platform,
            renderer,
            demo,
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
                    wgpu::CurrentSurfaceTexture::Success(f)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    wgpu::CurrentSurfaceTexture::Outdated
                    | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                        return;
                    }
                    other => {
                        eprintln!("Surface unavailable: {other:?}");
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
