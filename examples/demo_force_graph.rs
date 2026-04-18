//! Demo: force_graph — Obsidian/IDA-style force-directed graph viewer.
//!
//! Demonstrates ALL major force_graph features:
//!   - 5 NodeKind shapes: Regular, Tag, Unresolved, Attachment, Cluster
//!   - 6 color modes: Static, ByTag, ByCommunity, ByPageRank, ByBetweenness, Custom
//!   - Directed and undirected edges with weight-based thickness
//!   - Time-travel slider (nodes/edges have created_at timestamps)
//!   - Search-as-highlight (dims non-matching nodes instead of hiding them)
//!   - Minimap overlay (bottom-right corner, click to pan)
//!   - Pre-pinned cluster hub nodes
//!   - viewer.focus(id) on double-click → smooth camera pan
//!   - viewer.select_multi() via toolbar button
//!   - Color groups + edge colors + edge labels
//!   - Full event log (all GraphEvent variants)
//!   - Regenerate button, node/edge stats, sidebar
//!
//! Run: cargo run --example demo_force_graph --features force_graph,app_window

use dear_imgui_custom_mod::force_graph::{
    config::{ColorGroup, ColorGroupQuery, ColorMode, ForceConfig, LabelVisibility, SidebarKind, ViewerConfig},
    data::GraphData,
    event::GraphEvent,
    style::{EdgeStyle, NodeKind, NodeStyle},
    GraphViewer,
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

const TAG_META: &[(&str, char, [f32; 4])] = &[
    ("core",  '\u{25CF}', [0.40, 0.70, 1.00, 1.0]),
    ("api",   '\u{25B6}', [0.40, 0.90, 0.60, 1.0]),
    ("ui",    '\u{25C6}', [0.95, 0.60, 0.20, 1.0]),
    ("data",  '\u{25A0}', [0.80, 0.40, 0.90, 1.0]),
    ("infra", '\u{25B2}', [0.90, 0.30, 0.30, 1.0]),
    ("test",  '\u{2714}', [0.60, 0.85, 0.85, 1.0]),
];

fn tag_icon(tag: &str) -> char {
    TAG_META.iter().find(|(t, _, _)| *t == tag).map_or('\u{25CF}', |m| m.1)
}
fn tag_color(tag: &str) -> [f32; 4] {
    TAG_META.iter().find(|(t, _, _)| *t == tag).map_or([0.65, 0.65, 0.70, 1.0], |m| m.2)
}

// ─── LCG RNG ──────────────────────────────────────────────────────────────────

struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self { Self(seed) }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn next_f32(&mut self) -> f32 { (self.next() & 0x7FFF_FFFF) as f32 / 0x7FFF_FFFF_u32 as f32 }
    fn next_usize(&mut self, max: usize) -> usize { (self.next() as usize) % max.max(1) }
}

// ─── Graph construction ───────────────────────────────────────────────────────

