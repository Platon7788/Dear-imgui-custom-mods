//! Demonstrates the zero-boilerplate `AppWindow` + `AppHandler` API,
//! with the `notifications` toast-center integrated end-to-end.
//!
//! Run with:
//!   cargo run --example demo_app_window
//!
//! This example shows a full borderless application built with minimal
//! boilerplate. The `AppHandler` trait only requires `render()` — all other
//! methods are optional. A `NotificationCenter` is driven by every meaningful
//! user action so you can see the different severities, placements, custom
//! colors, actions, sticky toasts, and pause-on-hover behavior live.

use dear_imgui_custom_mod::app_window::{AppConfig, AppHandler, AppState, AppWindow, StartPosition};
use dear_imgui_custom_mod::confirm_dialog::{DialogConfig, DialogIcon, DialogResult, render_confirm_dialog};
use dear_imgui_custom_mod::notifications::{
    AnimationKind, NotificationCenter, NotificationEvent, Notification, Placement,
};
use dear_imgui_custom_mod::theme::Theme;
use dear_imgui_rs::Ui;

// ── Application state ─────────────────────────────────────────────────────────

struct DemoApp {
    counter:      i32,
    show_confirm: bool,
    log:          Vec<String>,
    current_theme: Theme,

    // Notification center — persists across frames.
    center:       NotificationCenter,
    // Simulation time — fed into `center.render(ui, dt)`.
    last_time:    std::time::Instant,
    // Combo selections for the demo toolbar.
    placement_idx: usize,
    animation_idx: usize,
}

impl Default for DemoApp {
    fn default() -> Self {
        // TopRight default — push the toasts below the 28 px borderless
        // titlebar so they don't visually overlap the title chrome.
        let mut center = NotificationCenter::new();
        center.config_mut().margin = [16.0, 44.0];
        Self {
            counter: 0,
            show_confirm: false,
            log: Vec::new(),
            current_theme: Theme::default(),
            center,
            last_time: std::time::Instant::now(),
            placement_idx: 0,  // TopRight
            animation_idx: 0,  // Fade
        }
    }
}

impl DemoApp {
    fn push_log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 100 {
            self.log.drain(..1);
        }
    }
}

// Helpers mapping combo indices → enum variants.
const PLACEMENTS: &[(&str, Placement)] = &[
    ("TopRight",     Placement::TopRight),
    ("TopLeft",      Placement::TopLeft),
    ("BottomRight",  Placement::BottomRight),
    ("BottomLeft",   Placement::BottomLeft),
    ("TopCenter",    Placement::TopCenter),
    ("BottomCenter", Placement::BottomCenter),
];
const ANIMATIONS: &[(&str, AnimationKind)] = &[
    ("Fade",    AnimationKind::Fade),
    ("SlideIn", AnimationKind::SlideIn),
    ("None",    AnimationKind::None),
];

// ── AppHandler implementation ─────────────────────────────────────────────────

