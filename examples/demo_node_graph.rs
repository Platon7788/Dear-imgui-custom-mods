//! Demo: NodeGraph — visual node editor showcase.
//!
//! Features demonstrated:
//!   - Multiple node types (Value, Math, Output) with typed pins
//!   - Bezier wire rendering with per-pin colors
//!   - Pan/zoom, multi-select, rectangle selection
//!   - Context menu (right-click canvas to add nodes)
//!   - Delete selected nodes (Delete key)
//!   - Snap-to-grid toggle
//!   - Mini-map navigation
//!   - Node collapse/expand
//!   - Wire yanking (Ctrl+click wire)
//!
//! Run: cargo run --example demo_node_graph

use dear_imgui_custom_mod::node_graph::*;
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

// ─── Pin types (for color coding) ──────────────────────────────────────────

const PIN_FLOAT: [u8; 3] = [0x5b, 0x9b, 0xd5]; // blue
const PIN_VEC2: [u8; 3] = [0x7b, 0xbb, 0x55]; // green
const PIN_COLOR: [u8; 3] = [0xd5, 0x5b, 0x9b]; // pink
const PIN_ANY: [u8; 3] = [0xa0, 0xa0, 0xa0]; // gray

// ─── Node types ────────────────────────────────────────────────────────────

#[derive(Clone)]
enum DemoNode {
    /// Constant float value with a slider.
    FloatValue { value: f32 },
    /// Constant Vec2 with two sliders.
    Vec2Value { x: f32, y: f32 },
    /// Constant color picker.
    ColorValue { color: [f32; 3] },
    /// Math operation: Add, Subtract, Multiply, Divide.
    MathOp { op: MathOpKind },
    /// Clamp: min/max.
    Clamp { min: f32, max: f32 },
    /// Mix/Lerp: blend two values.
    Mix { factor: f32 },
    /// Final output / display node. Label stored in node data.
    Output { label: String },
}

#[derive(Clone, Copy, PartialEq)]
enum MathOpKind {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl MathOpKind {
    fn label(self) -> &'static str {
        match self {
            Self::Add => "+  Add",
            Self::Subtract => "-  Subtract",
            Self::Multiply => "*  Multiply",
            Self::Divide => "/  Divide",
        }
    }

    fn all() -> &'static [MathOpKind] {
        &[Self::Add, Self::Subtract, Self::Multiply, Self::Divide]
    }
}

// ─── Viewer implementation ─────────────────────────────────────────────────

struct DemoViewer;

impl NodeGraphViewer<DemoNode> for DemoViewer {
    fn title<'a>(&'a self, node: &'a DemoNode) -> &'a str {
        match node {
            DemoNode::FloatValue { .. } => "Float",
            DemoNode::Vec2Value { .. } => "Vec2",
            DemoNode::ColorValue { .. } => "Color",
            DemoNode::MathOp { op } => match op {
                MathOpKind::Add => "Add",
                MathOpKind::Subtract => "Subtract",
                MathOpKind::Multiply => "Multiply",
                MathOpKind::Divide => "Divide",
            },
            DemoNode::Clamp { .. } => "Clamp",
            DemoNode::Mix { .. } => "Mix",
            DemoNode::Output { label } => label.as_str(),
        }
    }

    fn inputs(&self, node: &DemoNode) -> u8 {
        match node {
            DemoNode::FloatValue { .. } => 0,
            DemoNode::Vec2Value { .. } => 0,
            DemoNode::ColorValue { .. } => 0,
            DemoNode::MathOp { .. } => 2,
            DemoNode::Clamp { .. } => 1,
            DemoNode::Mix { .. } => 2,
            DemoNode::Output { .. } => 1,
        }
    }

    fn outputs(&self, node: &DemoNode) -> u8 {
        match node {
            DemoNode::FloatValue { .. } => 1,
            DemoNode::Vec2Value { .. } => 1,
            DemoNode::ColorValue { .. } => 1,
            DemoNode::MathOp { .. } => 1,
            DemoNode::Clamp { .. } => 1,
            DemoNode::Mix { .. } => 1,
            DemoNode::Output { .. } => 0,
        }
    }

