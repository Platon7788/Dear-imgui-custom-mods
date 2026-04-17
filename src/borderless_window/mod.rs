//! # borderless_window
//!
//! Reusable borderless-window titlebar for Rust + Dear ImGui on Windows.
//!
//! ## Features
//! - 5 built-in color themes (Dark, Light, Midnight, Solarized, Monokai)
//! - Minimize / Maximize / Close buttons with **small icon-only hover highlights**
//! - Optional close-confirmation mode
//! - 8-direction edge resize zone detection with hover-edge feedback
//! - Extra custom buttons
//! - Icon font–independent: buttons drawn as crisp draw-list primitives
//! - Returns [`TitlebarResult`] with `hover_edge` for cursor updates every frame
//! - Inactive (unfocused) color mode — dimmed title, bg, and icon when window loses OS focus
//! - Optional separator line visibility (`separator_visible`)
//! - Optional drag-zone hover hint (`show_drag_hint`)
//! - Icon click action (`WindowAction::IconClick`) for custom icon menus
//!
//! ## Minimal Setup
//!
//! ```rust,ignore
//! # use dear_imgui_rs::{Condition, StyleVar};
//! use dear_imgui_custom_mod::borderless_window::{
//!     BorderlessConfig, TitlebarState, WindowAction, render_titlebar,
//! };
//!
//! let cfg = BorderlessConfig::new("My App");
//! let mut state = TitlebarState::new();
//!
//! // Inside a full-screen zero-padding no-decoration ImGui window:
//! let res = render_titlebar(ui, &cfg, &mut state);
//!
//! // Update cursor every frame (no click needed):
//! if let Some(edge) = res.hover_edge { window.set_cursor(cursor_for_edge(edge)); }
//!
//! match res.action {
//!     WindowAction::Close          => event_loop.exit(),
//!     WindowAction::CloseRequested => show_confirm_dialog(&mut state),
//!     WindowAction::Minimize       => window.set_minimized(true),
//!     WindowAction::Maximize       => { let n = !state.maximized; window.set_maximized(n); state.set_maximized(n); }
//!     WindowAction::DragStart      => { window.drag_window().ok(); }
//!     WindowAction::ResizeStart(e) => { window.drag_resize_window(to_winit(e)).ok(); }
//!     WindowAction::Extra(id)      => { /* custom button */ }
//!     WindowAction::IconClick      => { /* icon was clicked */ }
//!     WindowAction::None           => {}
//! }
//! ```

pub mod actions;
pub mod config;
pub mod platform;
pub mod state;
pub mod theme;

pub use actions::{ResizeEdge, TitlebarResult, WindowAction};
pub use config::{BorderlessConfig, ButtonConfig, CloseMode, ExtraButton, TitleAlign};
pub use state::TitlebarState;
pub use theme::TitlebarColors;

use dear_imgui_rs::{DrawListMut, MouseButton, Ui};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

// ── Color helpers ─────────────────────────────────────────────────────────────

#[inline]
fn c32(c: [f32; 4]) -> u32 { rgba_f32(c[0], c[1], c[2], c[3]) }

// ── Draw-list icon primitives ────────────────────────────────────────────────
// All icons are drawn in a `[-r, +r]` unit space centred at `(cx, cy)`.
// This makes them font-independent and always crisp at any DPI.

/// Draw the Close (×) icon — two diagonal lines.
fn draw_icon_close(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    let d = r * 0.56;
    draw.add_line([cx - d, cy - d], [cx + d, cy + d], col).thickness(1.5).build();
    draw.add_line([cx + d, cy - d], [cx - d, cy + d], col).thickness(1.5).build();
}

/// Draw the Maximize (□) icon — square outline.
fn draw_icon_maximize(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    let p = r * 0.72;
    draw.add_rect([cx - p, cy - p], [cx + p, cy + p], col).thickness(1.5).build();
}