fn build_graph(seed: u64) -> GraphData {
    let mut rng = Lcg::new(seed);
    let mut graph = GraphData::with_capacity(80, 120);

    let names = [
        "Alpha","Beta","Gamma","Delta","Epsilon","Zeta","Eta","Theta",
        "Iota","Kappa","Lambda","Mu","Nu","Xi","Omicron","Pi","Rho",
        "Sigma","Tau","Upsilon","Phi","Chi","Psi","Omega","Andromeda",
        "Cassiopeia","Cygnus","Perseus","Orion","Lyra","Aquila","Vega",
        "Sirius","Altair","Deneb","Rigel","Betelgeuse","Aldebaran",
        "Pollux","Castor","Procyon","Regulus","Spica","Arcturus",
        "Antares","Fomalhaut","Acrux","Mimosa","Hadar","Canopus",
    ];

    // ── 50 Regular nodes — spread across time 0..100 ──────────────────────────
    let mut ids = Vec::with_capacity(80);
    for (i, name) in names.iter().enumerate() {
        let tag = TAGS[rng.next_usize(TAGS.len())];
        let created_at = i as f32 * 2.0 + rng.next_f32() * 1.5; // 0 .. ~100
        let tooltip = format!("{name} — #{tag}\ncreated_at: {created_at:.1}");
        ids.push(graph.add_node(
            NodeStyle::new(*name)
                .with_tag(tag)
                .with_icon(tag_icon(tag))
                .with_color(tag_color(tag))
                .with_tooltip(tooltip)
                .with_timestamp(created_at),
        ));
    }

    // ── 6 Tag hub nodes (NodeKind::Tag = square, large, pinned) ───────────────
    let mut hub_ids = Vec::with_capacity(6);
    for &(tag, icon, color) in TAG_META {
        let id = graph.add_node(
            NodeStyle::new(format!("#{tag}"))
                .with_kind(NodeKind::Tag)
                .with_tag(tag)
                .with_icon(icon)
                .with_color(color)
                .with_radius(18.0)
                .pinned(),
        );
        hub_ids.push(id);
        ids.push(id);
    }

    // ── 4 Unresolved stub nodes (NodeKind::Unresolved = diamond) ─────────────
    for name in &["???mod_x", "???dep_y", "???ext_z", "???todo"] {
        ids.push(graph.add_node(
            NodeStyle::new(*name)
                .with_kind(NodeKind::Unresolved)
                .with_color([0.55, 0.55, 0.55, 1.0])
                .with_timestamp(80.0 + rng.next_f32() * 20.0),
        ));
    }

    // ── 4 Attachment nodes (NodeKind::Attachment = small circle) ──────────────
    for name in &["attach_A", "attach_B", "attach_C", "attach_D"] {
        ids.push(graph.add_node(
            NodeStyle::new(*name)
                .with_kind(NodeKind::Attachment)
                .with_color([0.70, 0.70, 0.50, 1.0])
                .with_timestamp(50.0 + rng.next_f32() * 30.0),
        ));
    }

    // ── 2 Cluster nodes (NodeKind::Cluster = large octagon) ───────────────────
    let cluster_a = graph.add_node(
        NodeStyle::new("Cluster-A")
            .with_kind(NodeKind::Cluster)
            .with_color([0.30, 0.60, 0.80, 1.0])
            .with_radius(22.0)
            .pinned()
            .with_tooltip("Cluster A — aggregates 'core' and 'api' nodes"),
    );
    let cluster_b = graph.add_node(
        NodeStyle::new("Cluster-B")
            .with_kind(NodeKind::Cluster)
            .with_color([0.80, 0.40, 0.60, 1.0])
            .with_radius(22.0)
            .pinned()
            .with_tooltip("Cluster B — aggregates 'ui' and 'data' nodes"),
    );

    // ── Edges: preferential attachment (undirected) ────────────────────────────
    for i in 1..50 {
        let j = rng.next_usize(i);
        let w = 0.3 + rng.next_f32() * 0.7;
        let created_at = i as f32 * 2.0;
        let _ = graph.add_edge(
            ids[i], ids[j],
            EdgeStyle::new().with_timestamp(created_at),
            w, false,
        );
    }
    // Extra random edges.
    for _ in 0..30 {
        let a = rng.next_usize(50);
        let b = rng.next_usize(50);
        if a != b {
            let _ = graph.add_edge(ids[a], ids[b], EdgeStyle::new(), 0.4, false);
        }
    }

    // ── Directed edges: hub → members (show arrowheads) ───────────────────────
    for (hub_i, &(tag, _, _)) in TAG_META.iter().enumerate() {
        let hub = hub_ids[hub_i];
        for &target in ids[..50].iter() {
            if graph.node(target).is_some_and(|n| n.tags.contains(&tag)) {
                let _ = graph.add_edge(
                    hub, target,
                    EdgeStyle::new().with_color([0.7, 0.7, 0.7, 0.5]),
                    0.7, true, // directed = true → arrowhead
                );
            }
        }
    }

    // ── Cluster connections ────────────────────────────────────────────────────
    for &id in ids[..20].iter() {
        let _ = graph.add_edge(cluster_a, id, EdgeStyle::new(), 0.5, false);
    }
    for &id in ids[20..40].iter() {
        let _ = graph.add_edge(cluster_b, id, EdgeStyle::new(), 0.5, false);
    }

    // ── Attachment nodes connected to nearby regular nodes ─────────────────────
    let attach_start = 60; // idx in ids where attachment nodes begin
    for i in 0..4 {
        if let Some(&att) = ids.get(attach_start + i) {
            for _ in 0..3 {
                let target = rng.next_usize(50);
                let _ = graph.add_edge(att, ids[target], EdgeStyle::new(), 0.3, false);
            }
        }
    }

    graph
}

// ─── Demo state ───────────────────────────────────────────────────────────────

const COLOR_MODES: &[&str] = &[
    "Static", "By Tag", "By Community", "By PageRank", "By Betweenness",
];

struct DemoState {
    viewer: GraphViewer,
    graph: GraphData,
    event_log: Vec<String>,
    seed: u64,
    color_mode_idx: usize,
}

