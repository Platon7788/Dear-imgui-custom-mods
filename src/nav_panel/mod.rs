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
//! ## Usage
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::nav_panel::*;
//!
//! let cfg = NavPanelConfig::new(DockPosition::Left)
//!     .with_theme(NavTheme::Dark)
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

pub mod config;
pub mod state;
pub mod theme;

pub use config::{
    ButtonStyle, DockPosition, NavButton, NavItem, NavPanelConfig, SubMenuItem,
};
pub use state::NavPanelState;
pub use theme::{NavColors, NavTheme};

use dear_imgui_rs::{Condition, MouseButton, StyleColor, StyleVar, Ui, WindowFlags};

use crate::utils::color::pack_color_f32 as c32;
use crate::utils::text::calc_text_size;

// ── Event types ──────────────────────────────────────────────────────────────

/// An event produced by the navigation panel.
#[derive(Debug, Clone, PartialEq)]
pub enum NavEvent {
    /// A plain-action button was clicked.
    ButtonClicked(&'static str),
    /// A submenu item was clicked. `(button_id, item_id)`.
    SubMenuClicked(&'static str, &'static str),
    /// Toggle button was clicked. `visible` is the new visibility state.
    ToggleClicked(bool),
}

/// Result of rendering the nav panel for one frame.
#[derive(Debug, Clone)]
pub struct NavPanelResult {
    /// Events produced this frame.
    pub events: Vec<NavEvent>,
    /// Size occupied by the panel: `[width, height]`.
    pub occupied_size: [f32; 2],
}

// ── Main render function ─────────────────────────────────────────────────────

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
    let colors = cfg.theme.colors();
    let dt = ui.io().delta_time();
    let [mx, my] = ui.io().mouse_pos();
    let is_vertical = matches!(cfg.position, DockPosition::Left | DockPosition::Right);

    let mut events = Vec::new();

    // ── Animation ────────────────────────────────────────────────────────────
    let target = if state.visible { 1.0_f32 } else { 0.0 };
    if cfg.animate {
        let speed = cfg.animation_speed * dt;
        if state.animation_progress < target {
            state.animation_progress = (state.animation_progress + speed).min(1.0);
        } else if state.animation_progress > target {
            state.animation_progress = (state.animation_progress - speed).max(0.0);
        }
    } else {
        state.animation_progress = target;
    }

    let prog = state.animation_progress;
    if prog <= 0.0 {
        // Panel is fully hidden — draw a small restore tab on the edge.
        let origin = ui.cursor_screen_pos();
        let draw = ui.get_window_draw_list();
        let clicked = ui.is_mouse_clicked(MouseButton::Left);
        let colors = cfg.theme.colors();
        let tab_w = 16.0_f32;
        let tab_h = 36.0_f32;

        let (tx, ty, tw, th) = match cfg.position {
            DockPosition::Left   => (origin[0], origin[1] + 4.0, tab_w, tab_h),
            DockPosition::Right  => {
                let [aw, _] = ui.content_region_avail();
                (origin[0] + aw - tab_w, origin[1] + 4.0, tab_w, tab_h)
            }
            DockPosition::Top    => (origin[0] + 4.0, origin[1], tab_h, tab_w),
        };

        let tab_hov = mx >= tx && mx < tx + tw && my >= ty && my < ty + th;
        let bg = if tab_hov { colors.btn_hover } else { colors.bg };
        draw.add_rect([tx, ty], [tx + tw, ty + th], c32(bg))
            .filled(true).rounding(3.0).build();

        // Chevron arrow pointing outward (expand direction)
        let ic = c32(colors.toggle_icon);
        let acx = tx + tw * 0.5;
        let acy = ty + th * 0.5;
        let ar = tw.min(th) * 0.2;
        match cfg.position {
            DockPosition::Left => {
                // > pointing right (expand)
                draw.add_line([acx - ar * 0.4, acy - ar], [acx + ar * 0.4, acy], ic).thickness(1.5).build();
                draw.add_line([acx + ar * 0.4, acy], [acx - ar * 0.4, acy + ar], ic).thickness(1.5).build();
            }
            DockPosition::Right => {
                draw.add_line([acx + ar * 0.4, acy - ar], [acx - ar * 0.4, acy], ic).thickness(1.5).build();
                draw.add_line([acx - ar * 0.4, acy], [acx + ar * 0.4, acy + ar], ic).thickness(1.5).build();
            }
            DockPosition::Top => {
                draw.add_line([acx - ar, acy - ar * 0.4], [acx, acy + ar * 0.4], ic).thickness(1.5).build();
                draw.add_line([acx, acy + ar * 0.4], [acx + ar, acy - ar * 0.4], ic).thickness(1.5).build();
            }
        }

        if tab_hov {
            ui.tooltip_text("Show panel");
            if clicked {
                state.visible = true;
                events.push(NavEvent::ToggleClicked(true));
            }
        }

        // Also auto-show on edge hover (if configured)
        if cfg.auto_show_on_hover {
            let win_pos = ui.window_pos();
            let [win_w, _] = ui.window_size();
            let ox = cfg.content_offset_x;
            let oy = cfg.content_offset_y;
            let in_zone = match cfg.position {
                DockPosition::Left   => mx >= win_pos[0] + ox && mx < win_pos[0] + ox + cfg.edge_zone,
                DockPosition::Right  => mx > win_pos[0] + win_w - cfg.edge_zone,
                DockPosition::Top    => my >= win_pos[1] + oy && my < win_pos[1] + oy + cfg.edge_zone,
            };
            if in_zone { state.visible = true; }
        }

        return NavPanelResult { events, occupied_size: [0.0, 0.0] };
    }

