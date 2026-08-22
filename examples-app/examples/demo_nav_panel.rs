//! Demo: NavPanel + StatusBar — full navigation + status integration test.
//!
//! Run with:
//!   cargo run --example demo_nav_panel

use std::sync::Arc;

use dear_imgui_custom_mod::chrome::{Chrome, TitlebarConfig};
use dear_imgui_custom_mod::confirm_dialog::{
    DialogConfig, DialogIcon, DialogResult, render_confirm_dialog,
};
use dear_imgui_custom_mod::nav_panel::*;
use dear_imgui_custom_mod::status_bar::{Indicator, StatusBar, StatusBarConfig, StatusItem};
use dear_imgui_custom_mod::theme::Theme;
use dear_imgui_rs::{StyleColor, StyleVar, Ui};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

// ── Application state ────────────────────────────────────────────────────────

struct DemoApp {
    nav_state: NavPanelState,
    status_bar: StatusBar,
    current_theme: Theme,
    show_confirm: bool,
    log: Vec<String>,
    notification: u32,
    // Config values
    position_idx: i32,
    width: f32,
    height: f32,
    button_size: f32,
    indicator: f32,
    show_toggle: bool,
    auto_hide: bool,
    auto_show: bool,
    animate: bool,
    anim_speed: f32,
    btn_rounding: f32,
    btn_spacing: f32,
    btn_seps: bool,
    // Hover style ─────────────────────────────────────────────────────
    /// 0 = Flat, 1 = Emboss (2.5D raised look).
    hover_zoom_scale: f32,
    // Active style ─────────────────────────────────────────────────────
    /// 0 = Ring (default — transparent ring around icon), 1 = Bar (filled background + indicator strip).
    active_style_idx: i32,
    /// RGB part of the ring colour. Alpha lives in `active_ring_alpha` so
    /// the slider stays visible separate from the colour swatch.
    active_ring_color: [f32; 3],
    active_ring_alpha: f32,
    active_ring_thickness: f32,
    active_ring_padding: f32,
    // ── Chrome bridge state ─────────────────────────────────────────────
    /// Set when the user picks a new theme via the demo's UI. The harness
    /// (main loop) drains this each frame and forwards to `Chrome::set_theme`.
    pending_theme: Option<Theme>,
    /// Set when the user confirms the close dialog. The harness reads this
    /// after `render` and `process::exit(0)`s when true.
    pending_exit: bool,
}

impl DemoApp {
    fn new() -> Self {
        let mut nav_state = NavPanelState::new();
        nav_state.set_active("home");

        let mut status_bar = StatusBar::new("##status");
        status_bar.config = StatusBarConfig {
            height: 22.0,
            ..StatusBarConfig::default()
        };
        status_bar.left(StatusItem::indicator("Ready", Indicator::Success));
        status_bar.left(StatusItem::text("Ln 1, Col 1"));
        status_bar.right(StatusItem::text("UTF-8"));
        status_bar.right(StatusItem::text("NavPanel Demo"));

        Self {
            nav_state,
            status_bar,
            current_theme: Theme::Dark,
            show_confirm: false,
            log: vec!["NavPanel + StatusBar demo started.".into()],
            notification: 2,
            position_idx: 0,
            width: 28.0,
            height: 24.0,
            button_size: 24.0,
            indicator: 3.0,
            show_toggle: false,
            auto_hide: false,
            auto_show: true,
            animate: true,
            anim_speed: 6.0,
            btn_rounding: 6.0,
            btn_spacing: 4.0,
            btn_seps: true,
            // Hover-zoom default — `1.20` matches the lib's
            // out-of-the-box magnification factor. The historic
            // `Flat` (no zoom + tinted bg) variant was removed on
            // 2026-04-29; set the slider to `1.0` here for the
            // closest no-zoom equivalent.
            hover_zoom_scale: 1.20,
            // Active defaults — Ring + warm amber, matches the lib's
            // out-of-the-box look.
            active_style_idx: 0,
            active_ring_color: [0.95, 0.62, 0.20],
            active_ring_alpha: 1.0,
            active_ring_thickness: 1.5,
            active_ring_padding: 4.0,
            pending_theme: None,
            pending_exit: false,
        }
    }

