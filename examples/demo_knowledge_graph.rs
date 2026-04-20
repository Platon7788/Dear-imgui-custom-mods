//! Demo: knowledge_graph — Obsidian/IDA-style force-directed graph viewer.
//!
//! Demonstrates Phase B + C features:
//!   - 50+ nodes with preferential-attachment edges
//!   - Different NodeKind shapes: Regular (circle), Tag (square), Unresolved (diamond)
//!   - Built-in sidebar: Filter / Color Groups / Display / Physics controls
//!   - Color modes: ByTag (Okabe-Ito palette) and Static
//!   - Hover fade — non-neighbors dim when hovering
//!   - Drag, box-select, context menu (right-click)
//!   - Keyboard: arrows=pan, +/-=zoom, F=fit, Esc=clear, Space=toggle sim, P=pin
//!   - Regenerate button rebuilds the graph
//!
//! Run: cargo run --example demo_knowledge_graph --features knowledge_graph,app_window

use dear_imgui_custom_mod::knowledge_graph::{
    GraphViewer,
    config::{ColorGroupQuery, ColorMode, ForceConfig, SidebarKind, ViewerConfig},
    data::GraphData,
    event::GraphEvent,
    style::{EdgeStyle, NodeKind, NodeStyle},
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

// ─── Tags ─────────────────────────────────────────────────────────────────────

const TAGS: &[&str] = &["core", "api", "ui", "data", "infra", "test"];

// ─── LCG RNG ──────────────────────────────────────────────────────────────────

struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn next_f32(&mut self) -> f32 {
        (self.next() & 0x7FFF_FFFF) as f32 / 0x7FFF_FFFF_u32 as f32
    }
    fn next_usize(&mut self, max: usize) -> usize {
        (self.next() as usize) % max.max(1)
    }
}

// ─── Graph construction ───────────────────────────────────────────────────────

fn build_graph(seed: u64) -> GraphData {
    let mut rng = Lcg::new(seed);
    let mut graph = GraphData::with_capacity(60, 90);

    let names = [
        "Alpha",
        "Beta",
        "Gamma",
        "Delta",
        "Epsilon",
        "Zeta",
        "Eta",
        "Theta",
        "Iota",
        "Kappa",
        "Lambda",
        "Mu",
        "Nu",
        "Xi",
        "Omicron",
        "Pi",
        "Rho",
        "Sigma",
        "Tau",
        "Upsilon",
        "Phi",
        "Chi",
        "Psi",
        "Omega",
        "Andromeda",
        "Cassiopeia",
        "Cygnus",
        "Perseus",
        "Orion",
        "Lyra",
        "Aquila",
        "Vega",
        "Sirius",
        "Altair",
        "Deneb",
        "Rigel",
        "Betelgeuse",
        "Aldebaran",
        "Pollux",
        "Castor",
        "Procyon",
        "Regulus",
        "Spica",
        "Arcturus",
        "Antares",
        "Fomalhaut",
        "Acrux",
        "Mimosa",
        "Hadar",
        "Canopus",
    ];

    // 50 regular nodes with tags.
    let mut ids = Vec::with_capacity(60);
    for name in &names {
        let tag = TAGS[rng.next_usize(TAGS.len())];
        ids.push(graph.add_node(NodeStyle::new(*name).with_tag(tag)));
    }

    // 5 tag nodes (square shape).
    for tag in &["#core", "#api", "#ui", "#data", "#infra"] {
        ids.push(
            graph.add_node(
                NodeStyle::new(*tag)
                    .with_kind(NodeKind::Tag)
                    .with_tag(&tag[1..]),
            ),
        );
    }

    // 5 unresolved / stub nodes (diamond shape).
    for name in &[
        "???mod_x",
        "???dep_y",
        "???ext_z",
        "???missing_a",
        "???todo_b",
    ] {
        ids.push(graph.add_node(NodeStyle::new(*name).with_kind(NodeKind::Unresolved)));
    }

    // Preferential-attachment edges for the first 50 nodes.
    for i in 1..50 {
        let j = rng.next_usize(i);
        let w = 0.3 + rng.next_f32() * 0.7;
        let _ = graph.add_edge(ids[i], ids[j], EdgeStyle::new(), w, false);
    }
    // Extra random edges.
    for _ in 0..35 {
        let a = rng.next_usize(50);
        let b = rng.next_usize(50);
        if a != b {
            let _ = graph.add_edge(ids[a], ids[b], EdgeStyle::new(), 0.4, false);
        }
    }
    // Connect tag nodes to random regular nodes.
    for tag_idx in 50..55 {
        for _ in 0..5 {
            let target = rng.next_usize(50);
            let _ = graph.add_edge(ids[tag_idx], ids[target], EdgeStyle::new(), 0.6, false);
        }
    }

    graph
}

// ─── Demo state ───────────────────────────────────────────────────────────────

struct DemoState {
    viewer: GraphViewer,
    graph: GraphData,
    event_log: Vec<String>,
    seed: u64,
    color_mode_idx: usize,
}

const COLOR_MODE_NAMES: &[&str] = &["By Tag", "Static", "By PageRank", "By Betweenness"];

