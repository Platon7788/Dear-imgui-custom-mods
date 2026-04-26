//! demo_app_window_v2 — interactive smoke-test for the v2 native borderless
//! window on Win10 and Win11.
//!
//! Run with:
//! ```
//! cargo run --example demo_app_window_v2 --features app_window_v2
//! ```

use dear_imgui_custom_mod::app_window_v2::{
    AppConfigV2, AppHandlerV2, AppStateV2, AppWindowV2, CloseMode, ExtraButton, FpsMode, Theme,
    TitleAlign,
};
use dear_imgui_custom_mod::dear_imgui_rs::{Condition, StyleColor, Ui, WindowFlags};

// ── Test-case list ────────────────────────────────────────────────────────────

const TESTS: &[&str] = &[
    "Drag titlebar — smooth, no flicker",
    "Drag to screen edge → Aero Snap (left / right / top)",
    "Win11: drag to corner → quarter-snap",
    "Resize from any edge / corner (cursor auto-changes)",
    "Click Maximize → fills work area, NOT the taskbar",
    "Click Restore → window returns to previous size",
    "Drag works again after maximize → restore",
    "Win11: hover Maximize ~500 ms → Snap Layouts popup",
    "Click Minimize → animates to taskbar",
    "Click taskbar icon → restores correctly",
    "Maximize → Minimize → taskbar restore → back to maximized",
    "Double-click titlebar → toggle maximize / restore",
    "Win+Up / Win+Down / Win+Left / Win+Right",
    "Right-click titlebar → system menu",
    "Alt+F4 closes the window (confirm dialog appears)",
    "Unfocus → titlebar dims; refocus → undims (no flicker)",
    "Cycle theme (button or 'T' titlebar button) — colors update live",
    "Extra 'T' titlebar button increments counter",
];

// ── App handler ───────────────────────────────────────────────────────────────

struct Demo {
    checks:        [bool; 18],
    extra_count:   u32,
    last_extra:    Option<&'static str>,
    current_theme: Theme,
    confirm_open:  bool,
}

impl Demo {
    fn new() -> Self {
        Self {
            checks:        [false; 18],
            extra_count:   0,
            last_extra:    None,
            current_theme: Theme::Dark,
            confirm_open:  false,
        }
    }

    fn cycle_theme(&mut self, state: &mut AppStateV2) {
        self.current_theme = self.current_theme.next();
        state.set_theme(self.current_theme);
    }
}

impl AppHandlerV2 for Demo {
    fn render(&mut self, ui: &Ui, state: &mut AppStateV2) {
        let avail = ui.content_region_avail();

        // ── Confirm-close modal ──────────────────────────────────────────
        if self.confirm_open {
            let dw = 300.0f32;
            let dh = 110.0f32;
            let cx = (avail[0] - dw) * 0.5;
            let cy = (avail[1] - dh) * 0.5;

            ui.window("Close?")
                .position([cx, cy], Condition::Always)
                .size([dw, dh], Condition::Always)
                .flags(WindowFlags::NO_RESIZE | WindowFlags::NO_MOVE | WindowFlags::NO_COLLAPSE)
                .build(|| {
                    ui.dummy([0.0, 6.0]);
                    ui.text("  Close the window?");
                    ui.dummy([0.0, 10.0]);
                    ui.indent_by(20.0);
                    if ui.button_with_size("Yes — close", [110.0, 28.0]) {
                        state.confirm_close();
                        self.confirm_open = false;
                    }
                    ui.same_line_with_spacing(0.0, 8.0);
                    if ui.button_with_size("Cancel", [80.0, 28.0]) {
                        self.confirm_open = false;
                    }
                });
            return;
        }

        // ── Header ───────────────────────────────────────────────────────
        ui.dummy([0.0, 4.0]);
        ui.indent_by(14.0);
        ui.text("app_window_v2 — native borderless (Win32 WndProc subclass)");
        ui.unindent_by(14.0);
        ui.separator();
        ui.dummy([0.0, 4.0]);

        // ── Left/right split via same_line ────────────────────────────────
        let col_w = (avail[0] * 0.56).clamp(320.0, 600.0);

        // LEFT: test matrix
        ui.child_window("##tests")
            .size([col_w, avail[1] - 16.0])
            .build(ui, || {
                render_test_matrix(ui, &mut self.checks);
            });

        ui.same_line_with_spacing(0.0, 10.0);

        // RIGHT: controls + info
        ui.child_window("##info")
            .size([avail[0] - col_w - 10.0, avail[1] - 16.0])
            .build(ui, || {
                render_controls(ui, state, self);
                ui.dummy([0.0, 8.0]);
                render_diagnostics(ui, state);
            });
    }

    fn on_close_requested(&mut self, _state: &mut AppStateV2) {
        // Instead of closing immediately, show the confirm dialog.
        // `state.confirm_close()` is called inside the dialog's "Yes" button.
        self.confirm_open = true;
    }

