//! Demo: Borderless Window — production-quality borderless window.
//!
//! Features demonstrated:
//!   • True borderless `with_decorations(false)` winit window
//!   • 5 built-in themes selectable live (Dark/Light/Midnight/Solarized/Monokai)
//!   • Small icon-only hover highlights on buttons (xDx-style)
//!   • Primitive draw-list button icons — no extra font needed
//!   • Close confirmation dialog
//!   • Resize cursor icons on all 8 edges/corners
//!   • DWM dark-mode shadow on Windows (no startup white flash)
//!   • Content area starts exactly at titlebar_height — nothing hidden behind titlebar
//!   • Extra titlebar buttons (theme picker, about)
//!
//! Run: cargo run --example demo_borderless

use dear_imgui_custom_mod::borderless_window::{
    BorderlessConfig, ButtonConfig, CloseMode, ExtraButton,
    TitleAlign, TitlebarState, TitlebarTheme, WindowAction,
    actions::ResizeEdge, render_titlebar,
};
use dear_imgui_custom_mod::confirm_dialog::{DialogConfig, DialogIcon, DialogResult, DialogTheme, render_confirm_dialog};
use dear_imgui_rs::{Condition, StyleColor, StyleVar, Ui, WindowFlags};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{CursorIcon, ResizeDirection, Window},
};

// ─── winit helpers ────────────────────────────────────────────────────────────

fn to_winit_resize(edge: ResizeEdge) -> ResizeDirection {
    match edge {
        ResizeEdge::North     => ResizeDirection::North,
        ResizeEdge::South     => ResizeDirection::South,
        ResizeEdge::East      => ResizeDirection::East,
        ResizeEdge::West      => ResizeDirection::West,
        ResizeEdge::NorthEast => ResizeDirection::NorthEast,
        ResizeEdge::NorthWest => ResizeDirection::NorthWest,
        ResizeEdge::SouthEast => ResizeDirection::SouthEast,
        ResizeEdge::SouthWest => ResizeDirection::SouthWest,
    }
}

fn cursor_for_edge(edge: Option<ResizeEdge>) -> CursorIcon {
    match edge {
        None                        => CursorIcon::Default,
        Some(ResizeEdge::North)     => CursorIcon::NResize,
        Some(ResizeEdge::South)     => CursorIcon::SResize,
        Some(ResizeEdge::East)      => CursorIcon::EResize,
        Some(ResizeEdge::West)      => CursorIcon::WResize,
        Some(ResizeEdge::NorthEast) => CursorIcon::NeResize,
        Some(ResizeEdge::NorthWest) => CursorIcon::NwResize,
        Some(ResizeEdge::SouthEast) => CursorIcon::SeResize,
        Some(ResizeEdge::SouthWest) => CursorIcon::SwResize,
    }
}

#[cfg(windows)]
fn hwnd_of(window: &Window) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(h) = window.window_handle()
        && let RawWindowHandle::Win32(w) = h.as_raw()
    {
        return Some(w.hwnd.get());
    }
    None
}

// ─── Demo state ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTheme {
    Dark, Light, Midnight, Solarized, Monokai,
}

impl ActiveTheme {
    fn label(self) -> &'static str {
        match self {
            Self::Dark      => "Dark",
            Self::Light     => "Light",
            Self::Midnight  => "Midnight",
            Self::Solarized => "Solarized",
            Self::Monokai   => "Monokai",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Dark      => Self::Light,
            Self::Light     => Self::Midnight,
            Self::Midnight  => Self::Solarized,
            Self::Solarized => Self::Monokai,
            Self::Monokai   => Self::Dark,
        }
    }
    fn titlebar_theme(self) -> TitlebarTheme {
        match self {
            Self::Dark      => TitlebarTheme::Dark,
            Self::Light     => TitlebarTheme::Light,
            Self::Midnight  => TitlebarTheme::Midnight,
            Self::Solarized => TitlebarTheme::Solarized,
            Self::Monokai   => TitlebarTheme::Monokai,
        }
    }
    fn dialog_theme(self) -> DialogTheme {
        match self {
            Self::Dark      => DialogTheme::Dark,
            Self::Light     => DialogTheme::Light,
            Self::Midnight  => DialogTheme::Midnight,
            Self::Solarized => DialogTheme::Solarized,
            Self::Monokai   => DialogTheme::Monokai,
        }
    }
}

struct DemoState {
    cfg:   BorderlessConfig,
    state: TitlebarState,
    theme: ActiveTheme,
    log:   Vec<String>,
    show_confirm: bool,
    // config panel
    edit_h:     f32,
    edit_rz:    f32,
    edit_center: bool,
    edit_confirm: bool,
    edit_separator: bool,
    edit_drag_hint: bool,
}

