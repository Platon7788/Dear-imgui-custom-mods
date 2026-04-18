//! Demo: knowledge_graph — Obsidian-style force-directed graph viewer.
//!
//! Features demonstrated:
//!   - 50 random nodes with preferential-attachment edges
//!   - Pan (drag) / zoom (mouse wheel)
//!   - Click to select; Ctrl+click to toggle
//!   - Hover tooltip with node label
//!   - Event log (last 10 events)
//!   - Regenerate button — rebuilds the random graph
//!
//! Run: cargo run --example demo_knowledge_graph --features knowledge_graph,app_window

use dear_imgui_custom_mod::knowledge_graph::{
    config::{ForceConfig, ViewerConfig},
    data::GraphData,
    event::GraphEvent,
    style::{EdgeStyle, NodeStyle},
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

// ─── Tag definitions ─────────────────────────────────────────────────────────

const TAGS: &[&str] = &["core", "api", "ui", "data", "infra", "test", "docs"];

// ─── Random graph generation ──────────────────────────────────────────────────

/// Simple LCG pseudo-random number generator (no external dep).
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn next_f32(&mut self) -> f32 {
        (self.next() & 0x7FFF_FFFF) as f32 / 0x7FFF_FFFF as f32
    }
    fn next_usize(&mut self, max: usize) -> usize {
        (self.next() as usize) % max.max(1)
    }
}

