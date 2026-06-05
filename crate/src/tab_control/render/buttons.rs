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