impl DemoState {
    fn new() -> Self {
        let cfg = Self::build_cfg(ActiveTheme::Dark, false, false, true, true);
        Self {
            edit_h:         cfg.titlebar_height,
            edit_rz:        cfg.resize_zone,
            edit_center:    false,
            edit_confirm:   false,
            edit_separator: true,
            edit_drag_hint: true,
            cfg,
            state:         TitlebarState::new(),
            theme:         ActiveTheme::Dark,
            log:           vec!["Borderless window demo started.".into()],
            show_confirm:  false,
        }
    }

    fn build_cfg(theme: ActiveTheme, center: bool, confirm: bool, separator: bool, drag_hint: bool) -> BorderlessConfig {
        let mut cfg = BorderlessConfig::new("Borderless Window Demo")
            .with_theme(theme.titlebar_theme())
            .with_title_align(if center { TitleAlign::Center } else { TitleAlign::Left })
            .with_close_mode(if confirm { CloseMode::Confirm } else { CloseMode::Immediate })
            .with_icon("\u{25A0}")   // ■
            .with_buttons(
                ButtonConfig::default()
                    .add_extra(
                        ExtraButton::new("cycle_theme", "\u{25D0}", [0.80, 0.80, 0.50, 1.0])
                            .with_tooltip("Cycle to next theme"),
                    )
            );
        if !separator { cfg = cfg.without_separator(); }
        if !drag_hint { cfg = cfg.without_drag_hint(); }
        cfg
    }

    fn apply_theme(&mut self, t: ActiveTheme) {
        self.theme = t;
        self.rebuild_cfg();
        self.log.push(format!("Theme → {}", t.label()));
    }

    fn rebuild_cfg(&mut self) {
        self.cfg = Self::build_cfg(self.theme, self.edit_center, self.edit_confirm, self.edit_separator, self.edit_drag_hint);
        self.cfg.titlebar_height = self.edit_h;
        self.cfg.resize_zone     = self.edit_rz;
    }