impl DemoState {
    fn new() -> Self {
        let seed = 42;
        let graph = build_graph(seed);

        let mut config = ViewerConfig {
            background_grid: true,
            hover_fade_opacity: 0.15,
            color_mode: ColorMode::ByTag,
            ..ViewerConfig::default()
        };

        // Pre-load a sample color group: highlight "core" nodes in gold.
        use dear_imgui_custom_mod::knowledge_graph::config::ColorGroup;
        config.color_groups.push(ColorGroup::new(
            "core",
            ColorGroupQuery::Tag("core".into()),
            [1.0, 0.80, 0.20, 1.0],
        ));

        let viewer = GraphViewer::new("kg_main")
            .with_config(config)
            .with_force_config(ForceConfig::default())
            .with_sidebar(SidebarKind::Built);

        Self {
            viewer,
            graph,
            event_log: Vec::new(),
            seed,
            color_mode_idx: 0,
        }
    }

    fn regenerate(&mut self) {
        self.seed = self.seed.wrapping_add(1);
        self.graph = build_graph(self.seed);
        self.viewer.reset_layout(&mut self.graph);
        self.log(format!("Regenerated (seed={})", self.seed));
    }

    fn log(&mut self, msg: String) {
        if self.event_log.last() != Some(&msg) {
            self.event_log.push(msg);
        }
        if self.event_log.len() > 120 {
            self.event_log.remove(0);
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("Force Graph Demo")
            .size([1280.0, 800.0], Condition::FirstUseEver)
            .build(|| {
                // ── Toolbar ──────────────────────────────────────────────
                if ui.button("Regenerate") {
                    self.regenerate();
                }
                ui.same_line();
                if ui.button("Fit [F]") {
                    self.viewer.filter_mut().focused_node = None;
                    self.log("Fit to screen".into());
                }
                ui.same_line();
                if ui.button("Reset layout") {
                    self.viewer.reset_layout(&mut self.graph);
                    self.log("Layout reset".into());
                }
                ui.same_line();

                // Color mode combo.
                {
                    let _w = ui.push_item_width(130.0);
                    if let Some(_c) =
                        ui.begin_combo("##cmode", COLOR_MODE_NAMES[self.color_mode_idx])
                    {
                        for (i, label) in COLOR_MODE_NAMES.iter().enumerate() {
                            let selected = i == self.color_mode_idx;
                            if ui.selectable_config(*label).selected(selected).build() {
                                self.color_mode_idx = i;
                                self.viewer.config.color_mode = match i {
                                    0 => ColorMode::ByTag,
                                    1 => ColorMode::Static,
                                    2 => {
                                        self.graph.recompute_metrics_if_needed();
                                        ColorMode::ByPageRank
                                    }
                                    _ => {
                                        self.graph.recompute_metrics_if_needed();
                                        ColorMode::ByBetweenness
                                    }
                                };
                            }
                        }
                    }
                }
                ui.same_line();

                ui.text_disabled(format!(
                    "  {} nodes  {} edges",
                    self.graph.node_count(),
                    self.graph.edge_count()
                ));

                ui.separator();

                // ── Main area: graph + log ────────────────────────────────
                let avail = ui.content_region_avail();
                let log_w = 240.0_f32;
                let graph_w = avail[0] - log_w - 8.0;

                // Graph canvas.
                ui.child_window("##graph_canvas")
                    .size([graph_w, avail[1]])
                    .border(false)
                    .build(ui, || {
                        let events = self.viewer.render(ui, &mut self.graph);
                        for ev in &events {
                            let msg = match ev {
                                GraphEvent::NodeClicked(id) => {
                                    let lbl = self
                                        .graph
                                        .node(*id)
                                        .map(|s| s.label.as_str())
                                        .unwrap_or("?");
                                    format!("Click: {lbl}")
                                }
                                GraphEvent::NodeDoubleClicked(id) => {
                                    let lbl = self
                                        .graph
                                        .node(*id)
                                        .map(|s| s.label.as_str())
                                        .unwrap_or("?");
                                    format!("DblClick: {lbl}")
                                }
                                GraphEvent::NodeHovered(_) => return, // too noisy
                                GraphEvent::CameraChanged => return,  // too noisy
                                GraphEvent::NodeContextMenu(id, _) => {
                                    let lbl = self
                                        .graph
                                        .node(*id)
                                        .map(|s| s.label.as_str())
                                        .unwrap_or("?");
                                    format!("RClick: {lbl}")
                                }
                                GraphEvent::SelectionChanged(sel) => {
                                    format!("Selection: {} nodes", sel.len())
                                }
                                GraphEvent::FilterChanged => "Filter changed".into(),
                                GraphEvent::NodeMoved(id, _) => {
                                    let lbl = self
                                        .graph
                                        .node(*id)
                                        .map(|s| s.label.as_str())
                                        .unwrap_or("?");
                                    format!("Dragged: {lbl}")
                                }
                                GraphEvent::NodePinned(id, p) => {
                                    let lbl = self
                                        .graph
                                        .node(*id)
                                        .map(|s| s.label.as_str())
                                        .unwrap_or("?");
                                    format!("{} {lbl}", if *p { "Pinned:" } else { "Unpinned:" })
                                }
                                GraphEvent::NodeActivated(id) => {
                                    let lbl = self
                                        .graph
                                        .node(*id)
                                        .map(|s| s.label.as_str())
                                        .unwrap_or("?");
                                    format!("Activated: {lbl}")
                                }
                                GraphEvent::SelectionDeleteRequested(sel) => {
                                    format!("Delete {} nodes?", sel.len())
                                }
                                GraphEvent::FitToScreen => "Fit to screen".into(),
                                GraphEvent::SearchChanged(q) => format!("Search: {q}"),
                                GraphEvent::GroupChanged => "Color groups changed".into(),
                                GraphEvent::SimulationToggled(paused) => {
                                    format!("Sim {}", if *paused { "paused" } else { "running" })
                                }
                                GraphEvent::ResetLayout => "Layout reset".into(),
                            };
                            self.log(msg);
                        }
                    });

                ui.same_line();

                // Event log + legend.
                ui.child_window("##event_log")
                    .size([log_w, avail[1]])
                    .border(true)
                    .build(ui, || {
                        ui.text_disabled("Event log");
                        ui.separator();
                        let start = self.event_log.len().saturating_sub(20);
                        for entry in &self.event_log[start..] {
                            ui.text_wrapped(entry);
                        }
                        if ui.scroll_y() >= ui.scroll_max_y() - 4.0 {
                            ui.set_scroll_here_y(1.0);
                        }

                        ui.separator();
                        ui.text_disabled("Shapes");
                        ui.bullet_text("Circle  = Regular");
                        ui.bullet_text("Square  = Tag node");
                        ui.bullet_text("Diamond = Unresolved");

                        ui.separator();
                        ui.text_disabled("Keys");
                        ui.bullet_text("Arrows — pan");
                        ui.bullet_text("+/-    — zoom");
                        ui.bullet_text("F      — fit");
                        ui.bullet_text("Space  — pause sim");
                        ui.bullet_text("P      — pin selected");
                        ui.bullet_text("Esc    — clear selection");
                        ui.bullet_text("Del    — delete request");
                        ui.bullet_text("RClick — context menu");
                    });
            });
    }
}

// ─── wgpu + winit + ImGui boilerplate ────────────────────────────────────────

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
                        .with_inner_size(LogicalSize::new(1280.0, 800.0))
                        .with_title("Force Graph Demo — dear-imgui-custom-mod"),
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

