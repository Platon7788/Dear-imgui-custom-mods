//! Demo showing all four window kinds:
//!   `cargo run --example demo_app_window -- splash`
//!   `cargo run --example demo_app_window -- tool`
//!   `cargo run --example demo_app_window -- dialog`
//!   `cargo run --example demo_app_window -- main`   (default)
//!
//! Each variant is a separate run — you can't have multiple AppWindows in one
//! winit event loop without further plumbing.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

use dear_imgui_custom_mod::app_window::{
    AppConfig, AppHandler, AppState, AppWindow, ExtraButton, Theme,
};
use dear_imgui_custom_mod::confirm_dialog::{
    ConfirmStyle, DialogConfig, DialogIcon, DialogResult, render_confirm_dialog,
};
use dear_imgui_custom_mod::nav_panel::{
    DockPosition, NavButton, NavEvent, NavPanelConfig, NavPanelState, SubMenuItem, render_nav_panel,
};
use dear_imgui_custom_mod::status_bar::{
    Indicator, StatusBar, StatusBarConfig, StatusItem, StatusSection,
};
use dear_imgui_rs::{StyleVar, Ui};

// ── 1) Splash window ──────────────────────────────────────────────────────────
//
// No chrome at all — borderless, centred, stays on top, auto-closes after 3s.
// Demonstrates: with_opacity, with_corner_radius, with_auto_close.

struct Splash {
    started: std::time::Instant,
}
impl Default for Splash {
    fn default() -> Self {
        Self {
            started: std::time::Instant::now(),
        }
    }
}
impl AppHandler for Splash {
    fn render(&mut self, ui: &Ui, state: &mut AppState) {
        // Splash has a continuous progress animation tied to wall-clock
        // time. In event-driven mode we must keep asking the host to
        // render the next frame — otherwise the bar would freeze on the
        // 2-second idle pulse.
        state.keep_alive(1);

        let elapsed = self.started.elapsed().as_secs_f32();
        let total = 3.0_f32;
        let progress = (elapsed / total).clamp(0.0, 1.0);

        let avail = ui.content_region_avail();
        ui.set_cursor_pos([avail[0] * 0.5 - 60.0, avail[1] * 0.4]);
        ui.text("YOUR LOGO HERE");

        ui.set_cursor_pos([40.0, avail[1] - 40.0]);
        ui.text(format!("Loading…  {:>3.0}%", progress * 100.0));

        let dl = ui.get_window_draw_list();
        let win = ui.window_pos();
        let pad = 40.0;
        let y = win[1] + avail[1] - 18.0;
        let x0 = win[0] + pad;
        let x1 = win[0] + avail[0] - pad;
        dl.add_rect([x0, y], [x1, y + 4.0], 0x33FF_FFFFu32)
            .filled(true)
            .rounding(2.0)
            .build();
        dl.add_rect(
            [x0, y],
            [x0 + (x1 - x0) * progress, y + 4.0],
            0xFFFF_FFFFu32,
        )
        .filled(true)
        .rounding(2.0)
        .build();
    }
}

fn run_splash() {
    let cfg = AppConfig::splash("Splash", 600.0, 380.0)
        .with_theme(Theme::Midnight)
        .with_corner_radius(16)
        .with_opacity(0.92)                    // semi-transparent splash
        .with_auto_close(Duration::from_secs(3));
    AppWindow::new(cfg).run(Splash::default()).unwrap();
}

// ── 2) Tool window ────────────────────────────────────────────────────────────
//
// Compact titlebar, close-only, stays on top.
// Demonstrates: tool preset, stay_on_top.

struct ToolApp {
    search: String,
    items: Vec<&'static str>,
}
impl Default for ToolApp {
    fn default() -> Self {
        Self {
            search: String::new(),
            items: vec![
                "Color",
                "Font",
                "Spacing",
                "Border",
                "Shadow",
                "Animation",
                "Theme",
                "Layout",
                "Position",
                "Size",
            ],
        }
    }
}
impl AppHandler for ToolApp {
    fn render(&mut self, ui: &Ui, _state: &mut AppState) {
        ui.input_text("##search", &mut self.search)
            .hint("Filter…")
            .build();
        ui.spacing();
        let s = self.search.to_ascii_lowercase();
        for &item in &self.items {
            if s.is_empty() || item.to_ascii_lowercase().contains(&s) {
                ui.bullet_text(item);
            }
        }
    }
}

