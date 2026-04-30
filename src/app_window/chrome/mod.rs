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

pub use edge::{ResizeEdge, cursor_for_edge, edge_at, resize_direction};

use dear_imgui_rs::{DrawListMut, MouseButton, Ui};

use super::config::{CloseMode, TitleAlign, TitlebarConfig};
use super::state::TitlebarState;
use crate::theme::TitlebarColors;
use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

// ── Public types ──────────────────────────────────────────────────────────────

/// Action produced by the titlebar each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TitlebarAction {
    None,
    Minimize,
    Maximize,
    Close,
    CloseRequested,
    DragStart,
    IconClick,
    ResizeStart(ResizeEdge),
    Extra(&'static str),
}

#[derive(Debug, Clone, Copy)]
#[must_use = "titlebar actions must be dispatched"]
pub struct TitlebarResult {
    pub action: TitlebarAction,
    pub hover_edge: Option<ResizeEdge>,
}

impl TitlebarResult {
    pub(super) fn none() -> Self {
        Self {
            action: TitlebarAction::None,
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
    cfg: &TitlebarConfig,
    title: &str,
    colors: &TitlebarColors,
    state: &TitlebarState,
    resize_zone: f32,
    os_resizable: bool,
) -> TitlebarResult {
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
    let mut action = TitlebarAction::None;

    // Icon + title.
    let mut title_x = cursor[0] + cfg.title_padding_left;
    if let Some(ref icon) = cfg.icon {
        draw.add_text([title_x, text_y], c32(colors.icon), icon.as_str());
        title_x += calc_text_size(icon.as_str())[0] + 6.0;
    }
    let icon_end_x = title_x;

    if cfg.title_visible {
        match cfg.title_align {
            TitleAlign::Left => {
                draw.add_text([title_x, text_y], c32(colors.title), title);
            }
            TitleAlign::Center => {
                let tw = calc_text_size(title)[0];
                let cx = cursor[0] + (ww - btn_total_w - tw) * 0.5;
                draw.add_text([cx.max(title_x), text_y], c32(colors.title), title);
            }
        }
    }

    // Standard buttons drawn right-to-left.
    let mut bx = cursor[0] + ww;
    // Glyph scale on hover — same micro-magnification trick as
    // `nav_panel`'s `HoverStyle::Zoom`. Applied by multiplying the
    // glyph radius before handing it to `glyph::draw_*` (the icons
    // are drawn into a `[-r, +r]` unit space so this scales the
    // whole figure proportionally, no font atlas needed).
    let zoom = cfg.buttons.hover_zoom_scale;

    if cfg.buttons.close {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = button_hit(
            &draw,
            bx,
            btn_w,
            ir,
            ipad,
            cy_btn,
            c32(colors.btn_close_hover_bg),
            in_row,
            mx,
            cfg.buttons.show_hover_bg,
        );
        if hov && clicked && action == TitlebarAction::None {
            action = match cfg.close_mode {
                CloseMode::Immediate => TitlebarAction::Close,
                CloseMode::Confirm => TitlebarAction::CloseRequested,
            };
        }
        let r = if hov { ir * zoom } else { ir };
        glyph::draw_close(&draw, cx_btn, cy_btn, r, c32(colors.btn_close));
    }

    if cfg.buttons.maximize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = button_hit(
            &draw,
            bx,
            btn_w,
            ir,
            ipad,
            cy_btn,
            c32(colors.btn_hover_bg),
            in_row,
            mx,
            cfg.buttons.show_hover_bg,
        );
        if hov && clicked && action == TitlebarAction::None {
            action = TitlebarAction::Maximize;
        }
        let r = if hov { ir * zoom } else { ir };
        if state.maximized {
            glyph::draw_restore(&draw, cx_btn, cy_btn, r, c32(colors.btn_maximize));
        } else {
            glyph::draw_maximize(&draw, cx_btn, cy_btn, r, c32(colors.btn_maximize));
        }
    }

    if cfg.buttons.minimize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = button_hit(
            &draw,
            bx,
            btn_w,
            ir,
            ipad,
            cy_btn,
            c32(colors.btn_hover_bg),
            in_row,
            mx,
            cfg.buttons.show_hover_bg,
        );
        if hov && clicked && action == TitlebarAction::None {
            action = TitlebarAction::Minimize;
        }
        let r = if hov { ir * zoom } else { ir };
        glyph::draw_minimize(&draw, cx_btn, cy_btn, r, c32(colors.btn_minimize));
    }