impl AppHandler for DemoApp {
    fn render(&mut self, ui: &Ui, state: &mut AppState) {
        // Compute dt once per frame.
        let now = std::time::Instant::now();
        let dt = (now - self.last_time).as_secs_f32();
        self.last_time = now;

        let [avail_w, avail_h] = ui.content_region_avail();

        // ── Left panel ────────────────────────────────────────────────────────
        let panel_w = (avail_w * 0.48).max(260.0);
        ui.child_window("##left")
            .size([panel_w, avail_h])
            .border(false)
            .build(ui, || {
                ui.text("AppWindow Demo");
                ui.separator();
                ui.spacing();

                // Counter
                ui.text(format!("Counter: {}", self.counter));
                ui.same_line();
                if ui.button("+") {
                    self.counter += 1;
                    self.push_log(format!("Counter → {}", self.counter));
                    self.center.push(
                        Notification::info(format!("Counter → {}", self.counter))
                            .with_duration_secs(2.0)
                            .without_progress(),
                    );
                }
                ui.same_line();
                if ui.button("-") {
                    self.counter -= 1;
                    self.push_log(format!("Counter → {}", self.counter));
                    self.center.push(
                        Notification::info(format!("Counter → {}", self.counter))
                            .with_duration_secs(2.0)
                            .without_progress(),
                    );
                }
                ui.same_line();
                if ui.button("Reset") {
                    self.counter = 0;
                    self.push_log("Counter reset".to_string());
                    self.center.push(
                        Notification::warning("Counter reset")
                            .with_body("The counter has been rolled back to zero.")
                            .with_duration_secs(3.0),
                    );
                }

                ui.spacing();
                ui.separator();
                ui.spacing();

                // Window controls
                ui.text("Window");
                if ui.button("Maximize / Restore") {
                    state.toggle_maximized();
                    self.push_log("Toggled maximize".to_string());
                }
                if ui.button("Request Close") {
                    self.show_confirm = true;
                }

                ui.spacing();
                ui.separator();
                ui.spacing();

                // ── Notifications showcase ───────────────────────────────────
                ui.text("Notifications");

                if ui.button("Info") {
                    self.center.push(
                        Notification::info("New message")
                            .with_body("Click an action to respond."),
                    );
                }
                ui.same_line();
                if ui.button("Success") {
                    self.center.push(
                        Notification::success("Saved")
                            .with_body("Your changes have been written to disk.")
                            .with_duration_secs(3.0),
                    );
                }
                ui.same_line();
                if ui.button("Warning") {
                    self.center.push(
                        Notification::warning("Low disk space")
                            .with_body("Only 2.1 GB remaining on the system volume.")
                            .with_duration_secs(6.0),
                    );
                }
                ui.same_line();
                if ui.button("Error") {
                    self.center.push(
                        Notification::error("Sync failed")
                            .with_body("The remote rejected the push. Pull and retry.")
                            .with_action(1, "Retry")
                            .with_action(2, "Dismiss")
                            .sticky(),
                    );
                }
                ui.same_line();
                if ui.button("Debug") {
                    self.center.push(
                        Notification::debug("trace:frame")
                            .with_body(format!("dt={dt:.4}s, counter={}", self.counter))
                            .with_duration_secs(5.0),
                    );
                }

                ui.spacing();

                if ui.button("Sticky (no timer)") {
                    self.center.push(
                        Notification::info("Sticky notification")
                            .with_body("Close me with the × button.")
                            .sticky(),
                    );
                }
                ui.same_line();
                if ui.button("Custom color") {
                    self.center.push(
                        Notification::info("Magenta accent")
                            .with_body("Override the severity's default color.")
                            .with_custom_color([0.86, 0.30, 0.78, 1.0])
                            .with_duration_secs(5.0),
                    );
                }
                ui.same_line();
                if ui.button("With actions") {
                    self.center.push(
                        Notification::warning("Unsaved changes")
                            .with_body("Do you want to save before closing?")
                            .with_action(10, "Save")
                            .with_action(11, "Discard")
                            .sticky(),
                    );
                }

                ui.spacing();

                if ui.button("Burst ×5") {
                    for i in 0..5 {
                        self.center.push(
                            Notification::info(format!("Notification #{}", i + 1))
                                .with_body("Testing stack overflow behavior.")
                                .with_duration_secs(4.0 + i as f32 * 0.5),
                        );
                    }
                }
                ui.same_line();
                if ui.button("Dismiss all") {
                    self.center.dismiss_all();
                }

                ui.spacing();

                // Placement combo
                ui.text("Placement:");
                ui.same_line();
                ui.set_next_item_width(140.0);
                let current_p = PLACEMENTS[self.placement_idx].0;
                if let Some(_tok) = ui.begin_combo("##placement", current_p) {
                    for (i, (p_label, p)) in PLACEMENTS.iter().enumerate() {
                        let selected = i == self.placement_idx;
                        if ui.selectable_config(*p_label).selected(selected).build() {
                            self.placement_idx = i;
                            let cfg = self.center.config_mut();
                            cfg.placement = *p;
                            // Top-anchored placements must clear the 28 px
                            // borderless titlebar; bottom-anchored can hug
                            // the edge closely.
                            cfg.margin[1] = match *p {
                                Placement::TopRight
                                | Placement::TopLeft
                                | Placement::TopCenter => 44.0,
                                _ => 16.0,
                            };
                        }
                    }
                }

                // Animation combo
                ui.same_line();
                ui.text("Anim:");
                ui.same_line();
                ui.set_next_item_width(110.0);
                let current_a = ANIMATIONS[self.animation_idx].0;
                if let Some(_tok) = ui.begin_combo("##anim", current_a) {
                    for (i, (label, _)) in ANIMATIONS.iter().enumerate() {
                        let selected = i == self.animation_idx;
                        if ui.selectable_config(*label).selected(selected).build() {
                            self.animation_idx = i;
                            self.center.config_mut().animation = ANIMATIONS[i].1;
                        }
                    }
                }

                // Live tuning
                ui.spacing();
                let cfg = self.center.config_mut();
                let mut max_visible = cfg.max_visible as i32;
                ui.set_next_item_width(120.0);
                if ui.slider("Max visible", 1, 10, &mut max_visible) {
                    cfg.max_visible = max_visible.max(1) as usize;
                }
                ui.same_line();
                let mut pause = cfg.pause_on_hover;
                if ui.checkbox("Pause on hover", &mut pause) {
                    cfg.pause_on_hover = pause;
                }

                ui.spacing();
                ui.separator();
                ui.spacing();

                // Theme picker
                ui.text("Theme");
                let themes = [
                    ("Dark",      Theme::Dark),
                    ("Light",     Theme::Light),
                    ("Midnight",  Theme::Midnight),
                    ("Solarized", Theme::Solarized),
                    ("Monokai",   Theme::Monokai),
                ];
                for (label, theme) in &themes {
                    let active = std::mem::discriminant(&self.current_theme)
                        == std::mem::discriminant(theme);
                    if active {
                        let _c = ui.push_style_color(
                            dear_imgui_rs::StyleColor::Button,
                            [0.3, 0.5, 0.9, 1.0],
                        );
                        ui.button(label);
                    } else if ui.button(label) {
                        self.current_theme = *theme;
                        state.set_theme(*theme);
                        self.push_log(format!("Theme → {label}"));
                    }
                    ui.same_line();
                }
                ui.new_line();
            });

        // ── Right panel: log ──────────────────────────────────────────────────
        ui.same_line();
        ui.child_window("##right")
            .size([0.0, avail_h])
            .border(true)
            .build(ui, || {
                ui.text("Event log");
                ui.separator();
                for entry in &self.log {
                    ui.text_wrapped(entry);
                }
                if ui.scroll_y() >= ui.scroll_max_y() {
                    ui.set_scroll_here_y(1.0);
                }
            });

        // ── Close confirmation dialog ─────────────────────────────────────────
        if self.show_confirm {
            let dlg_theme = self.current_theme;
            let cfg = DialogConfig::new("Close Application", "Are you sure you want to close?")
                .with_icon(DialogIcon::Warning)
                .with_confirm_label("Close")
                .with_cancel_label("Cancel")
                .with_theme(dlg_theme);

            match render_confirm_dialog(ui, &cfg, &mut self.show_confirm) {
                DialogResult::Confirmed => state.exit(),
                DialogResult::Cancelled | DialogResult::Open => {}
            }
        }

        // ── Notification center — render last so toasts stay on top ──────────
        // Also keep the center's theme in sync with the app theme.
        self.center.config_mut().theme = self.current_theme;
        let events = self.center.render(ui, dt);

        // Convert events into log entries.
        for ev in events {
            match ev {
                NotificationEvent::Dismissed(id) => {
                    self.push_log(format!("toast #{id} dismissed"));
                }
                NotificationEvent::ActionClicked { id, action_id } => {
                    self.push_log(format!("toast #{id} action {action_id} clicked"));
                }
                NotificationEvent::Clicked(id) => {
                    self.push_log(format!("toast #{id} body clicked"));
                }
            }
        }
    }

    fn on_close_requested(&mut self, _state: &mut AppState) {
        self.show_confirm = true;
    }

    fn on_theme_changed(&mut self, theme: &Theme, _state: &mut AppState) {
        self.current_theme = *theme;
        self.push_log(format!("Theme changed to {theme:?}"));
        self.center.push(
            Notification::info(format!("Theme → {}", theme.display_name()))
                .with_duration_secs(2.0)
                .without_progress(),
        );
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let config = AppConfig::new("AppWindow + Notifications Demo", 1100.0, 680.0)
        .with_min_size(700.0, 420.0)
        .with_fps_limit(60)
        .with_start_position(StartPosition::CenterScreen)
        .with_theme(Theme::Dark);

    AppWindow::new(config)
        .run(DemoApp::default())
        .expect("event loop error");
}
