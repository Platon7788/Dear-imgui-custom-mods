//! Custom titlebar rendered with Dear ImGui's draw list.
//!
//! The host window is `WS_POPUP + WS_THICKFRAME` (winit
//! `with_decorations(false)`), so client area = window area and there is
//! no OS chrome. This module draws the entire titlebar into the wgpu
//! surface and detects clicks in ImGui space; resize edges are detected
//! here too and dispatched via
//! [`winit::window::Window::drag_resize_window`].

mod edge;
mod glyph;

pub use edge::{ResizeEdgeV2, cursor_for_edge, edge_at, resize_direction};

use dear_imgui_rs::{DrawListMut, MouseButton, Ui};

use super::config::{CloseModeV2, TitleAlignV2, TitlebarConfigV2};
use super::state::TitlebarStateV2;
use crate::theme::TitlebarColors;
use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

// ── Public types ──────────────────────────────────────────────────────────────

/// Action produced by the titlebar each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TitlebarActionV2 {
    None,
    Minimize,
    Maximize,
    Close,
    CloseRequested,
    DragStart,
    IconClick,
    ResizeStart(ResizeEdgeV2),
    Extra(&'static str),
}

#[derive(Debug, Clone, Copy)]
#[must_use = "titlebar actions must be dispatched"]
pub struct TitlebarResultV2 {
    pub action: TitlebarActionV2,
    pub hover_edge: Option<ResizeEdgeV2>,
}

impl TitlebarResultV2 {
    pub(super) fn none() -> Self {
        Self {
            action: TitlebarActionV2::None,
            hover_edge: None,
        }
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

/// Render the titlebar; returns the action and hovered resize edge for this frame.
///
/// Call as the first thing inside a full-screen, zero-padding ImGui window;
/// the caller is responsible for advancing the cursor below `cfg.height`
/// for any further content.
pub fn render_titlebar(
    ui: &Ui,
    cfg: &TitlebarConfigV2,
    title: &str,
    colors: &TitlebarColors,
    state: &TitlebarStateV2,
    resize_zone: f32,
    os_resizable: bool,
) -> TitlebarResultV2 {
    let cursor = ui.cursor_screen_pos();
    let win_pos = ui.window_pos();
    let win_size = ui.window_size();
    let draw = ui.get_window_draw_list();

    let h = cfg.height;
    let sep_h = cfg.separator_height;
    let btn_w = cfg.buttons.width;
    let ir = cfg.buttons.icon_radius;
    let ipad = cfg.buttons.icon_hover_pad;
    let [ww, wh] = win_size;
    let [mx, my] = ui.io().mouse_pos();

    // Background.
    draw.add_rect(
        [cursor[0], cursor[1]],
        [cursor[0] + ww, cursor[1] + h],
        c32(colors.bg),
    )
    .filled(true)
    .build();

    // Separator.
    if cfg.separator_visible {
        draw.add_rect(
            [cursor[0], cursor[1] + h - sep_h],
            [cursor[0] + ww, cursor[1] + h],
            c32(colors.separator),
        )
        .filled(true)
        .build();
    }

    // Layout.
    let std_count =
        cfg.buttons.minimize as usize + cfg.buttons.maximize as usize + cfg.buttons.close as usize;
    let btn_total_w = (std_count + cfg.extras.len()) as f32 * btn_w;
    let btn_area_x = cursor[0] + ww - btn_total_w;
    let [_, text_h] = calc_text_size("Mg");
    let text_y = cursor[1] + (h - text_h) * 0.5;
    let cy_btn = cursor[1] + h * 0.5;

    let in_row = my >= cursor[1] && my < cursor[1] + h && ui.is_window_hovered();
    let clicked = ui.is_mouse_clicked(MouseButton::Left);
    let mut action = TitlebarActionV2::None;

    // Icon + title.
    let mut title_x = cursor[0] + cfg.title_padding_left;
    if let Some(ref icon) = cfg.icon {
        draw.add_text([title_x, text_y], c32(colors.icon), icon.as_str());
        title_x += calc_text_size(icon.as_str())[0] + 6.0;
    }
    let icon_end_x = title_x;

    if cfg.title_visible {
        match cfg.title_align {
            TitleAlignV2::Left => {
                draw.add_text([title_x, text_y], c32(colors.title), title);
            }
            TitleAlignV2::Center => {
                let tw = calc_text_size(title)[0];
                let cx = cursor[0] + (ww - btn_total_w - tw) * 0.5;
                draw.add_text([cx.max(title_x), text_y], c32(colors.title), title);
            }
        }
    }

    // Standard buttons drawn right-to-left.
    let mut bx = cursor[0] + ww;

    if cfg.buttons.close {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        if button_hit(
            &draw,
            &mut action,
            bx,
            btn_w,
            ir,
            ipad,
            cy_btn,
            c32(colors.btn_close_hover_bg),
            in_row,
            mx,
            clicked,
        ) {
            action = match cfg.close_mode {
                CloseModeV2::Immediate => TitlebarActionV2::Close,
                CloseModeV2::Confirm => TitlebarActionV2::CloseRequested,
            };
        }
        glyph::draw_close(&draw, cx_btn, cy_btn, ir, c32(colors.btn_close));
    }

    if cfg.buttons.maximize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        if button_hit(
            &draw,
            &mut action,
            bx,
            btn_w,
            ir,
            ipad,
            cy_btn,
            c32(colors.btn_hover_bg),
            in_row,
            mx,
            clicked,
        ) {
            action = TitlebarActionV2::Maximize;
        }
        if state.maximized {
            glyph::draw_restore(
                &draw,
                cx_btn,
                cy_btn,
                ir,
                c32(colors.btn_maximize),
                c32(colors.bg_erase),
            );
        } else {
            glyph::draw_maximize(&draw, cx_btn, cy_btn, ir, c32(colors.btn_maximize));
        }
    }

    if cfg.buttons.minimize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        if button_hit(
            &draw,
            &mut action,
            bx,
            btn_w,
            ir,
            ipad,
            cy_btn,
            c32(colors.btn_hover_bg),
            in_row,
            mx,
            clicked,
        ) {
            action = TitlebarActionV2::Minimize;
        }
        glyph::draw_minimize(&draw, cx_btn, cy_btn, ir, c32(colors.btn_minimize));
    }

