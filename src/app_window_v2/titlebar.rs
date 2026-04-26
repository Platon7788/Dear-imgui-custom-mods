//! Pure-visual titlebar renderer.
//!
//! All hit-testing for v2 is done by the OS via the WndProc subclass
//! ([`super::win32::subclass`]). This module is **only** responsible for
//! drawing the visual elements (background, separator, icon, title, button
//! glyphs, hover highlights for buttons we know are hovered via OS hover
//! state) and for updating the [`SharedHitRegions`] each frame so the
//! WndProc knows which screen rectangles map to which semantic region.
//!
//! There is **no** drag detection, **no** resize edge detection, **no**
//! `WM_NCLBUTTONDOWN` faking — the OS does all of that natively because
//! we report the right HT* codes from `WM_NCHITTEST`.

use dear_imgui_rs::{DrawListMut, MouseButton, Ui};

use super::config::{TitleAlign, TitlebarConfig};
use super::hit_test::{HitRegions, HoveredNcButton, PixelRect, SharedHitRegions};
use super::state::TitlebarStateV2;
use crate::borderless_window::theme::TitlebarColors;
use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

/// One frame's titlebar render result.
///
/// - `extra_clicked`: id of an extra (custom) button clicked this frame.
/// - `minimize_clicked`: the minimize button was clicked this frame.
///   The minimize button returns `HTCLIENT` from `WM_NCHITTEST` so ImGui
///   owns its clicks; the caller must apply the Win11 pending_remax
///   workaround before calling `window.set_minimized(true)`.
#[derive(Debug, Clone, Default)]
pub(super) struct TitlebarFrame {
    pub extra_clicked: Option<&'static str>,
    pub minimize_clicked: bool,
}