    // ── Geometry ─────────────────────────────────────────────────────────────
    let origin = ui.cursor_screen_pos();
    let [avail_w, avail_h] = ui.content_region_avail();
    let clicked = ui.is_mouse_clicked(MouseButton::Left);

    let panel_w = if is_vertical { cfg.width * prog } else { avail_w };
    let panel_h = if is_vertical { avail_h } else { cfg.height * prog };

    // Panel rect — position depends on dock side
    let px = match cfg.position {
        DockPosition::Right => origin[0] + avail_w - panel_w,
        _ => origin[0],
    };
    let py = origin[1];

    let toggle_size = if is_vertical { cfg.width.min(panel_w) } else { cfg.height.min(panel_h) };
    let btn_s = cfg.button_size.min(if is_vertical { panel_w } else { panel_h });

    // ── Draw panel (scoped block so DrawListMut drops before submenu) ────────
    let panel_hovered;
    {
    let draw = ui.get_window_draw_list();

    // ── Background ───────────────────────────────────────────────────────────
    draw.add_rect(
        [px, py], [px + panel_w, py + panel_h],
        c32(colors.bg),
    ).filled(true).build();

    panel_hovered = mx >= px && mx < px + panel_w && my >= py && my < py + panel_h;

    // ── Toggle button ────────────────────────────────────────────────────────
    let mut cursor = 0.0_f32; // offset along main axis

    if cfg.show_toggle {
        let (tx, ty) = (px, py);
        let tcx = tx + toggle_size * 0.5;
        let tcy = ty + toggle_size * 0.5;

        let t_hov = mx >= tx && mx < tx + toggle_size && my >= ty && my < ty + toggle_size;
        if t_hov {
            draw.add_rect(
                [tx + 3.0, ty + 3.0],
                [tx + toggle_size - 3.0, ty + toggle_size - 3.0],
                c32(colors.btn_hover),
            ).filled(true).rounding(cfg.button_rounding).build();
            ui.tooltip_text("Toggle panel");
            if clicked {
                state.toggle();
                events.push(NavEvent::ToggleClicked(state.visible));
            }
        }
        // Directional arrow — points inward (collapse direction).
        // Left panel: «  Right panel: »  Top panel: ˄  Bottom panel: ˅
        let ic = c32(colors.toggle_icon);
        let ar = toggle_size * 0.18; // arrow half-size
        match cfg.position {
            DockPosition::Left => {
                // « double chevron left
                draw.add_line([tcx + ar * 0.2, tcy - ar], [tcx - ar * 0.6, tcy], ic).thickness(1.5).build();
                draw.add_line([tcx - ar * 0.6, tcy], [tcx + ar * 0.2, tcy + ar], ic).thickness(1.5).build();
                draw.add_line([tcx + ar, tcy - ar], [tcx + ar * 0.2, tcy], ic).thickness(1.5).build();
                draw.add_line([tcx + ar * 0.2, tcy], [tcx + ar, tcy + ar], ic).thickness(1.5).build();
            }
            DockPosition::Right => {
                // » double chevron right
                draw.add_line([tcx - ar * 0.2, tcy - ar], [tcx + ar * 0.6, tcy], ic).thickness(1.5).build();
                draw.add_line([tcx + ar * 0.6, tcy], [tcx - ar * 0.2, tcy + ar], ic).thickness(1.5).build();
                draw.add_line([tcx - ar, tcy - ar], [tcx - ar * 0.2, tcy], ic).thickness(1.5).build();
                draw.add_line([tcx - ar * 0.2, tcy], [tcx - ar, tcy + ar], ic).thickness(1.5).build();
            }
            DockPosition::Top => {
                // ˄˄ double chevron up
                draw.add_line([tcx - ar, tcy + ar * 0.2], [tcx, tcy - ar * 0.6], ic).thickness(1.5).build();
                draw.add_line([tcx, tcy - ar * 0.6], [tcx + ar, tcy + ar * 0.2], ic).thickness(1.5).build();
                draw.add_line([tcx - ar, tcy + ar], [tcx, tcy + ar * 0.2], ic).thickness(1.5).build();
                draw.add_line([tcx, tcy + ar * 0.2], [tcx + ar, tcy + ar], ic).thickness(1.5).build();
            }
        }
        cursor += toggle_size;
    }

    // ── Buttons ──────────────────────────────────────────────────────────────
    let mut btn_index = 0_usize;
    let total_buttons = cfg.items.iter().filter(|i| matches!(i, NavItem::Button(_))).count();

    for item in &cfg.items {
        match item {
            NavItem::Separator => {
                cursor += cfg.separator_padding;
                if is_vertical {
                    let sy = py + cursor;
                    let m = panel_w * 0.22;
                    draw.add_line([px + m, sy], [px + panel_w - m, sy], c32(colors.separator))
                        .thickness(1.0).build();
                } else {
                    let sx = px + cursor;
                    let m = panel_h * 0.22;
                    draw.add_line([sx, py + m], [sx, py + panel_h - m], c32(colors.separator))
                        .thickness(1.0).build();
                }
                cursor += cfg.separator_padding;
            }
            NavItem::Button(btn) => {
                let is_active = state.active == Some(btn.id);
                let is_submenu_open = state.open_submenu == Some(btn.id);

                // Button rect
                let (bx, by, bw, bh) = if is_vertical {
                    (px, py + cursor, panel_w, btn_s)
                } else {
                    (px + cursor, py, btn_s, panel_h)
                };
                let bcx = bx + bw * 0.5;
                let bcy = by + bh * 0.5;
                let hov = mx >= bx && mx < bx + bw && my >= by && my < by + bh;

                // Background
                if is_active || is_submenu_open {
                    draw.add_rect([bx, by], [bx + bw, by + bh], c32(colors.btn_active))
                        .filled(true).build();
                } else if hov {
                    draw.add_rect(
                        [bx + 3.0, by + 3.0], [bx + bw - 3.0, by + bh - 3.0],
                        c32(colors.btn_hover),
                    ).filled(true).rounding(cfg.button_rounding).build();
                }

                // Active indicator
                if is_active {
                    let t = cfg.indicator_thickness;
                    match cfg.position {
                        DockPosition::Left =>
                            draw.add_rect([bx, by + 6.0], [bx + t, by + bh - 6.0], c32(colors.indicator))
                                .filled(true).rounding(t * 0.5).build(),
                        DockPosition::Right =>
                            draw.add_rect([bx + bw - t, by + 6.0], [bx + bw, by + bh - 6.0], c32(colors.indicator))
                                .filled(true).rounding(t * 0.5).build(),
                        DockPosition::Top =>
                            draw.add_rect([bx + 6.0, by + bh - t], [bx + bw - 6.0, by + bh], c32(colors.indicator))
                                .filled(true).rounding(t * 0.5).build(),
                    }
                }

                // Icon text
                let icon_col = if is_active {
                    colors.icon_active
                } else {
                    btn.color.unwrap_or(colors.icon_default)
                };
                let [iw, ih] = calc_text_size(btn.icon);
                draw.add_text([bcx - iw * 0.5, bcy - ih * 0.5], c32(icon_col), btn.icon);

                // Label for horizontal mode
                if !is_vertical && cfg.button_style != ButtonStyle::IconOnly {
                    let [lw, lh] = calc_text_size(btn.tooltip);
                    match cfg.button_style {
                        ButtonStyle::LabelOnly =>
                            draw.add_text([bcx - lw * 0.5, bcy - lh * 0.5], c32(icon_col), btn.tooltip),
                        ButtonStyle::IconWithLabel =>
                            draw.add_text([bcx + iw * 0.5 + 4.0, bcy - lh * 0.5], c32(icon_col), btn.tooltip),
                        ButtonStyle::IconOnly => {}
                    }
                }

                // Badge — anchored to top-right of button cell
                if let Some(badge) = &btn.badge {
                    let badge_r = 5.5_f32;
                    let badge_cx = bx + bw - badge_r - 1.0;
                    let badge_cy = by + badge_r + 1.0;
                    draw.add_circle([badge_cx, badge_cy], badge_r, c32(colors.badge_bg))
                        .filled(true).build();
                    if !badge.is_empty() {
                        let [btw, bth] = calc_text_size(badge.as_str());
                        draw.add_text(
                            [badge_cx - btw * 0.5, badge_cy - bth * 0.5],
                            c32(colors.badge_text), badge.as_str(),
                        );
                    }
                }

                // Tooltip (respects global + per-button flag)
                if hov && !is_submenu_open && cfg.show_tooltips && btn.show_tooltip {
                    ui.tooltip_text(btn.tooltip);
                }

                // Click
                if hov && clicked {
                    if btn.submenu.is_empty() {
                        events.push(NavEvent::ButtonClicked(btn.id));
                        state.active = Some(btn.id);
                        state.open_submenu = None;
                    } else if is_submenu_open {
                        state.open_submenu = None;
                    } else {
                        state.open_submenu = Some(btn.id);
                    }
                }

                cursor += btn_s;

                // Inter-button spacing
                cursor += cfg.button_spacing;

                // Optional separator line between buttons
                btn_index += 1;
                if cfg.show_button_separators && btn_index < total_buttons {
                    let sep_col = c32(colors.separator);
                    if is_vertical {
                        let sy = py + cursor - cfg.button_spacing * 0.5;
                        let m = panel_w * 0.18;
                        draw.add_line([px + m, sy], [px + panel_w - m, sy], sep_col)
                            .thickness(1.0).build();
                    } else {
                        let sx = px + cursor - cfg.button_spacing * 0.5;
                        let m = panel_h * 0.18;
                        draw.add_line([sx, py + m], [sx, py + panel_h - m], sep_col)
                            .thickness(1.0).build();
                    }
                }
            }
        }
    }

    } // draw_list block ends — DrawListMut is dropped

