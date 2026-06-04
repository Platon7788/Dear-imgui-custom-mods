//! Chrome-drawing helpers for the nav panel: the hidden-tab restore
//! strip (panel collapsed), the collapse/toggle chevron, and the
//! centred icon-glyph blitter (with hover zoom).
//!
//! Split out of `render.rs` so each file stays under the 500-line
//! ceiling. These are `pub(super)` so the sibling `render` module can
//! call them; the rest of the module path keeps using them through
//! `super::render_chrome::*`.

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::pack_color_f32 as c32;
use crate::utils::text::calc_text_size;

use super::config::NavPanelConfig;
use super::enums::DockPosition;
use super::render::OverlayLayer;
use super::state::NavPanelState;
use super::{NavEvent, NavPanelResult};

// ── Hidden-tab fallback (panel collapsed to a thin restore strip) ────────────

#[allow(clippy::too_many_arguments)]
pub(super) fn render_hidden_tab(
    ui: &Ui,
    cfg: &NavPanelConfig,
    state: &mut NavPanelState,
    colors: &super::theme::NavColors,
    origin: [f32; 2],
    size: [f32; 2],
    layer: OverlayLayer,
    events: &mut Vec<NavEvent>,
) -> NavPanelResult {
    let draw = match layer {
        OverlayLayer::Background => ui.get_background_draw_list(),
        OverlayLayer::Foreground => ui.get_foreground_draw_list(),
        OverlayLayer::Window => ui.get_window_draw_list(),
    };
    let [mx, my] = ui.io().mouse_pos();
    let clicked = ui.is_mouse_clicked(MouseButton::Left);
    let tab_w = 16.0_f32;
    let tab_h = 36.0_f32;

    let (tx, ty, tw, th) = match cfg.position {
        DockPosition::Left => (origin[0], origin[1] + 4.0, tab_w, tab_h),
        DockPosition::Right => {
            let aw = size[0];
            (origin[0] + aw - tab_w, origin[1] + 4.0, tab_w, tab_h)
        }
        DockPosition::Top => (origin[0] + 4.0, origin[1], tab_h, tab_w),
    };

    let tab_hov = mx >= tx && mx < tx + tw && my >= ty && my < ty + th;
    let bg = if tab_hov { colors.btn_hover } else { colors.bg };
    draw.add_rect([tx, ty], [tx + tw, ty + th], c32(bg))
        .filled(true)
        .rounding(3.0)
        .build();

    // Chevron arrow pointing outward (expand direction).
    let ic = c32(colors.toggle_icon);
    let acx = tx + tw * 0.5;
    let acy = ty + th * 0.5;
    let ar = tw.min(th) * 0.2;
    match cfg.position {
        DockPosition::Left => {
            draw.add_line([acx - ar * 0.4, acy - ar], [acx + ar * 0.4, acy], ic)
                .thickness(1.5)
                .build();
            draw.add_line([acx + ar * 0.4, acy], [acx - ar * 0.4, acy + ar], ic)
                .thickness(1.5)
                .build();
        }
        DockPosition::Right => {
            draw.add_line([acx + ar * 0.4, acy - ar], [acx - ar * 0.4, acy], ic)
                .thickness(1.5)
                .build();
            draw.add_line([acx - ar * 0.4, acy], [acx + ar * 0.4, acy + ar], ic)
                .thickness(1.5)
                .build();
        }
        DockPosition::Top => {
            draw.add_line([acx - ar, acy - ar * 0.4], [acx, acy + ar * 0.4], ic)
                .thickness(1.5)
                .build();
            draw.add_line([acx, acy + ar * 0.4], [acx + ar, acy - ar * 0.4], ic)
                .thickness(1.5)
                .build();
        }
    }

    if tab_hov {
        let s = crate::i18n::nav_panel::strings(cfg.locale);
        crate::utils::themed_tooltip(ui, || ui.text(s.show_panel));
        if clicked {
            state.visible = true;
            events.push(NavEvent::ToggleClicked(true));
        }
    }

    if cfg.auto_show_on_hover {
        let ox = cfg.content_offset_x;
        let oy = cfg.content_offset_y;
        let in_zone = match cfg.position {
            DockPosition::Left => mx >= origin[0] + ox && mx < origin[0] + ox + cfg.edge_zone,
            DockPosition::Right => mx > origin[0] + size[0] - cfg.edge_zone,
            DockPosition::Top => my >= origin[1] + oy && my < origin[1] + oy + cfg.edge_zone,
        };
        if in_zone {
            state.visible = true;
        }
    }

    NavPanelResult {
        events: std::mem::take(events),
        occupied_size: [0.0, 0.0],
    }
}

