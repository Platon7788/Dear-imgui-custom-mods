//! Render the navigation toolbar: Back / Forward / Up / New Folder / New File / Refresh / Hidden.

use std::fmt::Write;

use dear_imgui_rs::{StyleVar, Ui};

use crate::{icons, theme};

use super::style::{
    btn_colors, cancel_btn, confirm_btn, icon_label, label_with_id, nav_btn, with_btn_style,
};
use crate::file_manager::actions::Action;
use crate::file_manager::config::{FileManagerConfig, FmStrings};

/// Borrow-bundle for [`render_toolbar`] — replaces the 11-argument signature
/// with a single context struct (mirrors [`TableCtx`](super::table::TableCtx)).
pub(crate) struct ToolbarCtx<'a> {
    pub strings: &'a FmStrings,
    pub has_parent: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub show_new_folder: &'a mut bool,
    pub new_folder_buf: &'a mut String,
    pub show_new_file: &'a mut bool,
    pub new_file_buf: &'a mut String,
    pub show_hidden: bool,
    pub config: &'a FileManagerConfig,
    pub buf: &'a mut String,
}

/// Render the navigation toolbar.
///
/// Disabled buttons are shown as grayed-out text. The "New Folder" / "New File"
/// buttons toggle inline input fields with Create/Cancel buttons.
/// Only one inline input can be open at a time.
pub(crate) fn render_toolbar(ui: &Ui, ctx: ToolbarCtx<'_>) -> Option<Action> {
    let ToolbarCtx {
        strings,
        has_parent,
        can_back,
        can_forward,
        show_new_folder,
        new_folder_buf,
        show_new_file,
        new_file_buf,
        show_hidden,
        config,
        buf,
    } = ctx;

    let mut action = None;
    let _spacing = ui.push_style_var(StyleVar::ItemSpacing([6.0, 4.0]));

    // Back button
    if can_back {
        let label = icon_label(buf, icons::ARROW_LEFT, strings.back);
        with_btn_style(ui, nav_btn(), || {
            if ui.button(label) {
                action = Some(Action::GoBack);
            }
        });
    } else {
        let label = icon_label(buf, icons::ARROW_LEFT, strings.back);
        ui.text_disabled(label);
    }
    ui.same_line();

    // Forward button
    if can_forward {
        let label = icon_label(buf, icons::ARROW_RIGHT, strings.forward);
        with_btn_style(ui, nav_btn(), || {
            if ui.button(label) {
                action = Some(Action::GoForward);
            }
        });
    } else {
        let label = icon_label(buf, icons::ARROW_RIGHT, strings.forward);
        ui.text_disabled(label);
    }
    ui.same_line();

    // Up button
    if has_parent {
        let label = icon_label(buf, icons::ARROW_UP, strings.up);
        with_btn_style(ui, nav_btn(), || {
            if ui.button(label) {
                action = Some(Action::GoParent);
            }
        });
    } else {
        let label = icon_label(buf, icons::ARROW_UP, strings.up);
        ui.text_disabled(label);
    }
    ui.same_line();

    // New folder button
    {
        let nf_colors = confirm_btn();
        let label = icon_label(buf, icons::FOLDER_PLUS, strings.new_folder);
        with_btn_style(ui, nf_colors, || {
            if ui.button(label) {
                let opening = !*show_new_folder;
                *show_new_folder = opening;
                if opening {
                    *show_new_file = false;
                    new_folder_buf.clear();
                }
            }
        });
    }
    ui.same_line();

    // New file button
    {
        let nf_colors = btn_colors(theme::ACCENT, theme::ACCENT_HOVER, theme::ACCENT_ACTIVE);
        let label = icon_label(buf, icons::FILE_PLUS, strings.new_file);
        with_btn_style(ui, nf_colors, || {
            if ui.button(label) {
                let opening = !*show_new_file;
                *show_new_file = opening;
                if opening {
                    *show_new_folder = false;
                    new_file_buf.clear();
                }
            }
        });
    }
    ui.same_line();

    // Refresh button
    {
        buf.clear();
        let _ = write!(buf, "{}##refresh", icons::REFRESH);
        with_btn_style(ui, nav_btn(), || {
            if ui.button(buf.as_str()) {
                action = Some(Action::Refresh);
            }
        });
    }
    ui.same_line();

    // Hidden files toggle
    {
        let icon = if show_hidden {
            icons::EYE
        } else {
            icons::EYE_OFF_OUTLINE
        };
        let label = icon_label(buf, icon, strings.show_hidden);
        let colors = if show_hidden {
            btn_colors(theme::ACCENT, theme::ACCENT_HOVER, theme::ACCENT_ACTIVE)
        } else {
            nav_btn()
        };
        with_btn_style(ui, colors, || {
            if ui.button(label) {
                action = Some(Action::ToggleHidden);
            }
        });
    }

    // New folder inline input
    if *show_new_folder {
        let input_w = if config.inline_input_width > 0.0 {
            config.inline_input_width
        } else {
            ui.content_region_avail()[0].min(300.0)
        };
        ui.set_next_item_width(input_w);
        if !ui.is_any_item_active() {
            ui.set_keyboard_focus_here();
        }
        let enter = ui
            .input_text("##newfolder", new_folder_buf)
            .enter_returns_true(true)
            .build();
        ui.same_line();

        with_btn_style(ui, confirm_btn(), || {
            let lbl = label_with_id(buf, strings.create, "nf_create");
            if (ui.button(lbl) || enter) && !new_folder_buf.is_empty() {
                action = Some(Action::CreateFolder(new_folder_buf.clone()));
            }
        });
        ui.same_line();
        with_btn_style(ui, cancel_btn(), || {
            let lbl = label_with_id(buf, strings.cancel, "nf_cancel");
            if ui.button(lbl) {
                *show_new_folder = false;
                new_folder_buf.clear();
            }
        });
    }

    // New file inline input
    if *show_new_file {
        let input_w = if config.inline_input_width > 0.0 {
            config.inline_input_width
        } else {
            ui.content_region_avail()[0].min(300.0)
        };
        ui.set_next_item_width(input_w);
        if !ui.is_any_item_active() {
            ui.set_keyboard_focus_here();
        }
        let enter = ui
            .input_text("##newfile", new_file_buf)
            .enter_returns_true(true)
            .build();
        ui.same_line();

        with_btn_style(ui, confirm_btn(), || {
            let lbl = label_with_id(buf, strings.create, "nfile_create");
            if (ui.button(lbl) || enter) && !new_file_buf.is_empty() {
                action = Some(Action::CreateFile(new_file_buf.clone()));
            }
        });
        ui.same_line();
        with_btn_style(ui, cancel_btn(), || {
            let lbl = label_with_id(buf, strings.cancel, "nfile_cancel");
            if ui.button(lbl) {
                *show_new_file = false;
                new_file_buf.clear();
            }
        });
    }

    action
}
