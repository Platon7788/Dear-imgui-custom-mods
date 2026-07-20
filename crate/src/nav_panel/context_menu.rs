//! Right-click reposition menu — a small flyout that lets the user re-dock
//! the panel (Left / Right / Top).
//!
//! It opens its own ImGui window (like [`submenu`](super::submenu)) so it
//! captures input focus and lives naturally in the popup layer above the
//! panel's background/foreground draw list. Selection is delivered as
//! [`NavEvent::PositionChangeRequested`]; the host applies it by rebuilding
//! the config with the new [`DockPosition`] (the panel never mutates its own
//! immutable config).
//!
//! Each entry carries a crate-wide [`themed_tooltip`](crate::utils::themed_tooltip)
//! on hover and a tiny dock-position diagram glyph so the direction reads at
//! a glance regardless of the active font's glyph coverage.

use dear_imgui_rs::{Condition, MouseButton, StyleColor, StyleVar, Ui, WindowFlags};

use crate::utils::color::pack_color_f32 as c32;
use crate::utils::text::line_height;

use super::NavEvent;
use super::config::NavPanelConfig;
use super::enums::DockPosition;
use super::state::NavPanelState;
use super::theme::NavColors;

/// Width of the dock-position diagram glyph drawn at the left of each row.
const GLYPH_W: f32 = 16.0;
/// Height of the dock-position diagram glyph.
const GLYPH_H: f32 = 12.0;

/// Render the reposition flyout anchored near `anchor` (screen-space cursor
/// position captured on the right-click). Closes on selection or on a click
/// outside the flyout.
pub(super) fn render_reposition_menu(
    ui: &Ui,
    cfg: &NavPanelConfig,
    anchor: [f32; 2],
    colors: &NavColors,
    state: &mut NavPanelState,
    events: &mut Vec<NavEvent>,
) {
    let s = crate::i18n::nav_panel::strings(cfg.locale);
    let entries = [
        (DockPosition::Left, s.dock_left, s.dock_left_hint),
        (DockPosition::Right, s.dock_right, s.dock_right_hint),
        (DockPosition::Top, s.dock_top, s.dock_top_hint),
    ];

    let [mx, my] = ui.io().mouse_pos();
    let clicked = ui.is_mouse_clicked(MouseButton::Left);

    // ── Geometry ─────────────────────────────────────────────────────────
    let pad = 4.0_f32;
    let item_h = cfg.submenu_item_height;
    let header_h = line_height(ui) + 6.0;
    let sep_h = 9.0_f32;
    let sm_w = cfg.submenu_min_width;
    let sm_h = pad + header_h + sep_h + entries.len() as f32 * item_h + pad;

    // Clamp inside the viewport so the flyout never spills off-screen.
    let [vw, vh] = ui.io().display_size();
    let sm_x = anchor[0].min((vw - sm_w - 4.0).max(0.0)).max(0.0);
    let sm_y = anchor[1].min((vh - sm_h - 4.0).max(0.0)).max(0.0);

    let _spad = ui.push_style_var(StyleVar::WindowPadding([pad, pad]));
    let _srnd = ui.push_style_var(StyleVar::WindowRounding(6.0));
    let _sbrd = ui.push_style_var(StyleVar::WindowBorderSize(1.0));
    let _sbg = ui.push_style_color(StyleColor::WindowBg, colors.submenu_bg);
    let _sbc = ui.push_style_color(StyleColor::Border, colors.submenu_border);

    ui.window("##nav_reposition_menu")
        .position([sm_x, sm_y], Condition::Always)
        .size([sm_w, sm_h], Condition::Always)
        .flags(
            WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_SCROLLBAR
                | WindowFlags::NO_COLLAPSE
                | WindowFlags::NO_SAVED_SETTINGS,
        )
        .build(|| {
            let draw = ui.get_window_draw_list();
            let pos = ui.window_pos();
            let mut iy = pos[1] + pad;

            // Header — dimmed title.
            let dim = dim_color(colors.submenu_text, 0.65);
            let text_y = iy + (header_h - line_height(ui)) * 0.5;
            draw.add_text([pos[0] + 12.0, text_y], c32(dim), s.position_title);
            iy += header_h;

            // Separator.
            let sy = iy + 4.0;
            draw.add_line(
                [pos[0] + 8.0, sy],
                [pos[0] + sm_w - 8.0, sy],
                c32(colors.submenu_separator),
            )
            .thickness(1.0)
            .build();
            iy += sep_h;

            // Position rows.
            for (dock, label, hint) in entries {
                let is_current = cfg.position == dock;
                let row_hov =
                    mx >= pos[0] + pad && mx < pos[0] + sm_w - pad && my >= iy && my < iy + item_h;

                if row_hov {
                    draw.add_rect(
                        [pos[0] + pad, iy],
                        [pos[0] + sm_w - pad, iy + item_h],
                        c32(colors.submenu_hover),
                    )
                    .filled(true)
                    .rounding(4.0)
                    .build();
                    crate::utils::themed_tooltip(ui, || ui.text(hint));
                    if clicked {
                        events.push(NavEvent::PositionChangeRequested(dock));
                        state.reposition_anchor = None;
                    }
                }

                // Accent for the currently-active position: a left bar plus a
                // tinted label, so the menu shows "where the panel is now".
                let accent = colors.indicator;
                if is_current {
                    draw.add_rect(
                        [pos[0] + pad, iy + 5.0],
                        [pos[0] + pad + 2.0, iy + item_h - 5.0],
                        c32(accent),
                    )
                    .filled(true)
                    .rounding(1.0)
                    .build();
                }

                // Dock-position diagram glyph.
                let glyph_x = pos[0] + 12.0;
                let glyph_y = iy + (item_h - GLYPH_H) * 0.5;
                let glyph_col = if is_current { accent } else { dim };
                draw_dock_glyph(&draw, [glyph_x, glyph_y], dock, glyph_col);

                // Label.
                let label_col = if is_current {
                    accent
                } else {
                    colors.submenu_text
                };
                let ly = iy + (item_h - line_height(ui)) * 0.5;
                draw.add_text([glyph_x + GLYPH_W + 8.0, ly], c32(label_col), label);

                iy += item_h;
            }
        });

    // Close on a click that lands outside the flyout.
    let over_menu = mx >= sm_x && mx < sm_x + sm_w && my >= sm_y && my < sm_y + sm_h;
    if clicked && !over_menu {
        state.reposition_anchor = None;
    }
}

