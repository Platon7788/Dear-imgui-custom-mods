//! # app_window_v2
//!
//! **Native borderless** window built on Win32 WndProc subclassing.
//!
//! Unlike [`app_window`](crate::app_window) which uses
//! `with_decorations(false)` + faked drag/resize via `WM_NCLBUTTONDOWN`,
//! v2 keeps a normal `WS_OVERLAPPEDWINDOW` and strips the chrome via
//! `WM_NCCALCSIZE → 0`. The OS sees a real window with no visible frame,
//! which means **everything below works natively**:
//!
//! - Drag (`HTCAPTION`) — including Aero Snap (drag to edge), Aero Shake
//! - Resize from any edge / corner (`HTLEFT`/`HTRIGHT`/.../`HTBOTTOMRIGHT`)
//! - Snap Layouts popup on hover-maximize (`HTMAXBUTTON`) — **Win11 only**
//!   (Win10 ignores; same code, no special-case needed)
//! - Double-click on titlebar to maximize / restore
//! - Win+Up / Win+Down / Win+Left / Win+Right / Win+Shift+Left/Right
//! - Minimize from maximized → taskbar → restore-from-taskbar restores
//!   **back to maximized** (the Win11 borderless lock-up bug **does not
//!   apply** here because the OS handles minimize natively)
//! - Snap Groups (Win11) — when this window is part of a snapped group,
//!   hovering the taskbar shows the group restore preview
//! - Native shadow from DWM (the system frame is invisible but still
//!   participates in DWM compositing)
//! - System menu via right-click on titlebar / Alt+Space
//! - Native cursor management (resize cursors auto-applied by OS)
//! - Native `WM_ACTIVATE` lifecycle — focused/unfocused state is correct
//!   from frame 0 with no debounce hacks
//!
//! ## Status
//!
//! **Experimental** — opt in via `--features app_window_v2`. Does not
//! conflict with v1 ([`app_window`](crate::app_window)); both can be
//! enabled simultaneously. Once stable, v1 is slated for deprecation.
//!
//! ## Minimal Example
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::app_window_v2::{
//!     AppConfigV2, AppHandlerV2, AppStateV2, AppWindowV2,
//! };
//! use dear_imgui_custom_mod::dear_imgui_rs::Ui;
//!
//! struct MyApp;
//! impl AppHandlerV2 for MyApp {
//!     fn render(&mut self, ui: &Ui, _state: &mut AppStateV2) {
//!         ui.text("Hello from app_window_v2!");
//!     }
//! }
//!
//! fn main() {
//!     AppWindowV2::new(AppConfigV2::new("My App", 1024.0, 720.0))
//!         .run(MyApp)
//!         .expect("run");
//! }
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass
#![allow(unreachable_pub)]

mod app;
pub mod config;
pub mod gpu;
pub mod handler;
pub mod hit_test;
pub mod state;
pub mod titlebar;

#[cfg(windows)]
pub mod win32;

pub use config::{
    AppConfigV2, ButtonConfig, CloseMode, ExtraButton, FpsMode, PowerMode, StartPosition,
    TitleAlign, TitlebarConfig,
};
pub use handler::AppHandlerV2;
pub use state::{AppStateV2, TitlebarStateV2};
pub use crate::theme::Theme;

use winit::event_loop::EventLoop;

/// Top-level facade — create with [`new`](Self::new), drive with [`run`](Self::run).
pub struct AppWindowV2 {
    config: AppConfigV2,
}

impl AppWindowV2 {
    /// Create a new v2 window with the given configuration.
    pub fn new(config: AppConfigV2) -> Self {
        Self { config }
    }

    /// Run the application event loop. Blocks until the window closes.
    pub fn run<H: AppHandlerV2 + 'static>(
        self,
        handler: H,
    ) -> Result<(), winit::error::EventLoopError> {
        let event_loop = EventLoop::new()?;
        let mut app = app::WinitAppV2::new(self.config, handler);
        event_loop.run_app(&mut app)
    }
}