#[inline]
fn c32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// Render the titlebar and update the shared hit regions.
///
/// `dpi_scale` is the physical-pixel scale factor — used to convert the
/// logical-pixel rectangles we lay out (in ImGui units) into physical
/// pixels for the WndProc's `WM_NCHITTEST` (which sees physical coords).
pub(super) fn render_titlebar_v2(
    ui: &Ui,
    cfg: &TitlebarConfig,
    state: &TitlebarStateV2,
    regions: &SharedHitRegions,
    dpi_scale: f32,
) -> TitlebarFrame {
    let mut frame = TitlebarFrame::default();

    let cursor = ui.cursor_screen_pos();
    let win_w = ui.io().display_size()[0];
    let draw = ui.get_window_draw_list();

    let colors: TitlebarColors = cfg.theme.titlebar();
    let dim = cfg.focus_dim && !state.focused;
    let bg_col = if dim { colors.bg_inactive } else { colors.bg };
    let title_col = if dim { colors.title_inactive } else { colors.title };
    let icon_col = if dim { colors.title_inactive } else { colors.icon };

    let h = cfg.titlebar_height;
    let sep_h = cfg.separator_height;
    let btn_w = cfg.buttons.width;
    let ir = cfg.buttons.icon_radius;

    // ── Background ────────────────────────────────────────────────────────
    draw.add_rect(
        [cursor[0], cursor[1]],
        [cursor[0] + win_w, cursor[1] + h],
        c32(bg_col),
    )
    .filled(true)
    .build();

    // ── Separator ─────────────────────────────────────────────────────────
    if cfg.separator_visible {
        draw.add_rect(
            [cursor[0], cursor[1] + h - sep_h],
            [cursor[0] + win_w, cursor[1] + h],
            c32(colors.separator),
        )
        .filled(true)
        .build();
    }

    // ── Layout metrics ────────────────────────────────────────────────────
    let num_std = cfg.buttons.show_close as usize
        + cfg.buttons.show_maximize as usize
        + cfg.buttons.show_minimize as usize;
    let btn_area_w = (num_std + cfg.buttons.extra.len()) as f32 * btn_w;
    let btn_area_start = cursor[0] + win_w - btn_area_w;

    let [_, text_h] = calc_text_size("Mg");
    let text_y = cursor[1] + (h - text_h) * 0.5;

    let [mx, my] = ui.io().mouse_pos();
    let in_row = my >= cursor[1] && my < cursor[1] + h;

    // Read NC-tracked hover/press state from the WndProc subclass so we can
    // render hover/press highlights for the system buttons (HTMINBUTTON /
    // HTMAXBUTTON / HTCLOSE). ImGui's mouse_pos() can't tell us that — once
    // those buttons are reported from WM_NCHITTEST the OS owns mouse input
    // over them and ImGui never sees the cursor.
    let nc_state = regions.read();
    let hovered_btn = nc_state.hovered_button;
    let pressed_btn = nc_state.pressed_button;

    // ── Icon + title ──────────────────────────────────────────────────────
    let mut title_x = cursor[0] + cfg.title_padding_left;
    let mut icon_rect_logical = [0.0f32; 4];
    if let Some(ref icon) = cfg.icon {
        let iw = calc_text_size(icon.as_str())[0];
        icon_rect_logical = [title_x, cursor[1], title_x + iw, cursor[1] + h];
        draw.add_text([title_x, text_y], c32(icon_col), icon.as_str());
        title_x += iw + 6.0;
    }
    let icon_end_x = title_x;
    match cfg.title_align {
        TitleAlign::Left => {
            draw.add_text([title_x, text_y], c32(title_col), cfg.title.as_str());
        }
        TitleAlign::Center => {
            let tw = calc_text_size(cfg.title.as_str())[0];
            let cx = cursor[0] + (win_w - btn_area_w - tw) * 0.5;
            draw.add_text(
                [cx.max(title_x), text_y],
                c32(title_col),
                cfg.title.as_str(),
            );
        }
    }

    // ── Buttons (right → left): close, maximize, minimize, extras ────────
    let cy_btn = cursor[1] + h * 0.5;
    let mut bx = cursor[0] + win_w;

    let mut close_rect_logical = [0.0f32; 4];
    let mut max_rect_logical = [0.0f32; 4];
    let mut min_rect_logical = [0.0f32; 4];
    let mut extra_rects_logical: Vec<([f32; 4], &'static str)> = Vec::new();

    let clicked = ui.is_mouse_clicked(MouseButton::Left);

    // Helper to draw a Win11-style hover highlight rect spanning the entire
    // button cell (matches the native Snap-Layouts hover affordance and looks
    // proper even when the icon is small).
    let draw_full_cell_rect = |draw: &DrawListMut<'_>, bx: f32, color: u32| {
        // Slight inset so the rectangles don't touch the titlebar edges
        // (matches Win11 native chrome).
        draw.add_rect(
            [bx, cursor[1]],
            [bx + btn_w, cursor[1] + h - cfg.separator_height.max(0.0)],
            color,
        )
        .filled(true)
        .build();
    };

    // Lighten / darken helper for the "pressed" tint — multiplies alpha so
    // the press state visibly differs from hover.
    let press_color = |base: [f32; 4]| -> [f32; 4] {
        [base[0], base[1], base[2], (base[3] * 1.4).min(1.0)]
    };

    // Close
    if cfg.buttons.show_close {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov_nc = hovered_btn == HoveredNcButton::Close;
        let press_nc = pressed_btn == HoveredNcButton::Close;
        if press_nc {
            draw_full_cell_rect(&draw, bx, c32(press_color(colors.btn_close_hover_bg)));
        } else if hov_nc {
            draw_full_cell_rect(&draw, bx, c32(colors.btn_close_hover_bg));
        }
        draw_icon_close(&draw, cx_btn, cy_btn, ir, c32(colors.btn_close));
        close_rect_logical = [bx, cursor[1], bx + btn_w, cursor[1] + h];
    }

    // Maximize / Restore
    if cfg.buttons.show_maximize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov_nc = hovered_btn == HoveredNcButton::Max;
        let press_nc = pressed_btn == HoveredNcButton::Max;
        if press_nc {
            draw_full_cell_rect(&draw, bx, c32(press_color(colors.btn_hover_bg)));
        } else if hov_nc {
            draw_full_cell_rect(&draw, bx, c32(colors.btn_hover_bg));
        }
        if state.maximized {
            draw_icon_restore(
                &draw,
                cx_btn,
                cy_btn,
                ir,
                c32(colors.btn_maximize),
                c32(colors.bg_erase),
            );
        } else {
            draw_icon_maximize(&draw, cx_btn, cy_btn, ir, c32(colors.btn_maximize));
        }
        max_rect_logical = [bx, cursor[1], bx + btn_w, cursor[1] + h];
    }

    // Minimize — HTCLIENT in WM_NCHITTEST, so ImGui owns hover/click.
    if cfg.buttons.show_minimize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        let press = hov && ui.is_mouse_down(MouseButton::Left);
        if press {
            draw_full_cell_rect(&draw, bx, c32(press_color(colors.btn_hover_bg)));
        } else if hov {
            draw_full_cell_rect(&draw, bx, c32(colors.btn_hover_bg));
        }
        if hov && clicked {
            frame.minimize_clicked = true;
        }
        draw_icon_minimize(&draw, cx_btn, cy_btn, ir, c32(colors.btn_minimize));
        min_rect_logical = [bx, cursor[1], bx + btn_w, cursor[1] + h];
    }

    // Extra buttons (right → left). Extras live in HTCLIENT (so ImGui sees
    // their clicks) — hover detection here uses ImGui mouse pos, not the
    // NC tracking from WndProc.
    for extra in cfg.buttons.extra.iter().rev() {
        bx -= btn_w;
        let cell_cx = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov {
            draw_full_cell_rect(&draw, bx, c32(colors.btn_hover_bg));
            if let Some(tip) = extra.tooltip {
                ui.tooltip_text(tip);
            }
            if clicked && frame.extra_clicked.is_none() {
                frame.extra_clicked = Some(extra.id);
            }
        }
        let [tw, th] = calc_text_size(extra.label);
        draw.add_text(
            [cell_cx - tw * 0.5, cy_btn - th * 0.5],
            c32(extra.color),
            extra.label,
        );
        extra_rects_logical.push(([bx, cursor[1], bx + btn_w, cursor[1] + h], extra.id));
    }

    // System buttons (close, max, min) and the icon are handled by the OS
    // via WM_NCHITTEST → HTCLOSE / HTMAXBUTTON / HTMINBUTTON / HTSYSMENU.
    // We never see the click through ImGui — the OS sends WM_SYSCOMMAND
    // which winit translates into the appropriate WindowEvent.

    // ── Update shared hit regions (logical → physical) ────────────────────
    let to_phys = |r: [f32; 4]| -> PixelRect {
        // Logical (ImGui) coords are window-relative; the WndProc reads
        // window-rect-relative coords. Subtract the window's screen origin
        // (== cursor[0]/cursor[1] which is the ImGui window origin == [0,0]
        // since our root window is positioned at [0,0]).
        let l = ((r[0] - cursor[0]) * dpi_scale).round() as i32;
        let t = ((r[1] - cursor[1]) * dpi_scale).round() as i32;
        let rr = ((r[2] - cursor[0]) * dpi_scale).round() as i32;
        let b = ((r[3] - cursor[1]) * dpi_scale).round() as i32;
        PixelRect { left: l, top: t, right: rr, bottom: b }
    };

    let regions_data = HitRegions {
        titlebar_height: (h * dpi_scale).round() as i32,
        resize_zone: (cfg.resize_zone * dpi_scale).round().max(1.0) as i32,
        // Caption = entire titlebar row minus the system-button area on
        // the right and minus the icon area on the left (icon → HTSYSMENU).
        caption: PixelRect {
            left: (icon_end_x * dpi_scale).round() as i32 - (cursor[0] * dpi_scale).round() as i32,
            top: 0,
            right: ((btn_area_start - cursor[0]) * dpi_scale).round() as i32,
            bottom: (h * dpi_scale).round() as i32,
        },
        min_btn: if cfg.buttons.show_minimize {
            to_phys(min_rect_logical)
        } else {
            PixelRect::empty()
        },
        max_btn: if cfg.buttons.show_maximize {
            to_phys(max_rect_logical)
        } else {
            PixelRect::empty()
        },
        close_btn: if cfg.buttons.show_close {
            to_phys(close_rect_logical)
        } else {
            PixelRect::empty()
        },
        icon_btn: if cfg.icon.is_some() {
            to_phys(icon_rect_logical)
        } else {
            PixelRect::empty()
        },
        extras: extra_rects_logical
            .into_iter()
            .map(|(r, _id)| to_phys(r))
            .collect(),
        is_maximized: state.maximized,
        passthrough: false,
        // These two are owned by the WndProc; SharedHitRegions::write()
        // preserves them — values written here are immediately overwritten.
        hovered_button: HoveredNcButton::None,
        pressed_button: HoveredNcButton::None,
    };
    regions.write(regions_data);

    frame
}

