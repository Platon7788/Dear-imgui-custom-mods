//! Render the main file listing as a 4-column ImGui Table with virtualization,
//! sortable headers, click/double-click selection, and keyboard navigation.

use dear_imgui_rs::{
    ListClipper, MouseButton, SelectableFlags, TableColumnFlags, TableFlags, Ui,
};

use crate::{icons, theme};

use super::icon_map::file_icon_for_ext;
use super::style::{CLR_FOLDER_TEXT, icon_label};
use crate::file_manager::actions::Action;
use crate::file_manager::config::{DialogMode, FileManagerConfig, FmStrings};
use crate::file_manager::entry::{FsEntry, SortColumn, SortOrder};

/// Combined result from [`render_file_table`]: a deferred action and/or a delete request.
pub(crate) struct FileTableResult {
    /// Deferred navigation/selection action to apply after the frame.
    pub action: Option<Action>,
    /// Index of entry the user wants to delete (needs confirmation first).
    pub request_delete: Option<usize>,
}

/// Bundle of mutable / shared references needed to render the file table.
///
/// P2-3: replaces 17 individual function arguments with a single context
/// borrow-bundle. Build it from [`FileManager`](crate::file_manager::FileManager)
/// fields at the call site (Rust's split-borrow rules let you take separate
/// `&mut` references to disjoint struct fields in one expression).
pub(crate) struct TableCtx<'a> {
    pub entries: &'a [FsEntry],
    pub selected_indices: &'a mut Vec<usize>,
    pub mode: DialogMode,
    pub multi_select: bool,
    pub filename_buf: &'a mut String,
    pub strings: &'a FmStrings,
    pub has_error: bool,
    pub sort_column: &'a mut SortColumn,
    pub sort_order: &'a mut SortOrder,
    pub rename_index: &'a mut Option<usize>,
    pub rename_buf: &'a mut String,
    pub context_menu_target: &'a mut Option<usize>,
    pub last_click_index: &'a mut Option<usize>,
    pub scroll_to_index: &'a mut Option<usize>,
    pub config: &'a FileManagerConfig,
    pub buf: &'a mut String,
}

