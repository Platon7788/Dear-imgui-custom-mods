//! Rendering functions for PageControl.
//!
//! All rendering is implemented as `pub(crate)` free-standing functions
//! to follow the project convention (see `file_manager/render.rs`) and
//! avoid borrow conflicts.

use std::fmt::Write;

use dear_imgui_rs::{Key, MouseButton, StyleColor, Ui, WindowFlags};

use crate::icons;
use crate::utils::color::rgb_arr as c32;
use crate::utils::text::calc_text_size;

use super::config::*;
use super::{PageControl, PageItem};

// ─── Label color (used for muted UI text) ───────────────────────────────────

const LABEL_COLOR: [f32; 4] = [0.54, 0.57, 0.63, 1.0];

/// Test whether the mouse is over the close button area of a tab.
#[inline]
fn is_close_hovered(
    mouse: [f32; 2],
    x1: f32,
    y0: f32,
    cfg: &PageControlConfig,
    clip_min_x: f32,
    clip_max_x: f32,
) -> bool {
    let cx = x1 - cfg.tab_padding_h - cfg.close_btn_size;
    let cy_center = y0 + cfg.tab_height * 0.5;
    mouse[0] >= cx.max(clip_min_x)
        && mouse[0] < (cx + cfg.close_btn_size).min(clip_max_x)
        && mouse[1] >= cy_center - cfg.close_btn_size * 0.5
        && mouse[1] < cy_center + cfg.close_btn_size * 0.5
}

// ─── Main entry point ───────────────────────────────────────────────────────

pub(crate) fn render_page_control<T: PageItem>(
    pc: &mut PageControl<T>,
    ui: &Ui,
) -> Option<PageAction> {
    // Reset per-frame context menu state
    pc.open_context_menu = false;

    // ── Tick tab-close animation ──────────────────────────────────────────
    if let Some((closing_id, ref mut frac)) = pc.closing_tab {
        let dt = ui.io().delta_time();
        *frac -= dt / 0.15; // 150ms animation
        if *frac <= 0.0 {
            // Animation finished — mark page for removal
            if let Some(page) = pc.pages.iter_mut().find(|p| p.id == closing_id) {
                page.open = false;
            }
            pc.closing_tab = None;
        }
    }

    let mut action: Option<PageAction> = None;

    if pc.pages.is_empty() {
        render_empty_placeholder(ui, &pc.config);
    } else {
        match pc.view {
            ContentView::Dashboard => {
                action = render_dashboard(pc, ui);
            }
            ContentView::Tabs => {
                action = render_tabs(pc, ui);
            }
            ContentView::Custom(_) => {
                // Custom views are rendered externally by the caller.
                // The component does nothing — caller checks pc.view and
                // renders its own content.
            }
        }
    }

    render_close_popup(pc, ui);

    // Process deferred closes
    let mut closed_id: Option<PageId> = None;
    pc.pages.retain(|p| {
        if !p.open {
            closed_id = Some(p.id);
            false
        } else {
            true
        }
    });
    if let Some(id) = closed_id {
        if pc.active == Some(id) {
            pc.active = pc.pages.last().map(|p| p.id);
            // Notify new active page
            if let Some(new_id) = pc.active
                && let Some(entry) = pc.pages.iter_mut().find(|p| p.id == new_id)
            {
                entry.item.on_activated();
            }
        }
        pc.invalidate_tab_widths();
        action = Some(PageAction::Closed(id));
    }

    action
}

// ─── Empty placeholder ──────────────────────────────────────────────────────

fn render_empty_placeholder(ui: &Ui, config: &PageControlConfig) {
    let avail = ui.content_region_avail();
    let strings = &config.strings;

    let icon = icons::VIEW_DASHBOARD_OUTLINE;
    let text_main = strings.no_pages;
    let text_hint = strings.empty_hint;

    let icon_sz = calc_text_size(icon);
    let main_sz = calc_text_size(text_main);
    let hint_sz = calc_text_size(text_hint);

    let spacing = 8.0;
    let total_h = icon_sz[1] + spacing + main_sz[1] + spacing * 0.5 + hint_sz[1];
    let start_y = (avail[1] - total_h) * 0.5;

    let cursor_start = ui.cursor_pos();

    ui.set_cursor_pos([
        cursor_start[0] + (avail[0] - icon_sz[0]) * 0.5,
        cursor_start[1] + start_y,
    ]);
    ui.text_colored(LABEL_COLOR, icon);

    ui.set_cursor_pos([
        cursor_start[0] + (avail[0] - main_sz[0]) * 0.5,
        cursor_start[1] + start_y + icon_sz[1] + spacing,
    ]);
    ui.text_colored(LABEL_COLOR, text_main);

    ui.set_cursor_pos([
        cursor_start[0] + (avail[0] - hint_sz[0]) * 0.5,
        cursor_start[1] + start_y + icon_sz[1] + spacing + main_sz[1] + spacing * 0.5,
    ]);
    ui.text_colored([0.40, 0.42, 0.48, 1.0], text_hint);
}

// ─── Dashboard (tile grid) ──────────────────────────────────────────────────

