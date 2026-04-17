//! Demonstrates the zero-boilerplate `AppWindow` + `AppHandler` API.
//!
//! Run with:
//!   cargo run --example demo_app_window
//!
//! This example shows a full borderless application built with minimal
//! boilerplate. The `AppHandler` trait only requires `render()` — all other
//! methods are optional.

use dear_imgui_custom_mod::app_window::{AppConfig, AppHandler, AppState, AppWindow, StartPosition};
use dear_imgui_custom_mod::confirm_dialog::{DialogConfig, DialogIcon, DialogResult, render_confirm_dialog};
use dear_imgui_custom_mod::theme::Theme;
use dear_imgui_rs::Ui;

// ── Application state ─────────────────────────────────────────────────────────

#[derive(Default)]
struct DemoApp {
    counter:      i32,
    show_confirm: bool,
    log:          Vec<String>,
    current_theme: Theme,
}

impl DemoApp {
    fn push_log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 100 {
            self.log.drain(..1);
        }
    }
}

// ── AppHandler implementation ─────────────────────────────────────────────────

impl AppHandler for DemoApp {
    // ── Main render callback ───────────────────────────────────────────────────
    fn render(&mut self, ui: &Ui, state: &mut AppState) {
        let [avail_w, avail_h] = ui.content_region_avail();

        // ── Left panel ────────────────────────────────────────────────────────
        let panel_w = (avail_w * 0.45).max(200.0);
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
                }
                ui.same_line();
                if ui.button("-") {
                    self.counter -= 1;
                    self.push_log(format!("Counter → {}", self.counter));
                }
                ui.same_line();
                if ui.button("Reset") {
                    self.counter = 0;
                    self.push_log("Counter reset".to_string());
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
    }

    // ── Close button / OS close request ───────────────────────────────────────
    fn on_close_requested(&mut self, _state: &mut AppState) {
        // Show our own confirm dialog instead of closing immediately.
        self.show_confirm = true;
    }

    // ── Theme change notification ──────────────────────────────────────────────
    fn on_theme_changed(&mut self, theme: &Theme, _state: &mut AppState) {
        self.current_theme = *theme;
        self.push_log(format!("Theme changed to {:?}", theme));
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let config = AppConfig::new("AppWindow Demo", 1000.0, 640.0)
        .with_min_size(600.0, 380.0)
        .with_fps_limit(60)
        .with_start_position(StartPosition::CenterScreen)
        .with_theme(Theme::Dark);

    AppWindow::new(config)
        .run(DemoApp::default())
        .expect("event loop error");
}