fn run_tool() {
    let cfg = AppConfig::tool("Properties", 320.0, 460.0)
        .with_theme(Theme::Dark)
        .stay_on_top();
    AppWindow::new(cfg).run(ToolApp::default()).unwrap();
}

// ── 3) Dialog window ──────────────────────────────────────────────────────────
//
// Fixed size, close-only, screen-centred, on top.
// Demonstrates: dialog preset, confirm/cancel pattern.

#[derive(Default)]
struct DialogApp {
    result: Option<bool>,
}
impl AppHandler for DialogApp {
    fn render(&mut self, ui: &Ui, state: &mut AppState) {
        ui.spacing();
        ui.text("Discard unsaved changes?");
        ui.spacing();
        ui.text_disabled("Your work will be lost.");

        let avail = ui.content_region_avail();
        ui.set_cursor_pos([avail[0] - 220.0, avail[1] - 32.0]);
        if ui.button("  Discard  ") {
            self.result = Some(true);
            state.exit();
        }
        ui.same_line();
        if ui.button("  Cancel   ") {
            self.result = Some(false);
            state.exit();
        }
    }
}

fn run_dialog() {
    let cfg = AppConfig::dialog("Confirm", 420.0, 160.0).with_theme(Theme::Light);
    AppWindow::new(cfg).run(DialogApp::default()).unwrap();
}

// ── 4) Main window ────────────────────────────────────────────────────────────
//
// Full custom chrome, extra buttons, close-confirm, dynamic title and opacity.
// Now also integrates `nav_panel` (left dock) and `status_bar` (bottom strip)
// to showcase a complete app skeleton.

struct MainApp {
    counter: i32,
    show_confirm: bool,
    theme: Theme,
    opacity: f32,
    page: &'static str,
    nav_state: NavPanelState,
    status_bar: StatusBar,
    notifications: u32,
    /// Last file path dropped onto the window — populated from
    /// `on_window_event` reading `WindowEvent::DroppedFile`.
    last_dropped: Option<std::path::PathBuf>,
    /// Counter incremented from a 1 Hz background thread; the thread
    /// also calls [`AppProxy::wake`] so the UI repaints to show the
    /// new value. Demonstrates cross-thread state delivery via shared
    /// atomics + proxy wake — no polling, no idle burn.
    background_ticks: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// Background-thread join handle. Held so the spawned proxy clone
    /// is not dropped early.
    _bg_thread: Option<std::thread::JoinHandle<()>>,
}

impl MainApp {
    fn new() -> Self {
        let mut nav_state = NavPanelState::new();
        nav_state.set_active("home");

        let mut status_bar = StatusBar::new("##v2_status");
        status_bar.config = StatusBarConfig {
            height: 22.0,
            ..StatusBarConfig::default()
        };
        // Pre-populate with steady items; we rebuild dynamic ones every frame
        // in `render` since they reflect mutating state (counter / theme / etc.).
        status_bar.left(StatusItem::indicator("Ready", Indicator::Success));
        status_bar.right(StatusItem::text("UTF-8"));
        status_bar.right(StatusItem::text("v0.9.0"));

        Self {
            counter: 0,
            show_confirm: false,
            theme: Theme::Dark,
            opacity: 1.0,
            page: "home",
            nav_state,
            status_bar,
            notifications: 3,
            last_dropped: None,
            background_ticks: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
            _bg_thread: None,
        }
    }

    fn build_nav(&self) -> NavPanelConfig {
        NavPanelConfig::new(DockPosition::Left)
            .with_theme(self.theme)
            .with_width(48.0)
            .with_button_size(36.0)
            .add_button(NavButton::action("home", "H", "Home"))
            .add_button(NavButton::action("counter", "+", "Counter"))
            .add_button(
                NavButton::action("alerts", "!", "Alerts")
                    .with_badge(self.notifications.to_string()),
            )
            .add_separator()
            .add_button(
                NavButton::submenu("more", "*", "More")
                    .add_item(SubMenuItem::new("about", "About"))
                    .add_item(SubMenuItem::new("docs", "Documentation"))
                    .add_separator()
                    .add_item(SubMenuItem::new("github", "GitHub repo")),
            )
            .add_button(NavButton::action("exit", "X", "Exit"))
    }

