//! Demo: CodeEditor — full feature showcase.
//!
//! Tests syntax highlighting, cursor positioning, keyboard navigation,
//! find/replace, themes, font zoom, auto-close brackets, multi-cursor, etc.
//!
//! Run: cargo run --example demo_code_editor

use dear_imgui_custom_mod::code_editor::{
    CodeEditor, EditorTheme, Language, LineMarker,
    CODE_EDITOR_FONT_PTR, MDI_FONT_DATA,
};
use dear_imgui_rs::{Condition, FontConfig, StyleColor, Ui};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

// ─── Sample code ────────────────────────────────────────────────────────────

const SAMPLE_RUST: &str = r##"use std::collections::HashMap;

/// A simple key-value store with optional expiration.
#[derive(Debug, Clone)]
pub struct Cache<V: Clone> {
    data: HashMap<String, (V, Option<std::time::Instant>)>,
    max_size: usize,
}

impl<V: Clone> Cache<V> {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: HashMap::with_capacity(max_size),
            max_size,
        }
    }

    /// Insert a value with optional TTL.
    pub fn insert(&mut self, key: impl Into<String>, value: V, ttl_secs: Option<u64>) {
        let expiry = ttl_secs.map(|s| {
            std::time::Instant::now() + std::time::Duration::from_secs(s)
        });
        if self.data.len() >= self.max_size {
            self.evict_expired();
        }
        self.data.insert(key.into(), (value, expiry));
    }

    /// Get a value by key (returns None if expired).
    pub fn get(&self, key: &str) -> Option<&V> {
        self.data.get(key).and_then(|(val, expiry)| {
            match expiry {
                Some(exp) if *exp < std::time::Instant::now() => None,
                _ => Some(val),
            }
        })
    }

    fn evict_expired(&mut self) {
        let now = std::time::Instant::now();
        self.data.retain(|_k, (_v, exp)| {
            exp.map_or(true, |e| e > now)
        });
    }

    pub fn len(&self) -> usize { self.data.len() }
    pub fn is_empty(&self) -> bool { self.data.is_empty() }
}

// region: Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut cache = Cache::new(100);
        cache.insert("hello", 42i32, None);
        assert_eq!(cache.get("hello"), Some(&42));
        assert_eq!(cache.get("missing"), None);
    }

    #[test]
    fn test_capacity() {
        let mut cache = Cache::new(2);
        cache.insert("a", 1, None);
        cache.insert("b", 2, None);
        cache.insert("c", 3, None); // triggers eviction
        assert!(cache.len() <= 3);
    }
}
// endregion

fn main() {
    let mut cache = Cache::new(1024);
    for i in 0..100 {
        let key = format!("item_{i}");
        let ttl = if i % 3 == 0 { Some(60) } else { None };
        cache.insert(key, i * 7, ttl);
    }
    println!("Cache size: {}", cache.len());

    // Color literals for swatch testing:
    let red   = 0xFFFF0000_u32;  // 0xAARRGGBB
    let green = "#00FF00";
    let blue  = "#0000FF";
    let _hex  = 0xFF8844;
    println!("Colors: {red} {green} {blue}");
}
"##;

const SAMPLE_TOML: &str = r#"[package]
name = "my-app"
version = "0.1.0"
edition = "2024"
description = "A demo application"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"

[dev-dependencies]
criterion = "0.5"

[[bin]]
name = "server"
path = "src/main.rs"

[profile.release]
opt-level = 3
lto = true
"#;

const SAMPLE_PLAIN: &str = r#"The quick brown fox jumps over the lazy dog.

Lorem ipsum dolor sit amet, consectetur adipiscing elit.
Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.

Tab	characters	should	align	correctly.
Mixed	tabs	and spaces   work too.

Unicode: Привет мир! 你好世界 🦀🔥
Special: → ← ↑ ↓ ≤ ≥ ≠ ∞ ± × ÷

Numbers: 42  3.14  0xFF  0b1010  1_000_000
Brackets: () {} [] <> (nested (brackets {work [fine]}))
"#;

const SAMPLE_HEX: &str = r#"// IPv4 packet header (20 bytes)
45 00 00 3C 1C 46 40 00 40 06 B1 E6 AC 10 0A 63
AC 10 0A 0C
// TCP header
00 50 CB 62 00 00 00 00 70 02 FF FF E2 1D 00 00
// Null and FF bytes
00 00 00 FF FF FF 00 FF
// Printable ASCII (Hello World!)
48 65 6C 6C 6F 20 57 6F 72 6C 64 21
// Control characters
01 02 03 7F 1B 0D 0A
// High bytes
80 90 A0 B0 C0 D0 E0 F0 FE
"#;

