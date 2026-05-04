//! Rendering for [`TabControl`](super::TabControl).
//!
//! Single-file renderer: tab strip + scroll buttons + overflow dropdown +
//! drag-and-drop reorder + close confirmation popup. One pre-pass hit-test
//! fills [`TabHitRow`]; both drawing and event handling read from the same
//! scratch buffer — no duplicate geometry.

use std::fmt::Write;

use dear_imgui_rs::{Key, MouseButton, Ui, WindowFlags};

use crate::icons;
use crate::utils::color::rgb_arr as c32;
use crate::utils::popup::danger_button;
use crate::utils::text::calc_text_size;

use super::config::TabControlConfig;
use super::types::*;
use super::{TabControl, TabItem};

// ─── Tunable constants ──────────────────────────────────────────────────────

/// Convert a `TabColors` `[u8; 3]` token to an RGBA `[f32; 4]` for
/// `Ui::text_colored` (which wants linear floats).
#[inline]
fn rgba(c: [u8; 3], a: f32) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        a,
    ]
}

/// Duration of the tab close animation, seconds.
const TAB_CLOSE_ANIMATION_SECS: f32 = 0.15;
/// Duration of the tab open animation, seconds.
const TAB_OPEN_ANIMATION_SECS: f32 = 0.12;
/// Maximum time between two clicks to count as a double-click, seconds.
const DOUBLE_CLICK_THRESHOLD_SECS: f64 = 0.35;
/// Pixels of horizontal mouse movement before a tab drag begins.
const DRAG_START_THRESHOLD_PX: f32 = 5.0;
/// Smooth-scroll exponential coefficient (higher = snappier).
/// Exponential-decay coefficient for the smooth-scroll easing —
/// `scroll_offset += diff * (1 - exp(-COEF * dt))` per frame. Higher
/// = faster ease. `28.0` lands on the active tab in ~3 frames @ 60 fps
/// (1/e time ≈ 36 ms) — reads as "instant but not jarring", per user
/// feedback 2026-04-30 ("уменьшить анимацию"). Earlier `14.0` felt
/// sluggish on long jumps; the temporary hard-snap (commit `feea46f`)
/// felt too abrupt — this is the middle ground.
pub(super) const SMOOTH_SCROLL_COEF: f32 = 28.0;
/// Per-side hit-area padding for close buttons (matches the visual hover bg).
const CLOSE_HIT_PAD: f32 = 2.0;

// Layout micro-constants live in `super::layout` — single source of truth
// shared with `compute_tab_width`.
use super::layout::{
    BADGE_INNER_PAD_X, BADGE_INNER_PAD_Y, BADGE_LEFT_GAP, ICON_TITLE_GAP, PINNED_SEPARATOR_VPAD,
    PINNED_SEPARATOR_W, STATUS_DOT_DIAM, STATUS_DOT_GAP,
};

// ─── Hit-test scratch ───────────────────────────────────────────────────────

/// One row of the tab hit-test scratch buffer, computed once per frame.
///
/// Fields: `(idx, x0, x1, tw, tab_hovered, close_hit)`.
pub(crate) type TabHitRow = (usize, f32, f32, f32, bool, bool);

// ─── Tab draw context (for style dispatch) ─────────────────────────────────

struct TabDraw<'a, T: TabItem> {
    draw: &'a dear_imgui_rs::DrawListMut<'a>,
    item: &'a T,
    cfg: &'a TabControlConfig,
    is_active: bool,
    hovered: bool,
    close_hovered: bool,
    /// Combined animation alpha (0..1) — fades tab content during open/close.
    anim_alpha: u8,
    accent: [u8; 3],
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    time: f32,
}

// ─── Main entry point ───────────────────────────────────────────────────────

pub(crate) fn render_tab_control<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) -> Option<TabAction> {
    pc.open_context_menu = false;

    tick_animations(pc, ui);

    let mut action: Option<TabAction> = None;

    if pc.tabs.is_empty() {
        if pc.config.show_empty_placeholder {
            render_empty_placeholder(ui, &pc.config);
        }
    } else {
        action = render_strip(pc, ui);
    }

    render_close_popup(pc, ui);

    // Process deferred closes (animation finished → tab.open=false)
    let mut closed_id: Option<TabId> = None;
    pc.tabs.retain(|t| {
        if !t.open {
            closed_id = Some(t.id);
            false
        } else {
            true
        }
    });
    if let Some(id) = closed_id {
        if pc.active == Some(id) {
            pc.active = pc.tabs.last().map(|t| t.id);
            if let Some(new_id) = pc.active
                && let Some(entry) = pc.tabs.iter_mut().find(|t| t.id == new_id)
            {
                entry.item.on_activated();
            }
        }
        pc.invalidate_tab_layout_cache();
        action = Some(TabAction::Closed(id));
    }

    action
}

// ─── Animation tick ─────────────────────────────────────────────────────────

fn tick_animations<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) {
    let dt = ui.io().delta_time();

    // Close animation
    if let Some((closing_id, ref mut frac)) = pc.closing_tab {
        *frac -= dt / TAB_CLOSE_ANIMATION_SECS;
        if *frac <= 0.0 {
            if let Some(tab) = pc.tabs.iter_mut().find(|t| t.id == closing_id) {
                tab.open = false;
            }
            pc.closing_tab = None;
        }
    }

    // Open animation: every tab with open_anim < 1 advances toward 1.
    // The animation only multiplies the rendered width inside
    // `fill_hit_scratch` — the cached *base* width doesn't change, so we
    // don't need to invalidate `tab_widths_cache` per frame. This saves a
    // `calc_text_size` per tab per frame for the duration of the animation.
    if pc.config.animate_open {
        let step = dt / TAB_OPEN_ANIMATION_SECS;
        for tab in &mut pc.tabs {
            if tab.open_anim < 1.0 {
                tab.open_anim = (tab.open_anim + step).min(1.0);
            }
        }
    }
}

// ─── Empty placeholder ──────────────────────────────────────────────────────

fn render_empty_placeholder(ui: &Ui, cfg: &TabControlConfig) {
    let avail = ui.content_region_avail();
    let strings = &cfg.strings;
    // Show the MDI dashboard glyph only when the host has registered
    // the icon font; otherwise the placeholder would read `? No tabs`
    // (M4 from session 035 audit, visible in user screenshot).
    let icon: Option<&str> = if cfg.icons_available {
        Some(icons::VIEW_DASHBOARD_OUTLINE)
    } else {
        None
    };

    let icon_sz = icon.map(calc_text_size).unwrap_or([0.0, 0.0]);
    let main_sz = calc_text_size(&strings.no_tabs);
    let hint_sz = calc_text_size(&strings.empty_hint);

    let spacing = 8.0;
    // Skip the icon row in the centring math when the icon is hidden,
    // so `No tabs` lands at the visual centre (not biased upward by
    // the gap that would have hosted the missing glyph).
    let icon_block = if icon.is_some() {
        icon_sz[1] + spacing
    } else {
        0.0
    };
    let total_h = icon_block + main_sz[1] + spacing * 0.5 + hint_sz[1];
    let start_y = (avail[1] - total_h) * 0.5;
    let cs = ui.cursor_pos();

    let label_color = rgba(cfg.colors.text_muted, 1.0);
    let hint_color = rgba(cfg.colors.text_muted, 0.7);

    if let Some(glyph) = icon {
        ui.set_cursor_pos([cs[0] + (avail[0] - icon_sz[0]) * 0.5, cs[1] + start_y]);
        ui.text_colored(label_color, glyph);
    }

    ui.set_cursor_pos([
        cs[0] + (avail[0] - main_sz[0]) * 0.5,
        cs[1] + start_y + icon_block,
    ]);
    ui.text_colored(label_color, &strings.no_tabs);

    ui.set_cursor_pos([
        cs[0] + (avail[0] - hint_sz[0]) * 0.5,
        cs[1] + start_y + icon_block + main_sz[1] + spacing * 0.5,
    ]);
    ui.text_colored(hint_color, &strings.empty_hint);
}