    fn refresh_status_bar(&mut self) {
        // Dynamic right-hand items that need to reflect live state.
        // Easiest is to clear + repopulate every frame; the bar's items are
        // a few `StatusItem` values, the cost is negligible.
        self.status_bar.clear();
        let (label, ind) = match self.notifications {
            0 => ("Idle", Indicator::Info),
            1..=4 => ("Active", Indicator::Success),
            _ => ("Busy", Indicator::Warning),
        };
        self.status_bar.left(StatusItem::indicator(label, ind));
        self.status_bar
            .left(StatusItem::text(format!("Page: {}", self.page)));
        self.status_bar
            .left(StatusItem::text(format!("Counter = {}", self.counter)));

        self.status_bar.right(
            StatusItem::clickable(format!("{:?}", self.theme)).with_tooltip("Click to cycle theme"),
        );
        self.status_bar.right(StatusItem::text(format!(
            "Opacity {:.0}%",
            self.opacity * 100.0
        )));
        self.status_bar.right(StatusItem::text("UTF-8"));
    }

    fn handle_nav_events(&mut self, events: Vec<NavEvent>, state: &mut AppState) {
        for ev in events {
            match ev {
                // `id` is `Cow<'static, str>` — match on the `&str` slice via
                // `as_ref()` so static and runtime IDs work identically.
                NavEvent::ButtonClicked(id) => match id.as_ref() {
                    "exit" => state.exit(),
                    "alerts" => self.notifications = 0,
                    "home" => self.page = "home",
                    "counter" => self.page = "counter",
                    _ => {}
                },
                NavEvent::SubMenuClicked(_, item) => match item.as_ref() {
                    "about" => self.page = "about",
                    "docs" => self.page = "docs",
                    _ => {}
                },
                NavEvent::ToggleClicked(_) => {}
            }
        }
    }

    fn render_page(&mut self, ui: &Ui, state: &mut AppState) {
        match self.page {
            "home" => {
                ui.text("Welcome to AppWindow demo.");
                ui.text_disabled("Click the left-dock buttons to switch pages.");
                ui.spacing();
                ui.separator();
                ui.text("Cross-thread proxy demo");
                ui.text_disabled(
                    "A background thread wakes the loop once per second; \
                     the counter advances even when the UI is otherwise idle.",
                );
                ui.text(format!(
                    "Background ticks: {}",
                    self.background_ticks
                        .load(std::sync::atomic::Ordering::Relaxed)
                ));
                ui.spacing();
                ui.separator();
                ui.text("Drag-drop hook demo");
                ui.text_disabled(
                    "Drop a file onto the window. `on_window_event` reads \
                     `WindowEvent::DroppedFile` — the path is invisible \
                     without that hook.",
                );
                match &self.last_dropped {
                    Some(path) => ui.text(format!("Last drop: {}", path.display())),
                    None => ui.text_disabled("(no file dropped yet)"),
                }
            }
            "counter" => {
                ui.text(format!("Counter: {}", self.counter));
                ui.separator();
                if ui.button("  +  ") {
                    self.counter += 1;
                }
                ui.same_line();
                if ui.button("  -  ") {
                    self.counter -= 1;
                }
                ui.same_line();
                if ui.button(" Reset ") {
                    self.counter = 0;
                }
            }
            "alerts" => {
                ui.text("Notification centre");
                ui.separator();
                ui.text(format!("Pending: {}", self.notifications));
                if ui.button("Add notification") {
                    self.notifications += 1;
                }
                ui.same_line();
                if ui.button("Dismiss all") {
                    self.notifications = 0;
                }
            }
            "about" => {
                ui.text("dear-imgui-custom-mod  ·  app_window demo");
                ui.text_disabled("Showcasing nav_panel + status_bar + confirm_dialog integration.");
            }
            "docs" => {
                ui.text("Documentation");
                ui.text_disabled("See docs/app_window.md and the rustdoc on AppConfig.");
            }
            other => {
                ui.text(format!("Unknown page: {other}"));
            }
        }

        // Window controls (always visible at the bottom of the page).
        ui.separator();
        if ui.button("Minimize") {
            state.minimize();
        }
        ui.same_line();
        let lbl = if state.titlebar.maximized {
            "Restore"
        } else {
            "Maximize"
        };
        if ui.button(lbl) {
            state.toggle_maximized();
        }
        ui.same_line();
        if ui.button("Exit (close-confirm)") {
            self.show_confirm = true;
        }
        ui.same_line();
        if ui.button("Set title = counter") {
            state.set_title(format!("App Window Demo  [counter = {}]", self.counter));
        }

        // Theme radio.
        ui.spacing();
        ui.text("Theme:");
        ui.same_line();
        for &(label, theme) in &[
            ("Dark", Theme::Dark),
            ("Midnight", Theme::Midnight),
            ("Light", Theme::Light),
            ("Solarized", Theme::Solarized),
            ("Monokai", Theme::Monokai),
        ] {
            if ui.radio_button_bool(label, self.theme == theme) {
                self.theme = theme;
                state.set_theme(theme);
            }
            ui.same_line();
        }
        ui.new_line();

        // Opacity.
        ui.text("Opacity:");
        ui.same_line();
        if ui.slider("##opacity", 0.2_f32, 1.0, &mut self.opacity) {
            state.set_opacity(self.opacity);
        }
    }
}