    // ── Submenu flyout (rendered after draw_list scope) ─────────────────────
    if let Some(open_id) = state.open_submenu {
        // Find the button and compute its screen rect
        let mut btn_cursor = if cfg.show_toggle {
            if is_vertical { cfg.width.min(panel_w) } else { cfg.height.min(panel_h) }
        } else {
            0.0
        };
        for item in &cfg.items {
            match item {
                NavItem::Separator => { btn_cursor += cfg.separator_padding * 2.0; }
                NavItem::Button(btn) => {
                    if btn.id == open_id && !btn.submenu.is_empty() {
                        let (bx, by, bw, bh) = if is_vertical {
                            (px, py + btn_cursor, panel_w, btn_s)
                        } else {
                            (px + btn_cursor, py, btn_s, panel_h)
                        };
                        render_submenu(ui, cfg, btn, bx, by, bw, bh, &colors, state, &mut events);
                        break;
                    }
                    btn_cursor += btn_s;
                }
            }
        }
    }

    // ── Auto-hide ────────────────────────────────────────────────────────────
    if cfg.auto_hide && !panel_hovered && state.was_hovered && state.open_submenu.is_none() {
        state.visible = false;
    }
    state.was_hovered = panel_hovered;

    let occupied = if is_vertical { [panel_w, avail_h] } else { [avail_w, panel_h] };