// ─── Tab strip + content ────────────────────────────────────────────────────

fn render_strip<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) -> Option<TabAction> {
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
    // Just sets `scroll_target`; the smooth-scroll loop above
    // (line 384–391) does the actual easing. Earlier the renderer
    // hard-snapped (`scroll_offset = scroll_target`) on every
    // activation to fix the "по чуть-чуть" sluggishness, but that
    // felt too abrupt — see user feedback 2026-04-30. The compromise
    // is a faster `SMOOTH_SCROLL_COEF` so the ease finishes in
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
    if !external_content
        && let Some(active_id) = pc.active
        && let Some(entry) = pc.tabs.iter_mut().find(|t| t.id == active_id)
    {
        if pc.config.body_inset_enabled {
            // Visible-frame layout (ASCII model from user feedback
            // 2026-04-30):
            //
            //   [Tab strip]
            //   |-------------------------------------|   <- outer (strip_bg)
            //   ||-----------------------------------||   <- inner (body_bg child window)
            //   ||                                   ||      gap = strip_bg
            //   ||      widgets clipped here         ||      inset by `pad`
            //   |-------------------------------------|
            //
            // Outer rect is painted directly on the parent's draw
            // list, then the cursor is offset by `pad` and the host's
            // `render_content` runs inside a *real* child window
            // sized to fit between the outer rect and `pad` on all
            // four sides. The child gives us proper clipping so
            // text wrap, button max width, etc. respect the inner
            // bounds — a draw-list-only approach (paint inner rect
            // + cursor offset) made widgets bleed past the right
            // edge because cursor offset doesn't reduce the line
            // width.
            let pad = pc.config.body_inset;
            let cur_screen = ui.cursor_screen_pos();
            let avail_full = ui.content_region_avail();
            let outer_min = cur_screen;
            let outer_max = [cur_screen[0] + avail_full[0], cur_screen[1] + avail_full[1]];

            let inner_w = avail_full[0] - 2.0 * pad[0];
            let inner_h = avail_full[1] - 2.0 * pad[1];

            // Degenerate-size short-circuit (window narrower than
            // `2 * pad`, etc.). ImGui's `BeginChild` triggers a
            // STATUS_STACK_BUFFER_OVERRUN panic on Windows when
            // given a 0-or-negative size — and painting the outer
            // `frame_bg` rectangle on its own (without the inner
            // child to overlay) would leave a `frame_bg` flash
            // around `render_content`. Falling back to the
            // no-frame path keeps the host alive AND avoids the
            // visual glitch the analyst flagged.
            if inner_w <= 1.0 || inner_h <= 1.0 {
                entry.item.render_content(ui);
            } else {
                // Scope the draw-list guard: `DrawListMut` is a
                // global mutex, and `child_window().build(...)`
                // below executes host code that itself may call
                // `ui.get_window_draw_list()` (e.g. VirtualTable
                // row painters / host overlays). Holding `draw`
                // across that nested `build` panics with "A
                // DrawListMut is already in use!" — drop it here
                // first.
                {
                    let draw = ui.get_window_draw_list();
                    // Outer fill — `frame_bg` paints the gap
                    // around the inner body. Default mirrors
                    // `strip_bg` so the strip + frame read as one
                    // chrome surface; hosts can recolour
                    // `frame_bg` independently without touching
                    // the tab strip itself.
                    draw.add_rect(outer_min, outer_max, c32(pc.config.colors.frame_bg, 255))
                        .filled(true)
                        .build();
                    // Optional active-frame border — outlined rect
                    // drawn OVER the frame fill so the host gets an
                    // "active pane" cue matching the strip's
                    // selected-tab hue (IDE-style highlight). Off
                    // by default; toggled via
                    // `TabControlConfig::body_inset_border`.
                    if pc.config.body_inset_border {
                        draw.add_rect(
                            outer_min,
                            outer_max,
                            c32(pc.config.colors.frame_border, 255),
                        )
                        .filled(false)
                        .thickness(pc.config.body_inset_border_thickness)
                        .build();
                    }
                }

                // Shift cursor inward — the upcoming child_window
                // picks its position from the current cursor.
                let cur = ui.cursor_pos();
                ui.set_cursor_pos([cur[0] + pad[0], cur[1] + pad[1]]);
                let inner_size = [inner_w, inner_h];
                // Borderless child filled with `body_bg` — clips
                // widgets to the inner rect. Push WindowPadding(0,0)
                // so the host's first widget sits flush against the
                // inner rect's top-left (host can re-add its own
                // breathing room inside `render_content` if needed).
                let _wp = ui.push_style_var(dear_imgui_rs::StyleVar::WindowPadding([0.0, 0.0]));
                let _bg = ui.push_style_color(
                    dear_imgui_rs::StyleColor::ChildBg,
                    rgba(pc.config.colors.body_bg, 1.0),
                );
                ui.child_window("##tab_content")
                    .size(inner_size)
                    .border(false)
                    .build(ui, || {
                        entry.item.render_content(ui);
                    });
            }
        } else {
            entry.item.render_content(ui);
        }
    }

    action
}

// ─── PRE-PASS: hit-test ─────────────────────────────────────────────────────

