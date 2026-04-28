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
//! - `render`    — per-frame layout + drawing (separate file).
//! - `submenu`   — flyout window (separate file).
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

pub mod config;
pub mod state;
pub mod theme;

mod render;
mod submenu;

pub use config::{ButtonStyle, DockPosition, NavButton, NavItem, NavPanelConfig, SubMenuItem};
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
    render::render_nav_panel_impl(ui, cfg, state, origin, size, false)
}

/// Overlay variant: renders the nav panel through `ui.get_foreground_draw_list()`
/// at an explicit screen-space position, without requiring a host ImGui window.
///
/// - `origin` — top-left of the panel region in **screen** coordinates.
/// - `size` — `[width, height]` of the region reserved for the panel.
///
/// The submenu flyout still spawns its own ImGui window (it needs input focus),
/// but the panel itself draws on the foreground draw list so content windows
/// behind it remain clickable.
pub fn render_nav_panel_overlay(
    ui: &Ui,
    cfg: &NavPanelConfig,
    state: &mut NavPanelState,
    origin: [f32; 2],
    size: [f32; 2],
) -> NavPanelResult {
    render::render_nav_panel_impl(ui, cfg, state, origin, size, true)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = NavPanelConfig::default();
        assert_eq!(cfg.position, DockPosition::Left);
        assert_eq!(cfg.width, 28.0);
        assert!(!cfg.auto_hide);
        assert!(!cfg.show_toggle);
        assert!(cfg.items.is_empty());
    }

    #[test]
    fn builder_chain() {
        let cfg = NavPanelConfig::new(DockPosition::Right)
            .with_theme(crate::theme::Theme::Solarized)
            .with_width(48.0)
            .with_auto_hide(true)
            .with_toggle_button(true)
            .with_animation_speed(10.0)
            .add_button(NavButton::action("home", "H", "Home").with_color([1.0, 0.0, 0.0, 1.0]))
            .add_separator()
            .add_button(
                NavButton::submenu("cfg", "C", "Config")
                    .add_item(SubMenuItem::new("a", "Item A").with_icon("*"))
                    .add_item(SubMenuItem::separator())
                    .add_item(SubMenuItem::new("b", "Item B").with_shortcut("Ctrl+B")),
            );

        assert_eq!(cfg.position, DockPosition::Right);
        assert_eq!(cfg.width, 48.0);
        assert!(cfg.auto_hide);
        assert!(cfg.show_toggle);
        assert_eq!(cfg.items.len(), 3);
    }

    #[test]
    fn state_active() {
        let mut s = NavPanelState::new();
        assert!(s.active.is_none());
        s.set_active("home");
        // Compare via deref so the test stays terse — `Cow<'static, str>`
        // derefs to `str`, and `Option::as_deref()` gives `Option<&str>`.
        assert_eq!(s.active.as_deref(), Some("home"));
        s.clear_active();
        assert!(s.active.is_none());
    }

    #[test]
    fn state_active_accepts_runtime_string() {
        // The whole point of `Cow<'static, str>`: we can hand it a `String`
        // built at runtime — no Box::leak, no &'static str hacks.
        let mut s = NavPanelState::new();
        let runtime_id = format!("page_{}", 42);
        s.set_active(runtime_id);
        assert_eq!(s.active.as_deref(), Some("page_42"));
    }

    #[test]
    fn state_visibility() {
        let mut s = NavPanelState::new();
        assert!(s.visible);
        s.hide();
        assert!(!s.visible);
        s.show();
        assert!(s.visible);
        s.toggle();
        assert!(!s.visible);
    }

    #[test]
    fn all_builtin_themes_resolve() {
        for &theme in crate::theme::Theme::ALL {
            let c = theme.nav();
            assert!(c.bg.iter().all(|&v| (0.0..=1.0).contains(&v)));
            assert!(c.indicator[3] > 0.0);
        }
    }

    #[test]
    fn nav_button_builders() {
        let btn = NavButton::action("test", "T", "Test")
            .with_color([1.0, 0.5, 0.0, 1.0])
            .with_badge("3");
        assert_eq!(btn.id, "test");
        assert_eq!(btn.color, Some([1.0, 0.5, 0.0, 1.0]));
        assert_eq!(btn.badge.as_deref(), Some("3"));
        assert!(btn.submenu.is_empty());
    }

    #[test]
    fn submenu_items() {
        let btn = NavButton::submenu("menu", "M", "Menu")
            .add_item(
                SubMenuItem::new("a", "Alpha")
                    .with_icon("*")
                    .with_shortcut("Ctrl+A"),
            )
            .add_separator()
            .add_item(SubMenuItem::new("b", "Beta"));
        assert_eq!(btn.submenu.len(), 3);
        assert!(matches!(&btn.submenu[1], SubMenuItem::Separator));
    }

    #[test]
    fn dock_positions() {
        assert_eq!(DockPosition::default(), DockPosition::Left);
        assert_ne!(DockPosition::Left, DockPosition::Right);
    }

    #[test]
    fn button_styles() {
        assert_eq!(ButtonStyle::default(), ButtonStyle::IconOnly);
        assert_ne!(ButtonStyle::IconOnly, ButtonStyle::IconWithLabel);
    }
}
