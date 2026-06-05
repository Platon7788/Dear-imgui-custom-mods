//! Per-tab visual rendering: style dispatch (pill / underline / square), tab
//! content (icon / title / status dot / badge / close button) and the
//! parametric close glyph.
//!
//! All drawing is draw-list based — no widget calls — so it works regardless
//! of `cfg.icons_available`.

use crate::utils::color::rgb_arr as c32;
use crate::utils::text::calc_text_size;

use super::super::TabItem;
use super::super::config::TabControlConfig;
use super::super::layout::{
    BADGE_INNER_PAD_X, BADGE_INNER_PAD_Y, BADGE_LEFT_GAP, ICON_TITLE_GAP, STATUS_DOT_DIAM,
    STATUS_DOT_GAP,
};
use super::super::types::*;
use super::{CLOSE_HIT_PAD, TabDraw};

// ─── Tab drawing (style dispatch) ───────────────────────────────────────────

pub(super) fn draw_tab<T: TabItem>(ctx: &TabDraw<'_, T>) {
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
        // Per-tab override wins for both glyph and letter fallback; pinned tabs
        // only show one of them, so a single resolution covers the visible
        // label whatever the host chose.
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
            let s: &str = ch.encode_utf8(&mut buf);
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