    fn build_nav(&self) -> NavPanelConfig {
        let position = match self.position_idx {
            1 => DockPosition::Right,
            2 => DockPosition::Top,
            _ => DockPosition::Left,
        };

        let mut home = NavButton::action("home", "H", "Home").with_color([0.30, 0.65, 1.00, 1.0]);
        if self.notification > 0 {
            home = home.with_badge(self.notification.to_string());
        }

        let active_style = if self.active_style_idx == 1 {
            ActiveStyle::Bar
        } else {
            ActiveStyle::Ring
        };
        let ring_rgba = [
            self.active_ring_color[0],
            self.active_ring_color[1],
            self.active_ring_color[2],
            self.active_ring_alpha,
        ];

        NavPanelConfig::new(position)
            .with_theme(Self::to_nav_theme(&self.current_theme))
            .with_width(self.width)
            .with_height(self.height)
            .with_button_size(self.button_size)
            .with_indicator_thickness(self.indicator)
            .with_toggle_button(self.show_toggle)
            .with_auto_hide(self.auto_hide)
            .with_auto_show_on_hover(self.auto_show)
            .with_animate(self.animate)
            .with_animation_speed(self.anim_speed)
            .with_button_rounding(self.btn_rounding)
            .with_button_spacing(self.btn_spacing)
            .with_button_separators(self.btn_seps)
            .with_content_offset_y(28.0) // titlebar height
            // ── New visual styles ─────────────────────────────────
            .with_hover_zoom_scale(self.hover_zoom_scale)
            .with_active_style(active_style)
            .with_active_ring_color(ring_rgba)
            .with_active_ring_thickness(self.active_ring_thickness)
            .with_active_ring_padding(self.active_ring_padding)
            .add_button(home)
            .add_button(NavButton::action("search", "S", "Search")
                .with_color([0.40, 0.82, 0.30, 1.0]))
            .add_button(NavButton::action("users", "U", "Users")
                .with_color([0.72, 0.42, 0.92, 1.0]))
            .add_button(NavButton::action("files", "F", "Files")
                .with_color([0.92, 0.68, 0.22, 1.0]))
            .add_button(NavButton::action("data", "D", "Database")
                .with_color([0.88, 0.78, 0.22, 1.0]))
            .add_separator()
            .add_button(NavButton::submenu("settings", "*", "Settings")
                .with_color([0.60, 0.60, 0.68, 1.0])
                .add_item(SubMenuItem::new("theme", "Cycle Theme"))
                .add_item(SubMenuItem::new("prefs", "Preferences").with_shortcut("Ctrl+,"))
                .add_item(SubMenuItem::separator())
                .add_item(SubMenuItem::new("about", "About")))
    }

    fn push_log(&mut self, msg: String) {
        self.log.push(msg);
        if self.log.len() > 300 {
            self.log.drain(..1);
        }
    }

    fn to_nav_theme(t: &Theme) -> Theme {
        *t
    }
    fn to_dialog_theme(t: &Theme) -> Theme {
        *t
    }

    fn cycle_theme(&mut self) {
        // `Theme::next()` walks every built-in variant — Catppuccin / Nord
        // join the rotation automatically. The chrome bridge in `main`
        // forwards `pending_theme` to `Chrome::set_theme` after render.
        let next = self.current_theme.next();
        self.current_theme = next;
        self.pending_theme = Some(next);
        self.push_log(format!("Theme -> {:?}", self.current_theme));
    }

    /// Drain the pending theme switch. Called by the harness once per frame
    /// after `render`.
    pub fn take_pending_theme(&mut self) -> Option<Theme> {
        self.pending_theme.take()
    }

