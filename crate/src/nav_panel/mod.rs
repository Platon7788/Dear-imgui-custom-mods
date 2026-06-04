//! # nav_panel
//!
//! Modern navigation panel (activity bar) for Dear ImGui.
//!
//! ## Features
//! - 3 docking positions: Left, Right, Top (Bottom reserved for StatusBar)
//! - Left/Right: vertical icon strip (VS Code activity bar style)
//! - Top: horizontal bar with `IconOnly`, `IconWithLabel`, or `LabelOnly` modes
//! - Flyout submenu on any button
//! - Auto-hide with slide animation + auto-show on edge hover
//! - Optional toggle (hamburger) button
//! - Active indicator bar
//! - Badge (notification dot / counter) on any button
//! - 6 built-in color themes + custom
//! - Custom icon colors per button
//!
//! ## Architecture
//!
//! The panel renders using the **parent window's draw list** — no extra ImGui
//! window is created (except for the submenu flyout). This means it integrates
//! seamlessly inside `app_window` or any full-screen host window.
//!
//! Call [`render_nav_panel`] inside your ImGui window. It draws the panel,
//! advances the cursor past it, and returns a [`NavPanelResult`] with events
//! and the occupied size.
//!
//! ## Module layout
//!
//! - [`config`]   — declarative types (positions, button styles, items).
//! - [`state`]    — [`NavPanelState`] — runtime mutable state.
//! - [`theme`]    — [`NavColors`] palette.
//! - `render`        — per-frame layout + button drawing (separate file).
//! - `render_chrome` — hidden-tab strip, toggle chevron, icon blitter.
//! - `submenu`       — flyout window (separate file).
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::nav_panel::*;
//!
//! let cfg = NavPanelConfig::new(DockPosition::Left)
//!     .with_theme(Theme::Dark)
//!     .add_button(NavButton::action("home", "H", "Home")
//!         .with_color([0.3, 0.6, 1.0, 1.0]))
//!     .add_separator()
//!     .add_button(NavButton::submenu("cfg", "S", "Settings")
//!         .add_item(SubMenuItem::new("prefs", "Preferences")));
//!
//! let mut state = NavPanelState::new();
//! state.set_active("home");
//!
//! let result = render_nav_panel(ui, &cfg, &mut state);
//! // Content area starts after result.occupied_size
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md

pub mod buttons;
pub mod config;
pub mod enums;
pub mod state;
pub mod theme;

mod render;
mod render_chrome;
mod submenu;

pub use buttons::{NavButton, NavItem, SubMenuItem};
pub use config::NavPanelConfig;
pub use enums::{ActiveStyle, ButtonStyle, DockPosition};
pub use state::NavPanelState;
pub use theme::NavColors;

use std::borrow::Cow;

use dear_imgui_rs::Ui;

// ── Event types ──────────────────────────────────────────────────────────────

/// An event produced by the navigation panel.
///
/// IDs are `Cow<'static, str>` so both static buttons and runtime-built
/// ones (loaded from JSON, plugin-generated) emit events of the same type.
/// Match against a `&str` via `id.as_ref()` or compare with `id == "foo"`.
#[derive(Debug, Clone, PartialEq)]
pub enum NavEvent {
    /// A plain-action button was clicked.
    ButtonClicked(Cow<'static, str>),
    /// A submenu item was clicked. `(button_id, item_id)`.
    SubMenuClicked(Cow<'static, str>, Cow<'static, str>),
    /// Toggle button was clicked. `visible` is the new visibility state.
    ToggleClicked(bool),
}

/// Result of rendering the nav panel for one frame.
///
/// `events` carries button / submenu / toggle clicks; silently dropping them
/// means ignoring user input. `#[must_use]` surfaces that at compile time.
#[derive(Debug, Clone)]
#[must_use = "nav events (button clicks, submenu selections) are delivered via this result"]
pub struct NavPanelResult {
    /// Events produced this frame.
    pub events: Vec<NavEvent>,
    /// Size occupied by the panel: `[width, height]`.
    pub occupied_size: [f32; 2],
}

// ── Public render entry points ──────────────────────────────────────────────

/// Render the navigation panel using the **current window's draw list**.
///
/// Call this inside a full-screen ImGui window (e.g. inside `AppHandler::render`).
/// The panel draws directly on the parent draw list — no extra ImGui window is
/// created (except for the submenu flyout).
///
/// After calling, advance your content cursor by `result.occupied_size`.
pub fn render_nav_panel(
    ui: &Ui,
    cfg: &NavPanelConfig,
    state: &mut NavPanelState,
) -> NavPanelResult {
    let origin = ui.cursor_screen_pos();
    let size = ui.content_region_avail();
    render::render_nav_panel_impl(ui, cfg, state, origin, size, render::OverlayLayer::Window)
}

/// Overlay variant: renders the nav panel through
/// `ui.get_background_draw_list()` at an explicit screen-space
/// position, without requiring a host ImGui window.
///
/// - `origin` — top-left of the panel region in **screen** coordinates.
/// - `size` — `[width, height]` of the region reserved for the panel.
///
/// **Z-order note (2026-04-29):** the panel paints into the
/// **background** draw list rather than the foreground one. Same
/// rationale as [`crate::status_bar::StatusBar::render_overlay`] —
/// chrome surfaces should sit below ImGui popups (tooltips,
/// context menus, modals) so a popup raised by another widget can
/// never get clipped by the panel. The submenu flyout still spawns
/// its own ImGui window (it needs input focus and lives in the
/// popup layer naturally).
///
/// **Host requirement:** the background draw list is drawn
/// **before** all ImGui windows. [`crate::app_window::AppWindow`]
/// hosts a full-window root behind every frame, so background-list
/// overlays are not visible under it — use
/// [`render_nav_panel_overlay_foreground`] instead so the panel
/// paints into the foreground draw list (above all windows).
pub fn render_nav_panel_overlay(
    ui: &Ui,
    cfg: &NavPanelConfig,
    state: &mut NavPanelState,
    origin: [f32; 2],
    size: [f32; 2],
) -> NavPanelResult {
    render::render_nav_panel_impl(
        ui,
        cfg,
        state,
        origin,
        size,
        render::OverlayLayer::Background,
    )
}

/// Foreground-overlay variant — paints into
/// `ui.get_foreground_draw_list()`, which lives **above** every
/// ImGui popup. Use only for kiosk-style HUDs that must obscure
/// tooltips and menus; for standard chrome bars prefer
/// [`render_nav_panel_overlay`] so the host's tooltips don't get
/// clipped.
pub fn render_nav_panel_overlay_foreground(
    ui: &Ui,
    cfg: &NavPanelConfig,
    state: &mut NavPanelState,
    origin: [f32; 2],
    size: [f32; 2],
) -> NavPanelResult {
    render::render_nav_panel_impl(
        ui,
        cfg,
        state,
        origin,
        size,
        render::OverlayLayer::Foreground,
    )
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