fn render_dashboard<T: PageItem>(pc: &mut PageControl<T>, ui: &Ui) -> Option<PageAction> {
    let cfg = &pc.config;
    let colors = &cfg.colors;
    let tile_w = cfg.tile_width;
    let tile_h = cfg.tile_height();
    let tile_gap = cfg.tile_gap;
    let tile_r = cfg.tile_rounding;
    let tile_pad = cfg.tile_padding;
    let header_h = cfg.tile_header_height;

    let win_pos = ui.window_pos();
    let avail = ui.content_region_avail();
    let mouse = ui.io().mouse_pos();
    let clicked = ui.is_mouse_clicked(MouseButton::Left);
    let right_clicked = ui.is_mouse_clicked(MouseButton::Right);
    let accept_clicks = ui.is_window_hovered();

    // ── Dashboard header ────────────────────────────────────────────────
    if let Some(ref title) = cfg.dashboard_title {
        pc.fmt_buf.clear();
        let _ = write!(pc.fmt_buf, "{title}");
        if cfg.dashboard_show_count {
            let _ = write!(pc.fmt_buf, " ({})", pc.pages.len());
        }
        ui.text_colored(LABEL_COLOR, &pc.fmt_buf);
        ui.spacing();
        ui.separator();
        ui.spacing();
    }

    let cursor_start = ui.cursor_pos();
    let origin_x = win_pos[0] + cursor_start[0];
    let origin_y = win_pos[1] + cursor_start[1];

    let grid_w = avail[0];
    let cols = cfg
        .dashboard_columns
        .unwrap_or_else(|| ((grid_w + tile_gap) / (tile_w + tile_gap)).floor().max(1.0) as usize);

    let mut action: Option<PageAction> = None;

    let can_close_tiles = cfg.closable;

    // Collect body areas and visibility for two-phase rendering (reuse scratch buffer)
    let win_bottom = win_pos[1] + ui.window_size()[1];
    // Tuple: (idx, tx, ty, hovered, close_hovered, body_area, custom_tile, tile_area)
    pc.tile_scratch.clear();

    for (i, _page) in pc.pages.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let tx = origin_x + col as f32 * (tile_w + tile_gap);
        let ty = origin_y + row as f32 * (tile_h + tile_gap);

        // Visibility culling
        if ty + tile_h < win_pos[1] || ty > win_bottom {
            continue;
        }

        let hovered = accept_clicks
            && mouse[0] >= tx
            && mouse[0] < tx + tile_w
            && mouse[1] >= ty
            && mouse[1] < ty + tile_h;

        // Close button hit-test (top-right corner of tile header)
        let close_btn_sz = cfg.close_btn_size + 4.0; // slightly larger hit area
        let close_hovered = if hovered && can_close_tiles && pc.pages[i].item.is_closable() {
            let cbx = tx + tile_w - tile_pad - close_btn_sz;
            let cby = ty + tile_pad;
            mouse[0] >= cbx
                && mouse[0] < cbx + close_btn_sz
                && mouse[1] >= cby
                && mouse[1] < cby + close_btn_sz
        } else {
            false
        };

        let custom_tile = pc.pages[i].item.has_custom_tile();
        let sep_y = ty + header_h;
        let body_area = [
            tx + tile_pad,
            sep_y + 2.0,
            tile_w - tile_pad * 2.0,
            tile_h - header_h - 2.0 - tile_pad,
        ];
        let tile_area = [
            tx + tile_pad,
            ty + tile_pad,
            tile_w - tile_pad * 2.0,
            tile_h - tile_pad * 2.0,
        ];

        pc.tile_scratch.push((
            i,
            tx,
            ty,
            hovered,
            close_hovered,
            body_area,
            custom_tile,
            tile_area,
        ));
    }

    let time = ui.time() as f32;

    // Phase 1: Draw all tile frames (backgrounds, headers, separators)
    {
        let draw = ui.get_window_draw_list();
        for &(idx, tx, ty, hovered, close_hovered, _, custom_tile, _) in &pc.tile_scratch {
            let page = &pc.pages[idx];

            // Hover lift: shift tile up 1px when hovered
            let ty = if hovered { ty - 1.0 } else { ty };

            // Tile background
            let bg = if hovered {
                colors.tile_hover
            } else {
                colors.tile_bg
            };
            draw.add_rect([tx, ty], [tx + tile_w, ty + tile_h], c32(bg, 255))
                .rounding(tile_r)
                .filled(true)
                .build();

            // Hover border + subtle shadow
            if hovered {
                // Shadow (slightly offset, low alpha)
                draw.add_rect(
                    [tx + 1.0, ty + 2.0],
                    [tx + tile_w + 1.0, ty + tile_h + 2.0],
                    c32([0x00, 0x00, 0x00], 40),
                )
                .rounding(tile_r)
                .filled(true)
                .build();
                // Re-draw background over shadow
                draw.add_rect([tx, ty], [tx + tile_w, ty + tile_h], c32(bg, 255))
                    .rounding(tile_r)
                    .filled(true)
                    .build();
                // Accent border
                draw.add_rect(
                    [tx, ty],
                    [tx + tile_w, ty + tile_h],
                    c32(colors.accent, 180),
                )
                .rounding(tile_r)
                .filled(false)
                .thickness(1.5)
                .build();
            }

            // Close button (always rendered, regardless of custom_tile)
            if can_close_tiles && page.item.is_closable() {
                let close_btn_sz = cfg.close_btn_size + 4.0;
                let cbx = tx + tile_w - tile_pad - close_btn_sz;
                let cby = ty + tile_pad;

                if close_hovered {
                    draw.add_rect(
                        [cbx, cby],
                        [cbx + close_btn_sz, cby + close_btn_sz],
                        c32(colors.close_hover, 120),
                    )
                    .rounding(4.0)
                    .filled(true)
                    .build();
                }

                let close_icon = icons::CLOSE;
                let csz = calc_text_size(close_icon);
                // Hide close button on non-hovered tiles
                let close_alpha = if close_hovered {
                    255
                } else if hovered {
                    150
                } else {
                    0
                };
                if close_alpha > 0 {
                    draw.add_text(
                        [
                            cbx + (close_btn_sz - csz[0]) * 0.5,
                            cby + (close_btn_sz - csz[1]) * 0.5,
                        ],
                        c32(colors.text, close_alpha),
                        close_icon,
                    );
                }
            }

            // Standard header + separator (only for non-custom tiles)
            if !custom_tile {
                // Status indicator dot — pulse for Warning/Error
                let status = page.item.status();
                let status_col = colors.status_color(status);
                let dot_x = tx + tile_pad;
                let dot_y = ty + tile_pad + 3.0;
                let dot_r = 4.0;
                let dot_alpha = if status == PageStatus::Warning || status == PageStatus::Error {
                    // Gentle pulse: alpha oscillates 180..255
                    let pulse = ((time * 3.0).sin() * 0.5 + 0.5) * 75.0 + 180.0;
                    pulse as u8
                } else {
                    255
                };
                draw.add_rect(
                    [dot_x, dot_y],
                    [dot_x + dot_r * 2.0, dot_y + dot_r * 2.0],
                    c32(status_col, dot_alpha),
                )
                .rounding(dot_r)
                .filled(true)
                .build();

                // Icon + Title
                let mut text_x = tx + tile_pad + 14.0;
                let text_y = ty + tile_pad;

                if let Some(icon) = page.item.icon() {
                    draw.add_text([text_x, text_y], c32(colors.accent, 220), icon);
                    text_x += calc_text_size(icon)[0] + 4.0;
                }

                draw.add_text([text_x, text_y], c32(colors.text, 255), page.item.title());

                // Subtitle (below title)
                if let Some(sub) = page.item.subtitle() {
                    let sub_y = text_y + calc_text_size(page.item.title())[1] + 2.0;
                    let mut line_y = sub_y;
                    for line in sub.split('\n') {
                        let line_h = calc_text_size(line)[1];
                        draw.add_text([tx + tile_pad, line_y], c32(colors.text_muted, 200), line);
                        line_y += line_h + 1.0;
                    }
                }

                // Separator between header and body
                let sep_y = ty + header_h;
                draw.add_line(
                    [tx + 6.0, sep_y],
                    [tx + tile_w - 6.0, sep_y],
                    c32(colors.separator, 120),
                )
                .build();
            }
        }
    } // draw is dropped here

    // Phase 2: Render tile bodies / custom tiles
    let mut tile_body_action: Option<(PageId, u64)> = None;
    for &(idx, _, _, _, _, body_area, custom_tile, tile_area) in &pc.tile_scratch {
        let page = &pc.pages[idx];
        if custom_tile {
            if let Some(act) = page.item.render_tile(ui, tile_area) {
                tile_body_action = Some((page.id, act));
            }
        } else if let Some(act) = page.item.render_tile_body(ui, body_area) {
            tile_body_action = Some((page.id, act));
        }
    }

    // Phase 3: Click handling
    for &(idx, _, _, hovered, close_hovered, _, _, _) in &pc.tile_scratch {
        let page_id = pc.pages[idx].id;
        if hovered {
            if clicked {
                if close_hovered {
                    // Close button clicked on tile
                    if cfg.confirm_close {
                        pc.pending_close = Some(page_id);
                        pc.pending_close_new = true;
                    } else if let Some(page) = pc.pages.iter_mut().find(|p| p.id == page_id) {
                        page.open = false;
                    }
                } else {
                    action = Some(PageAction::TileClicked(page_id));
                }
            }
            // Context menu (right-click)
            if right_clicked && cfg.context_menu {
                pc.context_page = Some(page_id);
                pc.open_context_menu = true;
            }
        }
    }

    // ── Dashboard "+" add tile ──────────────────────────────────────────
    if cfg.show_add_tile {
        let add_i = pc.pages.len();
        let add_col = add_i % cols;
        let add_row = add_i / cols;
        let atx = origin_x + add_col as f32 * (tile_w + tile_gap);
        let aty = origin_y + add_row as f32 * (tile_h + tile_gap);

        if aty + tile_h >= win_pos[1] && aty <= win_bottom {
            let add_hovered = accept_clicks
                && mouse[0] >= atx
                && mouse[0] < atx + tile_w
                && mouse[1] >= aty
                && mouse[1] < aty + tile_h;

            let draw = ui.get_window_draw_list();
            let bg = if add_hovered {
                colors.tile_hover
            } else {
                colors.tile_bg
            };
            draw.add_rect([atx, aty], [atx + tile_w, aty + tile_h], c32(bg, 180))
                .rounding(tile_r)
                .filled(true)
                .build();

            // Dashed border
            draw.add_rect(
                [atx, aty],
                [atx + tile_w, aty + tile_h],
                c32(colors.separator, if add_hovered { 200 } else { 120 }),
            )
            .rounding(tile_r)
            .filled(false)
            .thickness(1.0)
            .build();

            // "+" icon centered
            let plus = icons::PLUS;
            let psz = calc_text_size(plus);
            draw.add_text(
                [atx + (tile_w - psz[0]) * 0.5, aty + (tile_h - psz[1]) * 0.5],
                c32(colors.text_muted, if add_hovered { 255 } else { 150 }),
                plus,
            );

            if add_hovered && clicked {
                action = Some(PageAction::AddRequested);
            }
        }
    }

    // Tile body actions take priority over tile click (more specific)
    if let Some((pid, act)) = tile_body_action {
        action = Some(PageAction::TileBodyAction(pid, act));
    }

    // Reserve space for the grid so scrolling works
    let total_items = if cfg.show_add_tile {
        pc.pages.len() + 1
    } else {
        pc.pages.len()
    };
    let total_rows = total_items.div_ceil(cols);
    let total_h = total_rows as f32 * (tile_h + tile_gap);
    ui.set_cursor_pos([cursor_start[0], cursor_start[1] + total_h]);
    ui.dummy([0.0, 0.0]);

    action
}