    /// Returns `(action, hover_edge)` — action for winit dispatch,
    /// hover_edge for per-frame cursor update.
    fn render(&mut self, ui: &Ui) -> (WindowAction, Option<ResizeEdge>) {
        let display = ui.io().display_size();
        let _pad    = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _sp     = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));

        let mut action        = WindowAction::None;
        let mut hover_edge:   Option<ResizeEdge>   = None;
        let mut pending_theme: Option<ActiveTheme> = None;

        ui.window("##root")
            .size(display, Condition::Always)
            .position([0.0, 0.0], Condition::Always)
            .flags(
                WindowFlags::NO_TITLE_BAR
                    | WindowFlags::NO_RESIZE
                    | WindowFlags::NO_MOVE
                    | WindowFlags::NO_SCROLLBAR
                    | WindowFlags::NO_SCROLL_WITH_MOUSE
                    | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
                    | WindowFlags::NO_NAV_FOCUS,
            )
            .build(|| {
                // ── Titlebar (must be first) ────────────────────────────────
                let res = render_titlebar(ui, &self.cfg, &mut self.state);
                hover_edge = res.hover_edge;

                match res.action {
                    WindowAction::Extra("cycle_theme") => {
                        // Defer theme change — mutating self.cfg inside the
                        // imgui build closure can destabilise frame state.
                        pending_theme = Some(self.theme.next());
                    }
                    WindowAction::CloseRequested => {
                        self.show_confirm = true;
                    }
                    WindowAction::IconClick => {
                        self.log.push("Icon clicked".into());
                    }
                    other => action = other,
                }

                // ── Close confirmation dialog ────────────────────────────────
                if self.show_confirm {
                    let dlg_cfg = DialogConfig::new(
                            "Close Application",
                            "Are you sure you want to close?",
                        )
                        .with_icon(DialogIcon::Warning)
                        .with_confirm_label("Close")
                        .with_cancel_label("Cancel")
                        .with_theme(self.theme.dialog_theme());

                    match render_confirm_dialog(ui, &dlg_cfg, &mut self.show_confirm) {
                        DialogResult::Confirmed => self.state.confirm_close(),
                        DialogResult::Cancelled => self.state.cancel_close(),
                        DialogResult::Open      => {}
                    }
                }

                // ── Content (cursor already past titlebar) ──────────────────
                let _inner_p = ui.push_style_var(StyleVar::WindowPadding([8.0, 8.0]));
                let _inner_s = ui.push_style_var(StyleVar::ItemSpacing([6.0, 4.0]));

                let avail = ui.content_region_avail();
                let panel_w = 280.0;

                ui.child_window("##cfg_panel")
                    .size([panel_w, avail[1]])
                    .build(ui, || {
                        if let Some(t) = self.render_config_panel(ui) {
                            pending_theme = Some(t);
                        }
                    });

                ui.same_line();

                ui.child_window("##log_panel")
                    .size([avail[0] - panel_w - 1.0, avail[1]])
                    .build(ui, || self.render_log(ui));
            });

        // Apply deferred theme change AFTER the imgui frame is fully built.
        if let Some(t) = pending_theme {
            self.apply_theme(t);
        }

        (action, hover_edge)
    }

    /// Returns `Some(theme)` when the user clicked a theme button.
    /// The caller must apply it AFTER the imgui build closure completes.
    fn render_config_panel(&mut self, ui: &Ui) -> Option<ActiveTheme> {
        let mut selected_theme: Option<ActiveTheme> = None;
        ui.text("Titlebar");
        ui.separator();

        // Height
        ui.text("Height");
        ui.set_next_item_width(-1.0);
        if ui.slider("##h", 20.0_f32, 50.0, &mut self.edit_h) { self.rebuild_cfg(); }

        // Resize zone
        ui.text("Resize Zone (px)");
        ui.set_next_item_width(-1.0);
        if ui.slider("##rz", 3.0_f32, 16.0, &mut self.edit_rz) { self.rebuild_cfg(); }

        // Alignment
        if ui.checkbox("Center Title", &mut self.edit_center) { self.rebuild_cfg(); }

        // Confirm close
        if ui.checkbox("Confirm on Close", &mut self.edit_confirm) { self.rebuild_cfg(); }
        if ui.checkbox("Separator Line", &mut self.edit_separator) { self.rebuild_cfg(); }
        if ui.checkbox("Drag Hint", &mut self.edit_drag_hint) { self.rebuild_cfg(); }

        ui.spacing();
        ui.separator();
        ui.text("Buttons");

        let mut sm = self.cfg.buttons.show_minimize;
        let mut sx = self.cfg.buttons.show_maximize;
        if ui.checkbox("Minimize", &mut sm) { self.cfg.buttons.show_minimize = sm; }
        if ui.checkbox("Maximize", &mut sx) { self.cfg.buttons.show_maximize = sx; }

        ui.spacing();
        ui.separator();
        ui.text("Themes");
        ui.spacing();

        for t in [
            ActiveTheme::Dark, ActiveTheme::Light, ActiveTheme::Midnight,
            ActiveTheme::Solarized, ActiveTheme::Monokai,
        ] {
            let active = self.theme == t;
            if active {
                let col = self.cfg.theme.colors();
                let _c  = ui.push_style_color(StyleColor::Button,        col.btn_maximize);
                let _c2 = ui.push_style_color(StyleColor::ButtonHovered, col.btn_maximize);
                ui.button(t.label());
            } else if ui.button(t.label()) {
                selected_theme = Some(t);
            }
            ui.same_line();
        }
        ui.new_line();

        ui.spacing();
        ui.separator();
        ui.text("State");
        ui.text_disabled(if self.state.maximized { "Maximized" } else { "Normal" });

        selected_theme
    }

    fn render_log(&mut self, ui: &Ui) {
        // Cap log to prevent unbounded memory growth.
        const MAX_LOG: usize = 200;
        if self.log.len() > MAX_LOG {
            self.log.drain(..self.log.len() - MAX_LOG);
        }

        ui.text("Event Log");
        ui.separator();

        let avail = ui.content_region_avail();
        ui.child_window("##log_inner")
            .size([avail[0], avail[1] - 28.0])
            .build(ui, || {
                for msg in self.log.iter().rev().take(50) {
                    ui.text_disabled(msg);
                }
            });
        ui.spacing();
        if ui.button("Clear") { self.log.clear(); }
    }
}

// ─── GPU + winit boilerplate ──────────────────────────────────────────────────

struct GpuState {
    device: wgpu::Device, queue: wgpu::Queue, window: Arc<Window>,
    surface_cfg: wgpu::SurfaceConfiguration, surface: wgpu::Surface<'static>,
    context: dear_imgui_rs::Context, platform: WinitPlatform,
    renderer: WgpuRenderer, demo: DemoState,
    last_theme: ActiveTheme,
}

