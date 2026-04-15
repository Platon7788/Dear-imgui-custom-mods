//! Demonstrates the zero-boilerplate `AppWindow` + `AppHandler` API.
//!
//! Run with:
//!   cargo run --example demo_app_window
//!
//! This example shows a full borderless application built with minimal
//! boilerplate. The `AppHandler` trait only requires `render()` — all other
//! methods are optional.

use dear_imgui_custom_mod::app_window::{AppConfig, AppHandler, AppState, AppWindow, StartPosition};
use dear_imgui_custom_mod::borderless_window::TitlebarTheme;
use dear_imgui_rs::Ui;

// ── Application state ─────────────────────────────────────────────────────────

struct DemoApp {
    counter:      i32,
    show_confirm: bool,
    log:          Vec<String>,
    current_theme: TitlebarTheme,
}

impl Default for DemoApp {
    fn default() -> Self {
        Self {
            counter:       0,
            show_confirm:  false,
            log:           Vec::new(),
            current_theme: TitlebarTheme::default(),
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
                    ("Dark",      TitlebarTheme::Dark),
                    ("Light",     TitlebarTheme::Light),
                    ("Midnight",  TitlebarTheme::Midnight),
                    ("Nord",      TitlebarTheme::Nord),
                    ("Solarized", TitlebarTheme::Solarized),
                    ("Monokai",   TitlebarTheme::Monokai),
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
                        self.current_theme = theme.clone();
                        state.set_theme(theme.clone());
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
            let [dw, dh] = ui.io().display_size();
            const DLG_W: f32 = 280.0;
            const DLG_H: f32 = 110.0;
            const PAD:   f32 = 12.0;
            const BTN_W: f32 = 100.0;
            const GAP:   f32 = 12.0;

            let _pad = ui.push_style_var(dear_imgui_rs::StyleVar::WindowPadding([PAD, PAD]));
            ui.window("##close_confirm")
                .position(
                    [dw * 0.5 - DLG_W * 0.5, dh * 0.5 - DLG_H * 0.5],
                    dear_imgui_rs::Condition::Always,
                )
                .size([DLG_W, DLG_H], dear_imgui_rs::Condition::Always)
                .flags(
                    dear_imgui_rs::WindowFlags::NO_TITLE_BAR
                        | dear_imgui_rs::WindowFlags::NO_RESIZE
                        | dear_imgui_rs::WindowFlags::NO_MOVE
                        | dear_imgui_rs::WindowFlags::NO_SCROLLBAR,
                )
                .build(|| {
                    let content_w = DLG_W - PAD * 2.0;
                    let msg = "Close the application?";
                    let msg_w = ui.current_font().calc_text_size(
                        ui.current_font_size(), f32::MAX, -1.0, msg,
                    )[0];
                    let [_, ty] = ui.cursor_pos();
                    ui.set_cursor_pos([(content_w - msg_w) * 0.5, ty]);
                    ui.text(msg);

                    ui.spacing();
                    ui.separator();
                    ui.spacing();

                    let total = BTN_W * 2.0 + GAP;
                    let btn_x = (content_w - total) * 0.5;
                    let [_, by] = ui.cursor_pos();

                    ui.set_cursor_pos([btn_x, by]);
                    if ui.button("   Cancel   ") {
                        self.show_confirm = false;
                    }
                    ui.same_line();
                    ui.set_cursor_pos([btn_x + BTN_W + GAP, by]);
                    if ui.button("    Close    ") {
                        state.exit();
                    }
                });
        }
    }

    // ── Close button / OS close request ───────────────────────────────────────
    fn on_close_requested(&mut self, _state: &mut AppState) {
        // Show our own confirm dialog instead of closing immediately.
        self.show_confirm = true;
    }

    // ── Theme change notification ──────────────────────────────────────────────
    fn on_theme_changed(&mut self, theme: &TitlebarTheme, _state: &mut AppState) {
        self.current_theme = theme.clone();
        self.push_log(format!("Theme changed to {:?}", theme));
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let config = AppConfig::new("AppWindow Demo", 1000.0, 640.0)
        .with_min_size(600.0, 380.0)
        .with_fps_limit(60)
        .with_start_position(StartPosition::CenterScreen)
        .with_theme(TitlebarTheme::Dark);

    AppWindow::new(config)
        .run(DemoApp::default())
        .expect("event loop error");
}