// ── Toggle-button (the chevron at the top of the visible panel) ──────────────

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_toggle_button(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    ui: &Ui,
    cfg: &NavPanelConfig,
    state: &mut NavPanelState,
    colors: &super::theme::NavColors,
    px: f32,
    py: f32,
    toggle_size: f32,
    mx: f32,
    my: f32,
    clicked: bool,
    events: &mut Vec<NavEvent>,
) -> f32 {
    let (tx, ty) = (px, py);
    let tcx = tx + toggle_size * 0.5;
    let tcy = ty + toggle_size * 0.5;

    let t_hov = mx >= tx && mx < tx + toggle_size && my >= ty && my < ty + toggle_size;
    if t_hov {
        draw.add_rect(
            [tx + 3.0, ty + 3.0],
            [tx + toggle_size - 3.0, ty + toggle_size - 3.0],
            c32(colors.btn_hover),
        )
        .filled(true)
        .rounding(cfg.button_rounding)
        .build();
        let s = crate::i18n::nav_panel::strings(cfg.locale);
        crate::utils::themed_tooltip(ui, || ui.text(s.toggle_panel));
        if clicked {
            state.toggle();
            events.push(NavEvent::ToggleClicked(state.visible));
        }
    }

    // Directional double-chevron — points inward (collapse direction).
    let ic = c32(colors.toggle_icon);
    let ar = toggle_size * 0.18;
    match cfg.position {
        DockPosition::Left => {
            draw.add_line([tcx + ar * 0.2, tcy - ar], [tcx - ar * 0.6, tcy], ic)
                .thickness(1.5)
                .build();
            draw.add_line([tcx - ar * 0.6, tcy], [tcx + ar * 0.2, tcy + ar], ic)
                .thickness(1.5)
                .build();
            draw.add_line([tcx + ar, tcy - ar], [tcx + ar * 0.2, tcy], ic)
                .thickness(1.5)
                .build();
            draw.add_line([tcx + ar * 0.2, tcy], [tcx + ar, tcy + ar], ic)
                .thickness(1.5)
                .build();
        }
        DockPosition::Right => {
            draw.add_line([tcx - ar * 0.2, tcy - ar], [tcx + ar * 0.6, tcy], ic)
                .thickness(1.5)
                .build();
            draw.add_line([tcx + ar * 0.6, tcy], [tcx - ar * 0.2, tcy + ar], ic)
                .thickness(1.5)
                .build();
            draw.add_line([tcx - ar, tcy - ar], [tcx - ar * 0.2, tcy], ic)
                .thickness(1.5)
                .build();
            draw.add_line([tcx - ar * 0.2, tcy], [tcx - ar, tcy + ar], ic)
                .thickness(1.5)
                .build();
        }
        DockPosition::Top => {
            draw.add_line([tcx - ar, tcy + ar * 0.2], [tcx, tcy - ar * 0.6], ic)
                .thickness(1.5)
                .build();
            draw.add_line([tcx, tcy - ar * 0.6], [tcx + ar, tcy + ar * 0.2], ic)
                .thickness(1.5)
                .build();
            draw.add_line([tcx - ar, tcy + ar], [tcx, tcy + ar * 0.2], ic)
                .thickness(1.5)
                .build();
            draw.add_line([tcx, tcy + ar * 0.2], [tcx + ar, tcy + ar], ic)
                .thickness(1.5)
                .build();
        }
    }

    toggle_size
}

/// Draw the icon glyph centred at `center`. When `zoomed` is true
/// AND the configured zoom factor is greater than `1.0`, re-renders
/// the glyph at `font_size * cfg.hover_zoom_scale` via
/// `add_text_with_font`, recentred so the larger glyph stays at
/// `center`. Otherwise falls back to the standard `add_text` path.
///
/// `add_text_with_font` reuses the same atlas as the default text
/// renderer; the scaled output goes through GPU bilinear filtering,
/// which adds a touch of softness — acceptable for ≤1.5× scales,
/// the sweet spot for "macOS Dock"-style micro-magnification.
pub(super) fn draw_icon(
    ui: &Ui,
    draw: &dear_imgui_rs::DrawListMut<'_>,
    cfg: &NavPanelConfig,
    center: [f32; 2],
    icon: &str,
    main_color: [f32; 4],
    zoomed: bool,
) {
    let [iw, ih] = calc_text_size(icon);
    if zoomed && cfg.hover_zoom_scale > 1.0 {
        let font = ui.current_font();
        let base_size = ui.current_font_size();
        let scale = cfg.hover_zoom_scale;
        let scaled_size = base_size * scale;
        let scaled_w = iw * scale;
        let scaled_h = ih * scale;
        draw.add_text_with_font(
            font,
            scaled_size,
            [center[0] - scaled_w * 0.5, center[1] - scaled_h * 0.5],
            c32(main_color),
            icon,
            0.0,
            None,
        );
    } else {
        draw.add_text(
            [center[0] - iw * 0.5, center[1] - ih * 0.5],
            c32(main_color),
            icon,
        );
    }
}