        let segoe = "C:\\Windows\\Fonts\\segoeui.ttf";
        if std::path::Path::new(segoe).exists() {
            let data: &'static [u8] = Box::leak(std::fs::read(segoe).unwrap().into_boxed_slice());
            context
                .fonts()
                .add_font(&[dear_imgui_rs::FontSource::TtfData {
                    data,
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
            &Event::WindowEvent {
                window_id,
                event: event.clone(),
            },
        );
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

                let mut enc = gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("imgui"),
                    });
                {
                    let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("imgui_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.10,
                                    g: 0.11,
                                    b: 0.14,
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
                            .render_draw_data(draw_data, &mut rpass)
                            .expect("render");
                    }
                }
                gpu.queue.submit(std::iter::once(enc.finish()));
                frame.present();
                gpu.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(gpu) = self.gpu.as_ref() {
            gpu.window.request_redraw();
        }
    }
}

fn apply_dark_theme(style: &mut dear_imgui_rs::Style) {
    style.set_color(StyleColor::WindowBg, [0.11, 0.12, 0.15, 1.0]);
    style.set_color(StyleColor::ChildBg, [0.13, 0.14, 0.18, 1.0]);
    style.set_color(StyleColor::FrameBg, [0.16, 0.18, 0.22, 1.0]);
    style.set_color(StyleColor::TitleBgActive, [0.18, 0.20, 0.26, 1.0]);
    style.set_color(StyleColor::Header, [0.24, 0.28, 0.38, 1.0]);
    style.set_color(StyleColor::HeaderHovered, [0.30, 0.35, 0.48, 1.0]);
    style.set_color(StyleColor::Button, [0.22, 0.26, 0.36, 1.0]);
    style.set_color(StyleColor::ButtonHovered, [0.30, 0.35, 0.48, 1.0]);
    style.set_color(StyleColor::ButtonActive, [0.36, 0.42, 0.60, 1.0]);
    style.set_color(StyleColor::SliderGrab, [0.36, 0.42, 0.60, 1.0]);
    style.set_color(StyleColor::CheckMark, [0.50, 0.85, 0.50, 1.0]);
    style.set_color(StyleColor::Text, [0.88, 0.90, 0.92, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.50, 0.53, 0.60, 1.0]);
    style.set_color(StyleColor::Border, [0.24, 0.27, 0.33, 1.0]);
    style.set_window_rounding(5.0);
    style.set_frame_rounding(3.0);
    style.set_scrollbar_rounding(3.0);
    style.set_grab_rounding(3.0);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new()).expect("run_app");
}
