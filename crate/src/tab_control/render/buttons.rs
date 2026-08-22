//! Side buttons and modals: scroll arrows, overflow `…` dropdown, add `+`
//! button, and the close-confirmation modal.

use std::fmt::Write;

use dear_imgui_rs::{MouseButton, Ui, WindowFlags};

use crate::icons;
use crate::utils::color::rgb_arr as c32;
use crate::utils::popup::danger_button;
use crate::utils::text::calc_text_size;

use super::super::config::TabControlConfig;
use super::super::types::*;
use super::super::{TabControl, TabItem};
use super::rgba;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_scroll_buttons(
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
    let lcol = c32(colors.text, if lhover { 255 } else { 160 });
    if cfg.icons_available {
        let arrow = icons::CHEVRON_LEFT;
        let asz = calc_text_size(arrow);
        draw.add_text(
            [
                lx + (btn_w - asz[0]) * 0.5,
                strip_y + (strip_h - asz[1]) * 0.5,
            ],
            lcol,
            arrow,
        );
    } else {
        draw_chevron(&draw, lx + btn_w * 0.5, strip_y + strip_h * 0.5, true, lcol);
    }
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
    let rcol = c32(colors.text, if rhover { 255 } else { 160 });
    if cfg.icons_available {
        let arrow_r = icons::CHEVRON_RIGHT;
        let arsz = calc_text_size(arrow_r);
        draw.add_text(
            [
                rx + (btn_w - arsz[0]) * 0.5,
                strip_y + (strip_h - arsz[1]) * 0.5,
            ],
            rcol,
            arrow_r,
        );
    } else {
        draw_chevron(
            &draw,
            rx + btn_w * 0.5,
            strip_y + strip_h * 0.5,
            false,
            rcol,
        );
    }
    if accept_clicks && rhover && ui.is_mouse_down(MouseButton::Left) {
        *scroll_offset += cfg.scroll_speed * ui.io().delta_time();
    }

    let max_scroll = (total_w - scroll_area_w).max(0.0);
    *scroll_offset = scroll_offset.clamp(0.0, max_scroll);
}

pub(super) fn render_overflow_button(
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
    let dcol = c32(colors.text, if hovered { 255 } else { 170 });
    if cfg.icons_available {
        let dots = icons::DOTS_HORIZONTAL;
        let dsz = calc_text_size(dots);
        draw.add_text(
            [x + (w - dsz[0]) * 0.5, y0 + (cfg.tab_height - dsz[1]) * 0.5],
            dcol,
            dots,
        );
    } else {
        draw_dots(&draw, x + w * 0.5, y0 + cfg.tab_height * 0.5, dcol);
    }

    if hovered
        && !ui.is_mouse_clicked(MouseButton::Left)
        && !ui.is_mouse_clicked(MouseButton::Right)
    {
        crate::utils::themed_tooltip(ui, || ui.text(&cfg.strings.overflow_tooltip));
    }

    hovered && ui.is_mouse_clicked(MouseButton::Left)
}

pub(super) fn render_overflow_popup_body<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) {
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
        // Gate on `icons_available` — without the MDI font registered, the
        // codepoint renders as a `?` box. Layout / on-tab drawing already
        // check this; the popup was missed (M3 from session 035 audit,
        // visible in user screenshot).
        if icons_available && let Some(icon) = tab.item.icon() {
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

pub(super) fn render_add_button(
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
    let pcol = c32(colors.text, if hovered { 255 } else { 170 });
    if cfg.icons_available {
        let plus = icons::PLUS;
        let psz = calc_text_size(plus);
        draw.add_text(
            [x + (w - psz[0]) * 0.5, y0 + (cfg.tab_height - psz[1]) * 0.5],
            pcol,
            plus,
        );
    } else {
        draw_plus(&draw, x + w * 0.5, y0 + cfg.tab_height * 0.5, 4.0, pcol);
    }
    if hovered && !ui.is_mouse_clicked(MouseButton::Left) {
        crate::utils::themed_tooltip(ui, || ui.text(&cfg.strings.add_tab));
    }
    hovered && ui.is_mouse_clicked(MouseButton::Left)
}

// ─── Close confirmation popup ───────────────────────────────────────────────

pub(super) fn render_close_popup<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) {
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
        let name = pending.map_or("Unknown", |t| t.item.title());
        let is_dirty = pending.is_some_and(|t| t.item.status() == TabStatus::Dirty);

        pc.fmt_buf.clear();
        // Gate the MDI alert glyph on the icon font — it renders as a tofu box
        // otherwise, mirroring the strip's own icon gating. The label always shows.
        if pc.config.icons_available {
            let _ = write!(pc.fmt_buf, "{} ", icons::ALERT);
        }
        let _ = write!(pc.fmt_buf, "{}", strings.close);
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
        // Read just the one field we need instead of `ui.clone_style()`, which
        // deep-copies the entire ~700-byte `Style` struct. Mirrors the
        // `igGetStyle()` field-access pattern used elsewhere in the crate
        // (code_editor::render, virtual_table::render).
        // SAFETY: `igGetStyle` returns a valid pointer to the current frame's
        // style for the duration of the frame; `ItemSpacing` is a plain
        // `repr(C)` `ImVec2_c` with no invariants. Single-threaded UI.
        let spacing = unsafe { (*dear_imgui_rs::sys::igGetStyle()).ItemSpacing.x };
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

// ─── Vector side-button glyphs (font-free fallback) ─────────────────────────
//
// Drawn purely through the draw list when the MDI icon font isn't registered
// (`icons_available == false`), so the scroll arrows, overflow `…` and add `+`
// never render as tofu boxes. Mirrors `draw::draw_close_glyph`, which already
// renders the close cross without a font.

/// Chevron (`‹` / `›`) centered at `(cx, cy)`.
fn draw_chevron(draw: &dear_imgui_rs::DrawListMut<'_>, cx: f32, cy: f32, left: bool, col: u32) {
    let half = 4.0; // half-height of the chevron
    let w = 3.0; // horizontal spread
    let x_far = if left { cx + w * 0.5 } else { cx - w * 0.5 };
    let x_near = if left { cx - w * 0.5 } else { cx + w * 0.5 };
    draw.add_line([x_far, cy - half], [x_near, cy], col)
        .thickness(1.6)
        .build();
    draw.add_line([x_near, cy], [x_far, cy + half], col)
        .thickness(1.6)
        .build();
}

/// Three horizontal dots (`…`) centered at `(cx, cy)`.
fn draw_dots(draw: &dear_imgui_rs::DrawListMut<'_>, cx: f32, cy: f32, col: u32) {
    let r = 1.4;
    for dx in [-4.0_f32, 0.0, 4.0] {
        let x = cx + dx;
        draw.add_rect([x - r, cy - r], [x + r, cy + r], col)
            .rounding(r)
            .filled(true)
            .build();
    }
}

/// Plus sign (`+`) centered at `(cx, cy)`, arms of half-length `half`.
fn draw_plus(draw: &dear_imgui_rs::DrawListMut<'_>, cx: f32, cy: f32, half: f32, col: u32) {
    draw.add_line([cx, cy - half], [cx, cy + half], col)
        .thickness(1.6)
        .build();
    draw.add_line([cx - half, cy], [cx + half, cy], col)
        .thickness(1.6)
        .build();
}
