//! Demo: NavPanel + StatusBar — full navigation + status integration test.
//!
//! Run with:
//!   cargo run --example demo_nav_panel

use dear_imgui_custom_mod::app_window::{
    AppConfig, AppHandler, AppState, AppWindow, StartPosition,
};
use dear_imgui_custom_mod::confirm_dialog::{
    DialogConfig, DialogIcon, DialogResult, render_confirm_dialog,
};
use dear_imgui_custom_mod::nav_panel::*;
use dear_imgui_custom_mod::status_bar::{Indicator, StatusBar, StatusBarConfig, StatusItem};
use dear_imgui_custom_mod::theme::Theme;
use dear_imgui_rs::{StyleColor, StyleVar, Ui};

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

    fn cycle_theme(&mut self, state: &mut AppState) {
        let next = match &self.current_theme {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Midnight,
            Theme::Midnight => Theme::Solarized,
            Theme::Solarized => Theme::Monokai,
            _ => Theme::Dark,
        };
        self.current_theme = next;
        state.set_theme(next);
        self.push_log(format!("Theme -> {:?}", self.current_theme));
    }
}

// ── AppHandler ───────────────────────────────────────────────────────────────

impl AppHandler for DemoApp {
    fn render(&mut self, ui: &Ui, state: &mut AppState) {
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

                        let page = self.nav_state.active.unwrap_or("none");
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
                    if *id == "home" && self.notification > 0 {
                        self.notification = 0;
                        self.push_log("Notifications cleared".to_string());
                    }
                }
                NavEvent::SubMenuClicked(_btn, item) => {
                    self.push_log(format!("Submenu: {item}"));
                    match *item {
                        "theme" => self.cycle_theme(state),
                        "about" => self.push_log("NavPanel Demo v0.6.1".to_string()),
                        _ => {}
                    }
                }
                NavEvent::ToggleClicked(v) => {
                    self.push_log(format!("Toggle: {v}"));
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
                state.exit();
            }
        }
    }

    fn on_close_requested(&mut self, _state: &mut AppState) {
        self.show_confirm = true;
    }

    fn on_theme_changed(&mut self, theme: &Theme, _state: &mut AppState) {
        self.current_theme = *theme;
        self.push_log(format!("Theme: {:?}", theme));
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    let config = AppConfig::new("NavPanel Demo", 1100.0, 700.0)
        .with_min_size(800.0, 500.0)
        .with_fps_limit(60)
        .with_start_position(StartPosition::CenterScreen)
        .with_theme(Theme::Dark);

    AppWindow::new(config)
        .run(DemoApp::new())
        .expect("event loop error");
}
