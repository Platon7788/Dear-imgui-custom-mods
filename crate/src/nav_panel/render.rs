//! Per-frame nav-panel render — animation tick, hidden-tab restore,
//! button layout, dispatch into the submenu flyout.
//!
//! ## Hover-feedback philosophy (chrome vs content)
//!
//! As of 2026-04-29 the panel renders **two distinct hover signals**
//! depending on which kind of button the cursor is over:
//!
//! - **Content buttons** (every `NavButton` the host supplies):
//!   icon glyph re-renders at `cfg.hover_zoom_scale` (~`1.20×`)
//!   via `add_text_with_font`. **No background tint.** This is the
//!   macOS-Dock-style "lift" — the icon visually pops forward
//!   without a tinted rectangle anchoring the eye to the cell
//!   shape. Drives the `HoverStyle::Zoom` behaviour that used to
//!   be opt-in (the `Flat` alternative was removed when the project
//!   owner settled on the zoom look).
//!
//! - **Chrome buttons** (the hidden-tab restore strip when the
//!   panel is collapsed, and the toggle / collapse arrow at the
//!   panel head): historic `colors.btn_hover` rounded fill, no
//!   zoom. These buttons are infrequent, semi-permanent affordances
//!   that benefit from a clear "you can click here" rectangle —
//!   without the fill they're easy to miss against the panel bg.
//!
//! Mixing the two policies on purpose: the user spends 99% of their
//! cursor time over content buttons, where a zoom is the cleanest
//! signal; the chrome buttons fire `is_window_hovered` rare paths
//! where a one-off rectangle is the right level of contrast.
//!
//! Don't paint `btn_hover` on content buttons — the doubled signal
//! ("zoom + tint") makes the icon look smudged.

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::pack_color_f32 as c32;
use crate::utils::text::calc_text_size;

use super::buttons::NavItem;
use super::config::NavPanelConfig;
use super::enums::{ActiveStyle, ButtonStyle, DockPosition};
use super::state::NavPanelState;
use super::submenu::{ButtonRect, render_submenu};
use super::{NavEvent, NavPanelResult};

/// Which surface the overlay variants of `render_nav_panel` paint
/// onto. `Background` is the default — keeps tooltips and other
/// popup-layer windows visually above the panel, which is what a
/// chrome-style nav strip needs. `Foreground` is for the rare cases
/// where the panel must obscure popups (kiosk-mode HUDs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OverlayLayer {
    /// Paint into `ui.get_background_draw_list()` — below ImGui popups.
    Background,
    /// Paint into `ui.get_foreground_draw_list()` — above ImGui popups.
    Foreground,
    /// Paint into the active window's draw list — for the in-window
    /// `render_nav_panel` entry point.
    Window,
}

