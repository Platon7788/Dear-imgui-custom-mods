//! Stateless titlebar rendering and chrome-less whole-window resize.
//!
//! Split out of `mod.rs`; the public `render_titlebar` / `whole_window_resize`
//! entry points are re-exported by the parent so their paths stay stable.

use super::*;

// ── Stateless render functions ──────────────────────────────────────────────

/// Paint the titlebar into the current ImGui window's draw list, returning
/// the action and hovered resize edge for this frame.
///
/// Call as the **first** thing inside a full-screen, zero-padding ImGui
/// window. The caller is responsible for advancing the cursor below
/// `cfg.height` (`ui.set_cursor_pos([0.0, cfg.height])` + a zero-size
/// `dummy([0.0, 0.0])`) before rendering content beneath the titlebar.
///
/// Hover detection for the buttons uses the position-based check (no
/// `is_window_hovered`) so the chrome works correctly even when the
/// host wraps its content in a sibling child window inside the same root.
///
/// Modal popups (anything opened with `ui.open_popup`) suppress click
/// actions on titlebar buttons / drag / resize so a popup painted over
/// the titlebar can't fire its × by accident — hovered cursors still
/// resolve so the host can show the resize affordance.
///
/// # Parameters
///
/// - `palette` — colour set for bg / separator / title / icon / buttons.
///   Usually `theme.titlebar()` cached once per theme change.
/// - `maximized` — current OS-maximised state. Drives the
///   maximise-vs-restore glyph swap and suppresses edge-resize.
/// - `resize_zone` — edge / corner hit-zone width in logical pixels.
///   `6.0` is the [`Chrome`] default and matches Windows' DWM feel.
/// - `os_resizable` — `false` disables edge-hit detection (use for
///   fixed-size dialogs, splash screens). Independent of `maximized`.
pub fn render_titlebar(
    ui: &Ui,
    cfg: &TitlebarConfig,
    title: &str,
    palette: &TitlebarColors,
    maximized: bool,
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
    // Defensive clamp: `Buttons::with_hover_zoom_scale` enforces the same
    // 1.0..=2.0 range on the builder path, but a host that mutates the
    // field directly (or restores from a tampered `.ron`) can poke any
    // value through. Clamping here keeps the glyph from collapsing
    // (< 1.0) or going pixel-fuzzy (> 2.0).
    let zoom = cfg.buttons.hover_zoom_scale.clamp(1.0, 2.0);
    let [ww, wh] = win_size;
    let [mx, my] = ui.io().mouse_pos();

    let bg = c32(palette.bg);
    let separator = c32(palette.separator);
    let title_col = c32(palette.title);
    let icon_col = c32(palette.icon);
    let btn_min = c32(palette.btn_minimize);
    let btn_max = c32(palette.btn_maximize);
    let btn_close_col = c32(palette.btn_close);

    // Background.
    draw.add_rect([cursor[0], cursor[1]], [cursor[0] + ww, cursor[1] + h], bg)
        .filled(true)
        .build();

    // Separator.
    if cfg.separator_visible {
        draw.add_rect(
            [cursor[0], cursor[1] + h - sep_h],
            [cursor[0] + ww, cursor[1] + h],
            separator,
        )
        .filled(true)
        .build();
    }

    // Layout — buttons drawn right-to-left.
    let std_count =
        cfg.buttons.minimize as usize + cfg.buttons.maximize as usize + cfg.buttons.close as usize;
    let btn_total_w = std_count as f32 * btn_w;
    let btn_area_x = cursor[0] + ww - btn_total_w;
    let text_h = line_height(ui);
    let text_y = cursor[1] + (h - text_h) * 0.5;
    let cy_btn = cursor[1] + h * 0.5;

    // Hover detection: titlebar y-strip (cursor[1] .. cursor[1]+h). We
    // intentionally do NOT use `ui.is_window_hovered()` here — the host's
    // content child window can be a sibling of the chrome root in some
    // wiring patterns, and `is_window_hovered` returns false at hovered
    // child positions. Position-only is robust across all layouts; the
    // titlebar y-band is exclusive to chrome by construction.
    let in_row = my >= cursor[1] && my < cursor[1] + h;
    // Suppress action clicks while any popup is open — a modal popup
    // painted over the titlebar would otherwise let a single click both
    // dismiss the popup AND fire the underlying ×/maximise/minimise
    // button. Hover state still resolves so cursors still update.
    let popup_blocks = ui.is_popup_open_with_flags(
        "",
        PopupQueryFlags::ANY_POPUP_ID | PopupQueryFlags::ANY_POPUP_LEVEL,
    );
    let clicked = !popup_blocks && ui.is_mouse_clicked(MouseButton::Left);
    let double_clicked = !popup_blocks && ui.is_mouse_double_clicked(MouseButton::Left);
    let mut action = TitlebarAction::None;

    // Icon + title.
    let mut title_x = cursor[0] + cfg.title_padding_left;
    if let Some(ref icon) = cfg.icon {
        draw.add_text([title_x, text_y], icon_col, icon.as_str());
        title_x += calc_text_size(icon.as_str())[0] + 6.0;
    }

    if cfg.title_visible {
        match cfg.title_align {
            TitleAlign::Left => {
                draw.add_text([title_x, text_y], title_col, title);
            }
            TitleAlign::Center => {
                let tw = calc_text_size(title)[0];
                let cx = cursor[0] + (ww - btn_total_w - tw) * 0.5;
                draw.add_text([cx.max(title_x), text_y], title_col, title);
            }
        }
    }

    let mut bx = cursor[0] + ww;

    if cfg.buttons.close {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov && clicked && action == TitlebarAction::None {
            action = TitlebarAction::Close;
        }
        let r = if hov { ir * zoom } else { ir };
        glyph::draw_close(&draw, cx_btn, cy_btn, r, btn_close_col);
    }

    if cfg.buttons.maximize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov && clicked && action == TitlebarAction::None {
            action = TitlebarAction::Maximize;
        }
        let r = if hov { ir * zoom } else { ir };
        if maximized {
            glyph::draw_restore(&draw, cx_btn, cy_btn, r, btn_max);
        } else {
            glyph::draw_maximize(&draw, cx_btn, cy_btn, r, btn_max);
        }
    }

    if cfg.buttons.minimize {
        bx -= btn_w;
        let cx_btn = bx + btn_w * 0.5;
        let hov = in_row && mx >= bx && mx < bx + btn_w;
        if hov && clicked && action == TitlebarAction::None {
            action = TitlebarAction::Minimize;
        }
        let r = if hov { ir * zoom } else { ir };
        glyph::draw_minimize(&draw, cx_btn, cy_btn, r, btn_min);
    }

    // Resize hover (only when OS-resizable and not maximized).
    let lx = mx - win_pos[0];
    let ly = my - win_pos[1];
    let over_buttons = in_row && mx >= btn_area_x;
    let hover_edge = if os_resizable && !over_buttons && !maximized {
        edge_at(lx, ly, ww, wh, resize_zone)
    } else {
        None
    };

    if action == TitlebarAction::None
        && clicked
        && let Some(edge) = hover_edge
    {
        action = TitlebarAction::ResizeStart(edge);
    }

    if action == TitlebarAction::None && in_row && mx < btn_area_x && hover_edge.is_none() {
        if cfg.double_click_maximize && double_clicked {
            action = TitlebarAction::Maximize;
        } else if clicked {
            action = TitlebarAction::DragStart;
        }
    }

    TitlebarResult { action, hover_edge }
}

/// For chrome-less windows (splash / kiosk): detect resize edges over the
/// full window area without rendering a titlebar.
///
/// ```ignore
/// // Splash with chrome-less resize support
/// chrome.lock().unwrap().render_splash(ui, &window, |ui, area| {
///     ui.image(splash_logo).size(area.size).build();
/// });
/// ```
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