/// Build a random graph with 50 nodes and ~80 edges (preferential attachment).
fn build_random_graph(seed: u64) -> GraphData {
    let mut rng = Lcg::new(seed);
    let mut graph = GraphData::with_capacity(50, 80);

    // Add 50 nodes.
    let node_names = [
        "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta",
        "Iota", "Kappa", "Lambda", "Mu", "Nu", "Xi", "Omicron", "Pi", "Rho",
        "Sigma", "Tau", "Upsilon", "Phi", "Chi", "Psi", "Omega", "Andromeda",
        "Cassiopeia", "Cygnus", "Perseus", "Orion", "Lyra", "Aquila", "Vega",
        "Sirius", "Altair", "Deneb", "Rigel", "Betelgeuse", "Aldebaran",
        "Pollux", "Castor", "Procyon", "Regulus", "Spica", "Arcturus",
        "Antares", "Fomalhaut", "Acrux", "Mimosa", "Hadar", "Canopus",
    ];

    let ids: Vec<_> = node_names.iter().map(|name| {
        let tag = TAGS[rng.next_usize(TAGS.len())];
        graph.add_node(
            NodeStyle::new(*name)
                .with_tag(tag),
        )
    }).collect();

    // Add ~80 edges using preferential-attachment heuristic:
    // for each node i > 0, connect to a random earlier node.
    // Then add 30 extra random edges.
    for i in 1..ids.len() {
        let j = rng.next_usize(i);
        let w = 0.3 + rng.next_f32() * 0.7;
        let _ = graph.add_edge(ids[i], ids[j], EdgeStyle::new(), w, false);
    }
    for _ in 0..30 {
        let a = rng.next_usize(ids.len());
        let b = rng.next_usize(ids.len());
        if a != b {
            let w = 0.2 + rng.next_f32() * 0.8;
            let _ = graph.add_edge(ids[a], ids[b], EdgeStyle::new(), w, false);
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
}

impl DemoState {
    fn new() -> Self {
        let seed = 42;
        let graph = build_random_graph(seed);
        let viewer = GraphViewer::new("kg_main")
            .with_config(ViewerConfig {
                background_grid: true,
                ..ViewerConfig::default()
            })
            .with_force_config(ForceConfig::default());
        Self {
            viewer,
            graph,
            event_log: Vec::new(),
            seed,
        }
    }

    fn regenerate(&mut self) {
        self.seed = self.seed.wrapping_add(1);
        self.graph = build_random_graph(self.seed);
        self.viewer.reset_layout(&mut self.graph);
        self.event_log.push(format!("Graph regenerated (seed={})", self.seed));
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("Knowledge Graph Demo")
            .size([1200.0, 750.0], Condition::FirstUseEver)
            .build(|| {
                // Toolbar
                if ui.button("Regenerate") {
                    self.regenerate();
                }
                ui.same_line();
                ui.text(format!(
                    "Nodes: {}  Edges: {}",
                    self.graph.node_count(),
                    self.graph.edge_count()
                ));

                ui.separator();

                // Split: graph on left, event log on right.
                let avail = ui.content_region_avail();
                let log_w = 260.0_f32;
                let graph_w = avail[0] - log_w - 8.0;

                // ── Graph viewer ──
                {
                    let _child = ui.child_window("##graph_canvas")
                        .size([graph_w, avail[1]])
                        .border(false)
                        .build(ui, || {
                            let events = self.viewer.render(ui, &mut self.graph);
                            for ev in events {
                                let msg = match &ev {
                                    GraphEvent::NodeClicked(id) => {
                                        let label = self.graph.node(*id)
                                            .map(|s| s.label.as_str())
                                            .unwrap_or("?");
                                        format!("Click: {label}")
                                    }
                                    GraphEvent::NodeDoubleClicked(id) => {
                                        let label = self.graph.node(*id)
                                            .map(|s| s.label.as_str())
                                            .unwrap_or("?");
                                        format!("DblClick: {label}")
                                    }
                                    GraphEvent::NodeHovered(id) => {
                                        let label = self.graph.node(*id)
                                            .map(|s| s.label.as_str())
                                            .unwrap_or("?");
                                        format!("Hover: {label}")
                                    }
                                    GraphEvent::NodeContextMenu(id, _) => {
                                        let label = self.graph.node(*id)
                                            .map(|s| s.label.as_str())
                                            .unwrap_or("?");
                                        format!("RClick: {label}")
                                    }
                                    GraphEvent::SelectionChanged(sel) => {
                                        format!("Selection: {} nodes", sel.len())
                                    }
                                    GraphEvent::CameraChanged => "Camera moved".to_string(),
                                    GraphEvent::FilterChanged => "Filter changed".to_string(),
                                    GraphEvent::NodeMoved(id, _) => format!("Moved: {id:?}"),
                                    GraphEvent::NodePinned(id, p) => format!("Pin {id:?}: {p}"),
                                    GraphEvent::NodeActivated(id) => format!("Activated: {id:?}"),
                                    GraphEvent::SelectionDeleteRequested(_) => "Delete requested".to_string(),
                                    GraphEvent::FitToScreen => "Fit to screen".to_string(),
                                    GraphEvent::SearchChanged(q) => format!("Search: {q}"),
                                    GraphEvent::GroupChanged => "Groups changed".to_string(),
                                    GraphEvent::SimulationToggled(s) => format!("Sim paused: {s}"),
                                };
                                if self.event_log.last().as_deref() != Some(&msg) {
                                    self.event_log.push(msg);
                                }
                                if self.event_log.len() > 100 {
                                    self.event_log.remove(0);
                                }
                            }
                        });
                }

                ui.same_line();

                // ── Event log ──
                {
                    let _child = ui.child_window("##event_log")
                        .size([log_w, avail[1]])
                        .border(true)
                        .build(ui, || {
                            ui.text_disabled("Event log (last 10)");
                            ui.separator();
                            let start = self.event_log.len().saturating_sub(10);
                            for entry in &self.event_log[start..] {
                                ui.text_wrapped(entry);
                            }
                            // Auto-scroll to bottom.
                            if ui.scroll_y() >= ui.scroll_max_y() - 4.0 {
                                ui.set_scroll_here_y(1.0);
                            }
                        });
                }
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
                        .with_inner_size(LogicalSize::new(1200.0, 750.0))
                        .with_title("Knowledge Graph Demo"),
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

        let segoe_path = "C:\\Windows\\Fonts\\segoeui.ttf";
        if std::path::Path::new(segoe_path).exists() {
            let font_data = std::fs::read(segoe_path).expect("read font");
            let font_data: &'static [u8] = Box::leak(font_data.into_boxed_slice());
            context.fonts().add_font(&[dear_imgui_rs::FontSource::TtfData {
                data: font_data,
                size_pixels: Some(font_size),
                config: Some(
                    dear_imgui_rs::FontConfig::new()
                        .size_pixels(font_size)
                        .oversample_h(2),
                ),
            }]);
        } else {
            context.fonts().add_font(&[dear_imgui_rs::FontSource::DefaultFontData {
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
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                        return;
                    }
                    other => {
                        eprintln!("Surface unavailable: {other:?}");
                        return;
                    }
                };

                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

                gpu.platform
                    .prepare_frame(&gpu.window, &mut gpu.context);

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
                                    r: 0.12,
                                    g: 0.13,
                                    b: 0.16,
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
                            .expect("renderer.render");
                    }
                }
                gpu.queue.submit(std::iter::once(enc.finish()));
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
    style.set_color(StyleColor::WindowBg, [0.12, 0.13, 0.16, 1.0]);
    style.set_color(StyleColor::ChildBg, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::FrameBg, [0.16, 0.18, 0.22, 1.0]);
    style.set_color(StyleColor::TitleBgActive, [0.18, 0.20, 0.26, 1.0]);
    style.set_color(StyleColor::Header, [0.24, 0.28, 0.38, 1.0]);
    style.set_color(StyleColor::HeaderHovered, [0.30, 0.35, 0.48, 1.0]);
    style.set_color(StyleColor::Button, [0.24, 0.28, 0.38, 1.0]);
    style.set_color(StyleColor::ButtonHovered, [0.30, 0.35, 0.48, 1.0]);
    style.set_color(StyleColor::Text, [0.88, 0.90, 0.92, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.54, 0.57, 0.63, 1.0]);
    style.set_color(StyleColor::Border, [0.25, 0.28, 0.33, 1.0]);
    style.set_window_rounding(4.0);
    style.set_frame_rounding(3.0);
    style.set_scrollbar_rounding(3.0);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new()).expect("run_app");
}