// ─── Font management ────────────────────────────────────────────────────────

/// A font loaded into the ImGui atlas, ready for hot-swapping.
struct LoadedFont {
    name: &'static str,
    /// Raw `*mut ImFont` pointer (stable for the atlas lifetime).
    ptr: *mut dear_imgui_rs::sys::ImFont,
}

// SAFETY: ImFont pointers are stable for the lifetime of the ImGui Context
// and are only accessed from the main (render) thread.
unsafe impl Send for LoadedFont {}
unsafe impl Sync for LoadedFont {}

/// System monospace font candidates: (display_name, path).
const SYSTEM_MONO_FONTS: &[(&str, &str)] = &[
    ("Consolas",       "C:\\Windows\\Fonts\\consola.ttf"),
    ("Cascadia Code",  "C:\\Windows\\Fonts\\CascadiaCode.ttf"),
    ("Cascadia Mono",  "C:\\Windows\\Fonts\\CascadiaMono.ttf"),
    ("Fira Code",      "C:\\Windows\\Fonts\\FiraCode-Regular.ttf"),
    ("Courier New",    "C:\\Windows\\Fonts\\cour.ttf"),
    ("Lucida Console", "C:\\Windows\\Fonts\\lucon.ttf"),
];

/// Load all available monospace fonts + the built-in JetBrains Mono into the atlas.
/// Returns the list of loaded fonts and the index of the default one.
fn load_all_fonts(ctx: &mut dear_imgui_rs::Context, size_pixels: f32) -> (Vec<LoadedFont>, usize) {
    let mut fonts = Vec::new();
    let mdi_glyph_ranges: &[u32] = &[0xF0000, 0xF1FFF, 0];

    // Helper: add a font from bytes, merge MDI icons, return ImFont ptr.
    let add_font = |ctx: &mut dear_imgui_rs::Context, name: &'static str, data: &[u8]| -> Option<*mut dear_imgui_rs::sys::ImFont> {
        let cfg = FontConfig::new()
            .size_pixels(size_pixels)
            .oversample_h(2)
            .name(name);
        let mut atlas = ctx.fonts();
        let f = atlas.add_font_from_memory_ttf(data, size_pixels, Some(&cfg), None)?;
        let ptr = f.raw();
        drop(atlas);
        // Merge MDI icons
        let mdi_cfg = FontConfig::new()
            .size_pixels(size_pixels)
            .merge_mode(true)
            .name("MDI");
        let mut atlas2 = ctx.fonts();
        atlas2.add_font_from_memory_ttf(MDI_FONT_DATA, size_pixels, Some(&mdi_cfg), Some(mdi_glyph_ranges));
        drop(atlas2);
        Some(ptr)
    };

    // 1. All built-in fonts (Hack, JetBrains Mono NL, JetBrains Mono)
    use dear_imgui_custom_mod::code_editor::BuiltinFont;
    for variant in BuiltinFont::ALL {
        if let Some(ptr) = add_font(ctx, variant.display_name(), variant.data()) {
            fonts.push(LoadedFont { name: variant.display_name(), ptr });
        }
    }

    // 3. System monospace fonts (skip if not found on disk)
    for &(name, path) in SYSTEM_MONO_FONTS {
        if !std::path::Path::new(path).exists() {
            continue;
        }
        let Ok(data) = std::fs::read(path) else { continue };
        let data: &'static [u8] = Box::leak(data.into_boxed_slice());
        if let Some(ptr) = add_font(ctx, name, data) {
            fonts.push(LoadedFont { name, ptr });
        }
    }

    // Default = first font (JetBrains Mono NL)
    let default_idx = 0;
    if let Some(f) = fonts.get(default_idx) {
        CODE_EDITOR_FONT_PTR.store(f.ptr as usize, Ordering::SeqCst);
    }

    (fonts, default_idx)
}

// ─── Demo state ─────────────────────────────────────────────────────────────

struct DemoState {
    editors: Vec<(String, CodeEditor)>,
    active_tab: usize,
    show_config_panel: bool,
    theme_idx: usize,
    lang_idx: usize,
    fonts: Vec<LoadedFont>,
    font_idx: usize,
}