/// Two-pass fill: pinned tabs first (left-anchored, no scroll), then regular
/// tabs (with scroll, clipped to the regular area). Order in `hit_scratch`
/// matches the visual order, which keeps drawing/event code simple.
#[allow(clippy::too_many_arguments)]
fn fill_hit_scratch<T: TabItem>(
    pc: &mut TabControl<T>,
    accept_clicks: bool,
    mouse: [f32; 2],
    strip_x: f32,
    strip_y_top: f32,
    pinned_max_x: f32,
    regular_origin_x: f32,
    regular_clip_min_x: f32,
    regular_clip_max_x: f32,
) {
    pc.hit_scratch.clear();
    let cfg = &pc.config;
    let y0 = strip_y_top + cfg.strip_padding_v;
    let tab_h = cfg.tab_height;

    // Helper to compute animated width for a tab
    let anim_width = |idx: usize, tab_id: TabId, base_tw: f32| -> f32 {
        let close_frac = match pc.closing_tab {
            Some((cid, frac)) if cid == tab_id => frac.max(0.0),
            _ => 1.0,
        };
        let open_frac = pc.tabs[idx].open_anim.clamp(0.0, 1.0);
        base_tw * close_frac * open_frac
    };

    // Pass 1 — pinned (no scroll, hit-tested against pinned strip area)
    let mut tx = strip_x;
    for (i, tab) in pc.tabs.iter().enumerate() {
        if !tab.item.is_pinned() {
            continue;
        }
        let Some(&base_tw) = pc.tab_widths_cache.get(i) else {
            break;
        };
        let tw = anim_width(i, tab.id, base_tw);
        if tw < 1.0 {
            tx += cfg.tab_gap;
            continue;
        }
        let x0 = tx;
        let x1 = tx + tw;
        let outside = x1 < strip_x || x0 > pinned_max_x;
        let hovered = !outside
            && accept_clicks
            && mouse[0] >= x0.max(strip_x)
            && mouse[0] < x1.min(pinned_max_x)
            && mouse[1] >= y0
            && mouse[1] < y0 + tab_h;
        // Pinned tabs never expose a close button.
        pc.hit_scratch.push((i, x0, x1, tw, hovered, false));
        tx += tw + cfg.tab_gap;
    }

    // Pass 2 — regular (with scroll, clipped to regular strip)
    let mut tx = regular_origin_x - pc.scroll_offset;
    for (i, tab) in pc.tabs.iter().enumerate() {
        if tab.item.is_pinned() {
            continue;
        }
        let Some(&base_tw) = pc.tab_widths_cache.get(i) else {
            break;
        };
        let tw = anim_width(i, tab.id, base_tw);
        if tw < 1.0 {
            tx += cfg.tab_gap;
            continue;
        }
        let x0 = tx;
        let x1 = tx + tw;
        let outside = x1 < regular_clip_min_x || x0 > regular_clip_max_x;
        let hovered = !outside
            && accept_clicks
            && mouse[0] >= x0.max(regular_clip_min_x)
            && mouse[0] < x1.min(regular_clip_max_x)
            && mouse[1] >= y0
            && mouse[1] < y0 + tab_h;
        let can_close = cfg.closable && tab.item.is_closable();
        let close_hit = hovered
            && can_close
            && is_close_hovered(mouse, x1, y0, cfg, regular_clip_min_x, regular_clip_max_x);
        pc.hit_scratch.push((i, x0, x1, tw, hovered, close_hit));
        tx += tw + cfg.tab_gap;
    }
}

#[inline]
fn is_close_hovered(
    mouse: [f32; 2],
    x1: f32,
    y0: f32,
    cfg: &TabControlConfig,
    clip_min_x: f32,
    clip_max_x: f32,
) -> bool {
    let cx = x1 - cfg.tab_padding_h - cfg.close_btn_size;
    let cy_center = y0 + cfg.tab_height * 0.5;
    let half = cfg.close_btn_size * 0.5 + CLOSE_HIT_PAD;
    mouse[0] >= (cx - CLOSE_HIT_PAD).max(clip_min_x)
        && mouse[0] < (cx + cfg.close_btn_size + CLOSE_HIT_PAD).min(clip_max_x)
        && mouse[1] >= cy_center - half
        && mouse[1] < cy_center + half
}