    NavPanelResult { events, occupied_size: occupied }
}

// ── Submenu rendering (separate ImGui window) ────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_submenu(
    ui: &Ui,
    cfg: &NavPanelConfig,
    btn: &NavButton,
    bx: f32, by: f32, bw: f32, bh: f32,
    colors: &NavColors,
    state: &mut NavPanelState,
    events: &mut Vec<NavEvent>,
) {
    let [mx, my] = ui.io().mouse_pos();
    let clicked = ui.is_mouse_clicked(MouseButton::Left);

    let item_count = btn.submenu.iter().filter(|i| matches!(i, SubMenuItem::Item { .. })).count();
    let sep_count  = btn.submenu.iter().filter(|i| matches!(i, SubMenuItem::Separator)).count();
    let sm_h = item_count as f32 * cfg.submenu_item_height + sep_count as f32 * 9.0 + 8.0;
    let sm_w = cfg.submenu_min_width;

    let (sm_x, sm_y) = match cfg.position {
        DockPosition::Left   => (bx + bw + 2.0, by),
        DockPosition::Right  => (bx - sm_w - 2.0, by),
        DockPosition::Top    => (bx, by + bh + 2.0),
    };

    let _spad = ui.push_style_var(StyleVar::WindowPadding([4.0, 4.0]));
    let _srnd = ui.push_style_var(StyleVar::WindowRounding(6.0));
    let _sbrd = ui.push_style_var(StyleVar::WindowBorderSize(1.0));
    let _sbg  = ui.push_style_color(StyleColor::WindowBg, colors.submenu_bg);
    let _sbc  = ui.push_style_color(StyleColor::Border, colors.submenu_border);

    ui.window("##nav_submenu")
        .position([sm_x, sm_y], Condition::Always)
        .size([sm_w, sm_h], Condition::Always)
        .flags(
            WindowFlags::NO_TITLE_BAR | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE | WindowFlags::NO_SCROLLBAR
                | WindowFlags::NO_COLLAPSE,
        )
        .build(|| {
            let sm_draw = ui.get_window_draw_list();
            let sm_pos = ui.window_pos();
            let mut iy = sm_pos[1] + 4.0;

            for sub_item in &btn.submenu {
                match sub_item {
                    SubMenuItem::Separator => {
                        iy += 4.0;
                        sm_draw.add_line(
                            [sm_pos[0] + 8.0, iy],
                            [sm_pos[0] + sm_w - 8.0, iy],
                            c32(colors.submenu_separator),
                        ).thickness(1.0).build();
                        iy += 5.0;
                    }
                    SubMenuItem::Item { id, label, icon, shortcut } => {
                        let ih = cfg.submenu_item_height;
                        let item_hov = mx >= sm_pos[0] + 4.0 && mx < sm_pos[0] + sm_w - 4.0
                            && my >= iy && my < iy + ih;

                        if item_hov {
                            sm_draw.add_rect(
                                [sm_pos[0] + 4.0, iy],
                                [sm_pos[0] + sm_w - 4.0, iy + ih],
                                c32(colors.submenu_hover),
                            ).filled(true).rounding(4.0).build();
                            if clicked {
                                events.push(NavEvent::SubMenuClicked(btn.id, id));
                                state.open_submenu = None;
                            }
                        }

                        let text_y = iy + (ih - calc_text_size("M")[1]) * 0.5;
                        let mut tx = sm_pos[0] + 12.0;
                        if let Some(ico) = icon {
                            sm_draw.add_text([tx, text_y], c32(colors.submenu_text), ico);
                            tx += calc_text_size(ico)[0] + 8.0;
                        }
                        sm_draw.add_text([tx, text_y], c32(colors.submenu_text), label);
                        if let Some(sc) = shortcut {
                            let [scw, _] = calc_text_size(sc);
                            let dim = [
                                colors.submenu_text[0] * 0.6,
                                colors.submenu_text[1] * 0.6,
                                colors.submenu_text[2] * 0.6,
                                colors.submenu_text[3],
                            ];
                            sm_draw.add_text(
                                [sm_pos[0] + sm_w - 12.0 - scw, text_y],
                                c32(dim), sc,
                            );
                        }
                        iy += ih;
                    }
                }
            }
        });

    // Close on click outside both submenu and button
    let sm_hov = mx >= sm_x && mx < sm_x + sm_w && my >= sm_y && my < sm_y + sm_h;
    let btn_hov = mx >= bx && mx < bx + bw && my >= by && my < by + bh;
    if clicked && !sm_hov && !btn_hov {
        state.open_submenu = None;
    }
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
            .with_theme(NavTheme::Nord)
            .with_width(48.0)
            .with_auto_hide(true)
            .with_toggle_button(true)
            .with_animation_speed(10.0)
            .add_button(NavButton::action("home", "H", "Home").with_color([1.0, 0.0, 0.0, 1.0]))
            .add_separator()
            .add_button(NavButton::submenu("cfg", "C", "Config")
                .add_item(SubMenuItem::new("a", "Item A").with_icon("*"))
                .add_item(SubMenuItem::separator())
                .add_item(SubMenuItem::new("b", "Item B").with_shortcut("Ctrl+B")));

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
        assert_eq!(s.active, Some("home"));
        s.clear_active();
        assert!(s.active.is_none());
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
    fn all_six_themes_resolve() {
        for theme in [
            NavTheme::Dark, NavTheme::Light, NavTheme::Midnight,
            NavTheme::Nord, NavTheme::Solarized, NavTheme::Monokai,
        ] {
            let c = theme.colors();
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
            .add_item(SubMenuItem::new("a", "Alpha").with_icon("*").with_shortcut("Ctrl+A"))
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
