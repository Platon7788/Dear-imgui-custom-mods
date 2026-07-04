//! Render the footer bar: filter dropdown, filename input (SaveFile), Confirm/Cancel buttons.

use dear_imgui_rs::{StyleVar, Ui};

use crate::{icons, theme};

use super::style::{cancel_btn, confirm_btn, icon_label, with_btn_style};
use crate::file_manager::actions::Action;
use crate::file_manager::config::{DialogMode, FileFilter, FileManagerConfig, FmStrings};
use crate::file_manager::entry::FsEntry;

/// Borrow-bundle for [`render_footer`] — replaces the 10-argument signature
/// with a single context struct (mirrors [`TableCtx`](super::table::TableCtx)).
pub(crate) struct FooterCtx<'a> {
    pub strings: &'a FmStrings,
    pub mode: DialogMode,
    pub entries: &'a [FsEntry],
    pub selected_indices: &'a [usize],
    pub filename_buf: &'a mut String,
    pub filters: &'a [FileFilter],
    pub active_filter: usize,
    pub config: &'a FileManagerConfig,
    pub buf: &'a mut String,
}

/// Render the footer.
///
/// In SaveFile mode the filename input and buttons share a single row:
/// `[Filename: ___________] [Save] [Cancel]`
///
/// Returns `(confirmed, cancelled, filter_action)`.
pub(crate) fn render_footer(ui: &Ui, ctx: FooterCtx<'_>) -> (bool, bool, Option<Action>) {
    let FooterCtx {
        strings,
        mode,
        entries,
        selected_indices,
        filename_buf,
        filters,
        active_filter,
        config,
        buf,
    } = ctx;

    let mut confirmed = false;
    let mut cancelled = false;
    let mut action = None;

    let (confirm_label, confirm_icon, can_confirm) = match mode {
        DialogMode::SelectFolder => (strings.select_folder, icons::CHECK_BOLD, true),
        DialogMode::OpenFile => {
            let has_file = selected_indices
                .iter()
                .any(|&i| entries.get(i).is_some_and(|e| !e.is_dir));
            (strings.open, icons::CHECK_BOLD, has_file)
        }
        DialogMode::SaveFile => (
            strings.save,
            icons::CONTENT_SAVE,
            !filename_buf.trim().is_empty(),
        ),
    };

    let btn_w = config.button_width;
    let btn_h = config.button_height;
    let gap = 6.0_f32;
    let filter_w = config.filter_width;

    let _rounding = ui.push_style_var(StyleVar::FrameRounding(4.0));

    // ── Footer layout ──
    //
    //   SelectFolder / OpenFile (no filter):
    //     |       [Confirm]       |        [Cancel]       |
    //     ^---- half ----^                ^---- half ----^
    //     compact buttons centred in their half of the row
    //
    //   OpenFile + filter:
    //     [Filter ▼]   [Confirm]   [Cancel]
    //     compact buttons after the filter combo
    //
    //   SaveFile:
    //     [Filename: ____________]  [Confirm]  [Cancel]
    //     filename input flexes; buttons compact at the trailing edge
    //
    let has_filter = mode != DialogMode::SelectFolder && filters.len() > 1;

    if mode == DialogMode::SaveFile {
        ui.text_colored(theme::TEXT_SECONDARY, strings.filename);
        ui.same_line();

        let avail = ui.content_region_avail()[0];
        let reserved = btn_w * 2.0 + gap * 3.0;
        let input_w = (avail - reserved).max(120.0);
        ui.set_next_item_width(input_w);
        let enter = ui
            .input_text("##filename", filename_buf)
            .enter_returns_true(true)
            .build();
        if enter {
            confirmed = true;
        }
        ui.same_line_with_spacing(0.0, gap);
        render_btn_pair(
            ui,
            buf,
            &mut Pair {
                confirm_label,
                confirm_icon,
                can_confirm,
                cancel_text: strings.cancel,
                btn_w,
                btn_h,
                gap,
                confirmed: &mut confirmed,
                cancelled: &mut cancelled,
            },
        );
    } else if has_filter {
        ui.set_next_item_width(filter_w);
        let preview = if filters[active_filter].extensions.is_empty() {
            strings.all_files
        } else {
            &filters[active_filter].label
        };
        if let Some(_tok) = ui.begin_combo("##filter", preview) {
            for (i, filter) in filters.iter().enumerate() {
                let sel = i == active_filter;
                let display = if filter.extensions.is_empty() {
                    strings.all_files
                } else {
                    &filter.label
                };
                if ui.selectable_config(display).selected(sel).build() && active_filter != i {
                    action = Some(Action::SelectFilter(i));
                }
            }
        }
        ui.same_line_with_spacing(0.0, gap);
        render_btn_pair(
            ui,
            buf,
            &mut Pair {
                confirm_label,
                confirm_icon,
                can_confirm,
                cancel_text: strings.cancel,
                btn_w,
                btn_h,
                gap,
                confirmed: &mut confirmed,
                cancelled: &mut cancelled,
            },
        );
    } else {
        // SelectFolder / OpenFile-without-filter: compact buttons centred in
        // their half of the footer row.
        let cursor_start = ui.cursor_pos();
        let avail = ui.content_region_avail()[0];
        let half_w = avail * 0.5;
        let pad_in_half = ((half_w - btn_w) * 0.5).max(0.0);

        // Confirm button — centred in left half.
        ui.set_cursor_pos([cursor_start[0] + pad_in_half, cursor_start[1]]);
        if can_confirm {
            let label = icon_label(buf, confirm_icon, confirm_label);
            with_btn_style(ui, confirm_btn(), || {
                if ui.button_with_size(label, [btn_w, btn_h]) {
                    confirmed = true;
                }
            });
        } else {
            let label = icon_label(buf, confirm_icon, confirm_label);
            let _disabled = ui.push_style_var(StyleVar::Alpha(0.4));
            with_btn_style(ui, confirm_btn(), || {
                ui.button_with_size(label, [btn_w, btn_h]);
            });
        }

        // Cancel button — centred in right half.
        ui.same_line();
        ui.set_cursor_pos([cursor_start[0] + half_w + pad_in_half, cursor_start[1]]);
        let label = icon_label(buf, icons::CLOSE, strings.cancel);
        with_btn_style(ui, cancel_btn(), || {
            if ui.button_with_size(label, [btn_w, btn_h]) {
                cancelled = true;
            }
        });
    }

    (confirmed, cancelled, action)
}

