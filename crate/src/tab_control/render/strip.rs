//! The per-frame tab-strip driver.
//!
//! `render_strip` snapshots scalar config, lays out pinned + regular tabs,
//! runs [`fill_hit_scratch`](super::hittest::fill_hit_scratch) once, then hands
//! the scratch buffer to [`super::draw`] (drawing) and [`super::events`]
//! (interaction). The empty placeholder and body frame live in [`super::body`];
//! the hit-test and scroll math live in [`super::hittest`].

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::rgb_arr as c32;

use super::super::layout::{PINNED_SEPARATOR_VPAD, PINNED_SEPARATOR_W};
use super::super::types::*;
use super::super::{TabControl, TabItem};
use super::body::render_body;
use super::buttons::{
    render_add_button, render_overflow_button, render_overflow_popup_body, render_scroll_buttons,
};
use super::drag::handle_drag;
use super::draw::draw_tab;
use super::events::handle_tab_events;
use super::hittest::{fill_hit_scratch, scroll_into_view};
use super::keyboard::handle_keyboard;
use super::{SMOOTH_SCROLL_COEF, TabDraw};

pub(super) fn render_strip<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) -> Option<TabAction> {
    let mut action: Option<TabAction> = None;

    // Absorb any pinned-status changes the user may have performed since the
    // last frame, then refresh tab widths.
    pc.enforce_pinned_partition();
    pc.ensure_tab_widths();

    // ── Snapshot scalar config fields up-front so we can call mutating
    //    helpers on `pc` later without keeping a borrow on `pc.config`.
    let strip_h;
    let avail_w;
    let strip_x;
    let strip_y;
    let add_btn_w;
    let pinned_total_w;
    let regular_total_w: f32;
    let pinned_count;
    let separator_w;
    let regular_origin_base;
    let needs_scroll;
    let overflow_w;
    let regular_area_w;
    let scroll_area_w;
    let scroll_btn_w;
    let scroll_with_wheel;
    let scroll_speed;
    let smooth_scroll;
    let show_overflow_dropdown;
    let show_add_button;
    let draggable;
    let keyboard_nav;
    let external_content;
    let hover_activate_ms;
    let preview_hover_ms;
    {
        let cfg = &pc.config;
        strip_h = cfg.strip_height();
        let win_pos = ui.window_pos();
        let win_content_min = ui.cursor_start_pos();
        avail_w = ui.content_region_avail()[0];
        strip_x = win_pos[0] + win_content_min[0];
        strip_y = win_pos[1] + ui.cursor_pos()[1];

        add_btn_w = if cfg.show_add_button {
            cfg.scroll_btn_width
        } else {
            0.0
        };

        // Sum widths separately for pinned vs regular tabs.
        let mut pw: f32 = 0.0;
        let mut rw: f32 = 0.0;
        let mut pc_count: usize = 0;
        for (i, tab) in pc.tabs.iter().enumerate() {
            let w = pc.tab_widths_cache.get(i).copied().unwrap_or(0.0);
            if tab.item.is_pinned() {
                pw += w + cfg.tab_gap;
                pc_count += 1;
            } else {
                rw += w + cfg.tab_gap;
            }
        }
        if pc_count > 0 {
            pw -= cfg.tab_gap; // trim trailing gap
        }
        let regular_count = pc.tabs.len() - pc_count;
        if regular_count > 0 {
            rw -= cfg.tab_gap;
        }
        pinned_total_w = pw.max(0.0);
        regular_total_w = rw.max(0.0);
        pinned_count = pc_count;

        separator_w = if pinned_count > 0 {
            PINNED_SEPARATOR_W
        } else {
            0.0
        };
        regular_origin_base = strip_x + pinned_total_w + separator_w;

        let space_for_regular = avail_w - add_btn_w - pinned_total_w - separator_w;
        needs_scroll = regular_total_w > space_for_regular;

        overflow_w = if needs_scroll && cfg.show_overflow_dropdown {
            cfg.scroll_btn_width
        } else {
            0.0
        };
        regular_area_w = space_for_regular - overflow_w;
        scroll_area_w = if needs_scroll {
            regular_area_w - cfg.scroll_btn_width * 2.0
        } else {
            regular_area_w
        };
        scroll_btn_w = cfg.scroll_btn_width;
        scroll_with_wheel = cfg.scroll_with_wheel;
        scroll_speed = cfg.scroll_speed;
        smooth_scroll = cfg.smooth_scroll;
        show_overflow_dropdown = cfg.show_overflow_dropdown;
        show_add_button = cfg.show_add_button;
        draggable = cfg.draggable;
        keyboard_nav = cfg.keyboard_nav;
        external_content = cfg.external_content;
        hover_activate_ms = cfg.hover_activate_ms;
        preview_hover_ms = cfg.preview_hover_ms;
    } // cfg borrow dropped

    let mouse = ui.io().mouse_pos();
    let clicked = ui.is_mouse_clicked(MouseButton::Left);
    let middle_clicked = ui.is_mouse_clicked(MouseButton::Middle);
    let right_clicked = ui.is_mouse_clicked(MouseButton::Right);
    let accept_clicks = ui.is_window_hovered();

    // ── Strip background + bottom rule ──────────────────────────────────
    {
        let draw = ui.get_window_draw_list();
        let colors = &pc.config.colors;
        draw.add_rect(
            [strip_x, strip_y],
            [strip_x + avail_w, strip_y + strip_h],
            c32(colors.strip_bg, 255),
        )
        .filled(true)
        .build();
        draw.add_line(
            [strip_x, strip_y + strip_h],
            [strip_x + avail_w, strip_y + strip_h],
            c32(colors.separator, 200),
        )
        .build();
    }

    // ── Scroll buttons (left/right) for the regular tab area ───────────
    // Buttons sit just past the pinned strip (and its separator).
    let tabs_origin_x = if needs_scroll {
        render_scroll_buttons(
            ui,
            regular_origin_base,
            strip_y,
            regular_area_w,
            strip_h,
            &mut pc.scroll_target,
            regular_total_w,
            scroll_area_w,
            accept_clicks,
            mouse,
            &pc.config,
        );
        regular_origin_base + scroll_btn_w
    } else {
        pc.scroll_offset = 0.0;
        pc.scroll_target = 0.0;
        regular_origin_base
    };

    // ── Pinned/regular separator (vertical thin line) ──────────────────
    if pinned_count > 0 {
        let draw = ui.get_window_draw_list();
        let sep_x = strip_x + pinned_total_w + separator_w * 0.5;
        let sep_top = strip_y + PINNED_SEPARATOR_VPAD;
        let sep_bot = strip_y + strip_h - PINNED_SEPARATOR_VPAD;
        draw.add_line(
            [sep_x, sep_top],
            [sep_x, sep_bot],
            c32(pc.config.colors.separator, 220),
        )
        .thickness(1.0)
        .build();
    }

    // ── Wheel scroll over regular strip area ────────────────────────────
    if needs_scroll && scroll_with_wheel && accept_clicks {
        let in_regular_strip = mouse[1] >= strip_y
            && mouse[1] < strip_y + strip_h
            && mouse[0] >= regular_origin_base
            && mouse[0] < regular_origin_base + regular_area_w;
        if in_regular_strip {
            let wheel = ui.io().mouse_wheel();
            if wheel != 0.0 {
                pc.scroll_target -= wheel * scroll_speed * 0.5;
                let max_scroll = (regular_total_w - scroll_area_w).max(0.0);
                pc.scroll_target = pc.scroll_target.clamp(0.0, max_scroll);
            }
        }
    }

    // ── Smooth scroll interpolation ─────────────────────────────────────
    if smooth_scroll {
        let dt = ui.io().delta_time();
        let diff = pc.scroll_target - pc.scroll_offset;
        if diff.abs() < 0.5 {
            pc.scroll_offset = pc.scroll_target;
        } else {
            pc.scroll_offset += diff * (1.0 - (-SMOOTH_SCROLL_COEF * dt).exp());
        }
    } else {
        pc.scroll_offset = pc.scroll_target;
    }

    // ── Side buttons (overflow dropdown + add) ──────────────────────────
    if needs_scroll && show_overflow_dropdown {
        let ovx = strip_x + avail_w - add_btn_w - overflow_w;
        let opened = render_overflow_button(
            ui,
            ovx,
            strip_y,
            overflow_w,
            accept_clicks,
            mouse,
            &pc.config,
        );
        if opened {
            ui.open_popup(&pc.overflow_popup_id);
        }
    }
    render_overflow_popup_body(pc, ui);

    if show_add_button {
        let ax = strip_x + avail_w - add_btn_w;
        if render_add_button(ui, ax, strip_y, add_btn_w, accept_clicks, mouse, &pc.config) {
            action = Some(TabAction::AddRequested);
        }
    }

    // ── Apply deferred scroll-to-active (set by add/set_active/scroll_to_active) ─
    //
    // Just sets `scroll_target`; the smooth-scroll loop above does the actual
    // easing. Earlier the renderer hard-snapped (`scroll_offset =
    // scroll_target`) on every activation to fix the "по чуть-чуть"
    // sluggishness, but that felt too abrupt — see user feedback 2026-04-30.
    // The compromise is a faster `SMOOTH_SCROLL_COEF` so the ease finishes in
    // ~3 frames without snapping.
    if pc.pending_scroll_to_active {
        pc.pending_scroll_to_active = false;
        if let Some(active_id) = pc.active
            && let Some(idx) = pc.tabs.iter().position(|t| t.id == active_id)
        {
            scroll_into_view(pc, idx, scroll_area_w);
        }
    }

    // ── request_focus → activate (batched, with on_deactivated) ─────────
    let mut focus_request: Option<(TabId, usize)> = None;
    for (i, tab) in pc.tabs.iter_mut().enumerate() {
        if tab.request_focus {
            tab.request_focus = false;
            focus_request = Some((tab.id, i));
        }
    }
    if let Some((new_id, new_idx)) = focus_request {
        if let Some(old_id) = pc.active
            && old_id != new_id
            && let Some(old) = pc.tabs.iter_mut().find(|t| t.id == old_id)
        {
            old.item.on_deactivated();
        }
        pc.active = Some(new_id);
        if let Some(entry) = pc.tabs.iter_mut().find(|t| t.id == new_id) {
            entry.item.on_activated();
        }
        action = Some(TabAction::Activated(new_id));
        scroll_into_view(pc, new_idx, scroll_area_w);
    }

    // ── PRE-PASS: fill hit_scratch with geometry + hit state ────────────
    let pinned_max_x = strip_x + pinned_total_w;
    let reg_clip_min_x = tabs_origin_x;
    let reg_clip_max_x = tabs_origin_x + scroll_area_w;
    fill_hit_scratch(
        pc,
        accept_clicks,
        mouse,
        strip_x,
        strip_y,
        pinned_max_x,
        tabs_origin_x,
        reg_clip_min_x,
        reg_clip_max_x,
    );

    // ── Phase 1a: draw pinned tabs (no clip — fixed strip on the left) ──
    {
        let cfg = &pc.config;
        let draw = ui.get_window_draw_list();
        let time = ui.time() as f32;
        for &(idx, x0, x1, _tw, tab_hovered, close_hovered) in &pc.hit_scratch {
            let tab = &pc.tabs[idx];
            if !tab.item.is_pinned() {
                continue;
            }
            let is_active = pc.active == Some(tab.id);
            let y0 = strip_y + cfg.strip_padding_v;
            let y1 = y0 + cfg.tab_height;
            let anim_alpha = (tab.open_anim.clamp(0.0, 1.0) * 255.0) as u8;
            let accent = tab
                .item
                .tab_color()
                .unwrap_or_else(|| cfg.colors.status_color(tab.item.status()));
            let ctx = TabDraw {
                draw: &draw,
                item: &tab.item,
                cfg,
                is_active,
                hovered: tab_hovered,
                close_hovered,
                anim_alpha,
                accent,
                x0,
                y0,
                x1,
                y1,
                time,
            };
            draw_tab(&ctx);
        }
    }

    // ── Phase 1b: draw regular tabs (clipped to scroll area) ────────────
    {
        let cfg = &pc.config;
        let draw = ui.get_window_draw_list();
        let time = ui.time() as f32;
        let clip_min = [reg_clip_min_x, strip_y];
        let clip_max = [reg_clip_max_x, strip_y + strip_h + 1.0];
        draw.with_clip_rect(clip_min, clip_max, || {
            for &(idx, x0, x1, _tw, tab_hovered, close_hovered) in &pc.hit_scratch {
                let tab = &pc.tabs[idx];
                if tab.item.is_pinned() {
                    continue;
                }
                let is_active = pc.active == Some(tab.id);
                let y0 = strip_y + cfg.strip_padding_v;
                let y1 = y0 + cfg.tab_height;
                let anim_alpha = (tab.open_anim.clamp(0.0, 1.0) * 255.0) as u8;
                let accent = tab
                    .item
                    .tab_color()
                    .unwrap_or_else(|| cfg.colors.status_color(tab.item.status()));
                let ctx = TabDraw {
                    draw: &draw,
                    item: &tab.item,
                    cfg,
                    is_active,
                    hovered: tab_hovered,
                    close_hovered,
                    anim_alpha,
                    accent,
                    x0,
                    y0,
                    x1,
                    y1,
                    time,
                };
                draw_tab(&ctx);
            }
        });
    }

    // ── Phase 2: handle events using pre-computed scratch ───────────────
    handle_tab_events(
        pc,
        ui,
        &mut action,
        clicked,
        middle_clicked,
        right_clicked,
        hover_activate_ms,
        preview_hover_ms,
        scroll_area_w,
    );

    // ── Drag-and-drop (within own group: pinned ↔ pinned, regular ↔ regular) ─
    if draggable {
        handle_drag(pc, ui, mouse, strip_x, tabs_origin_x, strip_y, &mut action);
    }

    // ── Keyboard navigation (focus-gated) ───────────────────────────────
    if keyboard_nav && ui.is_window_focused() && !pc.tabs.is_empty() {
        handle_keyboard(pc, ui, scroll_area_w, &mut action);
    }

    // ── Advance cursor past the strip ───────────────────────────────────
    let content_start_y = ui.cursor_pos()[1] + strip_h + 2.0;
    ui.set_cursor_pos([ui.cursor_start_pos()[0], content_start_y]);
    ui.dummy([0.0, 0.0]);

    // ── Render active tab body ──────────────────────────────────────────
    //
    // Disjoint-field borrow: `&pc.config` (shared) and `entry` from
    // `pc.tabs.iter_mut()` (mutable) reference separate struct fields, which
    // the borrow checker accepts when written inline like this.
    if !external_content
        && let Some(active_id) = pc.active
        && let Some(entry) = pc.tabs.iter_mut().find(|t| t.id == active_id)
    {
        render_body(&pc.config, entry, ui);
    }

    action
}
