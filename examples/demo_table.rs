//! Demo: VirtualTable v2 — full feature showcase.
//!
//! Columns:
//!   # (fixed, centered, read-only), Active (checkbox), Name (text input),
//!   Category (combo), Value (spin float), Progress (progress bar),
//!   Color (color edit), Status (combo, colored text), Actions (button)
//!
//! Run: cargo run --example demo_table

use dear_imgui_custom_mod::virtual_table::{
    CellAlignment, CellEditor, CellStyle, CellValue, ColumnDef, EditTrigger, RowDensity, RowStyle,
    SelectionMode, TableConfig, VirtualTable, VirtualTableRow,
};
use dear_imgui_rs::{Condition, StyleColor, Ui};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::cmp::Ordering;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

// ─── Test row ───────────────────────────────────────────────────────────────

const CATEGORIES: &[&str] = &["Network", "Storage", "Compute", "Memory", "Security"];
const STATUSES: &[&str] = &["Active", "Pending", "Inactive", "Critical", "Complete"];

struct TestRow {
    id: usize,
    active: bool,
    name: String,
    category: usize,  // index into CATEGORIES
    value: f32,
    progress: f32,     // 0.0..1.0
    color: [f32; 4],
    status: usize,     // index into STATUSES
}

impl VirtualTableRow for TestRow {
    fn cell_value(&self, col: usize) -> CellValue {
        match col {
            0 => CellValue::Int(self.id as i64),
            1 => CellValue::Bool(self.active),
            2 => CellValue::Text(self.name.clone()),
            3 => CellValue::Choice(self.category),
            4 => CellValue::Float(self.value as f64),
            5 => CellValue::Progress(self.progress),
            6 => CellValue::Color(self.color),
            7 => CellValue::Choice(self.status),
            8 => CellValue::Custom, // button
            _ => CellValue::Text(String::new()),
        }
    }

    fn set_cell_value(&mut self, col: usize, value: &CellValue) {
        match col {
            1 => {
                if let CellValue::Bool(b) = value {
                    self.active = *b;
                }
            }
            2 => {
                if let CellValue::Text(s) = value {
                    self.name = s.clone();
                }
            }
            3 => {
                if let CellValue::Choice(idx) = value {
                    self.category = *idx;
                }
            }
            4 => {
                if let CellValue::Float(v) = value {
                    self.value = *v as f32;
                }
            }
            6 => {
                if let CellValue::Color(c) = value {
                    self.color = *c;
                }
            }
            7 => {
                if let CellValue::Choice(idx) = value {
                    self.status = *idx;
                }
            }
            _ => {}
        }
    }

    fn cell_display_text(&self, col: usize, buf: &mut String) {
        match col {
            0 => {
                use std::fmt::Write;
                let _ = write!(buf, "{}", self.id);
            }
            1 => buf.push_str(if self.active { "Yes" } else { "No" }),
            2 => buf.push_str(&self.name),
            3 => buf.push_str(CATEGORIES.get(self.category).unwrap_or(&"?")),
            4 => {
                use std::fmt::Write;
                let _ = write!(buf, "{:.2}", self.value);
            }
            5 => {
                use std::fmt::Write;
                let _ = write!(buf, "{:.0}%", self.progress * 100.0);
            }
            6 => {
                use std::fmt::Write;
                let c = self.color;
                let _ = write!(
                    buf,
                    "#{:02X}{:02X}{:02X}",
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8,
                );
            }
            7 => buf.push_str(STATUSES.get(self.status).unwrap_or(&"?")),
            _ => {}
        }
    }

    fn row_style(&self) -> Option<RowStyle> {
        if self.status == 3 {
            // Critical
            Some(RowStyle {
                bg_color: Some([0.35, 0.12, 0.12, 1.0]),
                text_color: Some([1.0, 0.7, 0.7, 1.0]),
                ..Default::default()
            })
        } else {
            None
        }
    }