struct App { gpu: Option<GpuState> }
impl App { fn new() -> Self { Self { gpu: None } } }

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() { return; }

        let window = Arc::new(event_loop.create_window(
            Window::default_attributes()
                .with_title("Borderless Window Demo")
                .with_inner_size(LogicalSize::new(1100.0_f64, 680.0_f64))
                .with_min_inner_size(LogicalSize::new(640.0_f64, 400.0_f64))
                .with_decorations(false)
                .with_resizable(true)
                .with_visible(false),
        ).expect("window"));

        #[cfg(windows)]
        if let Some(hwnd) = hwnd_of(&window) {
            dear_imgui_custom_mod::borderless_window::platform::set_titlebar_dark_mode(hwnd, true);
        }

        // Centre on primary monitor
        if let Some(mon) = event_loop.primary_monitor() {
            let mp = mon.position();
            let ms = mon.size();
            let ws = window.inner_size();
            window.set_outer_position(winit::dpi::PhysicalPosition::new(
                mp.x + (ms.width  as i32 - ws.width  as i32) / 2,
                mp.y + (ms.height as i32 - ws.height as i32) / 2,
            ));
        }
        window.set_visible(true);

        // Auto-select the best available backend for this platform/hardware.
        // On Windows: DX12 → DX11 → Vulkan (DX11 is a critical fallback for weak GPUs).
        // On other platforms: PRIMARY (Metal, Vulkan, WebGPU).
        #[cfg(target_os = "windows")]
        let backends = wgpu::Backends::DX12 | wgpu::Backends::VULKAN | wgpu::Backends::GL;
        #[cfg(not(target_os = "windows"))]
        let backends = wgpu::Backends::PRIMARY;

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");

        // Prefer high-performance GPU (discrete); on failure fall back to any adapter.
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface), force_fallback_adapter: false,
        }))
        .or_else(|_| block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            compatible_surface: Some(&surface), force_fallback_adapter: true,
        })))
        .expect("no wgpu adapter found");

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor::default()
        )).expect("device");

        let phys = window.inner_size();
        // Auto-detect surface format: prefer Bgra8UnormSrgb → Rgba8UnormSrgb → first available.
        // Using the same format everywhere (surface config + renderer + view) avoids
        // "Incompatible color attachments" validation errors.
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .find(|&&f| f == wgpu::TextureFormat::Bgra8UnormSrgb
                     || f == wgpu::TextureFormat::Rgba8UnormSrgb)
            .copied()
            .or_else(|| surface_caps.formats.first().copied())
            .expect("wgpu: adapter reports no supported surface formats");
        let surface_cfg = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: phys.width.max(1), height: phys.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &surface_cfg);

        let mut context = dear_imgui_rs::Context::create();
        let _ = context.set_ini_filename(None::<std::path::PathBuf>);
        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, HiDpiMode::Default, &mut context);
        // Clamp DPI scale: cap at 3x to avoid oversized font atlases on extreme HiDPI.
        let hidpi = (window.scale_factor() as f32).clamp(1.0, 3.0);
        let font_size = (15.0 * hidpi).round(); // bake at physical pixels
        context.io_mut().set_font_global_scale(1.0 / hidpi); // scale back to logical

        use dear_imgui_custom_mod::code_editor::BuiltinFont;
        context.fonts().add_font_from_memory_ttf(
            BuiltinFont::Hack.data(), font_size,
            Some(&dear_imgui_rs::FontConfig::new()
                .size_pixels(font_size).oversample_h(2).name("Hack")),
            None,
        );

        apply_theme_style(ActiveTheme::Dark, context.style_mut());

        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_format),
            &mut context,
        ).expect("renderer");

        self.gpu = Some(GpuState {
            device, queue, window, surface_cfg, surface,
            context, platform, renderer,
            demo: DemoState::new(),
            last_theme: ActiveTheme::Dark,
        });
    }

    fn window_event(
        &mut self, event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId, event: WindowEvent,
    ) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        gpu.platform.handle_event::<()>(
            &mut gpu.context, &gpu.window,
            &Event::WindowEvent { window_id, event: event.clone() },
        );

        match event {
            WindowEvent::CloseRequested => {
                // Route through the same confirm dialog as the titlebar Close button.
                gpu.demo.show_confirm = true;
            }
            WindowEvent::Focused(focused) => {
                gpu.demo.state.set_focused(focused);
            }
            WindowEvent::Resized(s) => {
                gpu.surface_cfg.width  = s.width.max(1);
                gpu.surface_cfg.height = s.height.max(1);
                gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f) => f,
                    wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                        // Surface size changed mid-frame — reconfigure next frame.
                        gpu.window.request_redraw();
                        f
                    }
                    wgpu::CurrentSurfaceTexture::Outdated
                    | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                        gpu.window.request_redraw();
                        return;
                    }
                    other => { eprintln!("surface error: {other:?}"); return; }
                };

                // ── Re-apply imgui style when theme changes ──────────────
                if gpu.demo.theme != gpu.last_theme {
                    apply_theme_style(gpu.demo.theme, gpu.context.style_mut());

                    #[cfg(windows)]
                    if let Some(hwnd) = hwnd_of(&gpu.window) {
                        dear_imgui_custom_mod::borderless_window::platform::set_titlebar_dark_mode(
                            hwnd, gpu.demo.theme != ActiveTheme::Light,
                        );
                    }
                    gpu.last_theme = gpu.demo.theme;
                }

                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);
                let ui = gpu.context.frame();

                let (action, hover_edge) = gpu.demo.render(ui);

                gpu.platform.prepare_render_with_ui(ui, &gpu.window);
                let draw_data = gpu.context.render();

                let clear = match gpu.demo.theme {
                    ActiveTheme::Light => wgpu::Color { r: 0.89, g: 0.89, b: 0.93, a: 1.0 },
                    ActiveTheme::Solarized => wgpu::Color { r: 0.03, g: 0.19, b: 0.23, a: 1.0 },
                    ActiveTheme::Monokai   => wgpu::Color { r: 0.12, g: 0.12, b: 0.12, a: 1.0 },
                    _ => wgpu::Color { r: 0.07, g: 0.07, b: 0.09, a: 1.0 },
                };

                let mut enc = gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("imgui") }
                );
                {
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("main"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view, resolve_target: None, depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(clear),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    if draw_data.total_vtx_count > 0
                        && let Err(e) = gpu.renderer.render_draw_data(draw_data, &mut pass)
                    {
                        eprintln!("demo_borderless: imgui render error: {e:?}");
                    }
                }
                gpu.queue.submit(Some(enc.finish()));
                frame.present();

                // ── Update resize cursor every frame ───────────────────
                gpu.window.set_cursor(cursor_for_edge(hover_edge));

                // ── Handle window action ────────────────────────────────
                match action {
                    WindowAction::Close => event_loop.exit(),
                    WindowAction::Minimize => { gpu.window.set_minimized(true); }
                    WindowAction::Maximize => {
                        let new = !gpu.demo.state.maximized;
                        gpu.window.set_maximized(new);
                        gpu.demo.state.set_maximized(new);
                        gpu.demo.log.push(format!("Maximize → {}", if new { "maximized" } else { "restored" }));
                    }
                    WindowAction::DragStart => { gpu.window.drag_window().ok(); }
                    WindowAction::ResizeStart(e) => { gpu.window.drag_resize_window(to_winit_resize(e)).ok(); }
                    _ => {}
                }

                gpu.window.request_redraw();
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(gpu) = self.gpu.as_ref() { gpu.window.request_redraw(); }
        // Cap at ~60 fps: prevents CPU busy-loop (ControlFlow::Poll spins 1000+ fps).
        // WaitUntil sleeps until the next event OR the timeout, whichever comes first,
        // so input events still wake the loop immediately.
        event_loop.set_control_flow(
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16)),
        );
    }
}