// ─── Event handling ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn handle_tab_events<T: TabItem>(
    pc: &mut TabControl<T>,
    ui: &Ui,
    action: &mut Option<TabAction>,
    clicked: bool,
    middle_clicked: bool,
    right_clicked: bool,
    hover_activate_ms: Option<u32>,
    preview_hover_ms: Option<u32>,
    scroll_area_w: f32,
) {
    let cfg_closable = pc.config.closable;
    let cfg_middle_close = pc.config.middle_click_close;
    let cfg_context = pc.config.context_menu;
    let cfg_confirm = pc.config.confirm_close;
    let cfg_animate = pc.config.animate_close;

    let mut close_target: Option<TabId> = None;
    let mut activate_target: Option<TabId> = None;
    let mut activate_idx: Option<usize> = None;
    let mut context_target: Option<TabId> = None;
    let mut double_click_target: Option<TabId> = None;
    let mut tooltip_idx: Option<usize> = None;
    let mut drag_idx: Option<(usize, f32)> = None;

    // Track which tab the mouse is currently over (any), and the inactive one
    // separately (used for hover-delayed activation).
    let mut hovered_id: Option<TabId> = None;
    let mut hovered_inactive_id: Option<TabId> = None;

    for &(idx, _x0, _x1, _tw, hovered, close_hit) in &pc.hit_scratch {
        if !hovered {
            continue;
        }
        let tab = &pc.tabs[idx];
        let can_close = cfg_closable && tab.item.is_closable();

        hovered_id = Some(tab.id);
        if pc.active != Some(tab.id) {
            hovered_inactive_id = Some(tab.id);
        }

        if clicked {
            if close_hit && can_close {
                close_target = Some(tab.id);
            } else {
                let now = ui.time();
                if pc.last_click_tab == Some(tab.id)
                    && (now - pc.last_click_time) < DOUBLE_CLICK_THRESHOLD_SECS
                {
                    double_click_target = Some(tab.id);
                    pc.last_click_tab = None;
                } else {
                    pc.last_click_time = now;
                    pc.last_click_tab = Some(tab.id);
                }
                drag_idx = Some((idx, ui.io().mouse_pos()[0]));
                if pc.active != Some(tab.id) {
                    activate_target = Some(tab.id);
                    activate_idx = Some(idx);
                }
            }
        }

        if middle_clicked && cfg_middle_close && can_close {
            close_target = Some(tab.id);
        }

        if right_clicked && cfg_context {
            context_target = Some(tab.id);
        }

        if !clicked && !middle_clicked && !right_clicked && tab.item.tooltip().is_some() {
            tooltip_idx = Some(idx);
        }
    }

    // Update hover_target based on which tab is hovered this frame.
    // hover_target = Some((id, time when hover began)).
    let now = ui.time();
    pc.hover_target = match (hovered_id, pc.hover_target) {
        (Some(id), Some((tracked, started))) if tracked == id => Some((tracked, started)),
        (Some(id), _) => Some((id, now)),
        (None, _) => None,
    };

    // Hover-delayed activation (Edge / Win11 style)
    if let Some(ms) = hover_activate_ms
        && activate_target.is_none()
        && let Some(id) = hovered_inactive_id
        && let Some((tracked, started)) = pc.hover_target
        && tracked == id
        && (now - started) * 1000.0 >= ms as f64
    {
        activate_target = Some(id);
        activate_idx = pc.tabs.iter().position(|t| t.id == id);
    }

    // Hover preview popup (Windows-taskbar-peek style). Triggered for
    // *inactive* tabs only — peeking the active tab is pointless since the
    // user is already looking at its content.
    //
    // We render directly into the tooltip window (no child_window) so the
    // tooltip auto-sizes vertically to the actual content. A
    // `SetNextWindowSizeConstraints` call locks the *width* to
    // `cfg.preview_size[0]`, which makes `text_wrapped` and similar widgets
    // wrap consistently while letting height grow freely up to the safety
    // ceiling. Result: full content visible, no scrollbar, ever.
    if let Some(ms) = preview_hover_ms
        && pc.drag_source_idx.is_none()              // never popup during drag
        && pc.active != hovered_id
        && let Some((tracked, started)) = pc.hover_target
        && hovered_id == Some(tracked)
        && (now - started) * 1000.0 >= ms as f64
        && let Some(idx) = pc.tabs.iter().position(|t| t.id == tracked)
        && pc.tabs[idx].item.show_preview()
    // per-tab opt-out
    {
        let preview_w = pc.config.preview_size[0];
        let preview_h_max = pc.config.preview_size[1].max(64.0) * 8.0; // generous upper bound
        let preview_scale = pc.config.preview_font_scale;
        let title = pc.tabs[idx].item.title().to_string();
        let tab_id = pc.tabs[idx].id;
        let item: &mut T = &mut pc.tabs[idx].item;

        // SAFETY: `igSetNextWindowSizeConstraints` is a side-effect-only ImGui
        // call that records the constraint for the *next* `Begin*`. Both
        // `ImVec2_c` values are plain `repr(C)` floats with no invariants;
        // the optional callback pointer is `None` and the userdata is null.
        // No ImGui state is mutated across-thread (single-threaded UI).
        unsafe {
            dear_imgui_rs::sys::igSetNextWindowSizeConstraints(
                dear_imgui_rs::sys::ImVec2_c {
                    x: preview_w,
                    y: 0.0,
                },
                dear_imgui_rs::sys::ImVec2_c {
                    x: preview_w,
                    y: preview_h_max,
                },
                None,
                std::ptr::null_mut(),
            );
        }

        crate::utils::themed_tooltip(ui, || {
            // ID stack push isolates widget IDs in the preview from any other
            // place rendering the same content tree (e.g. an inner TabControl
            // re-rendered as a thumbnail).
            let _id = ui.push_id(tab_id as usize);
            // `set_window_font_scale` is a per-window write; the
            // tooltip is its own window so the scale doesn't leak
            // outside this closure when the tooltip closes. The
            // explicit `1.0` restore at the tail covers nested
            // `render_preview` impls that opened sibling tooltips
            // during the body — those would otherwise inherit the
            // shrunk scale (M5 from session 034 audit). Real
            // save / restore would need unsafe `igGetCurrentWindow`
            // access; the `1.0` clamp is the practical compromise.
            ui.set_window_font_scale(preview_scale);
            ui.text(title);
            ui.separator();
            item.render_preview(ui);
            ui.set_window_font_scale(1.0);
        });
    }

    // Apply collected actions
    if let Some(id) = close_target {
        if cfg_confirm {
            pc.pending_close = Some(id);
            pc.pending_close_new = true;
        } else if cfg_animate {
            pc.closing_tab = Some((id, 1.0));
        } else if let Some(t) = pc.tabs.iter_mut().find(|t| t.id == id) {
            t.open = false;
        }
    }

    if let (Some(new_id), Some(idx)) = (activate_target, activate_idx) {
        if let Some(old_id) = pc.active
            && let Some(old) = pc.tabs.iter_mut().find(|t| t.id == old_id)
        {
            old.item.on_deactivated();
        }
        pc.active = Some(new_id);
        if let Some(entry) = pc.tabs.iter_mut().find(|t| t.id == new_id) {
            entry.item.on_activated();
        }
        *action = Some(TabAction::Activated(new_id));
        // Snap scroll to the clicked tab — the tab was probably
        // visible already, but partial-overlap clicks (tab half-
        // clipped at the right edge) feel more decisive when the
        // strip jumps the few remaining pixels in one frame.
        scroll_into_view(pc, idx, scroll_area_w);
    }

    if let Some((idx, x)) = drag_idx {
        pc.drag_source_idx = Some(idx);
        pc.drag_start_x = x;
        pc.dragging = false;
    }

    if let Some(id) = context_target {
        pc.context_tab = Some(id);
        pc.open_context_menu = true;
    }

    if let Some(id) = double_click_target {
        *action = Some(TabAction::DoubleClicked(id));
    }

    if let Some(idx) = tooltip_idx
        && let Some(tip) = pc.tabs[idx].item.tooltip()
    {
        crate::utils::themed_tooltip(ui, || ui.text(tip));
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_drag<T: TabItem>(
    pc: &mut TabControl<T>,
    ui: &Ui,
    mouse: [f32; 2],
    strip_x: f32,
    tabs_origin_x: f32,
    strip_y: f32,
    action: &mut Option<TabAction>,
) {
    let Some(src_idx) = pc.drag_source_idx else {
        return;
    };
    let mouse_held = ui.is_mouse_down(MouseButton::Left);
    if !mouse_held {
        pc.drag_source_idx = None;
        pc.dragging = false;
        return;
    }

    let dx = (mouse[0] - pc.drag_start_x).abs();
    if dx > DRAG_START_THRESHOLD_PX {
        pc.dragging = true;
    }
    if !pc.dragging {
        return;
    }

    // Pinned tabs only swap with other pinned, regular only with regular —
    // crossing the boundary is forbidden. This keeps the visual zones intact.
    let src_is_pinned = pc.tabs.get(src_idx).is_some_and(|t| t.item.is_pinned());

    // Find target index by walking midpoints in the appropriate origin.
    let cfg = &pc.config;
    let origin = if src_is_pinned {
        strip_x
    } else {
        tabs_origin_x - pc.scroll_offset
    };
    let mut tx2 = origin;
    let mut target_idx: Option<usize> = None;
    for j in 0..pc.tabs.len() {
        let same_group = pc.tabs[j].item.is_pinned() == src_is_pinned;
        if !same_group {
            continue;
        }
        let Some(&tw2) = pc.tab_widths_cache.get(j) else {
            break;
        };
        let mid = tx2 + tw2 * 0.5;
        if mouse[0] < mid {
            target_idx = Some(j);
            break;
        }
        tx2 += tw2 + cfg.tab_gap;
    }
    // Clamp target to last tab in the same group
    let last_in_group = pc
        .tabs
        .iter()
        .enumerate()
        .rev()
        .find(|(_, t)| t.item.is_pinned() == src_is_pinned)
        .map(|(i, _)| i);
    let target = target_idx.or(last_in_group).unwrap_or(src_idx);

    // Pinned invariant guarantees `pc.tabs` is partitioned (pinned prefix,
    // regular suffix). `target_idx` was selected from the same group as
    // `src_idx`, so `src_idx ± 1` stays within that group: a plain swap is
    // sufficient and never crosses the boundary.
    if target != src_idx && target < pc.tabs.len() {
        let new_src = if target < src_idx {
            pc.tabs.swap(src_idx, src_idx - 1);
            src_idx - 1
        } else {
            pc.tabs.swap(src_idx, src_idx + 1);
            src_idx + 1
        };
        // Release-mode guard (M3 from session 034 audit) — if a
        // future change to the `target_idx` selection ever returns
        // an out-of-group index, `enforce_pinned_partition` would
        // observe corruption only on the next frame. Bail out here
        // so the swap is reverted before the partition pass runs.
        if pc.tabs[new_src].item.is_pinned() != src_is_pinned {
            // Undo the offending swap.
            pc.tabs.swap(src_idx, new_src);
            return;
        }
        debug_assert_eq!(
            pc.tabs[new_src].item.is_pinned(),
            src_is_pinned,
            "drag swap escaped its group — pinned partition broken"
        );
        pc.drag_source_idx = Some(new_src);
        pc.tab_gen = pc.tab_gen.wrapping_add(1);
        let moved_id = pc.tabs[new_src].id;
        *action = Some(TabAction::Reordered(moved_id));
    }

    // Drag indicator + ghost tab
    let draw = ui.get_window_draw_list();
    let drag_y = strip_y + cfg.strip_padding_v;
    draw.add_line(
        [mouse[0], drag_y],
        [mouse[0], drag_y + cfg.tab_height],
        c32(cfg.colors.accent, 220),
    )
    .thickness(2.0)
    .build();

    if let Some(&tw) = pc.tab_widths_cache.get(src_idx) {
        let ghost_x0 = mouse[0] - tw * 0.5;
        let ghost_y0 = drag_y;
        let ghost_x1 = ghost_x0 + tw;
        let ghost_y1 = ghost_y0 + cfg.tab_height;
        let alpha_mul = 0.55_f32;
        let tab = &pc.tabs[src_idx];
        let accent = tab
            .item
            .tab_color()
            .unwrap_or_else(|| cfg.colors.status_color(tab.item.status()));
        let bg = c32(cfg.colors.tab_active, (235.0 * alpha_mul) as u8);
        draw.add_rect([ghost_x0, ghost_y0], [ghost_x1, ghost_y1], bg)
            .filled(true)
            .rounding(cfg.tab_rounding)
            .build();
        draw.add_rect([ghost_x0, ghost_y0], [ghost_x1, ghost_y1], c32(accent, 200))
            .filled(false)
            .thickness(1.5)
            .rounding(cfg.tab_rounding)
            .build();
        let title = tab.item.title();
        let ts = calc_text_size(title);
        let tx = ghost_x0 + (tw - ts[0]) * 0.5;
        let ty = ghost_y0 + (cfg.tab_height - ts[1]) * 0.5;
        // Drag ghost is a shadow of the dragged tab — honour the
        // per-tab `text_color()` override so a colored tab keeps its
        // hue while being dragged.
        let ghost_text = tab.item.text_color().unwrap_or(cfg.colors.text);
        let fg = c32(ghost_text, (240.0 * alpha_mul) as u8);
        draw.add_text([tx, ty], fg, title);
    }
}

fn handle_keyboard<T: TabItem>(
    pc: &mut TabControl<T>,
    ui: &Ui,
    scroll_area_w: f32,
    action: &mut Option<TabAction>,
) {
    let io = ui.io();
    let ctrl = io.key_ctrl();
    let shift = io.key_shift();

    let prev = ui.is_key_pressed(Key::LeftArrow);
    let next = ui.is_key_pressed(Key::RightArrow);
    let ctrl_w = ctrl && ui.is_key_pressed(Key::W);
    let ctrl_t = ctrl && ui.is_key_pressed(Key::T);
    let ctrl_tab = ctrl && ui.is_key_pressed(Key::Tab);

    let mut nav_idx: Option<usize> = None;

    // Arrow navigation (no modifier)
    if !ctrl && (prev || next) {
        if let Some(active_id) = pc.active {
            if let Some(idx) = pc.tabs.iter().position(|t| t.id == active_id) {
                let new_idx = if prev {
                    idx.saturating_sub(1)
                } else {
                    (idx + 1).min(pc.tabs.len() - 1)
                };
                if new_idx != idx {
                    pc.tabs[idx].item.on_deactivated();
                    let new_id = pc.tabs[new_idx].id;
                    pc.active = Some(new_id);
                    pc.tabs[new_idx].item.on_activated();
                    *action = Some(TabAction::Activated(new_id));
                    nav_idx = Some(new_idx);
                }
            }
        } else {
            let id = pc.tabs[0].id;
            pc.active = Some(id);
            pc.tabs[0].item.on_activated();
            *action = Some(TabAction::Activated(id));
            nav_idx = Some(0);
        }
    }

    // Ctrl+Tab / Ctrl+Shift+Tab — cycle (with wraparound, like browsers)
    if ctrl_tab && !pc.tabs.is_empty() {
        let cur = pc
            .active
            .and_then(|id| pc.tabs.iter().position(|t| t.id == id))
            .unwrap_or(0);
        let len = pc.tabs.len();
        let new_idx = if shift {
            (cur + len - 1) % len
        } else {
            (cur + 1) % len
        };
        if new_idx != cur {
            pc.tabs[cur].item.on_deactivated();
            let new_id = pc.tabs[new_idx].id;
            pc.active = Some(new_id);
            pc.tabs[new_idx].item.on_activated();
            *action = Some(TabAction::Activated(new_id));
            nav_idx = Some(new_idx);
        }
    }

    // Ctrl+1..9 — jump to nth tab (1-based; Ctrl+9 jumps to last)
    if ctrl {
        let digit_keys = [
            Key::Key1,
            Key::Key2,
            Key::Key3,
            Key::Key4,
            Key::Key5,
            Key::Key6,
            Key::Key7,
            Key::Key8,
            Key::Key9,
        ];
        for (i, key) in digit_keys.iter().enumerate() {
            if ui.is_key_pressed(*key) && !pc.tabs.is_empty() {
                let target = if i == 8 {
                    // Ctrl+9 → last (Chrome convention)
                    pc.tabs.len() - 1
                } else {
                    (i).min(pc.tabs.len() - 1)
                };
                let cur = pc
                    .active
                    .and_then(|id| pc.tabs.iter().position(|t| t.id == id));
                if cur != Some(target) {
                    if let Some(c) = cur {
                        pc.tabs[c].item.on_deactivated();
                    }
                    let new_id = pc.tabs[target].id;
                    pc.active = Some(new_id);
                    pc.tabs[target].item.on_activated();
                    *action = Some(TabAction::Activated(new_id));
                    nav_idx = Some(target);
                }
                break;
            }
        }
    }

    if let Some(idx) = nav_idx {
        scroll_into_view(pc, idx, scroll_area_w);
    }

    // Ctrl+T — request a new tab
    if ctrl_t {
        *action = Some(TabAction::AddRequested);
    }

    // Ctrl+W — close active
    if ctrl_w && let Some(active_id) = pc.active {
        let can_close = pc
            .tabs
            .iter()
            .find(|t| t.id == active_id)
            .is_some_and(|t| pc.config.closable && t.item.is_closable());
        if can_close {
            if pc.config.confirm_close {
                pc.pending_close = Some(active_id);
                pc.pending_close_new = true;
            } else if pc.config.animate_close {
                pc.closing_tab = Some((active_id, 1.0));
            } else if let Some(t) = pc.tabs.iter_mut().find(|t| t.id == active_id) {
                t.open = false;
            }
        }
    }
}

fn scroll_into_view<T: TabItem>(pc: &mut TabControl<T>, idx: usize, scroll_area_w: f32) {
    let cfg = &pc.config;
    let Some(&tw) = pc.tab_widths_cache.get(idx) else {
        return;
    };
    let mut tx: f32 = 0.0;
    for w in pc.tab_widths_cache.iter().take(idx) {
        tx += w + cfg.tab_gap;
    }
    if tx < pc.scroll_target {
        pc.scroll_target = tx;
    } else if tx + tw > pc.scroll_target + scroll_area_w {
        pc.scroll_target = tx + tw - scroll_area_w;
    }
}

// ─── Side buttons (scroll / overflow / add) ────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_scroll_buttons(
    ui: &Ui,
    strip_x: f32,
    strip_y: f32,
    avail_w: f32,
    strip_h: f32,
    scroll_offset: &mut f32,
    total_w: f32,
    scroll_area_w: f32,
    accept_clicks: bool,
    mouse: [f32; 2],
    cfg: &TabControlConfig,
) {
    let draw = ui.get_window_draw_list();
    let colors = &cfg.colors;
    let btn_w = cfg.scroll_btn_width;

    let lx = strip_x;
    let lhover = accept_clicks
        && mouse[0] >= lx
        && mouse[0] < lx + btn_w
        && mouse[1] >= strip_y
        && mouse[1] < strip_y + strip_h;

    let lbg = if lhover {
        colors.tab_hover
    } else {
        colors.strip_bg
    };
    draw.add_rect(
        [lx, strip_y],
        [lx + btn_w, strip_y + strip_h],
        c32(lbg, 255),
    )
    .filled(true)
    .build();
    let arrow = icons::CHEVRON_LEFT;
    let asz = calc_text_size(arrow);
    draw.add_text(
        [
            lx + (btn_w - asz[0]) * 0.5,
            strip_y + (strip_h - asz[1]) * 0.5,
        ],
        c32(colors.text, if lhover { 255 } else { 160 }),
        arrow,
    );
    if accept_clicks && lhover && ui.is_mouse_down(MouseButton::Left) {
        *scroll_offset -= cfg.scroll_speed * ui.io().delta_time();
    }

    let rx = strip_x + avail_w - btn_w;
    let rhover = accept_clicks
        && mouse[0] >= rx
        && mouse[0] < rx + btn_w
        && mouse[1] >= strip_y
        && mouse[1] < strip_y + strip_h;

    let rbg = if rhover {
        colors.tab_hover
    } else {
        colors.strip_bg
    };
    draw.add_rect(
        [rx, strip_y],
        [rx + btn_w, strip_y + strip_h],
        c32(rbg, 255),
    )
    .filled(true)
    .build();
    let arrow_r = icons::CHEVRON_RIGHT;
    let arsz = calc_text_size(arrow_r);
    draw.add_text(
        [
            rx + (btn_w - arsz[0]) * 0.5,
            strip_y + (strip_h - arsz[1]) * 0.5,
        ],
        c32(colors.text, if rhover { 255 } else { 160 }),
        arrow_r,
    );
    if accept_clicks && rhover && ui.is_mouse_down(MouseButton::Left) {
        *scroll_offset += cfg.scroll_speed * ui.io().delta_time();
    }

    let max_scroll = (total_w - scroll_area_w).max(0.0);
    *scroll_offset = scroll_offset.clamp(0.0, max_scroll);
}

fn render_overflow_button(
    ui: &Ui,
    x: f32,
    strip_y: f32,
    w: f32,
    accept_clicks: bool,
    mouse: [f32; 2],
    cfg: &TabControlConfig,
) -> bool {
    let draw = ui.get_window_draw_list();
    let colors = &cfg.colors;
    let y0 = strip_y + cfg.strip_padding_v;
    let y1 = y0 + cfg.tab_height;
    let hovered =
        accept_clicks && mouse[0] >= x && mouse[0] < x + w && mouse[1] >= y0 && mouse[1] < y1;

    let bg = if hovered {
        colors.tab_hover
    } else {
        colors.strip_bg
    };
    draw.add_rect([x, y0], [x + w, y1], c32(bg, 255))
        .rounding(cfg.tab_rounding * 0.5)
        .filled(true)
        .build();
    let dots = icons::DOTS_HORIZONTAL;
    let dsz = calc_text_size(dots);
    draw.add_text(
        [x + (w - dsz[0]) * 0.5, y0 + (cfg.tab_height - dsz[1]) * 0.5],
        c32(colors.text, if hovered { 255 } else { 170 }),
        dots,
    );

    if hovered
        && !ui.is_mouse_clicked(MouseButton::Left)
        && !ui.is_mouse_clicked(MouseButton::Right)
    {
        crate::utils::themed_tooltip(ui, || ui.text(&cfg.strings.overflow_tooltip));
    }

    hovered && ui.is_mouse_clicked(MouseButton::Left)
}

fn render_overflow_popup_body<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) {
    let Some(_token) = ui.begin_popup(&pc.overflow_popup_id) else {
        return;
    };
    let mut focus_id: Option<TabId> = None;
    let icons_available = pc.config.icons_available;
    for i in 0..pc.tabs.len() {
        let tab = &pc.tabs[i];
        let id = tab.id;
        let is_active = pc.active == Some(id);
        pc.fmt_buf.clear();
        // Gate on `icons_available` — without the MDI font registered,
        // the codepoint renders as a `?` box. Layout / on-tab drawing
        // already check this; the popup was missed (M3 from session
        // 035 audit, visible in user screenshot).
        if icons_available
            && let Some(icon) = tab.item.icon()
        {
            let _ = write!(pc.fmt_buf, "{} ", icon);
        }
        let _ = write!(pc.fmt_buf, "{}", tab.item.title());
        if ui
            .selectable_config(&pc.fmt_buf)
            .selected(is_active)
            .build()
            && !is_active
        {
            focus_id = Some(id);
        }
    }
    if let Some(id) = focus_id
        && let Some(entry) = pc.tabs.iter_mut().find(|t| t.id == id)
    {
        entry.request_focus = true;
        ui.close_current_popup();
    }
}

