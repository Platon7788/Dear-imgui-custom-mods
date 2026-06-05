//! Per-frame rendering driver for
//! [`FileManager`](super::FileManager).
//!
//! Split out of `mod.rs` (was > 500 lines, per CLAUDE.md). [`FileManager::render`]
//! is the single public per-frame entry point; it composes the `render::*`
//! widget functions, collects at most one deferred [`Action`](super::Action),
//! and applies it after the frame.

use dear_imgui_rs::{Key, Ui, WindowFlags};

use super::{Action, DialogMode, FileManager, render};

impl FileManager {
    // ─── Main render ────────────────────────────────────────────────

    /// Render the file manager dialog. Returns `true` when the user confirms selection.
    pub fn render(&mut self, ui: &Ui) -> bool {
        if !self.is_open {
            return false;
        }

        let strings = self.config.strings;

        if !self.loaded {
            self.refresh_directory();
        }

        let mut confirmed = false;
        let mut do_confirm_selection = false;
        let mut deferred: Option<Action> = None;

        let title = self.config.custom_title.unwrap_or(match self.mode {
            DialogMode::SelectFolder => strings.select_folder,
            DialogMode::OpenFile => strings.open_file,
            DialogMode::SaveFile => strings.save_file,
        });

        // Set window size before opening popup
        unsafe {
            #[allow(clippy::unnecessary_cast)]
            // ImGuiCond_Appearing is u32 on Linux, i32 on Windows
            dear_imgui_rs::sys::igSetNextWindowSize(
                dear_imgui_rs::sys::ImVec2 {
                    x: self.config.initial_size[0],
                    y: self.config.initial_size[1],
                },
                dear_imgui_rs::sys::ImGuiCond_Appearing as i32,
            );
            dear_imgui_rs::sys::igSetNextWindowSizeConstraints(
                dear_imgui_rs::sys::ImVec2 {
                    x: self.config.min_size[0],
                    y: self.config.min_size[1],
                },
                dear_imgui_rs::sys::ImVec2 {
                    x: f32::MAX,
                    y: f32::MAX,
                },
                None,
                std::ptr::null_mut(),
            );
        }

        if self.popup_needs_open {
            self.popup_needs_open = false;
            ui.open_popup(title);
        }

        // WindowPadding pushed before `begin()` so the popup itself adopts it.
        // Some themes set WindowPadding to ~[2, 2] which makes the drive bar /
        // breadcrumb / favorites label visibly hug the left edge — give the
        // popup a small inner gutter so its content doesn't merge with the
        // window border.
        let _padding = ui.push_style_var(dear_imgui_rs::StyleVar::WindowPadding([6.0, 6.0]));

        if let Some(_tok) = ui
            .begin_modal_popup_config(title)
            .flags(WindowFlags::NO_COLLAPSE)
            .begin()
        {
            let _rounding = ui.push_style_var(dear_imgui_rs::StyleVar::FrameRounding(3.0));

            // ── Drive selector ──
            if let Some(a) = render::render_drive_bar(
                ui,
                &self.drives,
                self.current_drive_letter(),
                &mut self.fmt_buf,
            ) {
                deferred = Some(a);
            }
            ui.spacing();

            // ── Toolbar ──
            if deferred.is_none()
                && let Some(a) = render::render_toolbar(
                    ui,
                    strings,
                    self.has_parent(),
                    self.history.can_go_back(),
                    self.history.can_go_forward(),
                    &mut self.show_new_folder,
                    &mut self.new_folder_buf,
                    &mut self.show_new_file,
                    &mut self.new_file_buf,
                    self.show_hidden,
                    &self.config,
                    &mut self.fmt_buf,
                )
            {
                deferred = Some(a);
            }

            // ── Breadcrumb / path bar ──
            if deferred.is_none() {
                if self.config.enable_breadcrumbs {
                    if let Some(a) = render::render_breadcrumb_bar(
                        ui,
                        &self.current_path,
                        &self.breadcrumb_segments,
                        &mut self.breadcrumb_editing,
                        &mut self.path_input_buf,
                    ) {
                        deferred = Some(a);
                    }
                } else {
                    // Fallback: simple text input path bar
                    let _bg = ui.push_style_color(
                        dear_imgui_rs::StyleColor::FrameBg,
                        crate::theme::BG_FRAME,
                    );
                    ui.text_colored(crate::theme::WARNING, crate::icons::FOLDER_OPEN);
                    ui.same_line_with_spacing(0.0, 6.0);
                    ui.set_next_item_width(ui.content_region_avail()[0]);
                    let enter = ui
                        .input_text("##pathbar", &mut self.path_input_buf)
                        .enter_returns_true(true)
                        .build();
                    if enter {
                        deferred = Some(Action::NavigateToInput(self.path_input_buf.clone()));
                    }
                }
            }
            ui.spacing();

            ui.separator();

            // ── Error ──
            if let Some(ref err) = self.error {
                let msg = err.format(strings);
                self.fmt_buf.clear();
                let _ = std::fmt::Write::write_fmt(
                    &mut self.fmt_buf,
                    format_args!("{} {}", crate::icons::ALERT, msg),
                );
                ui.text_colored(crate::theme::TEXT_ERROR, &self.fmt_buf);
                ui.spacing();
            }

            // ── Content area (favorites + file table) ──
            // Reserve space for the status row (one text line) + footer row
            // (button_height) + 3 vertical spacings (~item_spacing.y) + padding.
            // P3-2: derive from configured button height and font line height
            // instead of a hard-coded 64.0 — works correctly when the user
            // changes `button_height` or font size.
            let line_h = ui.text_line_height_with_spacing();
            let spacing_y = ui.clone_style().item_spacing()[1];
            let reserved = self.config.button_height + line_h + spacing_y * 3.0 + 4.0;
            let content_h = (ui.content_region_avail()[1] - reserved).max(100.0);

            let show_favorites = self.config.show_favorites && !self.favorites.entries.is_empty();

            if show_favorites {
                // Left panel: Favorites
                ui.child_window("##fm_favorites")
                    .size([self.config.favorites_width, content_h])
                    .border(true)
                    .build(ui, || {
                        if let Some(a) = render::render_favorites_panel(
                            ui,
                            &self.favorites,
                            &self.current_path,
                            strings,
                            &mut self.fmt_buf,
                        ) && deferred.is_none()
                        {
                            deferred = Some(a);
                        }
                    });
                ui.same_line();
            }

            // Right panel: File table
            {
                ui.child_window("##fm_table_area")
                    .size([0.0, content_h])
                    .build(ui, || {
                        // P2-3: bundle 17 args into a single TableCtx borrow.
                        let table_result = render::render_file_table(
                            ui,
                            render::TableCtx {
                                entries: &self.entries,
                                selected_indices: &mut self.selected_indices,
                                mode: self.mode,
                                multi_select: self.config.enable_multi_select,
                                filename_buf: &mut self.filename_buf,
                                strings,
                                has_error: self.error.is_some(),
                                sort_column: &mut self.sort_column,
                                sort_order: &mut self.sort_order,
                                rename_index: &mut self.rename_index,
                                rename_buf: &mut self.rename_buf,
                                context_menu_target: &mut self.context_menu_target,
                                last_click_index: &mut self.last_click_index,
                                scroll_to_index: &mut self.scroll_to_index,
                                config: &self.config,
                                buf: &mut self.fmt_buf,
                            },
                        );

                        if let Some(a) = table_result.action {
                            match a {
                                Action::ConfirmSelection => do_confirm_selection = true,
                                other => {
                                    if deferred.is_none() {
                                        deferred = Some(other);
                                    }
                                }
                            }
                        }

                        // Handle delete request from context menu (show confirmation)
                        if let Some(idx) = table_result.request_delete {
                            self.delete_target = Some(idx);
                            self.show_delete_confirm = true;
                        }
                    });
            }

            // ── Type-to-search ──
            self.handle_type_to_search(ui);

            // ── Status bar ──
            {
                self.fmt_buf.clear();
                let total = self.entries.len();
                let selected = self.selected_indices.len();
                let _ = std::fmt::Write::write_fmt(
                    &mut self.fmt_buf,
                    format_args!("{total} {}", strings.status_items),
                );
                if selected > 0 {
                    let _ = std::fmt::Write::write_fmt(
                        &mut self.fmt_buf,
                        format_args!("  ·  {selected} {}", strings.status_selected),
                    );
                }
                ui.text_colored(crate::theme::TEXT_MUTED, &self.fmt_buf);
            }

            ui.spacing();

            // ── Footer (filename input for SaveFile + buttons) ──
            let (foot_confirmed, foot_cancelled, foot_action) = render::render_footer(
                ui,
                strings,
                self.mode,
                &self.entries,
                &self.selected_indices,
                &mut self.filename_buf,
                &self.filters,
                self.active_filter,
                &self.config,
                &mut self.fmt_buf,
            );
            if foot_confirmed {
                do_confirm_selection = true;
            }
            if foot_cancelled {
                self.is_open = false;
                ui.close_current_popup();
            }
            if let Some(a) = foot_action
                && deferred.is_none()
            {
                deferred = Some(a);
            }

            // ── Escape: close in priority order ──
            // 1. Inline rename — clear rename buffer
            // 2. Open right-click context menu — dismiss it (P3-7)
            // 3. Open inline new-folder/new-file — they handle their own Esc via inputs
            // 4. Breadcrumb edit — handled inside its input
            // 5. Otherwise — close the dialog
            if ui.is_key_pressed(Key::Escape) {
                if self.rename_index.is_some() {
                    self.rename_index = None;
                    self.rename_buf.clear();
                } else if self.context_menu_target.is_some() {
                    self.context_menu_target = None;
                } else if !self.show_new_folder && !self.show_new_file && !self.breadcrumb_editing {
                    self.is_open = false;
                    ui.close_current_popup();
                }
            }

            // ── Ctrl+A to select all ──
            if ui.is_key_pressed(Key::A)
                && ui.io().key_ctrl()
                && !ui.is_any_item_active()
                && self.config.enable_multi_select
                && self.mode == DialogMode::OpenFile
            {
                // P1-9: iterator-based collect instead of indexed range loop.
                self.selected_indices = self
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| !e.is_dir)
                    .map(|(i, _)| i)
                    .collect();
            }

            // ── Ctrl+L to edit path ──
            if ui.is_key_pressed(Key::L)
                && ui.io().key_ctrl()
                && !ui.is_any_item_active()
                && self.config.enable_breadcrumbs
            {
                self.breadcrumb_editing = true;
                self.path_input_buf.clear();
                self.path_input_buf
                    .push_str(&self.current_path.to_string_lossy());
            }

            // ── Ctrl+H to toggle hidden files ──
            if ui.is_key_pressed(Key::H) && ui.io().key_ctrl() && !ui.is_any_item_active() {
                self.show_hidden = !self.show_hidden;
                self.refresh_directory();
            }

            // ── F2 to rename ──
            if ui.is_key_pressed(Key::F2)
                && !ui.is_any_item_active()
                && self.rename_index.is_none()
                && let Some(&idx) = self.selected_indices.first()
                && let Some(e) = self.entries.get(idx)
            {
                self.rename_index = Some(idx);
                self.rename_buf.clear();
                self.rename_buf.push_str(&e.name);
            }

            // ── Delete key ──
            if ui.is_key_pressed(Key::Delete)
                && !ui.is_any_item_active()
                && self.rename_index.is_none()
                && let Some(&idx) = self.selected_indices.first()
                && self.entries.get(idx).is_some()
            {
                self.delete_target = Some(idx);
                self.show_delete_confirm = true;
            }

            // ── Handle confirmation ──
            if do_confirm_selection {
                confirmed = self.try_confirm(ui);
            }

            // ── Overwrite confirmation modal ──
            if let Some(result) = render::render_overwrite_confirm(
                ui,
                strings,
                &mut self.show_overwrite_confirm,
                &mut self.fmt_buf,
            ) && result
            {
                self.finalize_selection();
                confirmed = true;
                ui.close_current_popup();
            }

            // ── Delete confirmation modal ──
            if let Some(result) = render::render_delete_confirm(
                ui,
                strings,
                &mut self.show_delete_confirm,
                self.delete_target
                    .and_then(|i| self.entries.get(i).map(|e| e.name.as_str())),
                &mut self.fmt_buf,
            ) {
                if result {
                    if let Some(idx) = self.delete_target {
                        deferred = Some(Action::DeleteEntry(idx));
                    }
                } else {
                    self.delete_target = None;
                }
            }
        }

        // Apply deferred action
        if let Some(action) = deferred {
            self.apply_action(action, ui);
        }

        confirmed
    }
}