// ─── Tab strip + content ────────────────────────────────────────────────────

fn render_tabs<T: PageItem>(pc: &mut PageControl<T>, ui: &Ui) -> Option<PageAction> {
    let mut action: Option<PageAction> = None;

    // Ensure tab width cache is up to date
    pc.ensure_tab_widths();

    let cfg = &pc.config;
    let colors = &cfg.colors;
    let strip_h = cfg.strip_height();

    let draw = ui.get_window_draw_list();
    let win_pos = ui.window_pos();
    let win_content_min = ui.cursor_start_pos();
    let avail_w = ui.content_region_avail()[0];

    let strip_x = win_pos[0] + win_content_min[0];
    let strip_y = win_pos[1] + ui.cursor_pos()[1];

    let tab_widths = &pc.tab_widths_cache;

    // Account for "+" button width when computing layout
    let add_btn_w = if cfg.show_add_button {
        cfg.scroll_btn_width
    } else {
        0.0
    };
    let view_toggle_w = if cfg.show_view_toggle {
        cfg.scroll_btn_width
    } else {
        0.0
    };
    // Reserve space for overflow dropdown when it will be shown
    let overflow_reserve = if cfg.show_overflow_dropdown {
        cfg.scroll_btn_width
    } else {
        0.0
    };
    let effective_avail_w = avail_w - add_btn_w - view_toggle_w - overflow_reserve;

    let total_tabs_w: f32 =
        tab_widths.iter().sum::<f32>() + cfg.tab_gap * (tab_widths.len() as f32 - 1.0).max(0.0);
    let needs_scroll = total_tabs_w > effective_avail_w;

    let scroll_area_w = if needs_scroll {
        effective_avail_w - cfg.scroll_btn_width * 2.0
    } else {
        effective_avail_w
    };

    let mouse = ui.io().mouse_pos();
    let clicked = ui.is_mouse_clicked(MouseButton::Left);
    let middle_clicked = ui.is_mouse_clicked(MouseButton::Middle);
    let right_clicked = ui.is_mouse_clicked(MouseButton::Right);
    let accept_clicks = ui.is_window_hovered();

    // ── Strip background ────────────────────────────────────────────────
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
        c32(colors.separator, 180),
    )
    .build();

    // ── Scroll buttons ──────────────────────────────────────────────────
    let tabs_origin_x;
    if needs_scroll {
        tabs_origin_x = strip_x + cfg.scroll_btn_width;
        render_scroll_buttons(
            &draw,
            ui,
            strip_x,
            strip_y,
            effective_avail_w,
            strip_h,
            &mut pc.scroll_target,
            total_tabs_w,
            scroll_area_w,
            accept_clicks,
            mouse,
            cfg,
        );
    } else {
        pc.scroll_offset = 0.0;
        pc.scroll_target = 0.0;
        tabs_origin_x = strip_x;
    }

    // ── Scroll wheel on tab strip ───────────────────────────────────────
    if needs_scroll && cfg.scroll_with_wheel && accept_clicks {
        let in_strip = mouse[1] >= strip_y && mouse[1] < strip_y + strip_h;
        if in_strip {
            let wheel = ui.io().mouse_wheel();
            if wheel != 0.0 {
                pc.scroll_target -= wheel * cfg.scroll_speed * 0.5;
                let max_scroll = (total_tabs_w - scroll_area_w).max(0.0);
                pc.scroll_target = pc.scroll_target.clamp(0.0, max_scroll);
            }
        }
    }

    // ── "+" add button ──────────────────────────────────────────────────
    if cfg.show_add_button {
        let add_x = strip_x + avail_w - add_btn_w;
        let add_y0 = strip_y + cfg.strip_padding_v;
        let add_y1 = add_y0 + cfg.tab_height;

        let add_hovered = accept_clicks
            && mouse[0] >= add_x
            && mouse[0] < add_x + add_btn_w
            && mouse[1] >= add_y0
            && mouse[1] < add_y1;

        let add_bg = if add_hovered {
            colors.tab_hover
        } else {
            colors.strip_bg
        };
        draw.add_rect(
            [add_x, add_y0],
            [add_x + add_btn_w, add_y1],
            c32(add_bg, 255),
        )
        .rounding(cfg.tab_rounding)
        .filled(true)
        .build();

        let plus = icons::PLUS;
        let psz = calc_text_size(plus);
        draw.add_text(
            [
                add_x + (add_btn_w - psz[0]) * 0.5,
                add_y0 + (cfg.tab_height - psz[1]) * 0.5,
            ],
            c32(colors.text, if add_hovered { 255 } else { 150 }),
            plus,
        );

        if clicked && add_hovered {
            action = Some(PageAction::AddRequested);
        }
    }

    // ── Overflow dropdown ───────────────────────────────────────────────
    if needs_scroll && cfg.show_overflow_dropdown {
        let overflow_w = cfg.scroll_btn_width;
        let overflow_x = strip_x + avail_w
            - add_btn_w
            - overflow_w
            - (if cfg.show_view_toggle {
                cfg.scroll_btn_width
            } else {
                0.0
            });
        let ov_y0 = strip_y + cfg.strip_padding_v;
        let ov_y1 = ov_y0 + cfg.tab_height;

        let ov_hovered = accept_clicks
            && mouse[0] >= overflow_x
            && mouse[0] < overflow_x + overflow_w
            && mouse[1] >= ov_y0
            && mouse[1] < ov_y1;

        let ov_bg = if ov_hovered {
            colors.tab_hover
        } else {
            colors.strip_bg
        };
        draw.add_rect(
            [overflow_x, ov_y0],
            [overflow_x + overflow_w, ov_y1],
            c32(ov_bg, 255),
        )
        .rounding(cfg.tab_rounding)
        .filled(true)
        .build();

        let dots = icons::DOTS_HORIZONTAL;
        let dsz = calc_text_size(dots);
        draw.add_text(
            [
                overflow_x + (overflow_w - dsz[0]) * 0.5,
                ov_y0 + (cfg.tab_height - dsz[1]) * 0.5,
            ],
            c32(colors.text, if ov_hovered { 255 } else { 150 }),
            dots,
        );

        if ov_hovered && !clicked && !right_clicked {
            ui.tooltip_text(cfg.strings.overflow_tooltip);
        }

        if clicked && ov_hovered {
            ui.open_popup("##pc_overflow");
        }
    }

    // Overflow popup (rendered outside clip region)
    if let Some(_token) = ui.begin_popup("##pc_overflow") {
        let mut focus_id: Option<PageId> = None;
        for i in 0..pc.pages.len() {
            let page = &pc.pages[i];
            let page_id = page.id;
            let is_active = pc.active == Some(page_id);
            pc.fmt_buf.clear();
            if let Some(icon) = page.item.icon() {
                let _ = write!(pc.fmt_buf, "{} ", icon);
            }
            let _ = write!(pc.fmt_buf, "{}", page.item.title());
            if ui
                .selectable_config(&pc.fmt_buf)
                .selected(is_active)
                .build()
                && !is_active
            {
                focus_id = Some(page_id);
            }
        }
        if let Some(id) = focus_id
            && let Some(entry) = pc.pages.iter_mut().find(|p| p.id == id)
        {
            entry.request_focus = true;
        }
    }

    // ── View toggle button ──────────────────────────────────────────────
    if cfg.show_view_toggle {
        let vt_w = cfg.scroll_btn_width;
        let vt_x = strip_x + avail_w - add_btn_w - vt_w;
        let vt_y0 = strip_y + cfg.strip_padding_v;
        let vt_y1 = vt_y0 + cfg.tab_height;

        let vt_hovered = accept_clicks
            && mouse[0] >= vt_x
            && mouse[0] < vt_x + vt_w
            && mouse[1] >= vt_y0
            && mouse[1] < vt_y1;

        let vt_bg = if vt_hovered {
            colors.tab_hover
        } else {
            colors.strip_bg
        };
        draw.add_rect([vt_x, vt_y0], [vt_x + vt_w, vt_y1], c32(vt_bg, 255))
            .rounding(cfg.tab_rounding)
            .filled(true)
            .build();

        let vt_icon = icons::VIEW_DASHBOARD_OUTLINE;
        let vtsz = calc_text_size(vt_icon);
        draw.add_text(
            [
                vt_x + (vt_w - vtsz[0]) * 0.5,
                vt_y0 + (cfg.tab_height - vtsz[1]) * 0.5,
            ],
            c32(colors.text, if vt_hovered { 255 } else { 150 }),
            vt_icon,
        );

        if vt_hovered && !clicked {
            ui.tooltip_text(cfg.strings.view_dashboard);
        }

        if clicked && vt_hovered {
            action = Some(PageAction::ViewToggled);
        }
    }

    // ── Handle request_focus ────────────────────────────────────────────
    for (i, page) in pc.pages.iter_mut().enumerate() {
        if page.request_focus {
            page.request_focus = false;
            pc.active = Some(page.id);
            page.item.on_activated();
            action = Some(PageAction::Activated(page.id));

            if let Some(&tw) = tab_widths.get(i) {
                let mut tx: f32 = 0.0;
                for w in tab_widths.iter().take(i) {
                    tx += w + cfg.tab_gap;
                }
                if tx < pc.scroll_target {
                    pc.scroll_target = tx;
                } else if tx + tw > pc.scroll_target + scroll_area_w {
                    pc.scroll_target = tx + tw - scroll_area_w;
                }
            }
        }
    }

    // ── Smooth scroll interpolation ─────────────────────────────────────
    if cfg.smooth_scroll {
        let dt = ui.io().delta_time();
        let diff = pc.scroll_target - pc.scroll_offset;
        if diff.abs() < 0.5 {
            pc.scroll_offset = pc.scroll_target;
        } else {
            pc.scroll_offset += diff * (1.0 - (-12.0 * dt).exp());
        }
    } else {
        pc.scroll_offset = pc.scroll_target;
    }

    // ── Draw tabs (clipped) ─────────────────────────────────────────────
    let clip_min = [tabs_origin_x, strip_y];
    let clip_max = [tabs_origin_x + scroll_area_w, strip_y + strip_h + 1.0];

    draw.with_clip_rect(clip_min, clip_max, || {
        let mut tx = tabs_origin_x - pc.scroll_offset;
        for (i, page) in pc.pages.iter().enumerate() {
            let Some(&base_tw) = tab_widths.get(i) else {
                break;
            };
            // Animated close: shrink tab width during closing animation
            let tw = if let Some((closing_id, frac)) = pc.closing_tab {
                if page.id == closing_id {
                    base_tw * frac.max(0.0)
                } else {
                    base_tw
                }
            } else {
                base_tw
            };
            if tw < 1.0 && pc.closing_tab.is_some_and(|(cid, _)| cid == page.id) {
                tx += cfg.tab_gap;
                continue; // too small to render
            }
            let is_active = pc.active == Some(page.id);
            let x0 = tx;
            let y0 = strip_y + cfg.strip_padding_v;
            let x1 = tx + tw;
            let y1 = y0 + cfg.tab_height;

            let tab_hovered = accept_clicks
                && mouse[0] >= x0.max(clip_min[0])
                && mouse[0] < x1.min(clip_max[0])
                && mouse[1] >= y0
                && mouse[1] < y1;

            // Compute close hover for visual feedback
            let can_close = cfg.closable && page.item.is_closable();
            let close_hovered = can_close
                && tab_hovered
                && is_close_hovered(mouse, x1, y0, cfg, clip_min[0], clip_max[0]);

            render_single_tab(
                &draw,
                &page.item,
                is_active,
                tab_hovered,
                close_hovered,
                x0,
                y0,
                x1,
                y1,
                cfg,
                ui.time() as f32,
            );

            tx += tw + cfg.tab_gap;
        }
    });

    // ── Click processing (separate pass — index-based to avoid borrow conflicts) ──
    {
        let mut close_target: Option<PageId> = None;
        let mut activate_target: Option<PageId> = None;
        let mut context_target: Option<PageId> = None;
        let mut double_click_target: Option<PageId> = None;
        let mut tooltip_idx: Option<usize> = None;

        let mut tx = tabs_origin_x - pc.scroll_offset;
        for i in 0..pc.pages.len() {
            let Some(&base_tw) = tab_widths.get(i) else {
                break;
            };
            let tw = if let Some((closing_id, frac)) = pc.closing_tab {
                if pc.pages[i].id == closing_id {
                    base_tw * frac.max(0.0)
                } else {
                    base_tw
                }
            } else {
                base_tw
            };
            let x0 = tx;
            let y0 = strip_y + cfg.strip_padding_v;
            let x1 = tx + tw;

            let hovered = accept_clicks
                && mouse[0] >= x0.max(clip_min[0])
                && mouse[0] < x1.min(clip_max[0])
                && mouse[1] >= y0
                && mouse[1] < y0 + cfg.tab_height;

            if hovered {
                let page = &pc.pages[i];
                let can_close = cfg.closable && page.item.is_closable();

                let close_hit =
                    can_close && is_close_hovered(mouse, x1, y0, cfg, clip_min[0], clip_max[0]);

                if clicked {
                    if close_hit {
                        close_target = Some(page.id);
                    } else {
                        // Double-click detection
                        let now = ui.time();
                        let dbl_threshold = 0.35; // seconds
                        if pc.last_click_tab == Some(page.id)
                            && (now - pc.last_click_time) < dbl_threshold
                        {
                            double_click_target = Some(page.id);
                            pc.last_click_tab = None;
                        } else {
                            pc.last_click_time = now;
                            pc.last_click_tab = Some(page.id);
                        }

                        // Start potential drag
                        pc.drag_source_idx = Some(i);
                        pc.drag_start_x = mouse[0];
                        pc.dragging = false;
                        if pc.active != Some(page.id) {
                            activate_target = Some(page.id);
                        }
                    }
                }

                if middle_clicked && cfg.middle_click_close && can_close {
                    close_target = Some(page.id);
                }

                if right_clicked && cfg.context_menu {
                    context_target = Some(page.id);
                }

                if !clicked && !middle_clicked && !right_clicked && page.item.tooltip().is_some() {
                    tooltip_idx = Some(i);
                }
            }

            tx += tw + cfg.tab_gap;
        }

        // Apply collected actions
        if let Some(id) = close_target {
            if cfg.confirm_close {
                pc.pending_close = Some(id);
                pc.pending_close_new = true;
            } else {
                pc.closing_tab = Some((id, 1.0));
            }
        }

        if let Some(new_id) = activate_target {
            if let Some(old_id) = pc.active
                && let Some(old) = pc.pages.iter_mut().find(|p| p.id == old_id)
            {
                old.item.on_deactivated();
            }
            pc.active = Some(new_id);
            if let Some(entry) = pc.pages.iter_mut().find(|p| p.id == new_id) {
                entry.item.on_activated();
            }
            action = Some(PageAction::Activated(new_id));
        }

        if let Some(id) = context_target {
            pc.context_page = Some(id);
            pc.open_context_menu = true;
        }

        if let Some(id) = double_click_target {
            action = Some(PageAction::DoubleClicked(id));
        }

        if let Some(idx) = tooltip_idx
            && let Some(tip) = pc.pages[idx].item.tooltip()
        {
            ui.tooltip_text(tip);
        }
    }

    // ── Drag-and-drop reorder ───────────────────────────────────────────
    if let Some(src_idx) = pc.drag_source_idx {
        let mouse_held = ui.is_mouse_down(MouseButton::Left);
        if !mouse_held {
            // Released — end drag
            pc.drag_source_idx = None;
            pc.dragging = false;
        } else {
            let dx = (mouse[0] - pc.drag_start_x).abs();
            // Threshold: 5px before starting actual drag
            if dx > 5.0 {
                pc.dragging = true;
            }

            if pc.dragging {
                // Find which tab index the mouse is currently over
                let mut tx2 = tabs_origin_x - pc.scroll_offset;
                let mut target_idx: Option<usize> = None;
                for j in 0..pc.pages.len() {
                    let Some(&tw2) = tab_widths.get(j) else { break };
                    let mid = tx2 + tw2 * 0.5;
                    if mouse[0] < mid {
                        target_idx = Some(j);
                        break;
                    }
                    tx2 += tw2 + cfg.tab_gap;
                }
                let target = target_idx.unwrap_or(pc.pages.len().saturating_sub(1));

                if target != src_idx && target < pc.pages.len() {
                    // Swap adjacent steps towards target
                    if target < src_idx {
                        pc.pages.swap(src_idx, src_idx - 1);
                        pc.drag_source_idx = Some(src_idx - 1);
                    } else {
                        pc.pages.swap(src_idx, src_idx + 1);
                        pc.drag_source_idx = Some(src_idx + 1);
                    }
                    pc.tab_gen += 1; // invalidate tab widths (can't call method due to cfg borrow)
                    let moved_id = pc.pages[if target < src_idx {
                        src_idx - 1
                    } else {
                        src_idx + 1
                    }]
                    .id;
                    action = Some(PageAction::Reordered(moved_id));
                }

                // Show drag cursor indicator line
                let drag_y = strip_y + cfg.strip_padding_v;
                draw.add_line(
                    [mouse[0], drag_y],
                    [mouse[0], drag_y + cfg.tab_height],
                    c32([0x60, 0x90, 0xFF], 200),
                )
                .thickness(2.0)
                .build();

                // Ghost tab: semi-transparent copy at mouse position
                if let Some(&tw) = tab_widths.get(src_idx) {
                    let ghost_x0 = mouse[0] - tw * 0.5;
                    let ghost_y0 = drag_y;
                    let ghost_x1 = ghost_x0 + tw;
                    let ghost_y1 = ghost_y0 + cfg.tab_height;
                    let ghost_alpha = 0.45_f32;
                    // Background pill
                    let accent = pc.pages[src_idx].item.tab_color().unwrap_or_else(|| {
                        cfg.colors.status_color(pc.pages[src_idx].item.status())
                    });
                    let bg = c32(
                        [
                            accent[0].saturating_add(40),
                            accent[1].saturating_add(40),
                            accent[2].saturating_add(40),
                        ],
                        (220.0 * ghost_alpha) as u8,
                    );
                    draw.add_rect([ghost_x0, ghost_y0], [ghost_x1, ghost_y1], bg)
                        .filled(true)
                        .rounding(cfg.tab_rounding)
                        .build();
                    // Ghost title (centered)
                    let title = pc.pages[src_idx].item.title();
                    let ts = calc_text_size(title);
                    let tx = ghost_x0 + (tw - ts[0]) * 0.5;
                    let ty = ghost_y0 + (cfg.tab_height - ts[1]) * 0.5;
                    let fg = c32([0xFF, 0xFF, 0xFF], (230.0 * ghost_alpha) as u8);
                    draw.add_text([tx, ty], fg, title);
                }
            }
        }
    }

    // ── Keyboard navigation ─────────────────────────────────────────────
    if cfg.keyboard_nav && accept_clicks && !pc.pages.is_empty() {
        let prev = ui.is_key_pressed(Key::LeftArrow);
        let next = ui.is_key_pressed(Key::RightArrow);
        let ctrl_w = ui.is_key_pressed(Key::W) && ui.io().key_ctrl();

        if prev || next {
            if let Some(active_id) = pc.active {
                if let Some(idx) = pc.pages.iter().position(|p| p.id == active_id) {
                    let new_idx = if prev {
                        idx.saturating_sub(1)
                    } else {
                        (idx + 1).min(pc.pages.len() - 1)
                    };
                    if new_idx != idx {
                        pc.pages[idx].item.on_deactivated();
                        let new_id = pc.pages[new_idx].id;
                        pc.active = Some(new_id);
                        pc.pages[new_idx].item.on_activated();
                        action = Some(PageAction::Activated(new_id));
                    }
                }
            } else {
                let id = pc.pages[0].id;
                pc.active = Some(id);
                pc.pages[0].item.on_activated();
                action = Some(PageAction::Activated(id));
            }
        }

        if ctrl_w && let Some(active_id) = pc.active {
            let can_close = pc
                .pages
                .iter()
                .find(|p| p.id == active_id)
                .is_some_and(|p| cfg.closable && p.item.is_closable());
            if can_close {
                if cfg.confirm_close {
                    pc.pending_close = Some(active_id);
                    pc.pending_close_new = true;
                } else {
                    pc.closing_tab = Some((active_id, 1.0));
                }
            }
        }
    }

    drop(draw);

    // ── Advance cursor past the tab strip ───────────────────────────────
    let content_start_y = ui.cursor_pos()[1] + strip_h + 2.0;
    ui.set_cursor_pos([ui.cursor_start_pos()[0], content_start_y]);
    // Submit a zero-size item so ImGui grows the parent bounds.
    // Without this, SetCursorPos alone triggers an assertion when
    // external_content is true or no active page exists.
    ui.dummy([0.0, 0.0]);

    // ── Content area: render active page ────────────────────────────────
    if !cfg.external_content
        && let Some(active_id) = pc.active
        && let Some(entry) = pc.pages.iter_mut().find(|p| p.id == active_id)
    {
        entry.item.render_content(ui);
    }

    action
}