fn render_add_button(
    ui: &Ui,
    x: f32,
    strip_y: f32,
    w: f32,
    accept_clicks: bool,
    mouse: [f32; 2],
    cfg: &TabControlConfig,
) -> bool {
    let draw = ui.get_window_draw_list();
    let colors = &cfg.colors;
    let y0 = strip_y + cfg.strip_padding_v;
    let y1 = y0 + cfg.tab_height;
    let hovered =
        accept_clicks && mouse[0] >= x && mouse[0] < x + w && mouse[1] >= y0 && mouse[1] < y1;

    let bg = if hovered {
        colors.tab_hover
    } else {
        colors.strip_bg
    };
    draw.add_rect([x, y0], [x + w, y1], c32(bg, 255))
        .rounding(cfg.tab_rounding * 0.5)
        .filled(true)
        .build();
    let plus = icons::PLUS;
    let psz = calc_text_size(plus);
    draw.add_text(
        [x + (w - psz[0]) * 0.5, y0 + (cfg.tab_height - psz[1]) * 0.5],
        c32(colors.text, if hovered { 255 } else { 170 }),
        plus,
    );
    if hovered && !ui.is_mouse_clicked(MouseButton::Left) {
        crate::utils::themed_tooltip(ui, || ui.text(&cfg.strings.add_tab));
    }
    hovered && ui.is_mouse_clicked(MouseButton::Left)
}