/// Render the main file listing.
///
/// ## Columns
///
/// | # | Name | Width | Content |
/// |---|------|-------|---------|
/// | 0 | Name | stretch | icon + filename (selectable spanning all columns) |
/// | 1 | Size | 80px | pre-computed human-readable size |
/// | 2 | Date Modified | 140px | pre-computed "YYYY-MM-DD HH:MM" |
/// | 3 | Type | 70px | extension or "Folder" |
///
/// ## Features
///
/// - **ListClipper**: only visible rows are rendered (virtualization)
/// - **Sortable headers**: click to sort, uses `table_get_sort_specs()`
/// - **Click/Ctrl+Click/Shift+Click**: single, multi, or range select
/// - **Double-click**: navigate into directory or confirm file selection
/// - **Keyboard**: Up/Down/PageUp/PageDown/Home/End/Enter/Backspace
pub(crate) fn render_file_table(ui: &Ui, ctx: TableCtx<'_>) -> FileTableResult {
    let TableCtx {
        entries,
        selected_indices,
        mode,
        multi_select,
        filename_buf,
        strings,
        has_error,
        sort_column,
        sort_order,
        rename_index,
        rename_buf,
        context_menu_target,
        last_click_index,
        scroll_to_index,
        config,
        buf,
    } = ctx;

    let mut result = FileTableResult {
        action: None,
        request_delete: None,
    };

    // P0-1: clamp stale selection indices left over from a previous directory.
    selected_indices.retain(|&i| i < entries.len());

    let options = dear_imgui_rs::TableOptions::new()
        .flags(
            TableFlags::RESIZABLE
                | TableFlags::SORTABLE
                | TableFlags::ROW_BG
                | TableFlags::SCROLL_Y
                | TableFlags::BORDERS_INNER_H
                | TableFlags::BORDERS_OUTER_H
                | TableFlags::BORDERS_OUTER_V,
        )
        .sizing_policy(dear_imgui_rs::TableSizingPolicy::FixedFit);

    // Dynamic column count based on config
    let col_count = 1
        + config.show_column_size as usize
        + config.show_column_date as usize
        + config.show_column_type as usize;

    let _table = match ui.begin_table_with_flags("##file_table", col_count, options) {
        Some(t) => t,
        None => return result,
    };

    // Column user IDs — used in the sort handler below to identify which
    // column was clicked without depending on column ordering / visibility.
    const COL_ID_NAME: u32 = 1;
    const COL_ID_SIZE: u32 = 2;
    const COL_ID_DATE: u32 = 3;
    const COL_ID_TYPE: u32 = 4;

    // Column setup — Name always present, others optional
    ui.table_setup_column(
        strings.col_name,
        TableColumnFlags::PREFER_SORT_ASCENDING,
        Some(dear_imgui_rs::TableColumnWidth::Stretch(0.0)),
        Some(dear_imgui_rs::Id::from(COL_ID_NAME)),
    );
    if config.show_column_size {
        ui.table_setup_column(
            strings.col_size,
            TableColumnFlags::NONE,
            Some(dear_imgui_rs::TableColumnWidth::Fixed(80.0)),
            Some(dear_imgui_rs::Id::from(COL_ID_SIZE)),
        );
    }
    if config.show_column_date {
        ui.table_setup_column(
            strings.col_date,
            TableColumnFlags::NONE,
            Some(dear_imgui_rs::TableColumnWidth::Fixed(140.0)),
            Some(dear_imgui_rs::Id::from(COL_ID_DATE)),
        );
    }
    if config.show_column_type {
        ui.table_setup_column(
            strings.col_type,
            TableColumnFlags::NONE,
            Some(dear_imgui_rs::TableColumnWidth::Fixed(70.0)),
            Some(dear_imgui_rs::Id::from(COL_ID_TYPE)),
        );
    }
    ui.table_setup_scroll_freeze(0, 1);
    ui.table_headers_row();

    // Sort handling
    if let Some(mut specs) = ui.table_get_sort_specs()
        && specs.is_dirty()
    {
        if let Some(s) = specs.iter().next() {
            let new_col = match s.column_user_id.map(dear_imgui_rs::Id::raw) {
                Some(id) if id == COL_ID_SIZE => SortColumn::Size,
                Some(id) if id == COL_ID_DATE => SortColumn::DateModified,
                Some(id) if id == COL_ID_TYPE => SortColumn::Type,
                _ => SortColumn::Name,
            };
            let new_order = if s.sort_direction == dear_imgui_rs::SortDirection::Ascending {
                SortOrder::Ascending
            } else {
                SortOrder::Descending
            };
            *sort_column = new_col;
            *sort_order = new_order;
            result.action = Some(Action::Resort);
        }
        specs.clear_dirty();
    }

    if entries.is_empty() && !has_error {
        ui.table_next_row();
        ui.table_next_column();
        ui.text_disabled(strings.empty_parens);
    } else {
        // ListClipper for virtualization
        let clip = ListClipper::new(entries.len());
        let tok = clip.begin(ui);

        for row_idx in tok.iter() {
            let idx = row_idx;
            let e = &entries[idx];
            let is_selected = selected_indices.contains(&idx);
            let is_renaming = *rename_index == Some(idx);

            ui.table_next_row();

            // Scroll to this row if requested (keyboard nav or type-to-search)
            if *scroll_to_index == Some(idx) {
                ui.set_scroll_here_y(0.5);
                *scroll_to_index = None;
            }

            // Column 0: Name with icon (selectable spanning all columns)
            ui.table_next_column();
            let _row_id = ui.push_id(idx);

            // Determine file icon
            let (file_icon, file_icon_color) = if e.is_dir {
                (icons::FOLDER, theme::WARNING)
            } else if let Some(f) = config.icon_override
                && let Some(result) = f(&e.extension)
            {
                result
            } else {
                file_icon_for_ext(&e.extension)
            };

            if is_renaming {
                // Inline rename input
                ui.text_colored(file_icon_color, file_icon);
                ui.same_line_with_spacing(0.0, 4.0);
                let rename_w = if config.inline_input_width > 0.0 {
                    ui.content_region_avail()[0].min(config.inline_input_width.max(200.0))
                } else {
                    ui.content_region_avail()[0].min(300.0)
                };
                ui.set_next_item_width(rename_w);
                if !ui.is_any_item_active() {
                    ui.set_keyboard_focus_here();
                }
                let enter = ui
                    .input_text("##rename", rename_buf)
                    .enter_returns_true(true)
                    .build();

                if enter && !rename_buf.is_empty() {
                    result.action = Some(Action::RenameEntry {
                        index: idx,
                        new_name: rename_buf.clone(),
                    });
                }
            } else {
                if ui
                    .selectable_config("##sel")
                    .flags(
                        SelectableFlags::SPAN_ALL_COLUMNS
                            | SelectableFlags::ALLOW_DOUBLE_CLICK
                            | SelectableFlags::ALLOW_OVERLAP,
                    )
                    .selected(is_selected)
                    .build()
                {
                    // Selection logic
                    let io = ui.io();
                    if multi_select && mode == DialogMode::OpenFile {
                        if io.key_shift() {
                            // Shift+Click: range select from last click to current
                            let anchor = last_click_index.unwrap_or(0);
                            let lo = anchor.min(idx);
                            let hi = anchor.max(idx);
                            selected_indices.clear();
                            for r in lo..=hi {
                                selected_indices.push(r);
                            }
                        } else if io.key_ctrl() {
                            // Ctrl+Click: toggle individual selection
                            if let Some(pos) = selected_indices.iter().position(|&r| r == idx) {
                                selected_indices.remove(pos);
                            } else {
                                selected_indices.push(idx);
                            }
                        } else {
                            selected_indices.clear();
                            selected_indices.push(idx);
                        }
                    } else {
                        selected_indices.clear();
                        selected_indices.push(idx);
                    }
                    *last_click_index = Some(idx);

                    // Update filename buf for SaveFile mode
                    if mode == DialogMode::SaveFile && !e.is_dir {
                        filename_buf.clear();
                        filename_buf.push_str(&e.name);
                    }

                    // Double-click handling
                    if ui.is_mouse_double_clicked(MouseButton::Left) {
                        if e.is_dir {
                            result.action = Some(Action::NavigateTo(e.path.clone()));
                        } else {
                            result.action = Some(Action::ConfirmSelection);
                        }
                    }
                }

                // Right-click context menu trigger
                if ui.is_item_clicked_with_button(MouseButton::Right) {
                    *context_menu_target = Some(idx);
                    selected_indices.clear();
                    selected_indices.push(idx);
                    ui.open_popup("##ctx_menu");
                }

                // Render icon + name on top of the selectable
                ui.same_line_with_spacing(0.0, 0.0);
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0] + 4.0, cursor[1]]);

                let alpha = if e.is_hidden { 0.55 } else { 1.0 };
                let icon_clr = [
                    file_icon_color[0],
                    file_icon_color[1],
                    file_icon_color[2],
                    alpha,
                ];
                ui.text_colored(icon_clr, file_icon);
                ui.same_line_with_spacing(0.0, 4.0);
                if e.is_dir {
                    ui.text_colored(
                        [
                            CLR_FOLDER_TEXT[0],
                            CLR_FOLDER_TEXT[1],
                            CLR_FOLDER_TEXT[2],
                            alpha,
                        ],
                        &e.name,
                    );
                } else if e.is_hidden {
                    ui.text_colored(theme::TEXT_MUTED, &e.name);
                } else {
                    ui.text(&e.name);
                }

                // Tooltip for truncated names (show full name on hover if clipped).
                // P1-1: cache the measured pixel width so we only call
                // `calc_text_size` once per entry across its lifetime, instead
                // of every hovered frame (~30 visible rows × 60 fps).
                if ui.is_item_hovered() {
                    let item_w = ui.item_rect_size()[0];
                    let mut text_w = e.name_pixel_width.get();
                    if text_w < 0.0 {
                        text_w = crate::utils::text::calc_text_size(&e.name)[0];
                        e.name_pixel_width.set(text_w);
                    }
                    if text_w > item_w {
                        crate::utils::themed_tooltip(ui, || ui.text(&e.name));
                    }
                }
            }

            // Column 1: Size (if visible)
            if config.show_column_size {
                ui.table_next_column();
                if !e.is_dir {
                    ui.text_colored(theme::TEXT_SECONDARY, &e.size_display);
                }
            }

            // Column 2: Date Modified (if visible)
            if config.show_column_date {
                ui.table_next_column();
                if !e.date_display.is_empty() {
                    ui.text_colored(theme::TEXT_SECONDARY, &e.date_display);
                }
            }

            // Column 3: Type (if visible)
            if config.show_column_type {
                ui.table_next_column();
                ui.text_colored(theme::TEXT_MUTED, &e.type_display);
            }
        }

        // ── Context menu popup ──
        if let Some(_tok) = ui.begin_popup("##ctx_menu")
            && let Some(target_idx) = *context_menu_target
            && let Some(target_entry) = entries.get(target_idx)
        {
            // Open (for dirs) or Confirm (for files)
            if target_entry.is_dir {
                let label = icon_label(buf, icons::FOLDER_OPEN, strings.open);
                if ui.selectable(label) {
                    result.action = Some(Action::NavigateTo(target_entry.path.clone()));
                    *context_menu_target = None;
                }
            } else {
                let label = icon_label(buf, icons::CHECK_BOLD, strings.open);
                if ui.selectable(label) {
                    result.action = Some(Action::ConfirmSelection);
                    *context_menu_target = None;
                }
            }

            ui.separator();

            // Rename
            let label = icon_label(buf, icons::PENCIL, strings.rename);
            if ui.selectable(label) {
                *rename_index = Some(target_idx);
                rename_buf.clear();
                rename_buf.push_str(&target_entry.name);
                *context_menu_target = None;
            }

            // Delete (request confirmation)
            let label = icon_label(buf, icons::TRASH_CAN_OUTLINE, strings.delete);
            if ui.selectable(label) {
                result.request_delete = Some(target_idx);
                *context_menu_target = None;
            }

            ui.separator();

            // Copy Path
            let label = icon_label(buf, icons::CONTENT_COPY, strings.copy_path);
            if ui.selectable(label) {
                result.action = Some(Action::CopyPath(target_idx));
                *context_menu_target = None;
            }
        }
    }

    // Keyboard navigation (extracted to `table_keyboard` to keep this file
    // within the 500-line budget). Overwrites any row-loop action only when a
    // navigation key actually fired — identical to the previous in-place logic.
    if let Some(a) =
        super::table_keyboard::handle_table_keyboard(ui, entries, selected_indices, scroll_to_index)
    {
        result.action = Some(a);
    }

    result
}
