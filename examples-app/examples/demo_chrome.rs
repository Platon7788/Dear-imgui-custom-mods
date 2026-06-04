//! Demo: `chrome::Chrome` + `dear-app` — canonical borderless-window pattern.
//!
//! Replaces the deleted `demo_app_window`. Shows the recommended wiring:
//! single full-display root window where the chrome titlebar + host content
//! coexist, dispatched via `dear-app`'s three callbacks.
//!
//! Run with:
//!   cargo run --example demo_chrome --release
//!
//! Highlights:
//! - **Borderless titlebar** with min / max / close + drag + double-click
//!   maximise + 8-direction edge resize + DPI-scaled rounded corners.
//! - **Runtime theme switch** (Dark / Light / Midnight / Solarized / Monokai
//!   / Catppuccin / Nord) — `Chrome::set_theme` refreshes the cached palette.
//! - **CloseMode::Confirm** flow — close button surfaces an in-app
//!   confirmation popup instead of exiting; the host calls `process::exit`
//!   only after explicit user confirmation.
//! - **Runtime config mutation** — `Chrome::config_mut()` flips button
//!   visibility on the fly without reconstructing the chrome.
//! - **Critical** `docking: { enable: false, auto_dockspace: false }` — without
//!   this dear-app's auto dockspace would absorb every click before chrome
//!   sees it.

use std::sync::{Arc, Mutex};

use dear_app::{AppBuilder, DockingConfig, RunnerConfig, Theme as DearAppTheme};
use dear_imgui_custom_mod::chrome::{Chrome, CloseMode, TitlebarConfig};
use dear_imgui_custom_mod::theme::Theme;
use dear_imgui_rs::{StyleVar, Ui, WindowFlags};
use winit::window::Window;

/// Per-app state owned by the `on_frame` closure.
struct DemoState {
    /// Active theme. `last_applied_theme` lets us call `Chrome::set_theme`
    /// only on transitions instead of every frame.
    theme: Theme,
    last_applied_theme: Theme,
    /// `true` while the close-confirm popup is open. Set by
    /// `take_close_request` when the user clicks × in `CloseMode::Confirm`.
    confirm_close_open: bool,
    /// Counter incremented by the demo button — proves clicks reach widgets.
    counter: u32,
    /// Toggles for the `Chrome::config_mut` runtime mutation demo.
    show_minimize: bool,
    show_maximize: bool,
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            last_applied_theme: Theme::Dark,
            confirm_close_open: false,
            counter: 0,
            show_minimize: true,
            show_maximize: true,
        }
    }
}

const THEMES: &[(Theme, &str)] = &[(Theme::Dark, "Dark"), (Theme::Light, "Light")];

fn main() {
    // ── Shared state — chrome wrapper plus a window handle stash ──────────
    // Both are accessed from three dear-app callbacks (`on_gpu_init`,
    // `on_event`, `on_frame`), so they live behind `Arc<Mutex<_>>`.
    let chrome = Arc::new(Mutex::new(
        Chrome::new(TitlebarConfig::default().with_close_confirm())
            .with_title("Chrome demo")
            .with_theme(Theme::Dark)
            .with_corner_radius(8),
    ));
    let win_stash: Arc<Mutex<Option<Arc<Window>>>> = Arc::new(Mutex::new(None));
    let state = Arc::new(Mutex::new(DemoState::default()));

    // ── dear-app runner config ────────────────────────────────────────────
    // The two `docking` flags MUST be `false` — see module-level doc.
    let runner_cfg = RunnerConfig {
        window_title: "Chrome demo".to_string(),
        window_size: (1100.0, 720.0),
        theme: Some(DearAppTheme::Dark),
        docking: DockingConfig {
            enable: false,
            auto_dockspace: false,
            ..DockingConfig::default()
        },
        ..RunnerConfig::default()
    };

    // ── Build & run ───────────────────────────────────────────────────────
    AppBuilder::new()
        .with_config(runner_cfg)
        .on_gpu_init({
            let chrome = chrome.clone();
            let win_stash = win_stash.clone();
            move |window, _device, _queue, _surface_cfg| {
                // Strip OS chrome, apply DWM dark mode + rounded corners,
                // shrink window if it came up at fullscreen-equivalent size.
                chrome.lock().unwrap().on_setup(window);
                *win_stash.lock().unwrap() = Some(window.clone());
            }
        })
        .on_event({
            let chrome = chrome.clone();
            let win_stash = win_stash.clone();
            move |event, _window, ctx| {
                if let Some(w) = win_stash.lock().unwrap().as_ref() {
                    chrome.lock().unwrap().on_event(event, w, ctx);
                }
            }
        })
        .on_frame({
            let chrome = chrome.clone();
            let state = state.clone();
            let win_stash = win_stash.clone();
            move |ui, _addons| {
                let Some(window) = win_stash.lock().unwrap().clone() else {
                    return;
                };

                // Render borderless titlebar + content. `render` wraps the
                // closure in a single full-display ImGui root — drag /
                // resize / minimise / maximise / close all dispatch to
                // `window` automatically. Close requests surface via
                // `take_close_request` below.
                {
                    let mut c = chrome.lock().unwrap();
                    let st = state.clone();
                    c.render(ui, &window, |ui, _area| {
                        let _pad = ui.push_style_var(StyleVar::WindowPadding([12.0, 12.0]));
                        let _spc = ui.push_style_var(StyleVar::ItemSpacing([8.0, 6.0]));
                        ui.child_window("##content")
                            .size([0.0, 0.0])
                            .border(false)
                            .build(ui, || {
                                render_content(ui, &mut st.lock().unwrap());
                            });
                    });
                }

                // Bridge demo state into chrome config / palette.
                // - Theme switch: only fires when the user actually changed
                //   the picker — `set_theme` is cheap but recreates the
                //   palette so we gate it.
                // - Button visibility: bool flips, free; sync every frame.
                {
                    let mut s = state.lock().unwrap();
                    let mut c = chrome.lock().unwrap();
                    if s.theme != s.last_applied_theme {
                        c.set_theme(s.theme);
                        s.last_applied_theme = s.theme;
                    }
                    c.config_mut().buttons.minimize = s.show_minimize;
                    c.config_mut().buttons.maximize = s.show_maximize;
                }

                // ── Close-request handling ────────────────────────────────
                // CloseMode::Confirm — user clicked ×, surface a popup
                // instead of exiting. The popup lives inside the chrome's
                // root render area (rendered next frame).
                if let Some(mode) = chrome.lock().unwrap().take_close_request() {
                    match mode {
                        CloseMode::Immediate => std::process::exit(0),
                        CloseMode::Confirm => {
                            state.lock().unwrap().confirm_close_open = true;
                        }
                    }
                }

                // The popup is rendered as part of the next frame — we open
                // it here via OpenPopup so it appears at the top Z-layer.
                if state.lock().unwrap().confirm_close_open {
                    ui.open_popup("##confirm_close");
                }
                draw_confirm_close_popup(ui, &mut state.lock().unwrap());
            }
        })
        .run()
        .expect("event loop terminated");
}