impl DemoState {
    fn new() -> Self {
        let seed = 42;
        let graph = build_graph(seed);

        let mut config = ViewerConfig {
            background_grid: true,
            hover_fade_opacity: 0.12,
            glow_on_hover: true,
            color_mode: ColorMode::Static,
            show_labels: LabelVisibility::BySize,
            min_label_zoom: 0.45,
            minimap: true,                // Phase D: minimap overlay enabled
            search_highlight_mode: true,  // Phase D: dim non-matches instead of hiding
            ..ViewerConfig::default()
        };

        for &(tag, _, color) in TAG_META {
            config.color_groups.push(ColorGroup::new(
                tag,
                ColorGroupQuery::Tag(tag.into()),
                color,
            ));
        }

        let viewer = GraphViewer::new("fg_main")
            .with_config(config)
            .with_force_config(ForceConfig::default())
            .with_sidebar(SidebarKind::Built);

        Self { viewer, graph, event_log: Vec::new(), seed, color_mode_idx: 0 }
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
        if self.event_log.len() > 150 {
            self.event_log.remove(0);
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("Force Graph Demo — all features")
            .size([1440.0, 900.0], Condition::FirstUseEver)
            .build(|| {
                // ── Toolbar ──────────────────────────────────────────────────
                if ui.button("Regenerate") { self.regenerate(); }
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

                // Select all via API.
                if ui.button("Select all") {
                    let all: Vec<_> = self.graph.nodes().map(|(id, _)| id).collect();
                    self.viewer.select_multi(&all);
                    self.log(format!("select_multi: {} nodes", all.len()));
                }
                ui.same_line();

                // Recompute metrics + switch color mode.
                {
                    let _w = ui.push_item_width(150.0);
                    if let Some(_c) = ui.begin_combo("##cmode", COLOR_MODES[self.color_mode_idx]) {
                        for (i, label) in COLOR_MODES.iter().enumerate() {
                            let sel = i == self.color_mode_idx;
                            if ui.selectable_config(*label).selected(sel).build() {
                                self.color_mode_idx = i;
                                self.viewer.config.color_mode = match i {
                                    0 => ColorMode::Static,
                                    1 => ColorMode::ByTag,
                                    2 => { self.graph.recompute_metrics_if_needed(); ColorMode::ByCommunity }
                                    3 => { self.graph.recompute_metrics_if_needed(); ColorMode::ByPageRank }
                                    _ => { self.graph.recompute_metrics_if_needed(); ColorMode::ByBetweenness }
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

                // ── Main area: graph + log ────────────────────────────────────
                let avail = ui.content_region_avail();
                let log_w = 260.0_f32;
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
                                    let lbl = self.graph.node(*id).map(|s| s.label.as_str()).unwrap_or("?");
                                    format!("Click: {lbl}")
                                }
                                GraphEvent::NodeDoubleClicked(id) => {
                                    // Double-click → smooth camera focus via viewer.focus()
                                    self.viewer.focus(*id);
                                    let lbl = self.graph.node(*id).map(|s| s.label.as_str()).unwrap_or("?");
                                    format!("Focus→ {lbl}")
                                }
                                GraphEvent::NodeHovered(_) => return,
                                GraphEvent::CameraChanged => return,
                                GraphEvent::NodeContextMenu(id, _) => {
                                    let lbl = self.graph.node(*id).map(|s| s.label.as_str()).unwrap_or("?");
                                    format!("RClick: {lbl}")
                                }
                                GraphEvent::SelectionChanged(sel) => {
                                    format!("Selection: {} nodes", sel.len())
                                }
                                GraphEvent::FilterChanged => "Filter changed".into(),
                                GraphEvent::SearchChanged(q) => format!("Search: \"{q}\""),
                                GraphEvent::GroupChanged => "Color groups changed".into(),
                                GraphEvent::NodeMoved(id, _) => {
                                    let lbl = self.graph.node(*id).map(|s| s.label.as_str()).unwrap_or("?");
                                    format!("Dragged: {lbl}")
                                }
                                GraphEvent::NodePinned(id, p) => {
                                    let lbl = self.graph.node(*id).map(|s| s.label.as_str()).unwrap_or("?");
                                    format!("{} {lbl}", if *p { "Pinned:" } else { "Unpinned:" })
                                }
                                GraphEvent::NodeActivated(id) => {
                                    let lbl = self.graph.node(*id).map(|s| s.label.as_str()).unwrap_or("?");
                                    format!("Activated: {lbl}")
                                }
                                GraphEvent::SelectionDeleteRequested(sel) => {
                                    format!("Delete {} nodes?", sel.len())
                                }
                                GraphEvent::FitToScreen => "Fit to screen".into(),
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
                        let start = self.event_log.len().saturating_sub(24);
                        for entry in &self.event_log[start..] {
                            ui.text_wrapped(entry);
                        }
                        if ui.scroll_y() >= ui.scroll_max_y() - 4.0 {
                            ui.set_scroll_here_y(1.0);
                        }

                        ui.separator();
                        ui.text_disabled("Node shapes");
                        ui.bullet_text("Circle  = Regular");
                        ui.bullet_text("Square  = Tag hub (pinned)");
                        ui.bullet_text("Diamond = Unresolved");
                        ui.bullet_text("Small   = Attachment");
                        ui.bullet_text("Octagon = Cluster (pinned)");

                        ui.separator();
                        ui.text_disabled("Edge types");
                        ui.bullet_text("Plain   = undirected");
                        ui.bullet_text("Arrow   = directed (hub→member)");

                        ui.separator();
                        ui.text_disabled("Features (Phase D)");
                        ui.bullet_text("Minimap: bottom-right corner");
                        ui.bullet_text("Time-travel: sidebar Filters");
                        ui.bullet_text("Search: dims non-matches");
                        ui.bullet_text("Export: sidebar bottom");

                        ui.separator();
                        ui.text_disabled("Keys");
                        ui.bullet_text("Arrows  pan");
                        ui.bullet_text("+/-     zoom");
                        ui.bullet_text("F       fit");
                        ui.bullet_text("Space   pause sim");
                        ui.bullet_text("P       pin selected");
                        ui.bullet_text("Esc     clear selection");
                        ui.bullet_text("Del     delete request");
                        ui.bullet_text("DblClick  focus node");
                        ui.bullet_text("RClick  context menu");
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

struct App { gpu: Option<GpuState> }
impl App { fn new() -> Self { Self { gpu: None } } }

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() { return; }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_inner_size(LogicalSize::new(1440.0, 900.0))
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
        })).expect("adapter");
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
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

        let segoe = "C:\\Windows\\Fonts\\segoeui.ttf";
        if std::path::Path::new(segoe).exists() {
            let data: &'static [u8] = Box::leak(std::fs::read(segoe).unwrap().into_boxed_slice());
            context.fonts().add_font(&[dear_imgui_rs::FontSource::TtfData {
                data,
                size_pixels: Some(font_size),
                config: Some(dear_imgui_rs::FontConfig::new().size_pixels(font_size).oversample_h(2)),
            }]);
        } else {
            context.fonts().add_font(&[dear_imgui_rs::FontSource::DefaultFontData {
                config: Some(dear_imgui_rs::FontConfig::new().size_pixels(font_size).oversample_h(2)),
                size_pixels: Some(font_size),
            }]);
        }

        apply_dark_theme(context.style_mut());

        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_cfg.format),
            &mut context,
        ).expect("renderer");

        self.gpu = Some(GpuState { device, queue, window, surface_cfg, surface,
            context, platform, renderer, demo: DemoState::new() });
    }

    fn window_event(
        &mut self, event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId, event: WindowEvent,
    ) {
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
                    wgpu::CurrentSurfaceTexture::Success(f)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                        return;
                    }
                    other => { eprintln!("Surface error: {other:?}"); return; }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);
                let ui = gpu.context.frame();
                gpu.demo.render(ui);
                gpu.platform.prepare_render_with_ui(ui, &gpu.window);
                let draw_data = gpu.context.render();
                let mut enc = gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("imgui") });
                {
                    let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("imgui_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color { r:0.10, g:0.11, b:0.14, a:1.0 }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    if draw_data.total_vtx_count > 0 {
                        gpu.renderer.render_draw_data(draw_data, &mut rpass).expect("render");
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
        if let Some(gpu) = self.gpu.as_ref() { gpu.window.request_redraw(); }
    }
}