// ─── Single tab dispatch ────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_single_tab<T: PageItem>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    item: &T,
    is_active: bool,
    hovered: bool,
    close_hovered: bool,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    cfg: &PageControlConfig,
    time: f32,
) {
    let colors = &cfg.colors;
    let tab_accent = item
        .tab_color()
        .unwrap_or_else(|| colors.status_color(item.status()));

    match cfg.tab_style {
        TabStyle::Pill => render_tab_pill(
            draw,
            item,
            is_active,
            hovered,
            close_hovered,
            x0,
            y0,
            x1,
            y1,
            cfg,
            tab_accent,
            time,
        ),
        TabStyle::Underline => render_tab_underline(
            draw,
            item,
            is_active,
            hovered,
            close_hovered,
            x0,
            y0,
            x1,
            y1,
            cfg,
            tab_accent,
            time,
        ),
        TabStyle::Card => render_tab_card(
            draw,
            item,
            is_active,
            hovered,
            close_hovered,
            x0,
            y0,
            x1,
            y1,
            cfg,
            tab_accent,
            time,
        ),
        TabStyle::Square => render_tab_square(
            draw,
            item,
            is_active,
            hovered,
            close_hovered,
            x0,
            y0,
            x1,
            y1,
            cfg,
            tab_accent,
            time,
        ),
    }
}