    fn on_extra_button(&mut self, id: &'static str, state: &mut AppStateV2) {
        if id == "theme" {
            self.cycle_theme(state);
        }
        self.extra_count += 1;
        self.last_extra   = Some(id);
    }

    fn on_theme_changed(&mut self, theme: &Theme, _state: &mut AppStateV2) {
        self.current_theme = *theme;
    }
}

// ── Sub-renderers ─────────────────────────────────────────────────────────────

fn render_test_matrix(ui: &Ui, checks: &mut [bool; 18]) {
    ui.text("Test matrix — tick each item as you verify it:");
    ui.dummy([0.0, 4.0]);

    for (i, &label) in TESTS.iter().enumerate() {
        let _id = ui.push_id(i as i32);
        ui.checkbox(label, &mut checks[i]);
    }

    ui.dummy([0.0, 6.0]);
    let done  = checks.iter().filter(|&&c| c).count();
    let total = checks.len();
    ui.text(format!("Progress: {done} / {total}"));
    if done == total {
        ui.same_line();
        ui.text_colored([0.3, 1.0, 0.3, 1.0], " ALL PASS ✓");
    }
    ui.dummy([0.0, 4.0]);
    if ui.button("Reset") {
        *checks = [false; 18];
    }
}

fn render_controls(ui: &Ui, state: &mut AppStateV2, demo: &mut Demo) {
    ui.text("Window controls:");
    ui.dummy([0.0, 3.0]);

    if ui.button_with_size("Minimize", [88.0, 24.0]) {
        state.minimize();
    }
    ui.same_line_with_spacing(0.0, 6.0);
    let max_label = if state.titlebar.maximized { "Restore" } else { "Maximize" };
    if ui.button_with_size(max_label, [88.0, 24.0]) {
        state.toggle_maximized();
    }
    ui.same_line_with_spacing(0.0, 6.0);
    if ui.button_with_size("Close", [70.0, 24.0]) {
        state.exit();
    }

    ui.dummy([0.0, 8.0]);
    ui.separator();
    ui.dummy([0.0, 6.0]);
    ui.text("Theme:");
    ui.dummy([0.0, 3.0]);

    for &t in &[
        Theme::Dark, Theme::Light, Theme::Midnight, Theme::Solarized, Theme::Monokai,
    ] {
        let active = demo.current_theme == t;
        let col: [f32; 4] = if active {
            [0.20, 0.55, 0.20, 1.0]
        } else {
            [0.35, 0.35, 0.35, 1.0]
        };
        let _c = ui.push_style_color(StyleColor::Button, col);
        if ui.button_with_size(theme_name(t), [78.0, 22.0]) {
            demo.current_theme = t;
            state.set_theme(t);
        }
        drop(_c);
        ui.same_line_with_spacing(0.0, 4.0);
    }
    ui.new_line();

    ui.dummy([0.0, 8.0]);
    ui.separator();
    ui.dummy([0.0, 6.0]);
    ui.text("Extra-button log:");
    ui.text(format!("  Clicks : {}", demo.extra_count));
    ui.text(format!("  Last id: {}", demo.last_extra.unwrap_or("—")));
}

fn render_diagnostics(ui: &Ui, state: &AppStateV2) {
    ui.dummy([0.0, 4.0]);
    ui.separator();
    ui.dummy([0.0, 6.0]);
    ui.text("Window state:");
    ui.text(format!("  Focused  : {}", state.titlebar.focused));
    ui.text(format!("  Maximized: {}", state.titlebar.maximized));

    ui.dummy([0.0, 8.0]);
    ui.separator();
    ui.dummy([0.0, 6.0]);
    ui.text("Platform:");

    #[cfg(windows)]
    {
        let win11 = dear_imgui_custom_mod::app_window_v2::win32::dwm::is_win11_dwm_corners();
        let s = if win11 {
            "Win11 (rounded corners active)"
        } else {
            "Win10 / pre-22H2"
        };
        ui.text(format!("  OS  : {s}"));
    }
    #[cfg(not(windows))]
    {
        ui.text("  OS  : non-Windows");
    }
}

fn theme_name(t: Theme) -> &'static str {
    match t {
        Theme::Dark      => "Dark",
        Theme::Light     => "Light",
        Theme::Midnight  => "Midnight",
        Theme::Solarized => "Solar",
        Theme::Monokai   => "Monokai",
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let cfg = AppConfigV2::new("app_window_v2 demo", 1200.0, 780.0)
        .with_theme(Theme::Dark)
        .with_title_align(TitleAlign::Left)
        .with_fps_mode(FpsMode::Auto)
        .with_close_mode(CloseMode::Confirm)
        .with_extra_button(
            ExtraButton::new("theme", "T", [0.85, 0.85, 0.85, 1.0])
                .with_tooltip("Cycle theme"),
        );

    AppWindowV2::new(cfg).run(Demo::new()).expect("run failed");
}