// ─── ImGui styles ─────────────────────────────────────────────────────────────

fn apply_imgui_dark(s: &mut dear_imgui_rs::Style) {
    s.set_window_rounding(0.0); s.set_frame_rounding(3.0);
    s.set_scrollbar_rounding(4.0); s.set_window_border_size(0.0);
    s.set_color(StyleColor::WindowBg,        [0.09, 0.09, 0.11, 1.0]);
    s.set_color(StyleColor::ChildBg,         [0.10, 0.11, 0.14, 1.0]);
    s.set_color(StyleColor::Border,          [0.20, 0.22, 0.27, 0.60]);
    s.set_color(StyleColor::FrameBg,         [0.14, 0.15, 0.19, 1.0]);
    s.set_color(StyleColor::FrameBgHovered,  [0.19, 0.21, 0.27, 1.0]);
    s.set_color(StyleColor::Button,          [0.18, 0.20, 0.25, 1.0]);
    s.set_color(StyleColor::ButtonHovered,   [0.26, 0.29, 0.36, 1.0]);
    s.set_color(StyleColor::ButtonActive,    [0.30, 0.50, 0.75, 1.0]);
    s.set_color(StyleColor::Header,          [0.18, 0.20, 0.25, 1.0]);
    s.set_color(StyleColor::HeaderHovered,   [0.24, 0.27, 0.34, 1.0]);
    s.set_color(StyleColor::Separator,       [0.20, 0.22, 0.27, 0.60]);
    s.set_color(StyleColor::Text,            [0.92, 0.93, 0.95, 1.0]);
    s.set_color(StyleColor::TextDisabled,    [0.42, 0.45, 0.52, 1.0]);
    s.set_color(StyleColor::SliderGrab,      [0.30, 0.50, 0.75, 1.0]);
    s.set_color(StyleColor::CheckMark,       [0.40, 0.63, 0.88, 1.0]);
    s.set_color(StyleColor::ScrollbarBg,     [0.06, 0.06, 0.08, 0.60]);
    s.set_color(StyleColor::ScrollbarGrab,   [0.22, 0.24, 0.30, 1.0]);
    s.set_color(StyleColor::TitleBg,         [0.09, 0.09, 0.11, 1.0]);
    s.set_color(StyleColor::TitleBgActive,   [0.11, 0.12, 0.15, 1.0]);
    s.set_color(StyleColor::PopupBg,         [0.11, 0.12, 0.16, 0.96]);
    s.set_color(StyleColor::SliderGrabActive,    [0.40, 0.60, 0.85, 1.0]);
    s.set_color(StyleColor::FrameBgActive,       [0.23, 0.25, 0.32, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabHovered,[0.28, 0.31, 0.38, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabActive, [0.35, 0.38, 0.48, 1.0]);
    s.set_color(StyleColor::ResizeGrip,          [0.22, 0.25, 0.34, 0.40]);
    s.set_color(StyleColor::ResizeGripHovered,   [0.30, 0.50, 0.75, 0.80]);
    s.set_color(StyleColor::ResizeGripActive,    [0.40, 0.63, 0.88, 1.0]);
    s.set_color(StyleColor::TextSelectedBg,      [0.30, 0.50, 0.75, 0.45]);
    s.set_color(StyleColor::ModalWindowDimBg,    [0.04, 0.04, 0.05, 0.70]);
}

fn apply_imgui_light(s: &mut dear_imgui_rs::Style) {
    s.set_window_rounding(0.0); s.set_frame_rounding(3.0);
    s.set_scrollbar_rounding(4.0); s.set_window_border_size(0.0);
    s.set_color(StyleColor::WindowBg,        [0.93, 0.93, 0.95, 1.0]);
    s.set_color(StyleColor::ChildBg,         [0.88, 0.88, 0.92, 1.0]);
    s.set_color(StyleColor::Border,          [0.68, 0.70, 0.76, 0.70]);
    s.set_color(StyleColor::FrameBg,         [0.82, 0.82, 0.87, 1.0]);
    s.set_color(StyleColor::FrameBgHovered,  [0.76, 0.76, 0.83, 1.0]);
    s.set_color(StyleColor::Button,          [0.80, 0.80, 0.87, 1.0]);
    s.set_color(StyleColor::ButtonHovered,   [0.72, 0.74, 0.83, 1.0]);
    s.set_color(StyleColor::ButtonActive,    [0.18, 0.48, 0.76, 1.0]);
    s.set_color(StyleColor::Header,          [0.78, 0.80, 0.88, 1.0]);
    s.set_color(StyleColor::HeaderHovered,   [0.70, 0.72, 0.83, 1.0]);
    s.set_color(StyleColor::Separator,       [0.65, 0.67, 0.74, 0.70]);
    s.set_color(StyleColor::Text,            [0.10, 0.10, 0.15, 1.0]);
    s.set_color(StyleColor::TextDisabled,    [0.50, 0.52, 0.58, 1.0]);
    s.set_color(StyleColor::SliderGrab,      [0.18, 0.48, 0.76, 1.0]);
    s.set_color(StyleColor::CheckMark,       [0.14, 0.40, 0.66, 1.0]);
    s.set_color(StyleColor::ScrollbarBg,     [0.85, 0.85, 0.88, 0.60]);
    s.set_color(StyleColor::ScrollbarGrab,   [0.65, 0.66, 0.72, 1.0]);
    s.set_color(StyleColor::TitleBg,         [0.88, 0.88, 0.92, 1.0]);
    s.set_color(StyleColor::TitleBgActive,   [0.94, 0.94, 0.96, 1.0]);
    s.set_color(StyleColor::PopupBg,         [0.94, 0.94, 0.96, 0.96]);
    s.set_color(StyleColor::SliderGrabActive,    [0.14, 0.40, 0.70, 1.0]);
    s.set_color(StyleColor::FrameBgActive,       [0.70, 0.72, 0.80, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabHovered,[0.57, 0.59, 0.67, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabActive, [0.48, 0.50, 0.60, 1.0]);
    s.set_color(StyleColor::ResizeGrip,          [0.68, 0.70, 0.78, 0.40]);
    s.set_color(StyleColor::ResizeGripHovered,   [0.18, 0.48, 0.76, 0.80]);
    s.set_color(StyleColor::ResizeGripActive,    [0.14, 0.40, 0.66, 1.0]);
    s.set_color(StyleColor::TextSelectedBg,      [0.18, 0.48, 0.76, 0.40]);
    s.set_color(StyleColor::ModalWindowDimBg,    [0.30, 0.30, 0.35, 0.50]);
}

fn apply_imgui_midnight(s: &mut dear_imgui_rs::Style) {
    s.set_window_rounding(0.0); s.set_frame_rounding(3.0);
    s.set_scrollbar_rounding(4.0); s.set_window_border_size(0.0);
    s.set_color(StyleColor::WindowBg,        [0.05, 0.05, 0.07, 1.0]);
    s.set_color(StyleColor::ChildBg,         [0.07, 0.07, 0.09, 1.0]);
    s.set_color(StyleColor::Border,          [0.14, 0.16, 0.20, 0.70]);
    s.set_color(StyleColor::FrameBg,         [0.11, 0.11, 0.15, 1.0]);
    s.set_color(StyleColor::FrameBgHovered,  [0.16, 0.17, 0.22, 1.0]);
    s.set_color(StyleColor::Button,          [0.13, 0.14, 0.18, 1.0]);
    s.set_color(StyleColor::ButtonHovered,   [0.20, 0.22, 0.29, 1.0]);
    s.set_color(StyleColor::ButtonActive,    [0.28, 0.50, 0.80, 1.0]);
    s.set_color(StyleColor::Header,          [0.14, 0.15, 0.20, 1.0]);
    s.set_color(StyleColor::HeaderHovered,   [0.19, 0.21, 0.28, 1.0]);
    s.set_color(StyleColor::Separator,       [0.14, 0.16, 0.20, 0.70]);
    s.set_color(StyleColor::Text,            [0.88, 0.88, 0.90, 1.0]);
    s.set_color(StyleColor::TextDisabled,    [0.38, 0.40, 0.46, 1.0]);
    s.set_color(StyleColor::SliderGrab,      [0.28, 0.50, 0.80, 1.0]);
    s.set_color(StyleColor::CheckMark,       [0.38, 0.63, 0.90, 1.0]);
    s.set_color(StyleColor::ScrollbarBg,     [0.04, 0.04, 0.06, 0.60]);
    s.set_color(StyleColor::ScrollbarGrab,   [0.18, 0.20, 0.27, 1.0]);
    s.set_color(StyleColor::TitleBg,         [0.05, 0.05, 0.07, 1.0]);
    s.set_color(StyleColor::TitleBgActive,   [0.07, 0.07, 0.09, 1.0]);
    s.set_color(StyleColor::PopupBg,         [0.08, 0.08, 0.11, 0.97]);
    s.set_color(StyleColor::SliderGrabActive,    [0.38, 0.62, 0.88, 1.0]);
    s.set_color(StyleColor::FrameBgActive,       [0.18, 0.19, 0.26, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabHovered,[0.22, 0.24, 0.32, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabActive, [0.30, 0.32, 0.42, 1.0]);
    s.set_color(StyleColor::ResizeGrip,          [0.17, 0.19, 0.26, 0.40]);
    s.set_color(StyleColor::ResizeGripHovered,   [0.28, 0.50, 0.80, 0.80]);
    s.set_color(StyleColor::ResizeGripActive,    [0.38, 0.63, 0.90, 1.0]);
    s.set_color(StyleColor::TextSelectedBg,      [0.28, 0.50, 0.80, 0.45]);
    s.set_color(StyleColor::ModalWindowDimBg,    [0.02, 0.02, 0.03, 0.75]);
}


fn apply_imgui_solarized(s: &mut dear_imgui_rs::Style) {
    // Solarized dark — base03 #002b36, accent #268BD2
    s.set_window_rounding(0.0); s.set_frame_rounding(3.0);
    s.set_scrollbar_rounding(4.0); s.set_window_border_size(0.0);
    s.set_color(StyleColor::WindowBg,        [0.00, 0.17, 0.21, 1.0]);
    s.set_color(StyleColor::ChildBg,         [0.03, 0.21, 0.26, 1.0]);
    s.set_color(StyleColor::Border,          [0.35, 0.43, 0.46, 0.55]);
    s.set_color(StyleColor::FrameBg,         [0.05, 0.24, 0.30, 1.0]);
    s.set_color(StyleColor::FrameBgHovered,  [0.08, 0.28, 0.35, 1.0]);
    s.set_color(StyleColor::Button,          [0.05, 0.24, 0.30, 1.0]);
    s.set_color(StyleColor::ButtonHovered,   [0.10, 0.30, 0.37, 1.0]);
    s.set_color(StyleColor::ButtonActive,    [0.15, 0.55, 0.82, 1.0]); // blue
    s.set_color(StyleColor::Header,          [0.05, 0.24, 0.30, 1.0]);
    s.set_color(StyleColor::HeaderHovered,   [0.09, 0.29, 0.36, 1.0]);
    s.set_color(StyleColor::Separator,       [0.35, 0.43, 0.46, 0.55]);
    s.set_color(StyleColor::Text,            [0.51, 0.58, 0.59, 1.0]); // base0
    s.set_color(StyleColor::TextDisabled,    [0.35, 0.43, 0.46, 1.0]);
    s.set_color(StyleColor::SliderGrab,      [0.15, 0.55, 0.82, 1.0]);
    s.set_color(StyleColor::CheckMark,       [0.15, 0.55, 0.82, 1.0]);
    s.set_color(StyleColor::ScrollbarBg,     [0.00, 0.14, 0.18, 0.60]);
    s.set_color(StyleColor::ScrollbarGrab,   [0.07, 0.27, 0.33, 1.0]);
    s.set_color(StyleColor::TitleBg,         [0.00, 0.17, 0.21, 1.0]);
    s.set_color(StyleColor::TitleBgActive,   [0.03, 0.21, 0.26, 1.0]);
    s.set_color(StyleColor::PopupBg,         [0.02, 0.19, 0.24, 0.97]);
    s.set_color(StyleColor::SliderGrabActive,    [0.20, 0.65, 0.90, 1.0]);
    s.set_color(StyleColor::FrameBgActive,       [0.08, 0.30, 0.37, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabHovered,[0.10, 0.32, 0.40, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabActive, [0.14, 0.38, 0.47, 1.0]);
    s.set_color(StyleColor::ResizeGrip,          [0.05, 0.27, 0.33, 0.40]);
    s.set_color(StyleColor::ResizeGripHovered,   [0.15, 0.55, 0.82, 0.80]);
    s.set_color(StyleColor::ResizeGripActive,    [0.15, 0.55, 0.82, 1.0]);
    s.set_color(StyleColor::TextSelectedBg,      [0.15, 0.55, 0.82, 0.40]);
    s.set_color(StyleColor::ModalWindowDimBg,    [0.00, 0.07, 0.09, 0.75]);
}

fn apply_imgui_monokai(s: &mut dear_imgui_rs::Style) {
    // Monokai Pro — #272822 bg, #F8F8F2 text, #F92672 pink, #A6E22E green
    s.set_window_rounding(0.0); s.set_frame_rounding(3.0);
    s.set_scrollbar_rounding(4.0); s.set_window_border_size(0.0);
    s.set_color(StyleColor::WindowBg,        [0.12, 0.12, 0.12, 1.0]);
    s.set_color(StyleColor::ChildBg,         [0.15, 0.15, 0.15, 1.0]);
    s.set_color(StyleColor::Border,          [0.22, 0.22, 0.22, 0.70]);
    s.set_color(StyleColor::FrameBg,         [0.19, 0.19, 0.19, 1.0]);
    s.set_color(StyleColor::FrameBgHovered,  [0.24, 0.24, 0.24, 1.0]);
    s.set_color(StyleColor::Button,          [0.20, 0.20, 0.20, 1.0]);
    s.set_color(StyleColor::ButtonHovered,   [0.28, 0.28, 0.28, 1.0]);
    s.set_color(StyleColor::ButtonActive,    [0.65, 0.89, 0.18, 1.0]); // green
    s.set_color(StyleColor::Header,          [0.20, 0.20, 0.20, 1.0]);
    s.set_color(StyleColor::HeaderHovered,   [0.27, 0.27, 0.27, 1.0]);
    s.set_color(StyleColor::Separator,       [0.22, 0.22, 0.22, 0.70]);
    s.set_color(StyleColor::Text,            [0.97, 0.97, 0.95, 1.0]); // #F8F8F2
    s.set_color(StyleColor::TextDisabled,    [0.45, 0.45, 0.43, 1.0]);
    s.set_color(StyleColor::SliderGrab,      [0.65, 0.89, 0.18, 1.0]);
    s.set_color(StyleColor::CheckMark,       [0.65, 0.89, 0.18, 1.0]);
    s.set_color(StyleColor::ScrollbarBg,     [0.09, 0.09, 0.09, 0.60]);
    s.set_color(StyleColor::ScrollbarGrab,   [0.24, 0.24, 0.24, 1.0]);
    s.set_color(StyleColor::TitleBg,         [0.12, 0.12, 0.12, 1.0]);
    s.set_color(StyleColor::TitleBgActive,   [0.15, 0.15, 0.15, 1.0]);
    s.set_color(StyleColor::PopupBg,         [0.14, 0.14, 0.14, 0.97]);
    s.set_color(StyleColor::SliderGrabActive,    [0.78, 0.98, 0.25, 1.0]);
    s.set_color(StyleColor::FrameBgActive,       [0.26, 0.26, 0.26, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabHovered,[0.30, 0.30, 0.30, 1.0]);
    s.set_color(StyleColor::ScrollbarGrabActive, [0.38, 0.38, 0.38, 1.0]);
    s.set_color(StyleColor::ResizeGrip,          [0.22, 0.22, 0.22, 0.40]);
    s.set_color(StyleColor::ResizeGripHovered,   [0.65, 0.89, 0.18, 0.80]);
    s.set_color(StyleColor::ResizeGripActive,    [0.65, 0.89, 0.18, 1.0]);
    s.set_color(StyleColor::TextSelectedBg,      [0.65, 0.89, 0.18, 0.35]);
    s.set_color(StyleColor::ModalWindowDimBg,    [0.04, 0.04, 0.04, 0.75]);
}

fn apply_theme_style(theme: ActiveTheme, s: &mut dear_imgui_rs::Style) {
    match theme {
        ActiveTheme::Dark      => apply_imgui_dark(s),
        ActiveTheme::Light     => apply_imgui_light(s),
        ActiveTheme::Midnight  => apply_imgui_midnight(s),
        ActiveTheme::Solarized => apply_imgui_solarized(s),
        ActiveTheme::Monokai   => apply_imgui_monokai(s),
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    // Initial control flow: Wait (about_to_wait will switch to WaitUntil ~60fps).
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut App::new()).expect("run");
}