// ── Icon primitives (font-independent draw-list shapes) ──────────────────────

fn draw_icon_close(draw: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32) {
    let d = r * 0.56;
    draw.add_line([cx - d, cy - d], [cx + d, cy + d], col)
        .thickness(1.5)
        .build();
    draw.add_line([cx + d, cy - d], [cx - d, cy + d], col)
        .thickness(1.5)
        .build();
}

fn draw_icon_maximize(draw: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32) {
    let p = r * 0.72;
    draw.add_rect([cx - p, cy - p], [cx + p, cy + p], col)
        .thickness(1.5)
        .build();
}

fn draw_icon_restore(draw: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32, bg: u32) {
    let p = r * 0.72;
    let sh = r * 0.38;
    draw.add_rect([cx - p + sh, cy - p - sh], [cx + p + sh, cy + p - sh], col)
        .thickness(1.2)
        .build();
    draw.add_rect([cx - p, cy - p + sh], [cx + p - sh, cy + p + sh], bg)
        .filled(true)
        .build();
    draw.add_rect([cx - p, cy - p + sh], [cx + p - sh, cy + p + sh], col)
        .thickness(1.5)
        .build();
}

fn draw_icon_minimize(draw: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32) {
    let p = r * 0.72;
    let y = cy + r * 0.40;
    draw.add_line([cx - p, y], [cx + p, y], col)
        .thickness(1.5)
        .build();
}