    fn input_label(&self, node: &DemoNode, input: u8) -> &str {
        match node {
            DemoNode::MathOp { .. } => {
                if input == 0 { "A" } else { "B" }
            }
            DemoNode::Clamp { .. } => "Value",
            DemoNode::Mix { .. } => {
                if input == 0 { "A" } else { "B" }
            }
            DemoNode::Output { .. } => "In",
            _ => "",
        }
    }

    fn output_label(&self, node: &DemoNode, _output: u8) -> &str {
        match node {
            DemoNode::FloatValue { .. } => "Value",
            DemoNode::Vec2Value { .. } => "XY",
            DemoNode::ColorValue { .. } => "RGB",
            DemoNode::MathOp { .. } => "Result",
            DemoNode::Clamp { .. } => "Out",
            DemoNode::Mix { .. } => "Out",
            _ => "",
        }
    }

    fn input_pin(&self, node: &DemoNode, _input: u8) -> PinInfo {
        match node {
            DemoNode::MathOp { .. } => PinInfo::circle(PIN_FLOAT),
            DemoNode::Clamp { .. } => PinInfo::circle(PIN_FLOAT),
            DemoNode::Mix { .. } => PinInfo::circle(PIN_FLOAT),
            DemoNode::Output { .. } => PinInfo::diamond(PIN_ANY),
            _ => PinInfo::default(),
        }
    }

    fn output_pin(&self, node: &DemoNode, _output: u8) -> PinInfo {
        match node {
            DemoNode::FloatValue { .. } => PinInfo::circle(PIN_FLOAT),
            DemoNode::Vec2Value { .. } => PinInfo::square(PIN_VEC2),
            DemoNode::ColorValue { .. } => PinInfo::triangle(PIN_COLOR),
            DemoNode::MathOp { .. } => PinInfo::circle(PIN_FLOAT),
            DemoNode::Clamp { .. } => PinInfo::circle(PIN_FLOAT),
            DemoNode::Mix { .. } => PinInfo::circle(PIN_FLOAT),
            _ => PinInfo::default(),
        }
    }

    fn has_body(&self, node: &DemoNode) -> bool {
        matches!(
            node,
            DemoNode::FloatValue { .. }
                | DemoNode::Vec2Value { .. }
                | DemoNode::ColorValue { .. }
                | DemoNode::Clamp { .. }
                | DemoNode::Mix { .. }
                | DemoNode::MathOp { .. }
        )
    }

    fn render_body(&self, ui: &Ui, node: &mut DemoNode, _id: NodeId) {
        match node {
            DemoNode::FloatValue { value } => {
                ui.set_next_item_width(100.0);
                ui.slider("##val", -10.0, 10.0, value);
            }
            DemoNode::Vec2Value { x, y } => {
                ui.set_next_item_width(100.0);
                ui.slider("##x", -10.0, 10.0, x);
                ui.set_next_item_width(100.0);
                ui.slider("##y", -10.0, 10.0, y);
            }
            DemoNode::ColorValue { color } => {
                ui.set_next_item_width(100.0);
                ui.color_edit3("##col", color);
            }
            DemoNode::Clamp { min, max } => {
                ui.set_next_item_width(50.0);
                ui.input_float("##min", min);
                ui.same_line();
                ui.set_next_item_width(50.0);
                ui.input_float("##max", max);
            }
            DemoNode::Mix { factor } => {
                ui.set_next_item_width(100.0);
                ui.slider("##fac", 0.0, 1.0, factor);
            }
            DemoNode::MathOp { op } => {
                let mut idx = MathOpKind::all()
                    .iter()
                    .position(|k| *k == *op)
                    .unwrap_or(0);
                let labels: Vec<&str> = MathOpKind::all().iter().map(|k| k.label()).collect();
                ui.set_next_item_width(110.0);
                if ui.combo_simple_string("##op", &mut idx, &labels) {
                    *op = MathOpKind::all()[idx];
                }
            }
            _ => {}
        }
    }