    /// Returns `true` once the user confirmed the close dialog. The harness
    /// `process::exit(0)`s when this fires.
    pub fn should_exit(&self) -> bool {
        self.pending_exit
    }

    /// Called by the harness when the chrome reports a close request. We
    /// surface the confirm dialog instead of exiting.
    pub fn on_close_clicked(&mut self) {
        self.show_confirm = true;
    }
}

// ── Render entry-point (called by the chrome harness) ───────────────────────

impl DemoApp {
    pub fn render(&mut self, ui: &Ui) {
        let [avail_w, avail_h] = ui.content_region_avail();
        let is_vertical = self.position_idx <= 1; // Left=0, Right=1
        let is_left = self.position_idx == 0;
        let is_top = self.position_idx == 2; // Top=2

        // ── Reserve space for status bar at bottom ───────────────────────────
        let status_h = self.status_bar.config.height;
        let main_h = avail_h - status_h;

        // ── Main zone (nav + content) — excludes status bar ──────────────────
        // Wrap in a child_window so nav_panel sees correct avail height.
        let mut nav_w = 0.0_f32;
        let mut nav_h_used = 0.0_f32;
        let mut pending_events: Vec<NavEvent> = Vec::new();

        ui.child_window("##main_zone")
            .size([avail_w, main_h])
            .border(false)
            .build(ui, || {
                let nav_cfg = self.build_nav();
                let nav_result = render_nav_panel(ui, &nav_cfg, &mut self.nav_state);
                nav_w = nav_result.occupied_size[0];
                nav_h_used = nav_result.occupied_size[1];
                pending_events.clone_from(&nav_result.events);

                // ── Content area ─────────────────────────────────────────────
                let [cx, cy] = ui.cursor_pos();
                if is_vertical {
                    let content_x = if is_left { cx + nav_w } else { cx };
                    ui.set_cursor_pos([content_x, cy]);
                } else {
                    let content_y = if is_top { cy + nav_h_used } else { cy };
                    ui.set_cursor_pos([cx, content_y]);
                }

                let content_w = if is_vertical {
                    avail_w - nav_w
                } else {
                    avail_w
                };
                let zone_h = ui.content_region_avail()[1];
                let content_h = zone_h;

                ui.child_window("##content")
                    .size([content_w, content_h])
                    .border(false)
                    .build(ui, || {
                        let _pad = ui.push_style_var(StyleVar::WindowPadding([10.0, 6.0]));
                        let _sp = ui.push_style_var(StyleVar::ItemSpacing([6.0, 4.0]));

                        // Snapshot the active id as an owned String so the
                        // borrow on `self.nav_state` is released before the
                        // child_window closures (which need `&mut self`).
                        let page: String = self
                            .nav_state
                            .active
                            .as_deref()
                            .unwrap_or("none")
                            .to_owned();
                        let page = page.as_str();
                        let panel_w = 260.0_f32;
                        let avail = ui.content_region_avail();

                        // Left: Config panel
                        ui.child_window("##cfg")
                            .size([panel_w, avail[1]])
                            .border(true)
                            .build(ui, || {
                                ui.text("NavPanel Config");
                                ui.separator();
                                ui.spacing();

                                // Position buttons
                                ui.text("Position");
                                for (i, label) in ["Left", "Right", "Top"].iter().enumerate() {
                                    if i > 0 {
                                        ui.same_line();
                                    }
                                    let active = self.position_idx == i as i32;
                                    if active {
                                        let _c = ui.push_style_color(
                                            StyleColor::Button,
                                            [0.3, 0.5, 0.9, 1.0],
                                        );
                                        ui.button(label);
                                    } else if ui.button(label) {
                                        self.position_idx = i as i32;
                                        self.push_log(format!("Position: {label}"));
                                    }
                                }
                                ui.text_disabled("Tip: right-click the panel to re-dock it");

                                ui.spacing();
                                ui.separator();
                                ui.text("Dimensions");
                                ui.set_next_item_width(130.0);
                                ui.slider("Width", 20.0, 60.0, &mut self.width);
                                ui.set_next_item_width(130.0);
                                ui.slider("Height", 20.0, 60.0, &mut self.height);
                                ui.set_next_item_width(130.0);
                                ui.slider("Btn size", 18.0, 48.0, &mut self.button_size);
                                ui.set_next_item_width(130.0);
                                ui.slider("Indicator", 1.0, 6.0, &mut self.indicator);
                                ui.set_next_item_width(130.0);
                                ui.slider("Rounding", 0.0, 16.0, &mut self.btn_rounding);
                                ui.set_next_item_width(130.0);
                                ui.slider("Spacing", 0.0, 8.0, &mut self.btn_spacing);
                                ui.checkbox("Button separators", &mut self.btn_seps);

                                ui.spacing();
                                ui.separator();
                                ui.text("Behavior");
                                ui.checkbox("Show toggle arrow", &mut self.show_toggle);
                                ui.checkbox("Auto hide", &mut self.auto_hide);
                                ui.checkbox("Auto show on hover", &mut self.auto_show);
                                ui.checkbox("Animate", &mut self.animate);
                                ui.set_next_item_width(130.0);
                                ui.slider("Speed", 2.0, 20.0, &mut self.anim_speed);

                                ui.spacing();
                                ui.separator();
                                ui.text("Hover");
                                // Single slider — `1.0` disables the
                                // zoom (icon stays the same size on
                                // hover). `1.15`–`1.30` is the macOS
                                // Dock sweet spot.
                                ui.set_next_item_width(130.0);
                                ui.slider("Zoom scale", 1.0, 2.0, &mut self.hover_zoom_scale);

                                ui.spacing();
                                ui.separator();
                                ui.text("Active Style");
                                for (i, label) in ["Ring", "Bar"].iter().enumerate() {
                                    if i > 0 {
                                        ui.same_line();
                                    }
                                    let active = self.active_style_idx == i as i32;
                                    if active {
                                        let _c = ui.push_style_color(
                                            StyleColor::Button,
                                            [0.3, 0.5, 0.9, 1.0],
                                        );
                                        ui.button(label);
                                    } else if ui.button(label) {
                                        self.active_style_idx = i as i32;
                                        self.push_log(format!("Active style: {label}"));
                                    }
                                }
                                // Ring-only tunables. Render even when Bar
                                // is selected (greyed) so toggling Ring
                                // doesn't reflow the panel — the indicator
                                // strip thickness ("Indicator" slider above)
                                // is the parallel control for Bar mode.
                                let ring = self.active_style_idx == 0;
                                if ring {
                                    ui.set_next_item_width(130.0);
                                    ui.color_edit3("Ring color", &mut self.active_ring_color);
                                    ui.set_next_item_width(130.0);
                                    ui.slider("Ring alpha", 0.0, 1.0, &mut self.active_ring_alpha);
                                    ui.set_next_item_width(130.0);
                                    ui.slider(
                                        "Ring thick",
                                        0.5,
                                        4.0,
                                        &mut self.active_ring_thickness,
                                    );
                                    ui.set_next_item_width(130.0);
                                    ui.slider("Ring pad", 0.0, 10.0, &mut self.active_ring_padding);
                                } else {
                                    let _d = ui.push_style_var(StyleVar::Alpha(0.40));
                                    ui.set_next_item_width(130.0);
                                    ui.color_edit3("Ring color", &mut self.active_ring_color);
                                    ui.set_next_item_width(130.0);
                                    ui.slider("Ring alpha", 0.0, 1.0, &mut self.active_ring_alpha);
                                    ui.set_next_item_width(130.0);
                                    ui.slider(
                                        "Ring thick",
                                        0.5,
                                        4.0,
                                        &mut self.active_ring_thickness,
                                    );
                                    ui.set_next_item_width(130.0);
                                    ui.slider("Ring pad", 0.0, 10.0, &mut self.active_ring_padding);
                                }

                                ui.spacing();
                                ui.separator();
                                ui.text("State");
                                ui.text(format!("  visible: {}", self.nav_state.visible));
                                ui.text(format!(
                                    "  progress: {:.2}",
                                    self.nav_state.animation_progress
                                ));
                                ui.text(format!("  active: {:?}", self.nav_state.active));

                                ui.spacing();
                                ui.separator();
                                ui.text("Actions");
                                if ui.button("Show") {
                                    self.nav_state.show();
                                }
                                ui.same_line();
                                if ui.button("Hide") {
                                    self.nav_state.hide();
                                }
                                ui.same_line();
                                if ui.button("+Badge") {
                                    self.notification += 1;
                                    self.push_log(format!("Badge: {}", self.notification));
                                }
                                ui.same_line();
                                if ui.button("Clear") {
                                    self.notification = 0;
                                }
                            });

                        ui.same_line();

                        // Right: Page + Log
                        ui.child_window("##page")
                            .size([0.0, avail[1]])
                            .border(false)
                            .build(ui, || {
                                ui.text(format!("Page: {page}"));
                                ui.separator();
                                ui.spacing();
                                // `page` is `&str` (from `as_deref()` on the
                                // `Cow<'static, str>` active id) — match works
                                // unchanged on string literals.
                                match page {
                                    "home" => {
                                        ui.text_wrapped(
                                            "NavPanel + StatusBar integration test. \
                                     Use config panel to adjust all properties.",
                                        );
                                    }
                                    "search" => ui.text("Search page"),
                                    "users" => ui.text("Users page"),
                                    "files" => ui.text("Files page"),
                                    "data" => ui.text("Database page"),
                                    _ => {
                                        ui.text("Select a page.");
                                    }
                                }

                                ui.spacing();
                                ui.separator();
                                ui.text("Event Log");
                                ui.separator();
                                let lh = ui.content_region_avail()[1];
                                ui.child_window("##log").size([0.0, lh]).border(true).build(
                                    ui,
                                    || {
                                        for entry in &self.log {
                                            ui.text_wrapped(entry);
                                        }
                                        if ui.scroll_y() >= ui.scroll_max_y() {
                                            ui.set_scroll_here_y(1.0);
                                        }
                                    },
                                );
                            });
                    }); // ##content
            }); // ##main_zone

        // Handle nav events (after child_window scope)
        for event in &pending_events {
            match event {
                NavEvent::ButtonClicked(id) => {
                    self.push_log(format!("Click: {id}"));
                    if id.as_ref() == "home" && self.notification > 0 {
                        self.notification = 0;
                        self.push_log("Notifications cleared".to_string());
                    }
                }
                NavEvent::SubMenuClicked(_btn, item) => {
                    self.push_log(format!("Submenu: {item}"));
                    match item.as_ref() {
                        "theme" => self.cycle_theme(),
                        "about" => self.push_log("NavPanel Demo v0.6.1".to_string()),
                        _ => {}
                    }
                }
                NavEvent::ToggleClicked(v) => {
                    self.push_log(format!("Toggle: {v}"));
                }
                NavEvent::PositionChangeRequested(pos) => {
                    // Right-click reposition menu → rebuild the config next
                    // frame with the chosen dock position. The demo owns the
                    // position via `position_idx`, so just remap it.
                    self.position_idx = match pos {
                        DockPosition::Left => 0,
                        DockPosition::Right => 1,
                        DockPosition::Top => 2,
                    };
                    self.push_log(format!("Reposition: {pos:?}"));
                }
            }
        }

        // ── Status bar (always at the very bottom) ───────────────────────────
        self.status_bar.render(ui);

        // ── Close dialog ─────────────────────────────────────────────────────
        if self.show_confirm {
            let cfg = DialogConfig::new("Close Application", "Are you sure?")
                .with_icon(DialogIcon::Warning)
                .with_confirm_label("Close")
                .with_cancel_label("Cancel")
                .with_theme(Self::to_dialog_theme(&self.current_theme));
            if let DialogResult::Confirmed = render_confirm_dialog(ui, &cfg, &mut self.show_confirm)
            {
                self.pending_exit = true;
            }
        }
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run app");
}

// ─── wgpu + winit + imgui + chrome boilerplate ──────────────────────────────
//
// dear-app 0.17 owns the window and its `Application`/frame contexts lend only
// `&Window` (and none in `frame()`), but `chrome` needs `&Arc<Window>` plus the
// window every frame for `render`. So this demo drives its own winit + wgpu loop
// (see demo_chrome for the base harness) and wires chrome into it directly.

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_cfg: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    context: dear_imgui_rs::Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    chrome: Chrome,
    app: DemoApp,
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
                        .with_title("NavPanel Demo")
                        .with_decorations(false),
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
            apply_limit_buckets: false,
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
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };
        surface.configure(&device, &surface_cfg);

        let mut context = dear_imgui_rs::Context::create();
        let _ = context.set_ini_filename(None::<std::path::PathBuf>);

        let mut platform = WinitPlatform::new(&mut context).expect("winit platform");
        platform
            .attach_window(window.clone(), HiDpiMode::Default, &mut context)
            .expect("attach window");

        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_cfg.format),
            &mut context,
        )
        .expect("renderer");

        // Borderless chrome: strip OS decorations, apply DWM dark mode +
        // rounded corners. Runs once, now that the window exists.
        let mut chrome = Chrome::new(TitlebarConfig::default().with_close_confirm())
            .with_title("NavPanel Demo")
            .with_theme(Theme::Dark);
        chrome.on_setup(&window);

        self.gpu = Some(GpuState {
            device,
            queue,
            window,
            surface_cfg,
            surface,
            context,
            platform,
            renderer,
            chrome,
            app: DemoApp::new(),
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

        // Chrome first — borderless drag / resize / maximise tracking plus its
        // Ctrl/Alt shortcut normalisation. It expects a full winit `Event`, so
        // wrap the `WindowEvent`; then forward to the imgui platform.
        let wrapped = winit::event::Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        gpu.chrome.on_event(&wrapped, &gpu.window, &mut gpu.context);
        let _ = gpu
            .platform
            .handle_window_event(&mut gpu.context, &gpu.window, &event);

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

                gpu.platform
                    .prepare_frame(&mut gpu.context, &gpu.window)
                    .expect("prepare_frame");

                let ui = gpu.context.frame();

                // Borderless titlebar + host content in one full-display root.
                gpu.chrome.render(ui, &gpu.window, |ui, _area| {
                    gpu.app.render(ui);
                });

                // Bridge demo state ↔ chrome.
                if let Some(theme) = gpu.app.take_pending_theme() {
                    gpu.chrome.set_theme(theme);
                }
                if gpu.chrome.take_close_request().is_some() {
                    gpu.app.on_close_clicked();
                }
                if gpu.app.should_exit() {
                    std::process::exit(0);
                }

                gpu.platform
                    .prepare_render(ui, &gpu.window)
                    .expect("prepare_render");

                let consumer = gpu.renderer.renderer_consumer().expect("renderer consumer");
                let pending = gpu.context.render(consumer);
                let framebuffer_extent =
                    dear_imgui_wgpu::FramebufferExtent::from_texture(&frame.texture);

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
                    gpu.renderer
                        .render(pending, &mut pass, framebuffer_extent)
                        .expect("render");
                }
                gpu.queue.submit(Some(encoder.finish()));
                gpu.queue.present(frame);
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
