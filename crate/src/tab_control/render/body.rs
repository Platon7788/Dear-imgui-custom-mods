//! Empty-state placeholder and the active-tab body frame.
//!
//! Split out of [`super::strip`] so the per-frame strip driver stays readable.
//! Behaviour is identical to the original inline blocks.

use dear_imgui_rs::Ui;

use crate::icons;
use crate::utils::color::rgb_arr as c32;
use crate::utils::text::calc_text_size;

use super::super::config::TabControlConfig;
use super::super::{TabEntry, TabItem};
use super::rgba;

// ─── Empty placeholder ──────────────────────────────────────────────────────

pub(super) fn render_empty_placeholder(ui: &Ui, cfg: &TabControlConfig) {
    let avail = ui.content_region_avail();
    let strings = &cfg.strings;
    // Show the MDI dashboard glyph only when the host has registered the icon
    // font; otherwise the placeholder would read `? No tabs` (M4 from session
    // 035 audit, visible in user screenshot).
    let icon: Option<&str> = if cfg.icons_available {
        Some(icons::VIEW_DASHBOARD_OUTLINE)
    } else {
        None
    };

    let icon_sz = icon.map(calc_text_size).unwrap_or([0.0, 0.0]);
    let main_sz = calc_text_size(&strings.no_tabs);
    let hint_sz = calc_text_size(&strings.empty_hint);

    let spacing = 8.0;
    // Skip the icon row in the centring math when the icon is hidden, so
    // `No tabs` lands at the visual centre (not biased upward by the gap that
    // would have hosted the missing glyph).
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

// ─── Active-tab body frame ──────────────────────────────────────────────────

/// Render the active tab's content, optionally wrapped in the inset body frame
/// (`body_inset_enabled`).
pub(super) fn render_body<T: TabItem>(cfg: &TabControlConfig, entry: &mut TabEntry<T>, ui: &Ui) {
    if !cfg.body_inset_enabled {
        entry.item.render_content(ui);
        return;
    }

    // Visible-frame layout (ASCII model from user feedback 2026-04-30):
    //
    //   [Tab strip]
    //   |-------------------------------------|   <- outer (strip_bg)
    //   ||-----------------------------------||   <- inner (body_bg child window)
    //   ||                                   ||      gap = strip_bg
    //   ||      widgets clipped here         ||      inset by `pad`
    //   |-------------------------------------|
    //
    // Outer rect is painted directly on the parent's draw list, then the
    // cursor is offset by `pad` and the host's `render_content` runs inside a
    // *real* child window sized to fit between the outer rect and `pad` on all
    // four sides. The child gives us proper clipping so text wrap, button max
    // width, etc. respect the inner bounds — a draw-list-only approach (paint
    // inner rect + cursor offset) made widgets bleed past the right edge
    // because cursor offset doesn't reduce the line width.
    let pad = cfg.body_inset;
    let cur_screen = ui.cursor_screen_pos();
    let avail_full = ui.content_region_avail();
    let outer_min = cur_screen;
    let outer_max = [cur_screen[0] + avail_full[0], cur_screen[1] + avail_full[1]];

    let inner_w = avail_full[0] - 2.0 * pad[0];
    let inner_h = avail_full[1] - 2.0 * pad[1];

    // Degenerate-size short-circuit (window narrower than `2 * pad`, etc.).
    // ImGui's `BeginChild` triggers a STATUS_STACK_BUFFER_OVERRUN panic on
    // Windows when given a 0-or-negative size — and painting the outer
    // `frame_bg` rectangle on its own (without the inner child to overlay)
    // would leave a `frame_bg` flash around `render_content`. Falling back to
    // the no-frame path keeps the host alive AND avoids the visual glitch.
    if inner_w <= 1.0 || inner_h <= 1.0 {
        entry.item.render_content(ui);
        return;
    }

    // Scope the draw-list guard: `DrawListMut` is a global mutex, and
    // `child_window().build(...)` below executes host code that itself may
    // call `ui.get_window_draw_list()` (e.g. VirtualTable row painters / host
    // overlays). Holding `draw` across that nested `build` panics with "A
    // DrawListMut is already in use!" — drop it here first.
    {
        let draw = ui.get_window_draw_list();
        // Outer fill — `frame_bg` paints the gap around the inner body.
        // Default mirrors `strip_bg` so the strip + frame read as one chrome
        // surface; hosts can recolour `frame_bg` independently without
        // touching the tab strip itself.
        draw.add_rect(outer_min, outer_max, c32(cfg.colors.frame_bg, 255))
            .filled(true)
            .build();
        // Optional active-frame border — outlined rect drawn OVER the frame
        // fill so the host gets an "active pane" cue matching the strip's
        // selected-tab hue (IDE-style highlight). Off by default; toggled via
        // `TabControlConfig::body_inset_border`.
        if cfg.body_inset_border {
            draw.add_rect(outer_min, outer_max, c32(cfg.colors.frame_border, 255))
                .filled(false)
                .thickness(cfg.body_inset_border_thickness)
                .build();
        }
    }

    // Shift cursor inward — the upcoming child_window picks its position from
    // the current cursor.
    let cur = ui.cursor_pos();
    ui.set_cursor_pos([cur[0] + pad[0], cur[1] + pad[1]]);
    let inner_size = [inner_w, inner_h];
    // Borderless child filled with `body_bg` — clips widgets to the inner
    // rect. Push WindowPadding(0,0) so the host's first widget sits flush
    // against the inner rect's top-left (host can re-add its own breathing
    // room inside `render_content` if needed).
    let _wp = ui.push_style_var(dear_imgui_rs::StyleVar::WindowPadding([0.0, 0.0]));
    let _bg = ui.push_style_color(
        dear_imgui_rs::StyleColor::ChildBg,
        rgba(cfg.colors.body_bg, 1.0),
    );
    ui.child_window("##tab_content")
        .size(inner_size)
        .border(false)
        .build(ui, || {
            entry.item.render_content(ui);
        });
}
