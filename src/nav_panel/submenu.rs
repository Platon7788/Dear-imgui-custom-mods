//! Submenu flyout — opens its own ImGui window so it captures input focus.

use dear_imgui_rs::{Condition, MouseButton, StyleColor, StyleVar, Ui, WindowFlags};

use crate::utils::color::pack_color_f32 as c32;
use crate::utils::text::calc_text_size;

use super::NavEvent;
use super::buttons::{NavButton, SubMenuItem};
use super::config::NavPanelConfig;
use super::enums::DockPosition;
use super::state::NavPanelState;
use super::theme::NavColors;

/// Position + size of the submenu trigger button (in screen coords).
#[derive(Debug, Clone, Copy)]
pub(super) struct ButtonRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Render the flyout panel for `btn`. Closes the submenu when the user
/// clicks outside both the trigger button and the flyout itself.
pub(super) fn render_submenu(
    ui: &Ui,
    cfg: &NavPanelConfig,
    btn: &NavButton,
    rect: ButtonRect,
    colors: &NavColors,
    state: &mut NavPanelState,
    events: &mut Vec<NavEvent>,
) {
    let [mx, my] = ui.io().mouse_pos();
    let clicked = ui.is_mouse_clicked(MouseButton::Left);

    let item_count = btn
        .submenu
        .iter()
        .filter(|i| matches!(i, SubMenuItem::Item { .. }))
        .count();
    let sep_count = btn
        .submenu
        .iter()
        .filter(|i| matches!(i, SubMenuItem::Separator))
        .count();
    let sm_h = item_count as f32 * cfg.submenu_item_height + sep_count as f32 * 9.0 + 8.0;
    let sm_w = cfg.submenu_min_width;

    let (sm_x, sm_y) = match cfg.position {
        DockPosition::Left => (rect.x + rect.w + 2.0, rect.y),
        DockPosition::Right => (rect.x - sm_w - 2.0, rect.y),
        DockPosition::Top => (rect.x, rect.y + rect.h + 2.0),
    };

    let _spad = ui.push_style_var(StyleVar::WindowPadding([4.0, 4.0]));
    let _srnd = ui.push_style_var(StyleVar::WindowRounding(6.0));
    let _sbrd = ui.push_style_var(StyleVar::WindowBorderSize(1.0));
    let _sbg = ui.push_style_color(StyleColor::WindowBg, colors.submenu_bg);
    let _sbc = ui.push_style_color(StyleColor::Border, colors.submenu_border);

    ui.window("##nav_submenu")
        .position([sm_x, sm_y], Condition::Always)
        .size([sm_w, sm_h], Condition::Always)
        .flags(
            WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_SCROLLBAR
                | WindowFlags::NO_COLLAPSE,
        )
        .build(|| {
            let sm_draw = ui.get_window_draw_list();
            let sm_pos = ui.window_pos();
            let mut iy = sm_pos[1] + 4.0;

            for sub_item in &btn.submenu {
                match sub_item {
                    SubMenuItem::Separator => {
                        iy += 4.0;
                        sm_draw
                            .add_line(
                                [sm_pos[0] + 8.0, iy],
                                [sm_pos[0] + sm_w - 8.0, iy],
                                c32(colors.submenu_separator),
                            )
                            .thickness(1.0)
                            .build();
                        iy += 5.0;
                    }
                    SubMenuItem::Item {
                        id,
                        label,
                        icon,
                        shortcut,
                    } => {
                        let ih = cfg.submenu_item_height;
                        let item_hov = mx >= sm_pos[0] + 4.0
                            && mx < sm_pos[0] + sm_w - 4.0
                            && my >= iy
                            && my < iy + ih;

                        if item_hov {
                            sm_draw
                                .add_rect(
                                    [sm_pos[0] + 4.0, iy],
                                    [sm_pos[0] + sm_w - 4.0, iy + ih],
                                    c32(colors.submenu_hover),
                                )
                                .filled(true)
                                .rounding(4.0)
                                .build();
                            if clicked {
                                events.push(NavEvent::SubMenuClicked(btn.id.clone(), id.clone()));
                                state.open_submenu = None;
                            }
                        }

                        let text_y = iy + (ih - calc_text_size("M")[1]) * 0.5;
                        let mut tx = sm_pos[0] + 12.0;
                        if let Some(ico) = icon {
                            sm_draw.add_text([tx, text_y], c32(colors.submenu_text), ico);
                            tx += calc_text_size(ico)[0] + 8.0;
                        }
                        sm_draw.add_text([tx, text_y], c32(colors.submenu_text), label);
                        if let Some(sc) = shortcut {
                            let [scw, _] = calc_text_size(sc);
                            let dim = [
                                colors.submenu_text[0] * 0.6,
                                colors.submenu_text[1] * 0.6,
                                colors.submenu_text[2] * 0.6,
                                colors.submenu_text[3],
                            ];
                            sm_draw.add_text([sm_pos[0] + sm_w - 12.0 - scw, text_y], c32(dim), sc);
                        }
                        iy += ih;
                    }
                }
            }
        });

    // Close on click outside both submenu and button.
    let sm_hov = mx >= sm_x && mx < sm_x + sm_w && my >= sm_y && my < sm_y + sm_h;
    let btn_hov = mx >= rect.x && mx < rect.x + rect.w && my >= rect.y && my < rect.y + rect.h;
    if clicked && !sm_hov && !btn_hov {
        state.open_submenu = None;
    }
}
