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

pub use config::{
    ActiveStyle, ButtonStyle, DockPosition, NavButton, NavItem, NavPanelConfig, SubMenuItem,
};
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
/// **Host requirement:** if the host is an `app_window` instance,
/// it must be configured with `AppConfig::raw_content()` so the
/// root window receives `WindowFlags::NO_BACKGROUND` — otherwise
/// the root's `WindowBg` fill clobbers the background draw list
/// and the panel becomes invisible.
///
/// For the legacy foreground behaviour (kiosk-style HUDs that
/// genuinely need to obscure popups) use
/// [`render_nav_panel_overlay_foreground`].
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
    fn hover_zoom_default_is_macos_dock_band() {
        // The default zoom factor sits in the visible-but-not-jarring
        // band (~`1.15`–`1.30`) — same magnification feel as the
        // macOS Dock. Set to exactly `1.0` to disable the effect.
        let cfg = NavPanelConfig::default();
        assert!(cfg.hover_zoom_scale > 1.0 && cfg.hover_zoom_scale <= 1.5);
    }

    #[test]
    fn hover_zoom_scale_is_clamped() {
        // 1.0 floor — anything below collapses to "no zoom" / makes
        // no sense (would shrink the glyph). 3.0 ceiling — beyond
        // that the glyph overflows the button cell and clips visibly.
        let too_small = NavPanelConfig::default().with_hover_zoom_scale(0.5);
        assert!(too_small.hover_zoom_scale >= 1.0);
        let too_big = NavPanelConfig::default().with_hover_zoom_scale(10.0);
        assert!(too_big.hover_zoom_scale <= 3.0);
        // In-range values pass through unchanged.
        let mid = NavPanelConfig::default().with_hover_zoom_scale(1.35);
        assert_eq!(mid.hover_zoom_scale, 1.35);
    }

    #[test]
    fn active_style_default_is_ring_with_orange() {
        // As of 2026-04-29 the default active-state visual is a
        // transparent ring around the icon (no background fill, no
        // indicator strip), tinted warm amber so it reads as
        // "orange focus" on every built-in theme. Flip back to the
        // historic filled-bar look via
        // `with_active_style(ActiveStyle::Bar)`.
        let cfg = NavPanelConfig::default();
        assert_eq!(cfg.active_style, ActiveStyle::Ring);
        let ring = cfg
            .active_ring_color
            .expect("ring colour must be Some by default");
        // Warm amber: red ≫ green > blue, fully opaque.
        assert!(
            ring[0] > 0.8,
            "default ring should be predominantly red/orange"
        );
        assert!(
            ring[0] > ring[1] && ring[1] > ring[2],
            "warm hue ordering R > G > B"
        );
        assert!(
            (ring[3] - 1.0).abs() < f32::EPSILON,
            "ring colour must be opaque"
        );
        assert!(cfg.active_ring_thickness > 0.0);
        assert!(cfg.active_ring_padding >= 0.0);
    }

    #[test]
    fn active_style_can_opt_back_into_bar() {
        // Backwards-compatibility escape hatch — callers that want the
        // pre-2026-04-29 filled-bar look just call this builder.
        let cfg = NavPanelConfig::default().with_active_style(ActiveStyle::Bar);
        assert_eq!(cfg.active_style, ActiveStyle::Bar);
    }

    #[test]
    fn active_style_ring_builders() {
        let cfg = NavPanelConfig::new(DockPosition::Left)
            .with_active_style(ActiveStyle::Ring)
            .with_active_ring_color([1.0, 0.65, 0.20, 1.0])
            .with_active_ring_thickness(2.0)
            .with_active_ring_padding(6.0);
        assert_eq!(cfg.active_style, ActiveStyle::Ring);
        assert_eq!(cfg.active_ring_color, Some([1.0, 0.65, 0.20, 1.0]));
        assert_eq!(cfg.active_ring_thickness, 2.0);
        assert_eq!(cfg.active_ring_padding, 6.0);
        // `without_active_ring_color` clears back to None (palette
        // indicator falls back through).
        let cleared = cfg.without_active_ring_color();
        assert!(cleared.active_ring_color.is_none());
    }

    #[test]
    fn active_ring_thickness_clamped_above_min() {
        // 0.0 / negative values would render an invisible (or
        // backwards) stroke — clamp keeps the ring visible.
        let cfg = NavPanelConfig::default().with_active_ring_thickness(0.0);
        assert!(cfg.active_ring_thickness >= 0.5);
        let cfg = NavPanelConfig::default().with_active_ring_thickness(-3.0);
        assert!(cfg.active_ring_thickness >= 0.5);
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