// ─── Tab content (shared by all styles) ─────────────────────────────────────

/// Draw the inner content of a tab: status dot, icon, title, badge, close button.
#[allow(clippy::too_many_arguments)]
fn render_tab_content<T: PageItem>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    item: &T,
    is_active: bool,
    hovered: bool,
    close_hovered: bool,
    x0: f32,
    y0: f32,
    x1: f32,
    cfg: &PageControlConfig,
    time: f32,
) {
    let colors = &cfg.colors;
    let tab_h = cfg.tab_height;
    let mut text_x = x0 + cfg.tab_padding_h;

    // Status dot — pulse for Warning/Error
    let status = item.status();
    let status_col = colors.status_color(status);
    let dot_r = 3.0;
    let dot_x = text_x;
    let dot_y = y0 + (tab_h - dot_r * 2.0) * 0.5;
    let dot_alpha = if status == PageStatus::Warning || status == PageStatus::Error {
        let pulse = ((time * 3.0).sin() * 0.5 + 0.5) * 75.0 + 180.0;
        pulse as u8
    } else {
        255
    };
    draw.add_rect(
        [dot_x, dot_y],
        [dot_x + dot_r * 2.0, dot_y + dot_r * 2.0],
        c32(status_col, dot_alpha),
    )
    .rounding(dot_r)
    .filled(true)
    .build();
    text_x += dot_r * 2.0 + 4.0;

    // Icon
    if let Some(icon) = item.icon() {
        let icon_sz = calc_text_size(icon);
        let iy = y0 + (tab_h - icon_sz[1]) * 0.5;
        draw.add_text([text_x, iy], c32(colors.accent, 220), icon);
        text_x += icon_sz[0] + 4.0;
    }

    // Title
    let text_color = if is_active {
        colors.text
    } else {
        colors.text_muted
    };
    let text_sz = calc_text_size(item.title());
    let text_y = y0 + (tab_h - text_sz[1]) * 0.5;
    draw.add_text([text_x, text_y], c32(text_color, 255), item.title());
    text_x += text_sz[0];

    // Badge
    if let Some(badge) = item.badge() {
        let badge_sz = calc_text_size(&badge.text);
        let bx = text_x + 4.0;
        let by = y0 + (tab_h - badge_sz[1] - 2.0) * 0.5;
        draw.add_rect(
            [bx, by],
            [bx + badge_sz[0] + 6.0, by + badge_sz[1] + 2.0],
            c32(badge.color, 200),
        )
        .rounding(4.0)
        .filled(true)
        .build();
        draw.add_text([bx + 3.0, by + 1.0], c32(colors.text, 255), &badge.text);
    }

    // Close button
    let can_close = cfg.closable && item.is_closable();
    if can_close {
        let cx = x1 - cfg.tab_padding_h - cfg.close_btn_size;
        let cy_center = y0 + tab_h * 0.5;
        let close_x0 = cx;
        let close_y0 = cy_center - cfg.close_btn_size * 0.5;

        if close_hovered {
            let pad = 2.0;
            draw.add_rect(
                [close_x0 - pad, close_y0 - pad],
                [
                    close_x0 + cfg.close_btn_size + pad,
                    close_y0 + cfg.close_btn_size + pad,
                ],
                c32(colors.close_hover, 120),
            )
            .rounding(4.0)
            .filled(true)
            .build();
        }

        let close_icon = icons::CLOSE;
        let csz = calc_text_size(close_icon);
        // Hide close button on non-hovered inactive tabs (VS Code style)
        let close_alpha = if close_hovered {
            255
        } else if hovered || is_active {
            150
        } else {
            0
        };
        if close_alpha > 0 {
            draw.add_text(
                [
                    close_x0 + (cfg.close_btn_size - csz[0]) * 0.5,
                    close_y0 + (cfg.close_btn_size - csz[1]) * 0.5,
                ],
                c32(colors.text, close_alpha),
                close_icon,
            );
        }
    }
}