    // Extra buttons (right-to-left). Same zoom + hover-bg-skip rules.
    for extra in cfg.extras.iter().rev() {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov {
            if cfg.buttons.show_hover_bg {
                draw.add_rect(
                    [cx_btn - ir - ipad, cy_btn - ir - ipad],
                    [cx_btn + ir + ipad, cy_btn + ir + ipad],
                    c32(colors.btn_hover_bg),
                )
                .filled(true)
                .rounding(3.0)
                .build();
            }
            if let Some(tip) = extra.tooltip {
                crate::utils::themed_tooltip(ui, || ui.text(tip));
            }
            if clicked && action == TitlebarAction::None {
                action = TitlebarAction::Extra(extra.id);
            }
        }
        // Extras are text-glyphs not vector icons — scale them via
        // a font-size override the same way `nav_panel::draw_icon`
        // does for the Zoom hover style.
        let [tw, th] = calc_text_size(extra.label);
        if hov && zoom > 1.0 {
            let font = ui.current_font();
            let base = ui.current_font_size();
            let scaled = base * zoom;
            let sw = tw * zoom;
            let sh = th * zoom;
            draw.add_text_with_font(
                font,
                scaled,
                [cx_btn - sw * 0.5, cy_btn - sh * 0.5],
                c32(extra.color),
                extra.label,
                0.0,
                None,
            );
        } else {
            draw.add_text(
                [cx_btn - tw * 0.5, cy_btn - th * 0.5],
                c32(extra.color),
                extra.label,
            );
        }
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
    if action == TitlebarAction::None
        && clicked
        && let Some(edge) = hover_edge
    {
        action = TitlebarAction::ResizeStart(edge);
    }

    // Icon click.
    if action == TitlebarAction::None && clicked && cfg.icon.is_some() {
        let icon_start = cursor[0] + cfg.title_padding_left;
        if in_row && mx >= icon_start && mx < icon_end_x {
            action = TitlebarAction::IconClick;
        }
    }

    // Drag / double-click maximize.
    if action == TitlebarAction::None && in_row && mx < btn_area_x && hover_edge.is_none() {
        if cfg.double_click_maximize && ui.is_mouse_double_clicked(MouseButton::Left) {
            action = TitlebarAction::Maximize;
        } else if clicked {
            action = TitlebarAction::DragStart;
        }
    }

    TitlebarResult { action, hover_edge }
}

// ── Whole-window resize (chrome-less / splash) ───────────────────────────────

/// For chrome-less windows: detect resize edges over the full window area.
///
/// Returns the same [`TitlebarResult`] shape as [`render_titlebar`]
/// so the caller pipeline (`gpu/mod.rs`) treats the chrome and the
/// chrome-less paths uniformly. Was historically a `(Option<edge>,
/// TitlebarAction)` tuple — drift cleaned up 2026-04-30 audit.
pub fn whole_window_resize(
    ui: &Ui,
    resize_zone: f32,
    os_resizable: bool,
    maximized: bool,
) -> TitlebarResult {
    if !os_resizable || maximized {
        return TitlebarResult::none();
    }
    let win_pos = ui.window_pos();
    let win_size = ui.window_size();
    let [mx, my] = ui.io().mouse_pos();
    let lx = mx - win_pos[0];
    let ly = my - win_pos[1];
    let edge = edge_at(lx, ly, win_size[0], win_size[1], resize_zone);
    let action = if let Some(e) = edge {
        if ui.is_mouse_clicked(MouseButton::Left) {
            TitlebarAction::ResizeStart(e)
        } else {
            TitlebarAction::None
        }
    } else {
        TitlebarAction::None
    };
    TitlebarResult {
        action,
        hover_edge: edge,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Hit-test a standard titlebar button. Returns whether the cursor
/// is over it; optionally paints the rounded coloured hover
/// background when `show_hover_bg == true` (the historic look —
/// disabled by default, see [`super::config::Buttons::show_hover_bg`]).
/// Click handling moved out of this helper so the caller can gate it
/// on `action == None` itself, keeping the per-button blocks linear
/// instead of threading `&mut action` through.
#[allow(clippy::too_many_arguments)]
fn button_hit(
    draw: &DrawListMut<'_>,
    bx: f32,
    btn_w: f32,
    ir: f32,
    ipad: f32,
    cy: f32,
    hover_bg: u32,
    in_row: bool,
    mx: f32,
    show_hover_bg: bool,
) -> bool {
    let hov = in_row && mx >= bx && mx < bx + btn_w;
    if hov && show_hover_bg {
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
    hov
}

#[inline]
fn c32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}