impl DemoState {
    fn new(fonts: Vec<LoadedFont>, font_idx: usize) -> Self {
        // Rust editor
        let mut rust_editor = CodeEditor::new("rust_editor");
        rust_editor.set_language(Language::Rust);
        rust_editor.set_text(SAMPLE_RUST);
        rust_editor.set_error_markers(vec![
            LineMarker {
                line: 31,
                message: "warning: unused variable `_k`".into(),
                is_error: false,
            },
            LineMarker {
                line: 55,
                message: "error[E0599]: method `missing` not found".into(),
                is_error: true,
            },
        ]);

        // TOML editor
        let mut toml_editor = CodeEditor::new("toml_editor");
        toml_editor.set_language(Language::Toml);
        toml_editor.set_text(SAMPLE_TOML);

        // Plain text editor
        let mut plain_editor = CodeEditor::new("plain_editor");
        plain_editor.set_language(Language::None);
        plain_editor.set_text(SAMPLE_PLAIN);

        // Hex editor
        let mut hex_editor = CodeEditor::new("hex_editor");
        hex_editor.set_language(Language::Hex);
        hex_editor.set_text(SAMPLE_HEX);
        hex_editor.config_mut().hex_auto_space = true;
        hex_editor.config_mut().hex_auto_uppercase = true;

        // Empty editor for testing
        let mut empty_editor = CodeEditor::new("empty_editor");
        empty_editor.set_language(Language::Rust);
        empty_editor.set_text("// Start typing here...\nfn main() {\n    \n}\n");

        DemoState {
            editors: vec![
                ("Rust".into(), rust_editor),
                ("TOML".into(), toml_editor),
                ("Hex Bytes".into(), hex_editor),
                ("Plain Text".into(), plain_editor),
                ("Scratch Pad".into(), empty_editor),
            ],
            active_tab: 0,
            show_config_panel: true,
            theme_idx: 0,
            lang_idx: 0,
            fonts,
            font_idx,
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("CodeEditor Demo")
            .size([1200.0, 750.0], Condition::FirstUseEver)
            .build(|| {
                // ── Tab bar ─────────────────────────────────────────
                if let Some(_tab_bar) = ui.tab_bar("##editor_tabs") {
                    for (i, (name, editor)) in self.editors.iter().enumerate() {
                        let label = if editor.is_modified() {
                            format!("{name} *###tab{i}")
                        } else {
                            format!("{name}###tab{i}")
                        };
                        if let Some(_tab) = ui.tab_item(&label) {
                            self.active_tab = i;
                        }
                    }
                }

                ui.separator();

                // ── Toolbar ─────────────────────────────────────────
                let (_, editor) = &self.editors[self.active_tab];
                let cursor = editor.cursor();
                ui.text(format!(
                    "Ln {}, Col {}  |  {} lines  |  Scale {:.0}%",
                    cursor.line + 1,
                    cursor.col + 1,
                    editor.line_count(),
                    editor.text_scale() * 100.0,
                ));
                ui.same_line_with_pos(ui.content_region_avail()[0] - 220.0);

                if ui.button("Find (Ctrl+F)") {
                    self.editors[self.active_tab].1.open_find();
                }
                ui.same_line();
                ui.checkbox("Config", &mut self.show_config_panel);

                ui.spacing();

                // ── Layout: editor + optional config panel ──────────
                let avail = ui.content_region_avail();
                let config_w = if self.show_config_panel { 260.0 } else { 0.0 };
                let editor_w = avail[0] - config_w - if self.show_config_panel { 8.0 } else { 0.0 };

                // Editor column
                ui.child_window("##editor_col")
                    .size([editor_w, avail[1]])
                    .build(ui, || {
                        self.editors[self.active_tab].1.render(ui);
                    });

                // Config panel
                if self.show_config_panel {
                    ui.same_line();
                    ui.child_window("##config_panel")
                        .size([config_w, avail[1]])
                        .build(ui, || {
                            self.render_config_panel(ui);
                        });
                }
            });
    }

    fn render_config_panel(&mut self, ui: &Ui) {
        let editor = &mut self.editors[self.active_tab].1;
        let config = editor.config_mut();

        ui.text("Configuration");
        ui.separator();

        // Theme selector
        let theme_names: Vec<&str> = EditorTheme::ALL.iter()
            .map(|t| t.display_name())
            .collect();
        ui.set_next_item_width(-1.0);
        if ui.combo_simple_string("Theme", &mut self.theme_idx, &theme_names) {
            config.set_theme(EditorTheme::ALL[self.theme_idx]);
        }

        // Language selector — sync index from active editor's language
        self.lang_idx = match config.language {
            Language::Rust => 0,
            Language::Toml => 1,
            Language::Ron  => 2,
            Language::Hex  => 3,
            Language::None => 4,
            _ => 4,
        };
        let lang_names = ["Rust", "TOML", "RON", "Hex Bytes", "Plain Text"];
        ui.set_next_item_width(-1.0);
        if ui.combo_simple_string("Language", &mut self.lang_idx, &lang_names) {
            let lang = match self.lang_idx {
                0 => Language::Rust,
                1 => Language::Toml,
                2 => Language::Ron,
                3 => Language::Hex,
                _ => Language::None,
            };
            let _ = config;
            editor.set_language(lang);
            return;
        }

        ui.spacing();
        ui.separator();
        ui.text("Display");
        ui.checkbox("Line Numbers", &mut config.show_line_numbers);
        ui.checkbox("Highlight Line", &mut config.highlight_current_line);
        ui.checkbox("Bracket Match", &mut config.bracket_matching);
        ui.checkbox("Show Whitespace", &mut config.show_whitespace);
        ui.checkbox("Word Wrap", &mut config.word_wrap);
        ui.checkbox("Color Swatches", &mut config.show_color_swatches);
        ui.checkbox("Smooth Scroll", &mut config.smooth_scrolling);
        ui.checkbox("English on Focus", &mut config.force_english_on_focus);

        ui.spacing();
        ui.separator();
        ui.text("Editing");
        ui.checkbox("Read Only", &mut config.read_only);
        ui.checkbox("Auto Indent", &mut config.auto_indent);
        ui.checkbox("Auto Close ()", &mut config.auto_close_brackets);
        ui.checkbox("Auto Close \"\"", &mut config.auto_close_quotes);
        ui.checkbox("Insert Spaces", &mut config.insert_spaces);

        let mut tab_size = config.tab_size as i32;
        ui.set_next_item_width(80.0);
        if ui.slider_config("Tab Size", 1, 8).build(&mut tab_size) {
            config.tab_size = tab_size as u8;
        }

        let mut blink = config.cursor_blink_rate;
        ui.set_next_item_width(80.0);
        if ui.slider("Blink", 0.0, 2.0, &mut blink) {
            config.cursor_blink_rate = blink;
        }

        let mut scroll_spd = config.scroll_speed;
        ui.set_next_item_width(80.0);
        if ui.slider("Scroll Spd", 1.0, 10.0, &mut scroll_spd) {
            config.scroll_speed = scroll_spd;
        }

        ui.spacing();
        ui.separator();
        ui.text("Font");

        // Font selector combo
        let font_names: Vec<&str> = self.fonts.iter().map(|f| f.name).collect();
        ui.set_next_item_width(-1.0);
        if ui.combo_simple_string("##font", &mut self.font_idx, &font_names) {
            let ptr = self.fonts[self.font_idx].ptr;
            CODE_EDITOR_FONT_PTR.store(ptr as usize, Ordering::SeqCst);
        }

        let mut scale = config.font_size_scale;
        ui.set_next_item_width(-1.0);
        if ui.slider("Scale##font_scale", 0.4, 4.0, &mut scale) {
            config.font_size_scale = scale;
        }

        ui.spacing();
        ui.separator();
        ui.text("Hex Mode");
        ui.checkbox("Hex Auto Space", &mut config.hex_auto_space);
        ui.checkbox("Hex Uppercase", &mut config.hex_auto_uppercase);

        ui.spacing();
        ui.separator();

        // Undo/redo status
        let _ = config;
        let editor = &self.editors[self.active_tab].1;
        ui.text_disabled(format!(
            "Undo: {}  Redo: {}",
            if editor.can_undo() { "Yes" } else { "No" },
            if editor.can_redo() { "Yes" } else { "No" },
        ));

        if editor.is_modified() {
            ui.text_colored([0.9, 0.7, 0.2, 1.0], "Modified");
        } else {
            ui.text_disabled("Saved");
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
                        .with_inner_size(LogicalSize::new(1200.0, 750.0))
                        .with_title("CodeEditor Demo"),
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

        // Load all available monospace fonts (built-in + system) into the atlas.
        let (fonts, font_idx) = load_all_fonts(&mut context, font_size);

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
            demo: DemoState::new(fonts, font_idx),
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
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("imgui_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.06,
                                    g: 0.06,
                                    b: 0.08,
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