// ─── Pill style (default) ───────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_tab_pill<T: PageItem>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    item: &T,
    is_active: bool,
    hovered: bool,
    close_hovered: bool,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    cfg: &PageControlConfig,
    tab_accent: [u8; 3],
    time: f32,
) {
    let colors = &cfg.colors;

    // Active tab glow (subtle shadow beneath)
    if is_active {
        draw.add_rect(
            [x0 + 1.0, y0 + 1.0],
            [x1 + 1.0, y1 + 1.0],
            c32(tab_accent, 30),
        )
        .rounding(cfg.tab_rounding)
        .filled(true)
        .build();
    }

    // Fully rounded background
    let bg = if is_active {
        colors.tab_active
    } else if hovered {
        colors.tab_hover
    } else {
        colors.tab_bg
    };
    draw.add_rect([x0, y0], [x1, y1], c32(bg, 255))
        .rounding(cfg.tab_rounding)
        .filled(true)
        .build();

    if is_active {
        let alpha = if item.status() == PageStatus::Active {
            200
        } else {
            120
        };
        draw.add_rect([x0, y0], [x1, y1], c32(tab_accent, alpha))
            .rounding(cfg.tab_rounding)
            .filled(false)
            .thickness(1.5)
            .build();

        if cfg.show_tab_underline {
            draw.add_rect([x0 + 4.0, y1 - 2.0], [x1 - 4.0, y1], c32(tab_accent, 255))
                .rounding(1.0)
                .filled(true)
                .build();
        }
    }

    render_tab_content(
        draw,
        item,
        is_active,
        hovered,
        close_hovered,
        x0,
        y0,
        x1,
        cfg,
        time,
    );
}

