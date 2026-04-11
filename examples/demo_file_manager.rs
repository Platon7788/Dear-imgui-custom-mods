//! Demo: FileManager v2 — full feature showcase.
//!
//! Run: cargo run --example demo_file_manager
//!
//! Features demonstrated:
//!   - Open Folder / Open File / Save File dialogs
//!   - Table view with Name, Size, Date, Type columns + sorting
//!   - Breadcrumb navigation, Back/Forward history
//!   - Favorites sidebar (Desktop, Documents, Downloads)
//!   - Keyboard navigation (arrows, Enter, Backspace, Escape)
//!   - Type-to-search, multi-select (Ctrl+Click)
//!   - File filters, overwrite confirmation
//!   - Drive selector (Windows)

use dear_imgui_custom_mod::file_manager::{FileFilter, FileManager, FileManagerConfig};
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

// ─── Demo state ─────────────────────────────────────────────────────────────

struct DemoState {
    fm: FileManager,
    last_result: String,
}

impl DemoState {
    fn new() -> Self {
        let config = FileManagerConfig {
            enable_multi_select: true,
            ..Default::default()
        };
        Self {
            fm: FileManager::new_with_config(config),
            last_result: String::new(),
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("FileManager v2 Demo")
            .size([600.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("FileManager v2 — Production-Ready File Dialog");
                ui.separator();
                ui.spacing();

                // ── Open buttons ──
                if ui.button_with_size("Open Folder", [140.0, 30.0]) {
                    self.fm.open_folder(None);
                }
                ui.same_line();
                if ui.button_with_size("Open File", [140.0, 30.0]) {
                    self.fm.open_file(
                        None,
                        vec![
                            FileFilter::new("Rust (*.rs)", &["rs"]),
                            FileFilter::new("TOML (*.toml)", &["toml"]),
                            FileFilter::new("Text (*.txt, *.md)", &["txt", "md"]),
                            FileFilter::all(),
                        ],
                    );
                }
                ui.same_line();
                if ui.button_with_size("Save File", [140.0, 30.0]) {
                    self.fm.save_file(
                        None,
                        "untitled.rs",
                        vec![
                            FileFilter::new("Rust (*.rs)", &["rs"]),
                            FileFilter::all(),
                        ],
                    );
                }

                ui.spacing();
                ui.separator();
                ui.spacing();

                // ── Result display ──
                if !self.last_result.is_empty() {
                    ui.text_colored([0.3, 0.85, 0.45, 1.0], &self.last_result);
                } else {
                    ui.text_disabled("No file selected yet. Click a button above.");
                }
            });

        // Render the modal dialog
        if self.fm.render(ui) {
            // Selection confirmed
            self.last_result.clear();
            let paths = self.fm.selected_paths();
            if !paths.is_empty() {
                for (i, p) in paths.iter().enumerate() {
                    if i > 0 {
                        self.last_result.push('\n');
                    }
                    self.last_result
                        .push_str(&format!("Selected: {}", p.display()));
                }
            } else if let Some(ref p) = self.fm.selected_path {
                self.last_result = format!("Selected: {}", p.display());
            }
        }
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
                        .with_inner_size(LogicalSize::new(700.0, 400.0))
                        .with_title("FileManager v2 Demo"),
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

        // Load Segoe UI (Windows) or default font
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

        // Load Material Design Icons font (merge mode)
        // Place materialdesignicons-webfont.ttf in assets/ to see icons.
        // Without it the dialog is fully functional, icons just show as boxes.
        let mdi_paths = [
            "assets/materialdesignicons-webfont.ttf",
            "fonts/materialdesignicons-webfont.ttf",
            "resources/materialdesignicons-webfont.ttf",
        ];
        for mdi_path in &mdi_paths {
            if std::path::Path::new(mdi_path).exists() {
                let mdi_cfg = dear_imgui_rs::FontConfig::new()
                    .size_pixels(font_size)
                    .merge_mode(true);
                // MDI codepoints live in U+F0000..U+F1FFF (Private Use Area)
                let glyph_ranges: &[u32] = &[0xF0000, 0xF1FFF, 0];
                context.fonts().add_font_from_file_ttf(
                    mdi_path,
                    font_size,
                    Some(&mdi_cfg),
                    Some(glyph_ranges),
                );
                break;
            }
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

    style.set_color(StyleColor::TableHeaderBg, [0.13, 0.14, 0.18, 1.0]);
    style.set_color(StyleColor::TableBorderStrong, [0.20, 0.22, 0.27, 0.80]);
    style.set_color(StyleColor::TableBorderLight, [0.16, 0.18, 0.22, 0.60]);
    style.set_color(StyleColor::TableRowBg, [0.00, 0.00, 0.00, 0.00]);
    style.set_color(StyleColor::TableRowBgAlt, [1.0, 1.0, 1.0, 0.025]);

    style.set_color(
        StyleColor::TextSelectedBg,
        [accent[0], accent[1], accent[2], 0.30],
    );
    style.set_color(StyleColor::Text, [0.92, 0.93, 0.95, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.42, 0.45, 0.52, 1.0]);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run");
}