    fn header_color(&self, node: &DemoNode) -> Option<[u8; 3]> {
        Some(match node {
            DemoNode::FloatValue { .. } => [0x2a, 0x5a, 0x8a], // blue
            DemoNode::Vec2Value { .. } => [0x3a, 0x6a, 0x2a], // green
            DemoNode::ColorValue { .. } => [0x7a, 0x2a, 0x5a], // pink
            DemoNode::MathOp { .. } => [0x5a, 0x4a, 0x2a], // amber
            DemoNode::Clamp { .. } => [0x4a, 0x3a, 0x5a], // purple
            DemoNode::Mix { .. } => [0x2a, 0x5a, 0x5a], // teal
            DemoNode::Output { .. } => [0x6a, 0x2a, 0x2a], // red
        })
    }

    fn node_tooltip(&self, node: &DemoNode) -> Option<&str> {
        Some(match node {
            DemoNode::FloatValue { .. } => "Outputs a constant float value",
            DemoNode::Vec2Value { .. } => "Outputs a 2D vector (X, Y)",
            DemoNode::ColorValue { .. } => "Outputs an RGB color",
            DemoNode::MathOp { .. } => "Performs arithmetic on two inputs",
            DemoNode::Clamp { .. } => "Clamps input to [min, max] range",
            DemoNode::Mix { .. } => "Linear interpolation between A and B",
            DemoNode::Output { .. } => "Final output / display node",
        })
    }

    fn node_width(&self, node: &DemoNode) -> Option<f32> {
        Some(match node {
            DemoNode::Vec2Value { .. } | DemoNode::Clamp { .. } => 150.0,
            DemoNode::MathOp { .. } => 160.0,
            _ => 140.0,
        })
    }

    fn body_height(&self, node: &DemoNode) -> Option<f32> {
        match node {
            // Two sliders (x + y) — need double row height
            DemoNode::Vec2Value { .. } => Some(54.0),
            // Color editor is taller than a single slider
            DemoNode::ColorValue { .. } => Some(42.0),
            _ => None, // use config.node_body_height default
        }
    }
}

// ─── Demo state ────────────────────────────────────────────────────────────

struct DemoState {
    ng: NodeGraph<DemoNode>,
    viewer: DemoViewer,
    /// Position where context menu was opened (graph space).
    ctx_menu_pos: Option<[f32; 2]>,
    /// Pending "dropped wire" — create node and auto-connect.
    dropped_wire_out: Option<(OutPinId, [f32; 2])>,
    dropped_wire_in: Option<(InPinId, [f32; 2])>,
}