impl AppHandler for MainApp {
    fn render(&mut self, ui: &Ui, state: &mut AppState) {
        // Close-confirm modal — uses the polished `confirm_dialog` widget.
        if self.show_confirm {
            let cfg = DialogConfig::new("Close application?", "Any unsaved work will be lost.")
                .with_icon(DialogIcon::Warning)
                .with_confirm_label("Close")
                .with_cancel_label("Cancel")
                .with_confirm_style(ConfirmStyle::Destructive)
                .with_theme(self.theme)
                .with_button_height(24.0)
                .with_button_gap(44.0)
                .with_border_thickness(0.75)
                .with_rounding(8.0)
                .with_padding(18.0);

            match render_confirm_dialog(ui, &cfg, &mut self.show_confirm) {
                DialogResult::Confirmed => state.confirm_close(),
                DialogResult::Cancelled => {}
                DialogResult::Open => {}
            }
        }

        // ── Layout: [ nav │ content ] on top, [ status_bar ] at the bottom ───
        let [avail_w, avail_h] = ui.content_region_avail();
        let status_h = self.status_bar.config.height;
        let main_h = (avail_h - status_h).max(0.0);

        // The nested child_windows live inside `##app_content` (which already
        // has WindowPadding [8, 8] pushed by the framework). Re-using the same
        // padding for `##v2_main` and `##v2_content` would *stack* the gutters
        // — the inner child would end up wider than its parent and ImGui would
        // add a phantom scrollbar. Push WindowPadding=[0,0] for the outer
        // `##v2_main` so nav + content butt right up against the framework
        // padding, and let `##v2_content` provide the real content padding.
        let _no_pad = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));

        let mut nav_events = Vec::new();
        ui.child_window("##v2_main")
            .size([avail_w, main_h])
            .border(false)
            .build(ui, || {
                let nav_cfg = self.build_nav();
                let nav_result = render_nav_panel(ui, &nav_cfg, &mut self.nav_state);
                nav_events = nav_result.events;
                let nav_w = nav_result.occupied_size[0];

                // Cursor is now after the nav panel — place content to its right.
                let [cx, cy] = ui.cursor_pos();
                ui.set_cursor_pos([cx + nav_w, cy]);

                // Use the *actual* remaining space inside `##v2_main`, not a
                // figure derived from the outer scope's `avail_w` (which
                // doesn't account for `##v2_main`'s own inner padding).
                let inner = ui.content_region_avail();
                let _pad = ui.push_style_var(StyleVar::WindowPadding([10.0, 8.0]));
                let _spc = ui.push_style_var(StyleVar::ItemSpacing([6.0, 6.0]));
                ui.child_window("##v2_content")
                    .size(inner)
                    .border(false)
                    .build(ui, || {
                        self.render_page(ui, state);
                    });
            });
        drop(_no_pad);

        // Drain nav events outside the child_window to avoid double-borrow on `self`.
        self.handle_nav_events(nav_events, state);

        // ── Status bar ─────────────────────────────────────────────────────────
        self.refresh_status_bar();
        // The right-section is laid out as: [theme, opacity, "UTF-8"]. The
        // theme item is at index 0 and the only clickable one — using
        // `(section, index)` is more robust than label-string matching.
        for ev in self.status_bar.render(ui) {
            if ev.section == StatusSection::Right && ev.index == 0 {
                self.theme = self.theme.next();
                state.set_theme(self.theme);
            }
        }
    }

    fn on_close_requested(&mut self, _state: &mut AppState) {
        self.show_confirm = true;
    }

    fn on_extra_button(&mut self, id: &'static str, state: &mut AppState) {
        if id == "theme" {
            // `Theme::next()` cycles through every variant in `Theme::ALL`,
            // so adding new themes (e.g. Catppuccin / Nord) automatically
            // joins the rotation without needing this match arm to grow.
            self.theme = self.theme.next();
            state.set_theme(self.theme);
        }
    }

    /// Demonstrates [`AppProxy`] — spawn a 1 Hz background heartbeat
    /// that wakes the (otherwise idle) UI loop via `proxy.wake()`. The
    /// counter advances even though the user is not interacting; this is
    /// the canonical pattern for HTTP / file-watch / IPC clients.
    fn on_ready(&mut self, state: &mut AppState) {
        let proxy = state.proxy();
        let counter = std::sync::Arc::clone(&self.background_ticks);
        self._bg_thread = Some(std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if proxy.wake().is_err() {
                    break; // event loop closed
                }
            }
        }));
    }

    /// Demonstrates [`AppHandler::on_window_event`] — read raw winit
    /// events the framework would otherwise hide. Most notably,
    /// **`DroppedFile`** delivers the actual `PathBuf`; without this
    /// hook the path is invisible to the handler.
    fn on_window_event(
        &mut self,
        event: &dear_imgui_custom_mod::winit::event::WindowEvent,
        _state: &mut AppState,
    ) -> bool {
        use dear_imgui_custom_mod::winit::event::WindowEvent;
        if let WindowEvent::DroppedFile(path) = event {
            self.last_dropped = Some(path.clone());
        }
        // Don't consume — let ImGui still see the event for hover/cursor.
        false
    }
}