/// Draw a tiny "window with a docked edge" diagram at `origin` (top-left).
/// A hollow rectangle stands for the workspace; a filled strip on the edge
/// matching `dock` shows where the panel sits. Font-independent, so it reads
/// correctly under any glyph range.
fn draw_dock_glyph(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    origin: [f32; 2],
    dock: DockPosition,
    col: [f32; 4],
) {
    let [x, y] = origin;
    let color = c32(col);
    // Workspace outline.
    draw.add_rect([x, y], [x + GLYPH_W, y + GLYPH_H], color)
        .thickness(1.0)
        .rounding(1.5)
        .build();
    // Docked strip (≈30 % of the relevant dimension).
    let strip = 4.0_f32;
    let (a, b) = match dock {
        DockPosition::Left => ([x + 1.0, y + 1.0], [x + strip, y + GLYPH_H - 1.0]),
        DockPosition::Right => (
            [x + GLYPH_W - strip, y + 1.0],
            [x + GLYPH_W - 1.0, y + GLYPH_H - 1.0],
        ),
        DockPosition::Top => ([x + 1.0, y + 1.0], [x + GLYPH_W - 1.0, y + strip]),
    };
    draw.add_rect(a, b, color)
        .filled(true)
        .rounding(1.0)
        .build();
}

/// Multiply an RGB colour's brightness by `f`, keeping alpha — used for the
/// dimmed header / inactive glyph tint.
fn dim_color(c: [f32; 4], f: f32) -> [f32; 4] {
    [c[0] * f, c[1] * f, c[2] * f, c[3]]
}