// ─── Underline style (Material Design) ─────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_tab_underline<T: PageItem>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    item: &T,
    is_active: bool,
    hovered: bool,
    close_hovered: bool,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    cfg: &PageControlConfig,
    tab_accent: [u8; 3],
    time: f32,
) {
    let colors = &cfg.colors;

    // Subtle hover background (no rounding — flat)
    if hovered && !is_active {
        draw.add_rect([x0, y0], [x1, y1], c32(colors.tab_hover, 120))
            .rounding(2.0)
            .filled(true)
            .build();
    }

    // Active: thick accent underline
    if is_active {
        let bar_h = 3.0;
        draw.add_rect([x0 + 2.0, y1 - bar_h], [x1 - 2.0, y1], c32(tab_accent, 255))
            .rounding(1.5)
            .filled(true)
            .build();
    }

    render_tab_content(
        draw,
        item,
        is_active,
        hovered,
        close_hovered,
        x0,
        y0,
        x1,
        cfg,
        time,
    );
}

// ─── Card style (Chrome/browser) ────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_tab_card<T: PageItem>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    item: &T,
    is_active: bool,
    hovered: bool,
    close_hovered: bool,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    cfg: &PageControlConfig,
    tab_accent: [u8; 3],
    time: f32,
) {
    let colors = &cfg.colors;

    if is_active {
        // Active card: top-rounded body + flat bottom extension
        // Top rounded part
        draw.add_rect([x0, y0], [x1, y1 - 2.0], c32(colors.tab_active, 255))
            .rounding(6.0)
            .filled(true)
            .build();
        // Flat bottom to cover rounded corners at bottom
        draw.add_rect([x0, y1 - 6.0], [x1, y1 + 1.0], c32(colors.tab_active, 255))
            .filled(true)
            .build();
        // Top accent bar
        draw.add_rect([x0 + 2.0, y0], [x1 - 2.0, y0 + 3.0], c32(tab_accent, 220))
            .rounding(1.5)
            .filled(true)
            .build();
    } else if hovered {
        draw.add_rect([x0, y0 + 2.0], [x1, y1 - 2.0], c32(colors.tab_hover, 180))
            .rounding(6.0)
            .filled(true)
            .build();
        draw.add_rect([x0, y1 - 6.0], [x1, y1], c32(colors.tab_hover, 180))
            .filled(true)
            .build();
    }

    render_tab_content(
        draw,
        item,
        is_active,
        hovered,
        close_hovered,
        x0,
        y0,
        x1,
        cfg,
        time,
    );
}

