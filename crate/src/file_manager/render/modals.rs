//! Render the overwrite-confirmation and delete-confirmation modal popups.

use std::fmt::Write;

use dear_imgui_rs::{Ui, WindowFlags};

use crate::icons;

use super::style::{cancel_btn, confirm_btn, icon_label, nav_btn, with_btn_style};
use crate::file_manager::config::FmStrings;

/// Render the overwrite-confirmation popup (SaveFile mode, target file exists).
///
/// Returns `Some(true)` = overwrite confirmed, `Some(false)` = cancelled,
/// `None` = still open / not opened this frame.
pub(crate) fn render_overwrite_confirm(
    ui: &Ui,
    strings: &FmStrings,
    should_open: &mut bool,
    buf: &mut String,
) -> Option<bool> {
    if *should_open {
        *should_open = false;
        ui.open_popup("##overwrite_confirm");
    }

    let mut result = None;
    if let Some(_tok) = ui
        .begin_modal_popup_config("##overwrite_confirm")
        .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
        .begin()
    {
        ui.text(strings.overwrite_message);
        ui.spacing();

        let label = icon_label(buf, icons::CHECK_BOLD, strings.yes);
        with_btn_style(ui, confirm_btn(), || {
            if ui.button_with_size(label, [80.0, 0.0]) {
                result = Some(true);
                ui.close_current_popup();
            }
        });
        ui.same_line();
        let label = icon_label(buf, icons::CLOSE, strings.no);
        with_btn_style(ui, cancel_btn(), || {
            if ui.button_with_size(label, [80.0, 0.0]) {
                result = Some(false);
                ui.close_current_popup();
            }
        });
    }

    result
}

/// Render the delete-confirmation popup.
///
/// Returns `Some(true)` = delete confirmed, `Some(false)` = cancelled,
/// `None` = not open this frame.
pub(crate) fn render_delete_confirm(
    ui: &Ui,
    strings: &FmStrings,
    should_open: &mut bool,
    entry_name: Option<&str>,
    buf: &mut String,
) -> Option<bool> {
    if *should_open {
        *should_open = false;
        ui.open_popup("##delete_confirm");
    }

    let mut result = None;
    if let Some(_tok) = ui
        .begin_modal_popup_config("##delete_confirm")
        .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
        .begin()
    {
        buf.clear();
        if let Some(name) = entry_name {
            let _ = write!(buf, "{} \"{}\"?", strings.confirm_delete_message, name);
        } else {
            let _ = write!(buf, "{}?", strings.confirm_delete_message);
        }
        ui.text(buf.as_str());
        ui.spacing();

        let label = icon_label(buf, icons::TRASH_CAN_OUTLINE, strings.yes);
        with_btn_style(ui, cancel_btn(), || {
            if ui.button_with_size(label, [80.0, 0.0]) {
                result = Some(true);
                ui.close_current_popup();
            }
        });
        ui.same_line();
        let label = icon_label(buf, icons::CLOSE, strings.no);
        with_btn_style(ui, nav_btn(), || {
            if ui.button_with_size(label, [80.0, 0.0]) {
                result = Some(false);
                ui.close_current_popup();
            }
        });
    }

    result
}