// ─── Tab drawing (style dispatch) ───────────────────────────────────────────

fn draw_tab<T: TabItem>(ctx: &TabDraw<'_, T>) {
    match ctx.cfg.tab_style {
        TabStyle::Pill => draw_tab_pill(ctx),
        TabStyle::Underline => draw_tab_underline(ctx),
        TabStyle::Square => draw_tab_square(ctx),
    }
    draw_tab_content(ctx);
}

// ─── Pill style ────────────────────────────────────────────────────────────

fn draw_tab_pill<T: TabItem>(ctx: &TabDraw<'_, T>) {
    let TabDraw {
        draw,
        cfg,
        is_active,
        hovered,
        accent,
        x0,
        y0,
        x1,
        y1,
        ..
    } = *ctx;
    let colors = &cfg.colors;
    let r = (y1 - y0) * 0.5; // fully rounded

    let bg = if is_active {
        colors.tab_active
    } else if hovered {
        colors.tab_hover
    } else {
        colors.tab_bg
    };
    draw.add_rect([x0, y0], [x1, y1], c32(bg, 255))
        .rounding(r)
        .filled(true)
        .build();

    if is_active {
        draw.add_rect([x0, y0], [x1, y1], c32(accent, 200))
            .rounding(r)
            .filled(false)
            .thickness(1.5)
            .build();
    }
}