    fn cell_style(&self, col: usize) -> Option<CellStyle> {
        if col == 7 {
            let color = match self.status {
                0 => [0.3, 0.85, 0.45, 1.0],  // Active — green
                1 => [0.9, 0.7, 0.2, 1.0],    // Pending — yellow
                2 => [0.5, 0.5, 0.5, 1.0],    // Inactive — gray
                3 => [1.0, 0.35, 0.35, 1.0],  // Critical — red
                4 => [0.3, 0.7, 1.0, 1.0],    // Complete — blue
                _ => return None,
            };
            Some(CellStyle {
                text_color: Some(color),
                alignment: Some(CellAlignment::Center),
                ..Default::default()
            })
        } else {
            None
        }
    }

    fn compare(&self, other: &Self, col: usize) -> Ordering {
        match col {
            0 => self.id.cmp(&other.id),
            1 => self.active.cmp(&other.active),
            2 => self.name.cmp(&other.name),
            3 => self.category.cmp(&other.category),
            4 => self.value.partial_cmp(&other.value).unwrap_or(Ordering::Equal),
            5 => self
                .progress
                .partial_cmp(&other.progress)
                .unwrap_or(Ordering::Equal),
            7 => self.status.cmp(&other.status),
            _ => Ordering::Equal,
        }
    }
}

// ─── Demo state ─────────────────────────────────────────────────────────────

struct DemoState {
    table: VirtualTable<TestRow>,
    next_id: usize,
    selection_mode_idx: usize,
    density_idx: usize,
    pending_delete: Option<usize>,
}

impl DemoState {
    fn new() -> Self {
        let config = TableConfig {
            resizable: true,
            reorderable: true,
            hideable: true,
            sortable: true,
            edit_trigger: EditTrigger::DoubleClick,
            selection_mode: SelectionMode::Single,
            ..Default::default()
        };

        let columns = vec![
            ColumnDef::new("#")
                .fixed(50.0)
                .align(CellAlignment::Center)
                .no_resize()
                .no_sort(),
            ColumnDef::new("Active")
                .fixed(60.0)
                .align(CellAlignment::Center)
                .editor(CellEditor::Checkbox),
            ColumnDef::new("Name")
                .stretch(1.0)
                .editor(CellEditor::TextInput),
            ColumnDef::new("Category")
                .fixed(120.0)
                .editor(CellEditor::ComboBox {
                    items: CATEGORIES.iter().map(|s| s.to_string()).collect(),
                }),
            ColumnDef::new("Value")
                .fixed(100.0)
                .align(CellAlignment::Right)
                .editor(CellEditor::SpinFloat {
                    step: 0.5,
                    step_fast: 5.0,
                }),
            ColumnDef::new("Progress")
                .fixed(130.0)
                .editor(CellEditor::ProgressBar),
            ColumnDef::new("Color")
                .fixed(80.0)
                .editor(CellEditor::ColorEdit),
            ColumnDef::new("Status")
                .fixed(100.0)
                .align(CellAlignment::Center)
                .editor(CellEditor::ComboBox {
                    items: STATUSES.iter().map(|s| s.to_string()).collect(),
                }),
            ColumnDef::new("Actions")
                .fixed(80.0)
                .align(CellAlignment::Center)
                .no_sort()
                .no_resize()
                .editor(CellEditor::Button {
                    label: "Delete".to_string(),
                }),
        ];

        let mut state = DemoState {
            table: VirtualTable::new("##demo_vtable", columns, 50_000, config),
            next_id: 0,
            selection_mode_idx: 1,
            density_idx: 0,
            pending_delete: None,
        };

        // Populate with 1000 initial rows
        state.add_rows(1000);
        state
    }