pub(super) fn render_nav_panel_impl(
    ui: &Ui,
    cfg: &NavPanelConfig,
    state: &mut NavPanelState,
    origin: [f32; 2],
    size: [f32; 2],
    layer: OverlayLayer,
) -> NavPanelResult {
    let colors = cfg.resolved_colors();
    let dt = ui.io().delta_time();
    let [mx, my] = ui.io().mouse_pos();
    let is_vertical = matches!(cfg.position, DockPosition::Left | DockPosition::Right);

    let mut events = Vec::new();

    // ── Animation tick ───────────────────────────────────────────────────────
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
        return render_hidden_tab(ui, cfg, state, &colors, origin, size, layer, &mut events);
    }

    // ── Geometry ─────────────────────────────────────────────────────────────
    let [avail_w, avail_h] = size;
    let clicked = ui.is_mouse_clicked(MouseButton::Left);

    let panel_w = if is_vertical {
        cfg.width * prog
    } else {
        avail_w
    };
    let panel_h = if is_vertical {
        avail_h
    } else {
        cfg.height * prog
    };

    let px = match cfg.position {
        DockPosition::Right => origin[0] + avail_w - panel_w,
        _ => origin[0],
    };
    let py = origin[1];

    let toggle_size = if is_vertical {
        cfg.width.min(panel_w)
    } else {
        cfg.height.min(panel_h)
    };
    let btn_s = cfg
        .button_size
        .min(if is_vertical { panel_w } else { panel_h });

    // ── Draw the panel (scoped so DrawListMut drops before submenu spawns) ──
    let panel_hovered;
    {
        let draw = match layer {
            OverlayLayer::Background => ui.get_background_draw_list(),
            OverlayLayer::Foreground => ui.get_foreground_draw_list(),
            OverlayLayer::Window => ui.get_window_draw_list(),
        };

        // Background
        draw.add_rect([px, py], [px + panel_w, py + panel_h], c32(colors.bg))
            .filled(true)
            .build();

        panel_hovered = mx >= px && mx < px + panel_w && my >= py && my < py + panel_h;

        let mut cursor = 0.0_f32;

        // Toggle (collapse) button
        if cfg.show_toggle {
            cursor += draw_toggle_button(
                &draw,
                ui,
                cfg,
                state,
                &colors,
                px,
                py,
                toggle_size,
                mx,
                my,
                clicked,
                &mut events,
            );
        }

        // Buttons + separators
        let total_buttons = cfg
            .items
            .iter()
            .filter(|i| matches!(i, NavItem::Button(_)))
            .count();
        let mut btn_index = 0_usize;

        for item in &cfg.items {
            match item {
                NavItem::Separator => {
                    cursor += cfg.separator_padding;
                    if is_vertical {
                        let sy = py + cursor;
                        let m = panel_w * 0.22;
                        draw.add_line([px + m, sy], [px + panel_w - m, sy], c32(colors.separator))
                            .thickness(1.0)
                            .build();
                    } else {
                        let sx = px + cursor;
                        let m = panel_h * 0.22;
                        draw.add_line([sx, py + m], [sx, py + panel_h - m], c32(colors.separator))
                            .thickness(1.0)
                            .build();
                    }
                    cursor += cfg.separator_padding;
                }
                NavItem::Button(btn) => {
                    // Compare by &str slice — `Cow<'static, str>` derefs to
                    // `str`, so we sidestep moving the cow out of the button.
                    let btn_id_str: &str = &btn.id;
                    let is_active = state.active.as_deref().is_some_and(|s| s == btn_id_str);
                    let is_submenu_open = state
                        .open_submenu
                        .as_deref()
                        .is_some_and(|s| s == btn_id_str);

                    let (bx, by, bw, bh) = if is_vertical {
                        (px, py + cursor, panel_w, btn_s)
                    } else {
                        (px + cursor, py, btn_s, panel_h)
                    };
                    let bcx = bx + bw * 0.5;
                    let bcy = by + bh * 0.5;
                    let hov = mx >= bx && mx < bx + bw && my >= by && my < by + bh;

                    // Background — Bar active style fills the whole
                    // cell; Ring leaves it flat (the focus ring around
                    // the icon is the only active-state cue). Hover
                    // never paints a background — the icon zoom
                    // (`HoverStyle::Zoom`-style behaviour, baked in
                    // since 2026-04-29) carries the entire signal.
                    if (is_active || is_submenu_open) && cfg.active_style == ActiveStyle::Bar {
                        draw.add_rect([bx, by], [bx + bw, by + bh], c32(colors.btn_active))
                            .filled(true)
                            .build();
                    }

                    // Active indicator — Bar style only. Drawn on the
                    // docked edge (left/right strip for vertical, bottom
                    // strip for top docking).
                    if is_active && cfg.active_style == ActiveStyle::Bar {
                        let t = cfg.indicator_thickness;
                        match cfg.position {
                            DockPosition::Left => draw
                                .add_rect(
                                    [bx, by + 6.0],
                                    [bx + t, by + bh - 6.0],
                                    c32(colors.indicator),
                                )
                                .filled(true)
                                .rounding(t * 0.5)
                                .build(),
                            DockPosition::Right => draw
                                .add_rect(
                                    [bx + bw - t, by + 6.0],
                                    [bx + bw, by + bh - 6.0],
                                    c32(colors.indicator),
                                )
                                .filled(true)
                                .rounding(t * 0.5)
                                .build(),
                            DockPosition::Top => draw
                                .add_rect(
                                    [bx + 6.0, by + bh - t],
                                    [bx + bw - 6.0, by + bh],
                                    c32(colors.indicator),
                                )
                                .filled(true)
                                .rounding(t * 0.5)
                                .build(),
                        }
                    }

                    // Icon text. With the Ring style we keep the icon
                    // colour the same in both active and inactive
                    // states — the ring carries the "active" signal
                    // visually, so re-tinting the glyph would
                    // double-up. Bar style still swaps to
                    // `icon_active` because the filled background
                    // changes the surface contrast.
                    let icon_col = if is_active && cfg.active_style == ActiveStyle::Bar {
                        colors.icon_active
                    } else {
                        btn.color.unwrap_or(colors.icon_default)
                    };
                    let icon_str: &str = &btn.icon;
                    let [iw, ih] = calc_text_size(icon_str);
                    // Zoom the icon glyph when the cursor is over the
                    // cell. Active / submenu-open buttons stay
                    // un-scaled — the active-state visual (Ring or
                    // Bar) already carries the "this one is selected"
                    // signal; doubling up with a scale would look
                    // jittery as the cursor moves.
                    draw_icon(ui, &draw, cfg, [bcx, bcy], icon_str, icon_col, hov);

                    // Ring style — stroked circle around the icon.
                    // Drawn AFTER the icon glyph so the ring sits in
                    // front (matters when the icon would otherwise clip
                    // the inside edge of the stroke). Radius is sized
                    // off the larger glyph dimension + padding, then
                    // clamped to fit inside the button cell so it can't
                    // overflow when the user picks an oversized font or
                    // a tiny `button_size`.
                    if (is_active || is_submenu_open) && cfg.active_style == ActiveStyle::Ring {
                        let glyph_radius = iw.max(ih) * 0.5;
                        let max_radius = (bw.min(bh) * 0.5) - cfg.active_ring_thickness * 0.5 - 1.0;
                        let radius =
                            (glyph_radius + cfg.active_ring_padding).min(max_radius.max(1.0));
                        let ring_col = cfg.active_ring_color.unwrap_or(colors.indicator);
                        draw.add_circle([bcx, bcy], radius, c32(ring_col))
                            .thickness(cfg.active_ring_thickness)
                            .build();
                    }

                    // Label for horizontal mode. Same Zoom treatment
                    // as the icon — keeps IconWithLabel visually
                    // coherent (a scaled icon next to a flat label
                    // would look broken). `IconOnly` skips the label
                    // entirely; using `Option` instead of `continue`
                    // so the rest of the per-button render (badge,
                    // tooltip, click) still runs.
                    if !is_vertical {
                        let label = btn.tooltip.as_str();
                        let [lw, _lh] = calc_text_size(label);
                        let label_center: Option<[f32; 2]> = match cfg.button_style {
                            ButtonStyle::LabelOnly => Some([bcx, bcy]),
                            ButtonStyle::IconWithLabel => {
                                // Centre point = right of icon + half the label width.
                                Some([bcx + iw * 0.5 + 4.0 + lw * 0.5, bcy])
                            }
                            ButtonStyle::IconOnly => None,
                        };
                        if let Some(c) = label_center {
                            draw_icon(ui, &draw, cfg, c, label, icon_col, hov);
                        }
                    }

                    // Badge
                    if let Some(badge) = &btn.badge {
                        let badge_r = 5.5_f32;
                        let badge_cx = bx + bw - badge_r - 1.0;
                        let badge_cy = by + badge_r + 1.0;
                        draw.add_circle([badge_cx, badge_cy], badge_r, c32(colors.badge_bg))
                            .filled(true)
                            .build();
                        if !badge.is_empty() {
                            let [btw, bth] = calc_text_size(badge.as_str());
                            draw.add_text(
                                [badge_cx - btw * 0.5, badge_cy - bth * 0.5],
                                c32(colors.badge_text),
                                badge.as_str(),
                            );
                        }
                    }

                    // Tooltip — routed through the crate-wide
                    // `themed_tooltip` helper so the look matches every
                    // other widget that opens a hover popup.
                    if hov && !is_submenu_open && cfg.show_tooltips && btn.show_tooltip {
                        crate::utils::themed_tooltip(ui, || ui.text(btn.tooltip.as_str()));
                    }

                    // Click
                    if hov && clicked {
                        if btn.submenu.is_empty() {
                            // Two clones because `btn.id: Cow<'static, str>`
                            // — borrowed flavour is just a pointer copy
                            // (Cow::Borrowed clone is `Copy`-cheap), owned
                            // flavour clones the heap String.
                            events.push(NavEvent::ButtonClicked(btn.id.clone()));
                            state.active = Some(btn.id.clone());
                            state.open_submenu = None;
                        } else if is_submenu_open {
                            state.open_submenu = None;
                        } else {
                            state.open_submenu = Some(btn.id.clone());
                        }
                    }

                    cursor += btn_s + cfg.button_spacing;

                    btn_index += 1;
                    if cfg.show_button_separators && btn_index < total_buttons {
                        let sep_col = c32(colors.separator);
                        if is_vertical {
                            let sy = py + cursor - cfg.button_spacing * 0.5;
                            let m = panel_w * 0.18;
                            draw.add_line([px + m, sy], [px + panel_w - m, sy], sep_col)
                                .thickness(1.0)
                                .build();
                        } else {
                            let sx = px + cursor - cfg.button_spacing * 0.5;
                            let m = panel_h * 0.18;
                            draw.add_line([sx, py + m], [sx, py + panel_h - m], sep_col)
                                .thickness(1.0)
                                .build();
                        }
                    }
                }
            }
        }
    } // draw_list dropped here

    // ── Submenu flyout (after the draw_list scope) ──────────────────────────
    // Borrow the open id as `&str` so we don't move the cow out of `state`
    // — `render_submenu` may want to *clear* `state.open_submenu` when an
    // item is clicked, which would conflict with an outstanding move.
    let open_id_str: Option<String> = state.open_submenu.as_deref().map(str::to_owned);
    if let Some(open_id) = open_id_str {
        let mut btn_cursor = if cfg.show_toggle {
            if is_vertical {
                cfg.width.min(panel_w)
            } else {
                cfg.height.min(panel_h)
            }
        } else {
            0.0
        };
        for item in &cfg.items {
            match item {
                NavItem::Separator => {
                    btn_cursor += cfg.separator_padding * 2.0;
                }
                NavItem::Button(btn) => {
                    if &*btn.id == open_id.as_str() && !btn.submenu.is_empty() {
                        let rect = if is_vertical {
                            ButtonRect {
                                x: px,
                                y: py + btn_cursor,
                                w: panel_w,
                                h: btn_s,
                            }
                        } else {
                            ButtonRect {
                                x: px + btn_cursor,
                                y: py,
                                w: btn_s,
                                h: panel_h,
                            }
                        };
                        render_submenu(ui, cfg, btn, rect, &colors, state, &mut events);
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

    let occupied = if is_vertical {
        [panel_w, avail_h]
    } else {
        [avail_w, panel_h]
    };

    NavPanelResult {
        events,
        occupied_size: occupied,
    }
}

// ── Hidden-tab fallback (panel collapsed to a thin restore strip) ────────────

#[allow(clippy::too_many_arguments)]
fn render_hidden_tab(
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
fn draw_toggle_button(
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
fn draw_icon(
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