/// Demo body — a single child window's worth of content.
fn render_content(ui: &Ui, state: &mut DemoState) {
    ui.text_colored(
        [1.0, 0.84, 0.0, 1.0],
        "Chrome demo — borderless titlebar + dear-app",
    );
    ui.separator();
    ui.spacing();

    ui.text_wrapped(
        "Drag the title text to move the window. Drag any edge / corner to \
         resize. Double-click the title to toggle maximise. The titlebar \
         buttons (− □ ×) work as expected.",
    );
    ui.spacing();

    // ── Theme picker ─────────────────────────────────────────────────────
    if ui.collapsing_header("Theme switch", dear_imgui_rs::TreeNodeFlags::DEFAULT_OPEN) {
        ui.text("Click a button to switch the chrome titlebar palette in real time:");
        let mut switched_to: Option<Theme> = None;
        for &(theme, label) in THEMES {
            if state.theme == theme {
                ui.text_colored([0.4, 0.85, 0.4, 1.0], format!("• {label} (active)"));
            } else if ui.button(label) {
                switched_to = Some(theme);
            }
            ui.same_line();
        }
        ui.new_line();
        if let Some(theme) = switched_to {
            state.theme = theme;
            // The chrome's palette is bridged via the demo's main loop —
            // see the `state` mutation block in `on_frame`.
        }
    }

    ui.spacing();

    // ── Runtime config mutation ──────────────────────────────────────────
    if ui.collapsing_header(
        "Runtime config mutation",
        dear_imgui_rs::TreeNodeFlags::DEFAULT_OPEN,
    ) {
        ui.text("Toggles flow into Chrome::config_mut() on the next frame:");
        ui.checkbox("Show minimize button", &mut state.show_minimize);
        ui.checkbox("Show maximize button", &mut state.show_maximize);
    }

    ui.spacing();

    // ── Click-through proof ──────────────────────────────────────────────
    if ui.collapsing_header(
        "Click-through proof",
        dear_imgui_rs::TreeNodeFlags::DEFAULT_OPEN,
    ) {
        ui.text("Counter increments on each click — confirms clicks reach widgets");
        ui.text(
            "(broken when `docking.auto_dockspace = true` — that dockspace \
             absorbs the click before any non-docked window sees it).",
        );
        if ui.button("Click me!") {
            state.counter += 1;
        }
        ui.same_line();
        ui.text(format!("Clicks: {}", state.counter));
    }

    ui.spacing();

    // ── Close-mode notice ────────────────────────────────────────────────
    if ui.collapsing_header(
        "CloseMode::Confirm",
        dear_imgui_rs::TreeNodeFlags::DEFAULT_OPEN,
    ) {
        ui.text_wrapped(
            "This chrome was built with `.with_close_confirm()`. Clicking × \
             surfaces the popup below (the host owns the dialog flow); the \
             window only exits after explicit confirmation.",
        );
    }
}

/// Modal confirmation rendered when the user clicks the close button while
/// the chrome's `close_mode == Confirm`. Two buttons: cancel resets the
/// demo state; confirm exits the process.
fn draw_confirm_close_popup(ui: &Ui, state: &mut DemoState) {
    let _pad = ui.push_style_var(StyleVar::WindowPadding([16.0, 16.0]));
    if let Some(_tok) = ui
        .begin_modal_popup_config("##confirm_close")
        .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
        .begin()
    {
        ui.text("Close the demo?");
        ui.spacing();
        ui.separator();
        ui.spacing();
        if ui.button_with_size("Cancel", [120.0, 0.0]) {
            state.confirm_close_open = false;
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button_with_size("Quit", [120.0, 0.0]) {
            std::process::exit(0);
        }
    }
}