/// Draw the Restore (❐) icon — two offset overlapping squares.
/// `bg` is the hover background colour used to "erase" the overlapping area.
fn draw_icon_restore(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32, bg: u32) {
    let p  = r * 0.72;
    let sh = r * 0.38; // shift
    // Back square (top-right)
    draw.add_rect([cx - p + sh, cy - p - sh], [cx + p + sh, cy + p - sh], col)
        .thickness(1.2).build();
    // Erase overlap with bg, then draw front square
    draw.add_rect([cx - p, cy - p + sh], [cx + p - sh, cy + p + sh], bg)
        .filled(true).build();
    draw.add_rect([cx - p, cy - p + sh], [cx + p - sh, cy + p + sh], col)
        .thickness(1.5).build();
}

/// Draw the Minimize (─) icon — single horizontal line.
fn draw_icon_minimize(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    let p = r * 0.72;
    let y = cy + r * 0.40;
    draw.add_line([cx - p, y], [cx + p, y], col).thickness(1.5).build();
}

// ── Resize edge helper ────────────────────────────────────────────────────────

fn resize_edge_at(lx: f32, ly: f32, w: f32, h: f32, rz: f32) -> Option<ResizeEdge> {
    if lx < 0.0 || lx >= w || ly < 0.0 || ly >= h { return None; }
    let l = lx < rz;
    let r = lx > w - rz;
    let t = ly < rz;
    let b = ly > h - rz;
    match (t, b, l, r) {
        (true,  _,    true,  _    ) => Some(ResizeEdge::NorthWest),
        (true,  _,    _,     true ) => Some(ResizeEdge::NorthEast),
        (_,     true, true,  _    ) => Some(ResizeEdge::SouthWest),
        (_,     true, _,     true ) => Some(ResizeEdge::SouthEast),
        (true,  _,    _,     _    ) => Some(ResizeEdge::North),
        (_,     true, _,     _    ) => Some(ResizeEdge::South),
        (_,     _,    true,  _    ) => Some(ResizeEdge::West),
        (_,     _,    _,     true ) => Some(ResizeEdge::East),
        _                           => None,
    }
}

// ── Main public function ──────────────────────────────────────────────────────

/// Render the borderless window titlebar.
///
/// Call this as the **first** thing inside a full-screen, zero-padding,
/// no-decoration ImGui window (see [`BorderlessConfig`] doc for window setup).
/// It draws the titlebar, advances the cursor to `y = titlebar_height`,
/// and returns a [`TitlebarResult`].
///
/// ### Content area
/// After this call, `ui.content_region_avail()` gives the remaining space
/// **below** the titlebar — no component will hide behind it.
pub fn render_titlebar(
    ui: &Ui,
    cfg: &BorderlessConfig,
    state: &mut TitlebarState,
) -> TitlebarResult {
    let cursor = ui.cursor_screen_pos();
    let win_pos = ui.window_pos();
    let win_size = ui.window_size();
    let draw = ui.get_window_draw_list();
    let result = render_titlebar_impl(ui, cfg, state, cursor, win_pos, win_size, &draw, true);
    // Advance content cursor past the titlebar (legacy contract).
    ui.set_cursor_pos([0.0, cfg.titlebar_height]);
    result
}

/// Overlay variant: renders the titlebar through `ui.get_foreground_draw_list()`
/// at an explicit position without requiring a host ImGui window.
///
/// Use this when you already have regular content windows in the frame and
/// don't want a fullscreen host layer sitting above them swallowing clicks —
/// typical pattern for applications that compose their own event loop /
/// layout rather than using [`crate::app_window::AppWindow`].
///
/// - `origin` — top-left of the titlebar strip in **screen** coordinates.
/// - `full_window_size` — outer window (display) size in logical pixels; used
///   so the 8-edge resize hit test can cover the whole OS window even though
///   the titlebar itself is just the top strip.
pub fn render_titlebar_overlay(
    ui: &Ui,
    cfg: &BorderlessConfig,
    state: &mut TitlebarState,
    origin: [f32; 2],
    full_window_size: [f32; 2],
) -> TitlebarResult {
    let draw = ui.get_foreground_draw_list();
    render_titlebar_impl(ui, cfg, state, origin, origin, full_window_size, &draw, false)
}