fn run_main() {
    // 16×16 RGBA gradient icon — demonstrates `with_window_icon_rgba` end-to-end.
    let icon = (0..16 * 16)
        .flat_map(|i| {
            let x = i % 16;
            let y = i / 16;
            let r = (x * 16) as u8;
            let g = (y * 16) as u8;
            [r, g, 255 - r, 255]
        })
        .collect::<Vec<u8>>();

    let cfg = AppConfig::main("App Window Demo", 1100.0, 680.0)
        .with_theme(Theme::Dark)
        .with_close_confirm()
        .with_window_icon_rgba(icon, 16, 16)
        .with_extra_button(
            ExtraButton::new("theme", "T", [0.6, 0.85, 1.0, 1.0]).with_tooltip("Cycle theme"),
        );
    // ─── Render-mode reference ───────────────────────────────────────────
    // Default = event-driven: idle pulse 2 s foreground, 5 s background.
    // CPU/GPU usage drops to ≈0% while the user is reading the screen.
    //
    // Tweak with builders (any one of these replaces the strategy):
    //
    //     .with_idle_pulse(Duration::from_millis(500))      // 2 fps clock pulse
    //     .with_unfocused_idle_pulse(Duration::from_secs(10)) // calmer background
    //     .without_idle_pulse()                              // input-only foreground
    //     .event_driven_minimal()                            // strictly zero idle
    //
    // Game-style alternative — always render at vsync:
    //
    //     .continuous_render()
    //     .with_fps_limit(60)                                // explicit cap
    AppWindow::new(cfg).run(MainApp::new()).unwrap();
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let kind = std::env::args().nth(1).unwrap_or_else(|| "main".into());
    match kind.as_str() {
        "splash" => run_splash(),
        "tool" => run_tool(),
        "dialog" => run_dialog(),
        _ => run_main(),
    }
}