impl DemoState {
    fn new() -> Self {
        let mut ng = NodeGraph::new("demo_ng");

        // Seed some nodes
        let val1 = ng.add_node(DemoNode::FloatValue { value: 3.0 }, [50.0, 50.0]);
        let val2 = ng.add_node(DemoNode::FloatValue { value: 7.5 }, [50.0, 200.0]);
        let add = ng.add_node(
            DemoNode::MathOp {
                op: MathOpKind::Add,
            },
            [280.0, 100.0],
        );
        let clamp = ng.add_node(
            DemoNode::Clamp {
                min: 0.0,
                max: 10.0,
            },
            [480.0, 100.0],
        );
        let output = ng.add_node(
            DemoNode::Output { label: "Result".into() },
            [680.0, 100.0],
        );

        let col = ng.add_node(
            DemoNode::ColorValue {
                color: [0.8, 0.3, 0.2],
            },
            [50.0, 350.0],
        );
        let _mix = ng.add_node(DemoNode::Mix { factor: 0.5 }, [280.0, 300.0]);
        let _vec = ng.add_node(
            DemoNode::Vec2Value { x: 1.0, y: -2.0 },
            [50.0, 500.0],
        );

        // Wire up: val1 -> add.A, val2 -> add.B, add -> clamp -> output
        ng.connect(
            OutPinId { node: val1, output: 0 },
            InPinId { node: add, input: 0 },
        );
        ng.connect(
            OutPinId { node: val2, output: 0 },
            InPinId { node: add, input: 1 },
        );
        ng.connect(
            OutPinId { node: add, output: 0 },
            InPinId { node: clamp, input: 0 },
        );
        ng.connect(
            OutPinId { node: clamp, output: 0 },
            InPinId { node: output, input: 0 },
        );

        // Keep color node unconnected for demo
        let _ = col;

        Self {
            ng,
            viewer: DemoViewer,
            ctx_menu_pos: None,
            dropped_wire_out: None,
            dropped_wire_in: None,
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("Node Graph Demo")
            .size([1200.0, 750.0], Condition::FirstUseEver)
            .build(|| {
                self.render_toolbar(ui);
                ui.spacing();

                // ── Render node graph ──────────────────────────────────
                let actions = self.ng.render(ui, &self.viewer);

                // ── Process actions ────────────────────────────────────
                for action in &actions {
                    match *action {
                        GraphAction::Connected(wire) => {
                            self.ng.graph.connect(wire.out_pin, wire.in_pin);
                        }
                        GraphAction::Disconnected(wire) => {
                            self.ng.graph.disconnect(wire.out_pin, wire.in_pin);
                        }
                        GraphAction::CanvasMenu(pos) => {
                            self.ctx_menu_pos = Some(pos);
                            ui.open_popup("##add_node");
                        }
                        GraphAction::NodeMenu(nid) => {
                            // Simple: just select it for now
                            self.ng.state.select_node(nid, false);
                        }
                        GraphAction::DeleteSelected => {
                            let to_delete: Vec<NodeId> = self.ng.selected();
                            for id in to_delete {
                                self.ng.remove_node(id);
                            }
                        }
                        GraphAction::DroppedWireOut(pin, pos) => {
                            self.dropped_wire_out = Some((pin, pos));
                            ui.open_popup("##add_node");
                        }
                        GraphAction::DroppedWireIn(pin, pos) => {
                            self.dropped_wire_in = Some((pin, pos));
                            ui.open_popup("##add_node");
                        }
                        _ => {}
                    }
                }

                // ── Context menu popup ─────────────────────────────────
                self.render_add_menu(ui);
            });
    }

    fn render_toolbar(&mut self, ui: &Ui) {
        if ui.button("Fit") {
            let avail = ui.content_region_avail();
            self.ng.fit_to_content(avail, &self.viewer);
        }
        ui.same_line();
        if ui.button("Reset View") {
            self.ng.reset_viewport();
        }
        ui.same_line();
        ui.checkbox("Grid", &mut self.ng.config.show_grid);
        ui.same_line();
        ui.checkbox("Snap", &mut self.ng.config.snap_to_grid);
        ui.same_line();
        ui.checkbox("Minimap", &mut self.ng.config.show_minimap);
        ui.same_line();

        let mut behind = self.ng.config.wire_layer == WireLayer::BehindNodes;
        if ui.checkbox("Wires Behind", &mut behind) {
            self.ng.config.wire_layer = if behind {
                WireLayer::BehindNodes
            } else {
                WireLayer::AboveNodes
            };
        }

        // ── Second toolbar row: advanced settings ──
        // Grid size
        ui.set_next_item_width(80.0);
        ui.slider_config("Grid Size", 8.0, 128.0)
            .build(&mut self.ng.config.grid_size);
        ui.same_line();

        // Grid rotation
        ui.set_next_item_width(80.0);
        ui.slider_config("Rotation", 0.0, 90.0)
            .build(&mut self.ng.config.grid_rotation);
        ui.same_line();

        // Wire style
        let mut wire_idx = match self.ng.config.wire_style {
            WireStyle::Bezier => 0,
            WireStyle::Line => 1,
            WireStyle::Orthogonal => 2,
        };
        ui.set_next_item_width(110.0);
        if ui.combo_simple_string("Wire##style", &mut wire_idx, &["Bezier", "Straight", "Orthogonal"]) {
            self.ng.config.wire_style = match wire_idx {
                1 => WireStyle::Line,
                2 => WireStyle::Orthogonal,
                _ => WireStyle::Bezier,
            };
        }
    }

    fn render_add_menu(&mut self, ui: &Ui) {
        if let Some(_popup) = ui.begin_popup("##add_node") {
            ui.text_disabled("Add Node");
            ui.separator();

            // Figure out the position for the new node
            let pos = self
                .ctx_menu_pos
                .or(self.dropped_wire_out.map(|(_, p)| p))
                .or(self.dropped_wire_in.map(|(_, p)| p))
                .unwrap_or([0.0, 0.0]);

            let mut created: Option<NodeId> = None;

            if ui.menu_item("Float Value") {
                created = Some(
                    self.ng
                        .add_node(DemoNode::FloatValue { value: 0.0 }, pos),
                );
            }
            if ui.menu_item("Vec2 Value") {
                created = Some(
                    self.ng
                        .add_node(DemoNode::Vec2Value { x: 0.0, y: 0.0 }, pos),
                );
            }
            if ui.menu_item("Color Value") {
                created = Some(self.ng.add_node(
                    DemoNode::ColorValue {
                        color: [1.0, 1.0, 1.0],
                    },
                    pos,
                ));
            }
            ui.separator();
            if ui.menu_item("Add") {
                created = Some(self.ng.add_node(
                    DemoNode::MathOp {
                        op: MathOpKind::Add,
                    },
                    pos,
                ));
            }
            if ui.menu_item("Subtract") {
                created = Some(self.ng.add_node(
                    DemoNode::MathOp {
                        op: MathOpKind::Subtract,
                    },
                    pos,
                ));
            }
            if ui.menu_item("Multiply") {
                created = Some(self.ng.add_node(
                    DemoNode::MathOp {
                        op: MathOpKind::Multiply,
                    },
                    pos,
                ));
            }
            if ui.menu_item("Divide") {
                created = Some(self.ng.add_node(
                    DemoNode::MathOp {
                        op: MathOpKind::Divide,
                    },
                    pos,
                ));
            }
            ui.separator();
            if ui.menu_item("Clamp") {
                created = Some(self.ng.add_node(
                    DemoNode::Clamp {
                        min: 0.0,
                        max: 1.0,
                    },
                    pos,
                ));
            }
            if ui.menu_item("Mix") {
                created = Some(
                    self.ng.add_node(DemoNode::Mix { factor: 0.5 }, pos),
                );
            }
            ui.separator();
            if ui.menu_item("Output") {
                created = Some(self.ng.add_node(
                    DemoNode::Output { label: "Output".into() },
                    pos,
                ));
            }

            // Auto-connect if wire was dropped
            if let Some(new_id) = created {
                if let Some((out_pin, _)) = self.dropped_wire_out.take() {
                    // Connect dropped output → new node's first input
                    if let Some(node) = self.ng.graph.get_node(new_id) {
                        let inputs = self.viewer.inputs(&node.value);
                        if inputs > 0 {
                            self.ng.connect(out_pin, InPinId { node: new_id, input: 0 });
                        }
                    }
                } else if let Some((in_pin, _)) = self.dropped_wire_in.take() {
                    // Connect new node's first output → dropped input
                    if let Some(node) = self.ng.graph.get_node(new_id) {
                        let outputs = self.viewer.outputs(&node.value);
                        if outputs > 0 {
                            self.ng.connect(OutPinId { node: new_id, output: 0 }, in_pin);
                        }
                    }
                }
                self.ctx_menu_pos = None;
                ui.close_current_popup();
            }
        } else {
            // Popup closed without selection — clear pending state
            self.ctx_menu_pos = None;
            self.dropped_wire_out = None;
            self.dropped_wire_in = None;
        }
    }
}

// ─── wgpu + winit + imgui boilerplate ─────────────────────────────────────

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
                        .with_title("Node Graph Demo"),
                )
                .expect("window"),
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window.clone())
            .expect("surface");
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

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                gpu.platform
                    .prepare_frame(&gpu.window, &mut gpu.context);

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
                    let mut pass =
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("imgui_pass"),
                            color_attachments: &[Some(
                                wgpu::RenderPassColorAttachment {
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
                                },
                            )],
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
    style.set_color(
        StyleColor::ScrollbarGrabHovered,
        [0.30, 0.33, 0.40, 1.0],
    );
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

    style.set_color(StyleColor::Text, [0.92, 0.93, 0.95, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.42, 0.45, 0.52, 1.0]);
    style.set_color(
        StyleColor::TextSelectedBg,
        [accent[0], accent[1], accent[2], 0.30],
    );

    style.set_color(StyleColor::PlotHistogram, accent_hi);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run");
}