// ─── Underline style ───────────────────────────────────────────────────────

fn draw_tab_underline<T: TabItem>(ctx: &TabDraw<'_, T>) {
    let TabDraw {
        draw,
        cfg,
        is_active,
        hovered,
        accent,
        x0,
        y0,
        x1,
        y1,
        ..
    } = *ctx;
    let colors = &cfg.colors;

    if hovered && !is_active {
        draw.add_rect([x0, y0 + 1.0], [x1, y1 - 1.0], c32(colors.tab_hover, 130))
            .rounding(2.0)
            .filled(true)
            .build();
    }
    if is_active {
        draw.add_rect([x0 + 2.0, y1 - 3.0], [x1 - 2.0, y1], c32(accent, 255))
            .rounding(1.5)
            .filled(true)
            .build();
    }
}

// ─── Square style ──────────────────────────────────────────────────────────

fn draw_tab_square<T: TabItem>(ctx: &TabDraw<'_, T>) {
    let TabDraw {
        draw,
        cfg,
        is_active,
        hovered,
        accent,
        x0,
        y0,
        x1,
        y1,
        ..
    } = *ctx;
    let colors = &cfg.colors;
    let bg = if is_active {
        colors.tab_active
    } else if hovered {
        colors.tab_hover
    } else {
        colors.tab_bg
    };
    draw.add_rect([x0, y0], [x1, y1 - 2.0], c32(bg, 255))
        .rounding(3.0)
        .filled(true)
        .build();
    draw.add_rect([x0, y1 - 4.0], [x1, y1], c32(bg, 255))
        .filled(true)
        .build();
    if is_active && cfg.show_tab_underline {
        draw.add_rect([x0 + 2.0, y1 - 2.0], [x1 - 2.0, y1], c32(accent, 255))
            .filled(true)
            .build();
    }
    if !is_active {
        draw.add_line([x0, y1], [x1, y1], c32(colors.separator, 120))
            .build();
    }
}

// ─── Tab content (icon, title, dot, badge, close) ──────────────────────────

fn draw_tab_content<T: TabItem>(ctx: &TabDraw<'_, T>) {
    let TabDraw {
        draw,
        item,
        cfg,
        is_active,
        hovered,
        close_hovered,
        anim_alpha,
        x0,
        y0,
        x1,
        time,
        ..
    } = *ctx;
    let colors = &cfg.colors;
    let tab_h = cfg.tab_height;

    // Modulate every alpha by the open-animation alpha
    let a = |base: u8| -> u8 { ((base as u32 * anim_alpha as u32) / 255) as u8 };

    // ── Pinned variant: compact, centered glyph or single letter ────────
    if item.is_pinned() {
        // Per-tab override wins for both glyph and letter fallback;
        // pinned tabs only show one of them, so a single resolution
        // covers the visible label whatever the host chose.
        let label_color = item.text_color().unwrap_or(if is_active {
            colors.text
        } else {
            colors.text_muted
        });
        let alpha = if is_active { 255 } else { 220 };
        let cx_center = (x0 + x1) * 0.5;
        let cy_center = y0 + tab_h * 0.5;
        if cfg.icons_available
            && let Some(icon) = item.icon()
        {
            let sz = calc_text_size(icon);
            draw.add_text(
                [cx_center - sz[0] * 0.5, cy_center - sz[1] * 0.5],
                c32(label_color, a(alpha)),
                icon,
            );
        } else {
            // Fallback: first character of the title (uppercased)
            let ch = item
                .title()
                .chars()
                .next()
                .map(|c| c.to_uppercase().next().unwrap_or(c))
                .unwrap_or('?');
            let mut buf = [0u8; 4];
            let s: &str = &*ch.encode_utf8(&mut buf);
            let sz = calc_text_size(s);
            draw.add_text(
                [cx_center - sz[0] * 0.5, cy_center - sz[1] * 0.5],
                c32(label_color, a(alpha)),
                s,
            );
        }
        // Tiny status dot in the bottom-right corner — preserves at-a-glance
        // signal. Honors the global toggle and per-tab opt-out (None / Active).
        let status = item.status();
        let pinned_dot_disabled =
            !cfg.show_status_dot || status == TabStatus::None || status == TabStatus::Active;
        if !pinned_dot_disabled {
            let dot_r = 2.5;
            let dx = x1 - dot_r * 2.0 - 2.0;
            let dy = y0 + tab_h - dot_r * 2.0 - 2.0;
            let col = item
                .dot_color()
                .unwrap_or_else(|| colors.status_color(status));
            draw.add_rect(
                [dx, dy],
                [dx + dot_r * 2.0, dy + dot_r * 2.0],
                c32(col, a(255)),
            )
            .rounding(dot_r)
            .filled(true)
            .build();
        }
        return;
    }

    // ── Regular tab ─────────────────────────────────────────────────────
    let mut text_x = x0 + cfg.tab_padding_h;

    // Status dot
    //   - skipped entirely when globally disabled or `TabStatus::None`
    //     (and the layout reserve is also skipped — title moves left)
    //   - skipped for `Dirty` (close-button slot shows the dirty indicator)
    //     but the reserve is kept so the layout doesn't jump when status flips
    let status = item.status();
    let dot_disabled = !cfg.show_status_dot || status == TabStatus::None;
    if !dot_disabled && status != TabStatus::Dirty {
        let status_col = item
            .dot_color()
            .unwrap_or_else(|| colors.status_color(status));
        let dot_d = STATUS_DOT_DIAM;
        let dot_x = text_x;
        let dot_y = y0 + (tab_h - dot_d) * 0.5;
        let dot_alpha = if matches!(status, TabStatus::Warning | TabStatus::Error) {
            ((((time * 3.0).sin() * 0.5) + 0.5) * 75.0 + 180.0) as u8
        } else {
            255
        };
        draw.add_rect(
            [dot_x, dot_y],
            [dot_x + dot_d, dot_y + dot_d],
            c32(status_col, a(dot_alpha)),
        )
        .rounding(dot_d * 0.5)
        .filled(true)
        .build();
    }
    if !dot_disabled {
        // Reserve the slot whether we drew or not (keeps Dirty stable too).
        text_x += STATUS_DOT_DIAM + STATUS_DOT_GAP;
    }

    // Icon — only when the icon font is registered
    if cfg.icons_available
        && let Some(icon) = item.icon()
    {
        let icon_sz = calc_text_size(icon);
        let iy = y0 + (tab_h - icon_sz[1]) * 0.5;
        let icon_alpha = if is_active { 255 } else { 230 };
        draw.add_text([text_x, iy], c32(colors.accent, a(icon_alpha)), icon);
        text_x += icon_sz[0] + ICON_TITLE_GAP;
    }

    // Title — per-tab override wins, then active/inactive default.
    let text_color = item.text_color().unwrap_or(if is_active {
        colors.text
    } else {
        colors.text_muted
    });
    let text_sz = calc_text_size(item.title());
    let text_y = y0 + (tab_h - text_sz[1]) * 0.5;
    draw.add_text([text_x, text_y], c32(text_color, a(255)), item.title());
    text_x += text_sz[0];

    // Badge pill: outer gap + inner_pad_x | text | inner_pad_x
    if let Some(badge) = item.badge() {
        let bsz = calc_text_size(&badge.text);
        let bx = text_x + BADGE_LEFT_GAP;
        let by = y0 + (tab_h - bsz[1] - BADGE_INNER_PAD_Y * 2.0) * 0.5;
        let bw = bsz[0] + BADGE_INNER_PAD_X * 2.0;
        let bh = bsz[1] + BADGE_INNER_PAD_Y * 2.0;
        draw.add_rect([bx, by], [bx + bw, by + bh], c32(badge.color, a(210)))
            .rounding(4.0)
            .filled(true)
            .build();
        draw.add_text(
            [bx + BADGE_INNER_PAD_X, by + BADGE_INNER_PAD_Y],
            c32(colors.text, a(255)),
            &badge.text,
        );
    }

    // Right-side button: close icon, or dirty indicator (replaced by close on hover)
    let can_close = cfg.closable && item.is_closable();
    let cx = x1 - cfg.tab_padding_h - cfg.close_btn_size;
    let cy_center = y0 + tab_h * 0.5;
    let close_x0 = cx;
    let close_y0 = cy_center - cfg.close_btn_size * 0.5;

    let show_dirty = status == TabStatus::Dirty && !hovered;

    if show_dirty {
        let r = cfg.close_btn_size * 0.30;
        let cx0 = close_x0 + cfg.close_btn_size * 0.5 - r;
        let cy0 = close_y0 + cfg.close_btn_size * 0.5 - r;
        draw.add_rect(
            [cx0, cy0],
            [cx0 + r * 2.0, cy0 + r * 2.0],
            c32(colors.status_dirty, a(230)),
        )
        .rounding(r)
        .filled(true)
        .build();
    } else if can_close || status == TabStatus::Dirty {
        let close_alpha = if close_hovered {
            255
        } else if hovered || is_active {
            180
        } else {
            0
        };
        if close_alpha > 0 {
            draw_close_glyph(
                draw,
                cfg,
                close_x0,
                close_y0,
                close_hovered,
                a(close_alpha),
                a(140),
            );
        }
    }
}