/// Render the trailing `[Confirm] [Cancel]` button pair as a compact group
/// (used by SaveFile and OpenFile+filter layouts where a leading element has
/// already consumed the left side of the row).
struct Pair<'a> {
    confirm_label: &'a str,
    confirm_icon: &'a str,
    can_confirm: bool,
    cancel_text: &'a str,
    btn_w: f32,
    btn_h: f32,
    gap: f32,
    confirmed: &'a mut bool,
    cancelled: &'a mut bool,
}

fn render_btn_pair(ui: &Ui, buf: &mut String, p: &mut Pair<'_>) {
    if p.can_confirm {
        let label = icon_label(buf, p.confirm_icon, p.confirm_label);
        let mut clicked = false;
        with_btn_style(ui, confirm_btn(), || {
            clicked = ui.button_with_size(label, [p.btn_w, p.btn_h]);
        });
        if clicked {
            *p.confirmed = true;
        }
    } else {
        let label = icon_label(buf, p.confirm_icon, p.confirm_label);
        let _disabled = ui.push_style_var(StyleVar::Alpha(0.4));
        with_btn_style(ui, confirm_btn(), || {
            ui.button_with_size(label, [p.btn_w, p.btn_h]);
        });
    }

    ui.same_line_with_spacing(0.0, p.gap);
    let label = icon_label(buf, icons::CLOSE, p.cancel_text);
    let mut clicked = false;
    with_btn_style(ui, cancel_btn(), || {
        clicked = ui.button_with_size(label, [p.btn_w, p.btn_h]);
    });
    if clicked {
        *p.cancelled = true;
    }
}
