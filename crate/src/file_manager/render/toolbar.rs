//! Render the navigation toolbar: New Folder / New File / Hidden (left) and the
//! Up / Refresh / Back / Forward navigation cluster (right-aligned, icon-only).

use std::fmt::Write;

use dear_imgui_rs::{StyleVar, Ui};

use crate::utils::text::calc_text_size;
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

/// Horizontal spacing between toolbar items (matches the pushed `ItemSpacing`).
const TOOLBAR_SPACING_X: f32 = 6.0;

/// Render the navigation toolbar.
///
/// Layout mirrors `fldr.svg`: the file-action buttons (New Folder, New File,
/// Hidden) sit on the left; the navigation cluster (Up, Refresh, Back, Forward)
/// is icon-only, right-aligned, with hover tooltips. Disabled nav buttons keep
/// their footprint (dimmed via `Alpha`) so the right-alignment stays stable.
/// The inline New Folder / New File name inputs open on their own line below.
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
    let _spacing = ui.push_style_var(StyleVar::ItemSpacing([TOOLBAR_SPACING_X, 4.0]));

    // ── Left: file-action buttons ───────────────────────────────────────
    {
        let label = icon_label(buf, icons::FOLDER_PLUS, strings.new_folder);
        with_btn_style(ui, confirm_btn(), || {
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
    {
        let label = icon_label(buf, icons::FILE_PLUS, strings.new_file);
        let colors = btn_colors(theme::ACCENT, theme::ACCENT_HOVER, theme::ACCENT_ACTIVE);
        with_btn_style(ui, colors, || {
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

    // ── Right: navigation cluster (icon-only, right-aligned) ─────────────
    {
        let nav_icons = [
            icons::ARROW_UP,
            icons::REFRESH,
            icons::ARROW_LEFT,
            icons::ARROW_RIGHT,
        ];
        // Exact button width = glyph width + 2×FramePadding.x (read live, since
        // the toolbar never pushes its own FramePadding and the host theme's
        // value drives it). An estimate here would clip the last button off the
        // right edge on themes with larger padding.
        let fp_x = ui.clone_style().frame_padding()[0];
        let mut cluster_w = TOOLBAR_SPACING_X * (nav_icons.len() as f32 - 1.0);
        for ic in nav_icons {
            cluster_w += calc_text_size(ic)[0] + fp_x * 2.0;
        }

        ui.same_line();
        let cur_x = ui.cursor_pos()[0];
        let avail = ui.content_region_avail()[0];
        ui.set_cursor_pos_x((cur_x + avail - cluster_w).max(cur_x));

        let mut nav = |icon: &str, id: &str, tip: &str, enabled: bool, act: Action| {
            buf.clear();
            let _ = write!(buf, "{icon}##{id}");
            // Disabled: dim + freeze hover/active to the base color so the
            // button doesn't light up under the cursor (the `&& enabled` guard
            // already swallows the click). The Alpha guard is scoped to the
            // button block so it doesn't also fade the tooltip below.
            let colors = if enabled {
                nav_btn()
            } else {
                let base = nav_btn()[0];
                [base, base, base]
            };
            {
                let _dim = (!enabled).then(|| ui.push_style_var(StyleVar::Alpha(0.4)));
                with_btn_style(ui, colors, || {
                    if ui.button(buf.as_str()) && enabled {
                        action = Some(act);
                    }
                });
            }
            if ui.is_item_hovered() {
                crate::utils::themed_tooltip(ui, || ui.text(tip));
            }
        };

        nav(
            icons::ARROW_UP,
            "fm_up",
            strings.up,
            has_parent,
            Action::GoParent,
        );
        ui.same_line();
        nav(
            icons::REFRESH,
            "fm_refresh",
            strings.refresh,
            true,
            Action::Refresh,
        );
        ui.same_line();
        nav(
            icons::ARROW_LEFT,
            "fm_back",
            strings.back,
            can_back,
            Action::GoBack,
        );
        ui.same_line();
        nav(
            icons::ARROW_RIGHT,
            "fm_forward",
            strings.forward,
            can_forward,
            Action::GoForward,
        );
    }

    // ── Inline New Folder / New File inputs (own line, below the row) ────
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
