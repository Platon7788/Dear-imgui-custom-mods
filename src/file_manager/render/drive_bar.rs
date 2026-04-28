//! Render a horizontal row of drive buttons (Windows: `C:\`, `D:\`; Unix: `/`).

use dear_imgui_rs::{StyleColor, StyleVar, Ui};

use crate::{icons, theme};

use super::style::{btn_colors, icon_label, nav_btn, with_btn_style};
use crate::file_manager::actions::Action;

/// Render the drive selector. The current drive is highlighted with accent
/// colors. Clicking a drive navigates to its root.
pub(crate) fn render_drive_bar(
    ui: &Ui,
    drives: &[String],
    current_drive: Option<char>,
    buf: &mut String,
) -> Option<Action> {
    let mut action = None;
    let _spacing = ui.push_style_var(StyleVar::ItemSpacing([4.0, 4.0]));
    let _rounding = ui.push_style_var(StyleVar::FrameRounding(4.0));

    for drive in drives {
        let drive_letter = drive.chars().next().unwrap_or('?');
        let is_current = current_drive == Some(drive_letter);

        let colors = if is_current {
            btn_colors(theme::ACCENT, theme::ACCENT_HOVER, theme::ACCENT_ACTIVE)
        } else {
            nav_btn()
        };

        let label = icon_label(buf, icons::HARDDISK, drive);
        with_btn_style(ui, colors, || {
            if is_current {
                let _tc = ui.push_style_color(StyleColor::Text, [0.90, 0.94, 1.00, 1.0]);
                if ui.button(label) {
                    action = Some(Action::NavigateTo(std::path::PathBuf::from(drive.as_str())));
                }
            } else if ui.button(label) {
                action = Some(Action::NavigateTo(std::path::PathBuf::from(drive.as_str())));
            }
        });
        ui.same_line();
    }
    ui.new_line();
    action
}