    fn add_rows(&mut self, count: usize) {
        let names = [
            "Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel", "India",
            "Juliet", "Kilo", "Lima",
        ];
        for _ in 0..count {
            self.next_id += 1;
            let id = self.next_id;
            self.table.push(TestRow {
                id,
                active: !id.is_multiple_of(3),
                name: format!("{}_{:04}", names[id % names.len()], id),
                category: id % CATEGORIES.len(),
                value: ((id as f32 * 7.31) % 100.0 * 100.0).round() / 100.0,
                progress: (id as f32 * 0.031) % 1.0,
                color: [
                    ((id * 37) % 255) as f32 / 255.0,
                    ((id * 73) % 255) as f32 / 255.0,
                    ((id * 113) % 255) as f32 / 255.0,
                    1.0,
                ],
                status: id % STATUSES.len(),
            });
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("VirtualTable v2 Demo")
            .size([1100.0, 680.0], Condition::FirstUseEver)
            .build(|| {
                // ── Toolbar ─────────────────────────────────────────
                ui.text(format!("Rows: {}", self.table.len()));
                ui.same_line();
                ui.checkbox("Auto-scroll", &mut self.table.config.auto_scroll);
                ui.same_line();
                if ui.button("+ Add 100") {
                    self.add_rows(100);
                }
                ui.same_line();
                if ui.button("Clear") {
                    self.table.clear();
                    self.next_id = 0;
                }
                ui.same_line();
                ui.separator();
                ui.same_line();
                ui.checkbox("Resizable", &mut self.table.config.resizable);
                ui.same_line();
                ui.checkbox("Sortable", &mut self.table.config.sortable);
                ui.same_line();
                ui.checkbox("Reorderable", &mut self.table.config.reorderable);
                ui.same_line();
                ui.checkbox("H-Lines", &mut self.table.config.show_row_lines);
                ui.same_line();
                ui.checkbox("V-Lines", &mut self.table.config.show_column_lines);
                ui.same_line();
                let densities = ["Normal", "Compact", "Dense"];
                ui.set_next_item_width(90.0);
                if ui.combo_simple_string("Density", &mut self.density_idx, &densities) {
                    self.table.config.row_density = match self.density_idx {
                        1 => RowDensity::Compact,
                        2 => RowDensity::Dense,
                        _ => RowDensity::Normal,
                    };
                }
                ui.same_line();

                let modes = ["None", "Single", "Multi"];
                ui.set_next_item_width(90.0);
                if ui.combo_simple_string("Selection", &mut self.selection_mode_idx, &modes) {
                    self.table.config.selection_mode = match self.selection_mode_idx {
                        0 => SelectionMode::None,
                        1 => SelectionMode::Single,
                        _ => SelectionMode::Multi,
                    };
                }

                // Selected info
                if let Some(sel) = self.table.selected_row() {
                    ui.same_line();
                    ui.text_colored([0.5, 0.8, 1.0, 1.0], format!("Selected: #{sel}"));
                }

                ui.spacing();

                // ── Table ───────────────────────────────────────────
                self.table.render(ui);

                // Handle delete button clicks
                if let Some((row_idx, _col)) = self.table.button_clicked {
                    self.pending_delete = Some(row_idx);
                }

                // Confirmation popup
                if let Some(row_idx) = self.pending_delete {
                    ui.open_popup("Confirm Delete");
                    if let Some(_popup) = ui.begin_modal_popup_config("Confirm Delete")
                        .flags(dear_imgui_rs::WindowFlags::ALWAYS_AUTO_RESIZE)
                        .begin()
                    {
                        ui.text(format!("Delete row #{row_idx}?"));
                        ui.spacing();
                        if ui.button("Yes") {
                            self.table.remove(row_idx);
                            self.pending_delete = None;
                            ui.close_current_popup();
                        }
                        ui.same_line();
                        if ui.button("Cancel") {
                            self.pending_delete = None;
                            ui.close_current_popup();
                        }
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
                        .with_inner_size(LogicalSize::new(1100.0, 700.0))
                        .with_title("VirtualTable v2 Demo"),
                )
                .expect("window"),
        );

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
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

        // Load a font with full Unicode support (Cyrillic, CJK, etc.)
        // Try Segoe UI (Windows) first, fallback to default
        let segoe_path = "C:\\Windows\\Fonts\\segoeui.ttf";
        if std::path::Path::new(segoe_path).exists() {
            let font_data = std::fs::read(segoe_path).expect("read font");
            // Leak the data so it lives for 'static — acceptable for a demo
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(gpu) = self.gpu.as_ref() {
            gpu.window.request_redraw();
        }
    }
}

fn apply_dark_theme(style: &mut dear_imgui_rs::Style) {
    // Geometry
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

    // Accent: soft blue
    let accent = [0.40, 0.63, 0.88, 1.0];
    let accent_dim = [0.30, 0.50, 0.75, 1.0];
    let accent_hi = [0.50, 0.73, 0.95, 1.0];

    // Background palette
    style.set_color(StyleColor::WindowBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::ChildBg, [0.10, 0.10, 0.13, 1.0]);
    style.set_color(StyleColor::PopupBg, [0.11, 0.12, 0.15, 0.96]);
    style.set_color(StyleColor::Border, [0.20, 0.22, 0.27, 0.70]);

    // Frames (inputs, combos, sliders)
    style.set_color(StyleColor::FrameBg, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::FrameBgHovered, [0.19, 0.20, 0.26, 1.0]);
    style.set_color(StyleColor::FrameBgActive, [0.24, 0.26, 0.33, 1.0]);

    // Title bar
    style.set_color(StyleColor::TitleBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::TitleBgActive, [0.12, 0.13, 0.17, 1.0]);

    // Scrollbar
    style.set_color(StyleColor::ScrollbarBg, [0.08, 0.08, 0.10, 0.60]);
    style.set_color(StyleColor::ScrollbarGrab, [0.22, 0.24, 0.30, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabHovered, [0.30, 0.33, 0.40, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabActive, accent_dim);

    // Widgets
    style.set_color(StyleColor::CheckMark, accent);
    style.set_color(StyleColor::SliderGrab, accent_dim);
    style.set_color(StyleColor::SliderGrabActive, accent);
    style.set_color(StyleColor::Button, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::ButtonHovered, [0.26, 0.29, 0.36, 1.0]);
    style.set_color(StyleColor::ButtonActive, accent_dim);

    // Headers (table headers, collapsing headers)
    style.set_color(StyleColor::Header, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::HeaderHovered, [0.24, 0.27, 0.34, 1.0]);
    style.set_color(StyleColor::HeaderActive, accent_dim);

    // Separator
    style.set_color(StyleColor::Separator, [0.20, 0.22, 0.27, 0.60]);

    // Tabs
    style.set_color(StyleColor::Tab, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::TabHovered, accent_dim);
    style.set_color(StyleColor::TabSelected, [0.22, 0.24, 0.30, 1.0]);

    // Table
    style.set_color(StyleColor::TableHeaderBg, [0.13, 0.14, 0.18, 1.0]);
    style.set_color(StyleColor::TableBorderStrong, [0.20, 0.22, 0.27, 0.80]);
    style.set_color(StyleColor::TableBorderLight, [0.16, 0.18, 0.22, 0.60]);
    style.set_color(StyleColor::TableRowBg, [0.00, 0.00, 0.00, 0.00]);
    style.set_color(StyleColor::TableRowBgAlt, [1.0, 1.0, 1.0, 0.025]);

    // Selection & text
    style.set_color(StyleColor::TextSelectedBg, [accent[0], accent[1], accent[2], 0.30]);
    style.set_color(StyleColor::Text, [0.92, 0.93, 0.95, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.42, 0.45, 0.52, 1.0]);

    // Progress bar
    style.set_color(StyleColor::PlotHistogram, accent_hi);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run");
}

// ─── Tests ──────────────────────────────────────────────────────────────────
//
// Run:  cargo test --example demo_table
// Stress: cargo test --example demo_table -- --ignored --nocapture

#[cfg(test)]
mod tests {
    use dear_imgui_custom_mod::virtual_table::ring_buffer::{RingBuffer, MAX_TABLE_ROWS};

    // ── RingBuffer unit tests ────────────────────────────────────────

    #[test]
    fn ring_empty() {
        let ring: RingBuffer<i32> = RingBuffer::new(4);
        assert_eq!(ring.len(), 0);
        assert!(ring.is_empty());
        assert!(ring.get(0).is_none());
    }

    #[test]
    fn ring_push_no_wrap() {
        let mut ring = RingBuffer::new(4);
        ring.push(10);
        ring.push(20);
        ring.push(30);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.get(0), Some(&10));
        assert_eq!(ring.get(1), Some(&20));
        assert_eq!(ring.get(2), Some(&30));
        assert!(ring.get(3).is_none());
    }

    #[test]
    fn ring_push_wraps_around() {
        let mut ring = RingBuffer::new(3);
        ring.push(1);
        ring.push(2);
        ring.push(3);
        ring.push(4);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.get(0), Some(&2));
        assert_eq!(ring.get(1), Some(&3));
        assert_eq!(ring.get(2), Some(&4));

        ring.push(5);
        assert_eq!(ring.get(0), Some(&3));
        assert_eq!(ring.get(1), Some(&4));
        assert_eq!(ring.get(2), Some(&5));
    }

    #[test]
    fn ring_clear() {
        let mut ring = RingBuffer::new(4);
        ring.push(String::from("hello"));
        ring.push(String::from("world"));
        assert_eq!(ring.len(), 2);
        ring.clear();
        assert_eq!(ring.len(), 0);
        assert!(ring.is_empty());
        ring.push(String::from("again"));
        assert_eq!(ring.get(0).map(|s| s.as_str()), Some("again"));
    }

    #[test]
    fn ring_capacity_one() {
        let mut ring = RingBuffer::new(1);
        ring.push(42);
        assert_eq!(ring.get(0), Some(&42));
        ring.push(99);
        assert_eq!(ring.get(0), Some(&99));
    }

    #[test]
    fn ring_get_mut() {
        let mut ring = RingBuffer::new(4);
        ring.push(10);
        ring.push(20);
        *ring.get_mut(1).unwrap() = 999;
        assert_eq!(ring.get(1), Some(&999));
    }

    #[test]
    fn ring_sort() {
        let mut ring = RingBuffer::new(5);
        ring.push(30);
        ring.push(10);
        ring.push(50);
        ring.push(20);
        ring.push(40);
        ring.sort_by(|a, b| a.cmp(b));
        let vals: Vec<_> = ring.iter().copied().collect();
        assert_eq!(vals, vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn ring_sort_wrapped() {
        let mut ring = RingBuffer::new(3);
        ring.push(1);
        ring.push(2);
        ring.push(3);
        ring.push(4); // wraps: [4, 2, 3] physical, logical [2, 3, 4]
        ring.push(5); // wraps: [4, 5, 3] physical, logical [3, 4, 5]
        ring.sort_by(|a, b| b.cmp(a)); // reverse
        let vals: Vec<_> = ring.iter().copied().collect();
        assert_eq!(vals, vec![5, 4, 3]);
    }

    #[test]
    fn ring_iter() {
        let mut ring = RingBuffer::new(3);
        ring.push(1);
        ring.push(2);
        ring.push(3);
        ring.push(4);
        let vals: Vec<_> = ring.iter().copied().collect();
        assert_eq!(vals, vec![2, 3, 4]);
    }

    #[test]
    fn ring_iter_mut() {
        let mut ring = RingBuffer::new(3);
        ring.push(1);
        ring.push(2);
        ring.push(3);
        for v in ring.iter_mut() {
            *v *= 10;
        }
        let vals: Vec<_> = ring.iter().copied().collect();
        assert_eq!(vals, vec![10, 20, 30]);
    }

    // ── RingBuffer stress test ───────────────────────────────────────

    #[test]
    #[ignore]
    fn stress_test_ring_buffer() {
        use std::time::Instant;

        fn elapsed_ms(start: Instant) -> f64 {
            start.elapsed().as_secs_f64() * 1000.0
        }

        let sep = "=".repeat(60);
        println!("\n{sep}");
        println!("  STRESS TEST: RingBuffer");
        println!("{sep}\n");

        for &size in &[500_000usize, 1_000_000] {
            assert!(size <= MAX_TABLE_ROWS, "test size exceeds MAX_TABLE_ROWS");
            let mut rb = RingBuffer::<u64>::new(size);

            // Bulk push
            let t = Instant::now();
            for i in 0..size as u64 {
                rb.push(i);
            }
            let push_ms = elapsed_ms(t);
            println!("[RING PUSH {size}]  {push_ms:.1} ms ({:.0} ns/op)", push_ms * 1_000_000.0 / size as f64);

            // Random access
            let t = Instant::now();
            let mut sum = 0u64;
            for i in (0..rb.len()).step_by(10) {
                sum += rb.get(i).copied().unwrap_or(0);
            }
            let access_ms = elapsed_ms(t);
            let ops = rb.len() / 10;
            println!("[RING ACCESS {size}]  {ops} ops in {access_ms:.3} ms (sum={sum})");

            // Sort
            let t = Instant::now();
            rb.sort_by(|a, b| b.cmp(a));
            let sort_ms = elapsed_ms(t);
            println!("[RING SORT {size}]  {sort_ms:.1} ms");

            println!();
        }
    }
}