#[allow(clippy::too_many_arguments)]
fn render_titlebar_impl(
    ui: &Ui,
    cfg: &BorderlessConfig,
    state: &mut TitlebarState,
    cursor: [f32; 2],
    win_pos: [f32; 2],
    window_size: [f32; 2],
    draw: &DrawListMut<'_>,
    use_window_hovered: bool,
) -> TitlebarResult {
    // ── Confirmed-close from previous frame ───────────────────────────────────
    if state.confirmed_close {
        state.confirmed_close = false;
        return TitlebarResult { action: WindowAction::Close, hover_edge: None };
    }

    let colors = cfg.resolved_colors();
    let dim       = cfg.focus_dim && !state.focused;
    let bg_col    = if dim { colors.bg_inactive    } else { colors.bg    };
    let title_col = if dim { colors.title_inactive } else { colors.title };
    let icon_col  = if dim { colors.title_inactive } else { colors.icon  };
    let h      = cfg.titlebar_height;
    let sep_h  = cfg.separator_height;
    let btn_w  = cfg.buttons.width;
    let ir     = cfg.buttons.icon_radius;   // icon half-size
    let ipad   = cfg.buttons.icon_hover_pad; // hover rect padding around icon

    let [win_w, win_h] = window_size;
    let [mx, my] = ui.io().mouse_pos();

    // ── Background ────────────────────────────────────────────────────────────
    draw.add_rect(
        [cursor[0],        cursor[1]],
        [cursor[0] + win_w, cursor[1] + h],
        c32(bg_col),
    ).filled(true).build();

    // ── Separator line ────────────────────────────────────────────────────────
    if cfg.separator_visible {
        draw.add_rect(
            [cursor[0],        cursor[1] + h - sep_h],
            [cursor[0] + win_w, cursor[1] + h],
            c32(colors.separator),
        ).filled(true).build();
    }

    // ── Layout metrics ────────────────────────────────────────────────────────
    let num_std   = cfg.buttons.show_close as usize
        + cfg.buttons.show_maximize as usize
        + cfg.buttons.show_minimize as usize;
    let btn_area_w    = (num_std + cfg.buttons.extra.len()) as f32 * btn_w;
    let btn_area_start = cursor[0] + win_w - btn_area_w;

    let [_, text_h] = calc_text_size("Mg");
    let text_y = cursor[1] + (h - text_h) * 0.5;

    // Titlebar row hover check. In the in-window path we also require
    // `ui.is_window_hovered()` so other ImGui surfaces on top of the host
    // steal the hover; in the overlay path there is no host window to gate
    // on, so we just test the mouse rect directly.
    let in_row = my >= cursor[1] && my < cursor[1] + h
        && (!use_window_hovered || ui.is_window_hovered());
    let clicked = ui.is_mouse_clicked(MouseButton::Left);

    // ── Icon + title text ─────────────────────────────────────────────────────
    let mut title_x = cursor[0] + cfg.title_padding_left;
    if let Some(ref icon) = cfg.icon {
        draw.add_text([title_x, text_y], c32(icon_col), icon.as_str());
        title_x += calc_text_size(icon.as_str())[0] + 6.0;
    }
    let icon_end_x = title_x;
    match cfg.title_align {
        TitleAlign::Left => {
            draw.add_text([title_x, text_y], c32(title_col), cfg.title.as_str());
        }
        TitleAlign::Center => {
            let tw = calc_text_size(cfg.title.as_str())[0];
            let cx = cursor[0] + (win_w - btn_area_w - tw) * 0.5;
            draw.add_text([cx.max(title_x), text_y], c32(title_col), cfg.title.as_str());
        }
    }

    // ── Buttons (right-to-left: Close, Maximize, Minimize, Extra…) ───────────
    let mut action = WindowAction::None;
    let mut bx = cursor[0] + win_w;

    let cy_btn = cursor[1] + h * 0.5; // vertical center of button cell

    // Helper: check hover, draw highlight, check click for one button cell.
    // Returns true if this button was clicked (and action is still None).
    macro_rules! btn_cell {
        ($bx:expr, $hover_bg:expr) => {{
            let cell_x = $bx;
            let cell_cx = cell_x + btn_w * 0.5;
            let hov = in_row && mx >= cell_x && mx < cell_x + btn_w;
            if hov {
                // Small hover rect centred on the icon — NOT full button height.
                draw.add_rect(
                    [cell_cx - ir - ipad, cy_btn - ir - ipad],
                    [cell_cx + ir + ipad, cy_btn + ir + ipad],
                    c32($hover_bg),
                ).filled(true).rounding(3.0).build();
            }
            hov && clicked && action == WindowAction::None
        }};
    }

    // Close
    if cfg.buttons.show_close {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        if btn_cell!(bx, colors.btn_close_hover_bg) {
            action = match cfg.close_mode {
                CloseMode::Immediate => WindowAction::Close,
                CloseMode::Confirm   => WindowAction::CloseRequested,
            };
        }
        draw_icon_close(draw, cx_btn, cy_btn, ir, c32(colors.btn_close));
    }

    // Maximize / Restore
    if cfg.buttons.show_maximize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        if btn_cell!(bx, colors.btn_hover_bg) {
            action = WindowAction::Maximize;
        }
        if state.maximized {
            draw_icon_restore(draw, cx_btn, cy_btn, ir, c32(colors.btn_maximize), c32(colors.bg_erase));
        } else {
            draw_icon_maximize(draw, cx_btn, cy_btn, ir, c32(colors.btn_maximize));
        }
    }

    // Minimize
    if cfg.buttons.show_minimize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        if btn_cell!(bx, colors.btn_hover_bg) {
            action = WindowAction::Minimize;
        }
        draw_icon_minimize(draw, cx_btn, cy_btn, ir, c32(colors.btn_minimize));
    }

    // Extra buttons (right-to-left)
    for extra in cfg.buttons.extra.iter().rev() {
        bx -= btn_w;
        let cell_cx = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov {
            draw.add_rect(
                [cell_cx - ir - ipad, cy_btn - ir - ipad],
                [cell_cx + ir + ipad, cy_btn + ir + ipad],
                c32(colors.btn_hover_bg),
            ).filled(true).rounding(3.0).build();
            if let Some(tip) = extra.tooltip { ui.tooltip_text(tip); }
            if clicked && action == WindowAction::None {
                action = WindowAction::Extra(extra.id);
            }
        }
        let [tw, th] = calc_text_size(extra.label);
        draw.add_text(
            [cell_cx - tw * 0.5, cy_btn - th * 0.5],
            c32(extra.color),
            extra.label,
        );
    }

    // ── Resize zone hover (every frame, no click needed) ─────────────────────
    let lx = mx - win_pos[0];
    let ly = my - win_pos[1];
    // Suppress resize hover while mouse is over button area or window is maximized.
    let over_buttons = in_row && mx >= btn_area_start;
    let hover_edge = if !over_buttons && !state.maximized {
        resize_edge_at(lx, ly, win_w, win_h, cfg.resize_zone)
    } else {
        None
    };

    // ── Drag-zone hover hint ──────────────────────────────────────────────────
    if cfg.show_drag_hint && in_row && mx < btn_area_start && hover_edge.is_none() {
        draw.add_rect(
            [cursor[0], cursor[1]],
            [btn_area_start, cursor[1] + h],
            c32(colors.drag_hint),
        ).filled(true).build();
    }

    // ── Resize click ──────────────────────────────────────────────────────────
    if action == WindowAction::None && clicked
        && let Some(edge) = hover_edge
    {
        action = WindowAction::ResizeStart(edge);
    }

    // ── Icon click ────────────────────────────────────────────────────────────
    if action == WindowAction::None && clicked && cfg.icon.is_some() {
        let icon_start = cursor[0] + cfg.title_padding_left;
        if in_row && mx >= icon_start && mx < icon_end_x {
            action = WindowAction::IconClick;
        }
    }

    // ── Titlebar drag / double-click maximize ─────────────────────────────────
    if action == WindowAction::None && in_row && mx < btn_area_start {
        if cfg.double_click_maximize && ui.is_mouse_double_clicked(MouseButton::Left) {
            action = WindowAction::Maximize;
        } else if clicked {
            action = WindowAction::DragStart;
        }
    }

    let _ = h; // suppress unused warning when no cursor advance here

    TitlebarResult { action, hover_edge }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = BorderlessConfig::default();
        assert_eq!(cfg.titlebar_height, 28.0);
        assert_eq!(cfg.resize_zone, 6.0);
        assert!(cfg.buttons.show_close);
        assert!(cfg.double_click_maximize);
        assert_eq!(cfg.close_mode, CloseMode::Immediate);
    }

    #[test]
    fn builder_chain() {
        let cfg = BorderlessConfig::new("Test")
            .with_theme(crate::theme::Theme::Solarized)
            .with_titlebar_height(32.0)
            .with_close_mode(CloseMode::Confirm)
            .with_title_align(TitleAlign::Center)
            .without_minimize();
        assert_eq!(cfg.title, "Test");
        assert_eq!(cfg.titlebar_height, 32.0);
        assert_eq!(cfg.close_mode, CloseMode::Confirm);
        assert_eq!(cfg.title_align, TitleAlign::Center);
        assert!(!cfg.buttons.show_minimize);
    }

    #[test]
    fn resize_edge_corners() {
        assert_eq!(resize_edge_at(1.0, 1.0, 800.0, 600.0, 6.0), Some(ResizeEdge::NorthWest));
        assert_eq!(resize_edge_at(799.0, 1.0, 800.0, 600.0, 6.0), Some(ResizeEdge::NorthEast));
        assert_eq!(resize_edge_at(1.0, 599.0, 800.0, 600.0, 6.0), Some(ResizeEdge::SouthWest));
        assert_eq!(resize_edge_at(799.0, 599.0, 800.0, 600.0, 6.0), Some(ResizeEdge::SouthEast));
    }

    #[test]
    fn resize_edge_sides() {
        assert_eq!(resize_edge_at(400.0, 1.0, 800.0, 600.0, 6.0), Some(ResizeEdge::North));
        assert_eq!(resize_edge_at(400.0, 599.0, 800.0, 600.0, 6.0), Some(ResizeEdge::South));
        assert_eq!(resize_edge_at(1.0, 300.0, 800.0, 600.0, 6.0), Some(ResizeEdge::West));
        assert_eq!(resize_edge_at(799.0, 300.0, 800.0, 600.0, 6.0), Some(ResizeEdge::East));
        assert_eq!(resize_edge_at(400.0, 300.0, 800.0, 600.0, 6.0), None);
    }

    #[test]
    fn resize_edge_outside() {
        assert_eq!(resize_edge_at(-1.0, 0.0, 800.0, 600.0, 6.0), None);
        assert_eq!(resize_edge_at(0.0, -1.0, 800.0, 600.0, 6.0), None);
    }

    #[test]
    fn state_confirmed_close() {
        let mut s = TitlebarState::new();
        assert!(!s.confirmed_close);
        s.confirm_close();
        assert!(s.confirmed_close);
        s.cancel_close();
        assert!(!s.confirmed_close);
    }

    #[test]
    fn all_builtin_themes_resolve() {
        for &theme in crate::theme::Theme::ALL {
            let c = theme.titlebar();
            // Every bg should be valid RGBA (values in [0,1])
            assert!(c.bg.iter().all(|&v| (0.0..=1.0).contains(&v)));
        }
    }

    #[test]
    fn extra_button_tooltip() {
        let btn = ExtraButton::new("id", "★", [1.0, 1.0, 0.0, 1.0]).with_tooltip("tip");
        assert_eq!(btn.tooltip, Some("tip"));
    }

    #[test]
    fn titlebar_result_none() {
        let r = TitlebarResult::none();
        assert_eq!(r.action, WindowAction::None);
        assert!(r.hover_edge.is_none());
    }
}