    // Extra buttons (right-to-left).
    for extra in cfg.extras.iter().rev() {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov {
            draw.add_rect(
                [cx_btn - ir - ipad, cy_btn - ir - ipad],
                [cx_btn + ir + ipad, cy_btn + ir + ipad],
                c32(colors.btn_hover_bg),
            )
            .filled(true)
            .rounding(3.0)
            .build();
            if let Some(tip) = extra.tooltip {
                ui.tooltip_text(tip);
            }
            if clicked && action == TitlebarActionV2::None {
                action = TitlebarActionV2::Extra(extra.id);
            }
        }
        let [tw, th] = calc_text_size(extra.label);
        draw.add_text(
            [cx_btn - tw * 0.5, cy_btn - th * 0.5],
            c32(extra.color),
            extra.label,
        );
    }

    // Resize hover (only when window is OS-resizable and not maximized).
    let lx = mx - win_pos[0];
    let ly = my - win_pos[1];
    let over_buttons = in_row && mx >= btn_area_x;
    let hover_edge = if os_resizable && !over_buttons && !state.maximized {
        edge_at(lx, ly, ww, wh, resize_zone)
    } else {
        None
    };

    // Resize click.
    if action == TitlebarActionV2::None
        && clicked
        && let Some(edge) = hover_edge
    {
        action = TitlebarActionV2::ResizeStart(edge);
    }

    // Icon click.
    if action == TitlebarActionV2::None && clicked && cfg.icon.is_some() {
        let icon_start = cursor[0] + cfg.title_padding_left;
        if in_row && mx >= icon_start && mx < icon_end_x {
            action = TitlebarActionV2::IconClick;
        }
    }

    // Drag / double-click maximize.
    if action == TitlebarActionV2::None && in_row && mx < btn_area_x && hover_edge.is_none() {
        if cfg.double_click_maximize && ui.is_mouse_double_clicked(MouseButton::Left) {
            action = TitlebarActionV2::Maximize;
        } else if clicked {
            action = TitlebarActionV2::DragStart;
        }
    }

    TitlebarResultV2 { action, hover_edge }
}

// ── Whole-window resize (chrome-less / splash) ───────────────────────────────

/// For chrome-less windows: detect resize edges over the full window area.
pub fn whole_window_resize(
    ui: &Ui,
    resize_zone: f32,
    os_resizable: bool,
    maximized: bool,
) -> (Option<ResizeEdgeV2>, TitlebarActionV2) {
    if !os_resizable || maximized {
        return (None, TitlebarActionV2::None);
    }
    let win_pos = ui.window_pos();
    let win_size = ui.window_size();
    let [mx, my] = ui.io().mouse_pos();
    let lx = mx - win_pos[0];
    let ly = my - win_pos[1];
    let edge = edge_at(lx, ly, win_size[0], win_size[1], resize_zone);
    let action = if let Some(e) = edge {
        if ui.is_mouse_clicked(MouseButton::Left) {
            TitlebarActionV2::ResizeStart(e)
        } else {
            TitlebarActionV2::None
        }
    } else {
        TitlebarActionV2::None
    };
    (edge, action)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn button_hit(
    draw: &DrawListMut<'_>,
    action: &mut TitlebarActionV2,
    bx: f32,
    btn_w: f32,
    ir: f32,
    ipad: f32,
    cy: f32,
    hover_bg: u32,
    in_row: bool,
    mx: f32,
    clicked: bool,
) -> bool {
    let hov = in_row && mx >= bx && mx < bx + btn_w;
    if hov {
        let cx = bx + btn_w * 0.5;
        draw.add_rect(
            [cx - ir - ipad, cy - ir - ipad],
            [cx + ir + ipad, cy + ir + ipad],
            hover_bg,
        )
        .filled(true)
        .rounding(3.0)
        .build();
    }
    hov && clicked && *action == TitlebarActionV2::None
}

#[inline]
fn c32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}
