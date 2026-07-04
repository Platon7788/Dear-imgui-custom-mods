//! Render the footer bar: Cancel (flush-left) and the primary confirm button
//! (flush-right), with the filter dropdown (OpenFile) or filename input
//! (SaveFile) centered between them.

use dear_imgui_rs::{StyleVar, Ui};

use crate::utils::text::calc_text_size;
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
/// Layout mirrors `fldr.svg`: Cancel is flush to the left edge, the primary
/// confirm button flush to the right edge, and the mode-specific middle content
/// (filename input for SaveFile, filter dropdown for OpenFile) sits between
/// them. The primary button is dimmed (but keeps its slot) when the current
/// selection can't be confirmed.
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
    let filter_w = config.filter_width;
    let gap = 6.0_f32;

    let _rounding = ui.push_style_var(StyleVar::FrameRounding(4.0));

    // The row spans the full content width: Cancel flush-left, primary
    // flush-right, mode-specific middle content centered between.
    let row_x = ui.cursor_pos()[0];
    let avail = ui.content_region_avail()[0];
    let right_x = row_x + avail - btn_w;
    let mid_start = row_x + btn_w + gap;

    // ── Cancel (flush-left) ──
    {
        let label = icon_label(buf, icons::CLOSE, strings.cancel);
        with_btn_style(ui, cancel_btn(), || {
            if ui.button_with_size(label, [btn_w, btn_h]) {
                cancelled = true;
            }
        });
    }

    // ── Middle: filename (SaveFile) or filter dropdown (OpenFile) ──
    if mode == DialogMode::SaveFile {
        ui.same_line();
        ui.set_cursor_pos_x(mid_start);
        ui.text_colored(theme::TEXT_SECONDARY, strings.filename);
        ui.same_line();
        let lbl_w = calc_text_size(strings.filename)[0];
        let input_w = ((right_x - gap) - (mid_start + lbl_w + gap)).max(80.0);
        ui.set_next_item_width(input_w);
        let enter = ui
            .input_text("##filename", filename_buf)
            .enter_returns_true(true)
            .build();
        if enter {
            confirmed = true;
        }
    } else if mode != DialogMode::SelectFolder && filters.len() > 1 {
        ui.same_line();
        ui.set_cursor_pos_x(mid_start);
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
    }

    // ── Primary confirm (flush-right) ──
    ui.same_line();
    ui.set_cursor_pos_x(right_x);
    let label = icon_label(buf, confirm_icon, confirm_label);
    let _dim = (!can_confirm).then(|| ui.push_style_var(StyleVar::Alpha(0.4)));
    with_btn_style(ui, confirm_btn(), || {
        if ui.button_with_size(label, [btn_w, btn_h]) && can_confirm {
            confirmed = true;
        }
    });

    (confirmed, cancelled, action)
}