// ─── Close-glyph drawing (parametric) ──────────────────────────────────────

/// Draw the close button using `cfg.close_glyph`. All variants render
/// through the draw list (no font dependency).
fn draw_close_glyph(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    cfg: &TabControlConfig,
    x: f32,
    y: f32,
    hovered: bool,
    glyph_alpha: u8,
    bg_alpha: u8,
) {
    let colors = &cfg.colors;
    let s = cfg.close_btn_size;
    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    // Hover background — same for every glyph variant.
    if hovered && bg_alpha > 0 {
        let pad = CLOSE_HIT_PAD;
        let bg_col = c32(colors.close_hover, bg_alpha);
        let rect_min = [x - pad, y - pad];
        let rect_max = [x + s + pad, y + s + pad];
        match cfg.close_glyph {
            CloseGlyph::CircleX => {
                let r = (s * 0.5 + pad).max(2.0);
                draw.add_rect([cx - r, cy - r], [cx + r, cy + r], bg_col)
                    .rounding(r)
                    .filled(true)
                    .build();
            }
            _ => {
                draw.add_rect(rect_min, rect_max, bg_col)
                    .rounding(4.0)
                    .filled(true)
                    .build();
            }
        }
    }

    let glyph_col = c32(colors.text, glyph_alpha);
    let inset = s * 0.25;
    let (thickness, draw_outline) = match cfg.close_glyph {
        CloseGlyph::Cross => (1.4, false),
        CloseGlyph::CrossBold => (2.0, false),
        CloseGlyph::SquareX => (1.6, true),
        CloseGlyph::CircleX => (1.6, false),
    };

    // Outline (square or circle) — drawn behind the cross
    if draw_outline {
        // Thin square outline
        draw.add_rect(
            [x + 0.5, y + 0.5],
            [x + s - 0.5, y + s - 0.5],
            c32(colors.separator, glyph_alpha.saturating_sub(60)),
        )
        .rounding(2.5)
        .filled(false)
        .thickness(1.0)
        .build();
    } else if matches!(cfg.close_glyph, CloseGlyph::CircleX) {
        let r = s * 0.5 - 0.5;
        draw.add_rect(
            [cx - r, cy - r],
            [cx + r, cy + r],
            c32(colors.separator, glyph_alpha.saturating_sub(60)),
        )
        .rounding(r)
        .filled(false)
        .thickness(1.0)
        .build();
    }

    // Diagonal cross (always)
    draw.add_line(
        [x + inset, y + inset],
        [x + s - inset, y + s - inset],
        glyph_col,
    )
    .thickness(thickness)
    .build();
    draw.add_line(
        [x + s - inset, y + inset],
        [x + inset, y + s - inset],
        glyph_col,
    )
    .thickness(thickness)
    .build();
}

// ─── Close confirmation popup ───────────────────────────────────────────────

fn render_close_popup<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) {
    let strings = &pc.config.strings;

    if pc.pending_close_new {
        pc.pending_close_new = false;
        ui.open_popup(&pc.close_popup_id);
    }

    let mut should_clear = false;

    if let Some(_token) = ui
        .begin_modal_popup_config(&pc.close_popup_id)
        .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
        .begin()
    {
        let pending = pc
            .pending_close
            .and_then(|id| pc.tabs.iter().find(|t| t.id == id));
        let name = pending.map(|t| t.item.title()).unwrap_or("Unknown");
        let is_dirty = pending.is_some_and(|t| t.item.status() == TabStatus::Dirty);

        pc.fmt_buf.clear();
        let _ = write!(pc.fmt_buf, "{} {}", icons::ALERT, &strings.close);
        ui.text(&pc.fmt_buf);
        ui.spacing();
        ui.text_colored(rgba(pc.config.colors.text, 1.0), name);
        ui.spacing();
        let confirm_text: &str = if is_dirty {
            &strings.close_confirm_dirty
        } else {
            &strings.close_confirm
        };
        ui.text_colored(rgba(pc.config.colors.text_muted, 1.0), confirm_text);
        ui.spacing();
        ui.separator();
        ui.spacing();

        let btn_w = 120.0_f32;
        let spacing = ui.clone_style().item_spacing()[0];
        let total_w = btn_w * 2.0 + spacing;
        let avail_w = ui.content_region_avail()[0];
        let offset = ((avail_w - total_w) * 0.5).max(0.0);
        ui.set_cursor_pos([ui.cursor_pos()[0] + offset, ui.cursor_pos()[1]]);

        if danger_button(ui, &strings.close, [btn_w, 0.0]) {
            if let Some(id) = pc.pending_close.take() {
                if pc.config.animate_close {
                    pc.closing_tab = Some((id, 1.0));
                } else if let Some(t) = pc.tabs.iter_mut().find(|t| t.id == id) {
                    t.open = false;
                }
            }
            ui.close_current_popup();
        }

        ui.same_line();

        if ui.button_with_size(&strings.cancel, [btn_w, 0.0]) {
            should_clear = true;
            ui.close_current_popup();
        }
    }

    if should_clear {
        pc.pending_close = None;
    }
}