// ─── Square style (classic) ─────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_tab_square<T: PageItem>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    item: &T,
    is_active: bool,
    hovered: bool,
    close_hovered: bool,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    cfg: &PageControlConfig,
    tab_accent: [u8; 3],
    time: f32,
) {
    let colors = &cfg.colors;

    // Top-rounded body + flat bottom
    let bg = if is_active {
        colors.tab_active
    } else if hovered {
        colors.tab_hover
    } else {
        colors.tab_bg
    };
    draw.add_rect([x0, y0], [x1, y1 - 2.0], c32(bg, 255))
        .rounding(4.0)
        .filled(true)
        .build();
    draw.add_rect([x0, y1 - 4.0], [x1, y1], c32(bg, 255))
        .filled(true)
        .build();

    if is_active {
        // Border on three sides (left, top, right)
        let border_col = c32(tab_accent, 150);
        draw.add_line([x0, y1], [x0, y0 + 4.0], border_col).build();
        draw.add_line([x0 + 4.0, y0], [x1 - 4.0, y0], border_col)
            .build();
        draw.add_line([x1, y0 + 4.0], [x1, y1], border_col).build();

        if cfg.show_tab_underline {
            draw.add_rect([x0 + 2.0, y1 - 2.0], [x1 - 2.0, y1], c32(tab_accent, 255))
                .filled(true)
                .build();
        }
    } else {
        // Bottom border for inactive tabs
        draw.add_line([x0, y1], [x1, y1], c32(colors.separator, 100))
            .build();
    }

    render_tab_content(
        draw,
        item,
        is_active,
        hovered,
        close_hovered,
        x0,
        y0,
        x1,
        cfg,
        time,
    );
}

// ─── Scroll buttons ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_scroll_buttons(
    draw: &dear_imgui_rs::DrawListMut<'_>,
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
    cfg: &PageControlConfig,
) {
    let colors = &cfg.colors;
    let btn_w = cfg.scroll_btn_width;

    // Left button
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
        c32(colors.text, if lhover { 255 } else { 150 }),
        arrow,
    );
    if accept_clicks && lhover && ui.is_mouse_down(MouseButton::Left) {
        *scroll_offset -= cfg.scroll_speed * ui.io().delta_time();
    }

    // Right button
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
        c32(colors.text, if rhover { 255 } else { 150 }),
        arrow_r,
    );
    if accept_clicks && rhover && ui.is_mouse_down(MouseButton::Left) {
        *scroll_offset += cfg.scroll_speed * ui.io().delta_time();
    }

    // Clamp scroll
    let max_scroll = (total_w - scroll_area_w).max(0.0);
    *scroll_offset = scroll_offset.clamp(0.0, max_scroll);
}

// ─── Close confirmation popup ───────────────────────────────────────────────

fn render_close_popup<T: PageItem>(pc: &mut PageControl<T>, ui: &Ui) {
    let strings = &pc.config.strings;

    // Build popup ID on the stack — fmt_buf is reused inside the popup body
    let popup_id = format!("##pc_close_{}", pc.imgui_id);

    if pc.pending_close_new {
        pc.pending_close_new = false;
        ui.open_popup(&popup_id);
    }

    let mut should_clear = false;

    if let Some(_token) = ui
        .begin_modal_popup_config(&popup_id)
        .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
        .begin()
    {
        let name = pc
            .pending_close
            .and_then(|id| pc.pages.iter().find(|p| p.id == id))
            .map(|p| p.item.title())
            .unwrap_or("Unknown");

        pc.fmt_buf.clear();
        let _ = write!(pc.fmt_buf, "{} {}", icons::ALERT, strings.close);
        ui.text(&pc.fmt_buf);
        ui.spacing();
        ui.text_colored([0.88, 0.90, 0.92, 1.0], name);
        ui.spacing();
        ui.text_colored(LABEL_COLOR, strings.close_confirm);
        ui.spacing();
        ui.separator();
        ui.spacing();

        let btn_w = 120.0_f32;
        let spacing = ui.clone_style().item_spacing()[0];
        let total_w = btn_w * 2.0 + spacing;
        let avail_w = ui.content_region_avail()[0];
        let offset = ((avail_w - total_w) * 0.5).max(0.0);
        ui.set_cursor_pos([ui.cursor_pos()[0] + offset, ui.cursor_pos()[1]]);

        let _r = [
            ui.push_style_color(StyleColor::Button, [0.70, 0.22, 0.22, 1.0]),
            ui.push_style_color(StyleColor::ButtonHovered, [0.82, 0.30, 0.30, 1.0]),
            ui.push_style_color(StyleColor::ButtonActive, [0.60, 0.18, 0.18, 1.0]),
        ];
        if ui.button_with_size(strings.close, [btn_w, 0.0]) {
            if let Some(id) = pc.pending_close.take() {
                pc.closing_tab = Some((id, 1.0));
            }
            ui.close_current_popup();
        }
        drop(_r);

        ui.same_line();

        if ui.button_with_size(strings.cancel, [btn_w, 0.0]) {
            should_clear = true;
            ui.close_current_popup();
        }
    }

    if should_clear {
        pc.pending_close = None;
    }
}