fn apply_dark_theme(style: &mut dear_imgui_rs::Style) {
    style.set_color(StyleColor::WindowBg,      [0.11, 0.12, 0.15, 1.0]);
    style.set_color(StyleColor::ChildBg,       [0.13, 0.14, 0.18, 1.0]);
    style.set_color(StyleColor::FrameBg,       [0.16, 0.18, 0.22, 1.0]);
    style.set_color(StyleColor::TitleBgActive, [0.18, 0.20, 0.26, 1.0]);
    style.set_color(StyleColor::Header,        [0.24, 0.28, 0.38, 1.0]);
    style.set_color(StyleColor::HeaderHovered, [0.30, 0.35, 0.48, 1.0]);
    style.set_color(StyleColor::Button,        [0.22, 0.26, 0.36, 1.0]);
    style.set_color(StyleColor::ButtonHovered, [0.30, 0.35, 0.48, 1.0]);
    style.set_color(StyleColor::ButtonActive,  [0.36, 0.42, 0.60, 1.0]);
    style.set_color(StyleColor::SliderGrab,    [0.36, 0.42, 0.60, 1.0]);
    style.set_color(StyleColor::CheckMark,     [0.50, 0.85, 0.50, 1.0]);
    style.set_color(StyleColor::Text,          [0.88, 0.90, 0.92, 1.0]);
    style.set_color(StyleColor::TextDisabled,  [0.50, 0.53, 0.60, 1.0]);
    style.set_color(StyleColor::Border,        [0.24, 0.27, 0.33, 1.0]);
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
