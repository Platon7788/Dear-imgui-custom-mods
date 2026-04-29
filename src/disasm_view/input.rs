//! Input handling for [`super::DisasmView`] — keyboard navigation
//! (cursor moves, page-up/down, Home/End, Enter to follow branch, G for
//! goto, Ctrl+A select-all, F9 breakpoint, Alt+arrow nav history),
//! mouse interaction (click / shift / ctrl / drag-select / double-click
//! to edit / right-click context menu), and the inline edit commit path.

use super::config::DisasmDataProvider;
use super::{DisasmView, EditColumn, EditState};
use crate::utils::clipboard::set_clipboard;

impl DisasmView {
    pub(super) fn handle_keyboard(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        use dear_imgui_rs::Key;

        let count = provider.instruction_count();
        if count == 0 {
            return;
        }

        let ctrl = ui.io().key_ctrl();
        let alt = ui.io().key_alt();
        let shift = ui.io().key_shift();

        // Inline edit active → skip navigation.
        if self.edit.is_some() {
            self.handle_edit_keyboard(ui, provider);
            return;
        }

        // Helper: move cursor and handle shift-selection.
        let move_cursor = |s: &mut Self, new_idx: usize| {
            if shift {
                let anchor = s.sel_anchor.unwrap_or(s.cursor_idx.unwrap_or(0));
                s.select_range(anchor, new_idx);
            } else {
                s.selection.clear();
                s.selection.insert(new_idx);
                s.sel_anchor = Some(new_idx);
            }
            s.cursor_idx = Some(new_idx);
        };

        // Arrow keys.
        if ui.is_key_pressed(Key::UpArrow) && !alt {
            let idx = self.cursor_idx.unwrap_or(0);
            if idx > 0 {
                let new = idx - 1;
                move_cursor(self, new);
                self.ensure_visible(new, ui);
            }
        }
        if ui.is_key_pressed(Key::DownArrow) && !alt {
            let idx = self.cursor_idx.unwrap_or(0);
            if idx + 1 < count {
                let new = idx + 1;
                move_cursor(self, new);
                self.ensure_visible(new, ui);
            }
        }

        // Page Up/Down.
        if ui.is_key_pressed(Key::PageUp) {
            let visible = (ui.window_size()[1] / self.line_height) as usize;
            let new = self.cursor_idx.unwrap_or(0).saturating_sub(visible);
            move_cursor(self, new);
            self.scroll_to = Some(new);
        }
        if ui.is_key_pressed(Key::PageDown) {
            let visible = (ui.window_size()[1] / self.line_height) as usize;
            let new = (self.cursor_idx.unwrap_or(0) + visible).min(count - 1);
            move_cursor(self, new);
            self.scroll_to = Some(new);
        }

        // Home/End.
        if ui.is_key_pressed(Key::Home) {
            move_cursor(self, 0);
            self.scroll_to = Some(0);
        }
        if ui.is_key_pressed(Key::End) {
            move_cursor(self, count - 1);
            self.scroll_to = Some(count - 1);
        }

        // Ctrl+A — select all. Layout-independence is provided by
        // `crate::input::keyboard::try_inject_ctrl_alt_shortcut` at the
        // host level (see app_window_v2 and demo_disasm_view).
        if ctrl && ui.is_key_pressed(Key::A) {
            for i in 0..count {
                self.selection.insert(i);
            }
        }

        // Enter → follow branch target.
        if ui.is_key_pressed(Key::Enter)
            && let Some(idx) = self.cursor_idx
            && let Some(instr) = provider.instruction(idx)
            && let Some(target) = instr.branch_target()
        {
            self.goto_address(target, provider);
        }

        // G → goto address popup.
        if ui.is_key_pressed(Key::G) && !ctrl {
            self.show_goto = true;
            self.goto_buf.clear();
        }

        // Ctrl+C → copy selected instruction.
        if ctrl && ui.is_key_pressed(Key::C) {
            self.copy_selected(provider);
        }

        // F9 → toggle breakpoint.
        if ui.is_key_pressed(Key::F9)
            && let Some(idx) = self.cursor_idx
            && let Some(instr) = provider.instruction(idx)
        {
            provider.toggle_breakpoint(instr.address());
        }

        // Alt+Left → nav back.
        if alt && ui.is_key_pressed(Key::LeftArrow) {
            self.nav_back(provider);
        }
        // Alt+Right → nav forward.
        if alt && ui.is_key_pressed(Key::RightArrow) {
            self.nav_forward(provider);
        }
    }

    fn handle_edit_keyboard(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        _provider: &mut dyn DisasmDataProvider,
    ) {
        // InputText widget handles all input now.
        // Only Escape needs manual handling (ImGui InputText doesn't cancel on Esc by default).
        if ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
            self.edit = None;
        }
    }

    pub(super) fn commit_edit(&self, edit: EditState, provider: &mut dyn DisasmDataProvider) {
        if let Some(instr) = provider.instruction(edit.idx) {
            let addr = instr.address();
            match edit.column {
                EditColumn::Bytes => {
                    let bytes: Vec<u8> = edit
                        .buf
                        .split_whitespace()
                        .filter_map(|tok| u8::from_str_radix(tok, 16).ok())
                        .collect();
                    if !bytes.is_empty() {
                        provider.write_bytes(addr, &bytes);
                    }
                }
                EditColumn::Mnemonic => {
                    provider.assemble(addr, &edit.buf);
                }
            }
        }
    }

    pub(super) fn handle_mouse(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        if !ui.is_window_hovered() {
            return;
        }

        // Mouse wheel scroll.
        let wheel = ui.io().mouse_wheel();
        if wheel != 0.0 {
            let rows = (-wheel * 3.0) as isize;
            let scroll_y = ui.scroll_y();
            let new_scroll = (scroll_y + rows as f32 * self.line_height).max(0.0);
            ui.set_scroll_y(new_scroll);
        }

        let ctrl = ui.io().key_ctrl();
        let shift = ui.io().key_shift();

        // Click to select — with Ctrl/Shift modifiers.
        if ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Left) {
            if let Some(idx) = self.mouse_to_instruction(ui, provider) {
                if let Some(edit) = &self.edit
                    && edit.idx != idx
                {
                    self.edit = None;
                }

                if shift {
                    let anchor = self.sel_anchor.unwrap_or(self.cursor_idx.unwrap_or(0));
                    self.select_range(anchor, idx);
                    self.cursor_idx = Some(idx);
                } else if ctrl {
                    if self.selection.contains(&idx) {
                        self.selection.remove(&idx);
                    } else {
                        self.selection.insert(idx);
                    }
                    self.cursor_idx = Some(idx);
                    self.sel_anchor = Some(idx);
                } else {
                    self.selection.clear();
                    self.selection.insert(idx);
                    self.cursor_idx = Some(idx);
                    self.sel_anchor = Some(idx);
                    self.drag_origin = Some(idx);
                }
            } else {
                // Clicked outside — cancel edit and clear selection.
                self.edit = None;
            }
        }

        // Drag to extend selection.
        if ui.is_mouse_dragging(dear_imgui_rs::MouseButton::Left)
            && let Some(origin) = self.drag_origin
            && let Some(idx) = self.mouse_to_instruction(ui, provider)
            && idx != self.cursor_idx.unwrap_or(usize::MAX)
        {
            self.select_range(origin, idx);
            self.cursor_idx = Some(idx);
        }

        // Release drag.
        if ui.is_mouse_released(dear_imgui_rs::MouseButton::Left) {
            self.drag_origin = None;
        }

        // Double-click to edit (if editable).
        if ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
            && self.config.editable
            && let Some(idx) = self.mouse_to_instruction(ui, provider)
            && let Some(instr) = provider.instruction(idx)
        {
            let bytes_str: String = instr
                .bytes()
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            self.edit = Some(EditState {
                idx,
                column: EditColumn::Bytes,
                buf: bytes_str,
                frames: 0,
            });
        }

        // Right-click context menu.
        if ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Right)
            && let Some(idx) = self.mouse_to_instruction(ui, provider)
        {
            self.cursor_idx = Some(idx);
            self.context_idx = Some(idx);
            self.show_context_menu = true;
        }
    }

    fn mouse_to_instruction(
        &self,
        ui: &dear_imgui_rs::Ui,
        provider: &dyn DisasmDataProvider,
    ) -> Option<usize> {
        let [_mx, my] = ui.io().mouse_pos();
        let [_win_x, win_y] = ui.cursor_screen_pos();
        let scroll_y = ui.scroll_y();
        let origin_y = win_y + scroll_y;
        let header_h = if self.config.show_header {
            self.line_height
        } else {
            0.0
        };

        let rel_y = my - origin_y - header_h;
        if rel_y < 0.0 {
            return None;
        }

        let scroll_offset = (scroll_y / self.line_height) as usize;
        let row = (rel_y / self.line_height) as usize + scroll_offset;

        if row < provider.instruction_count() {
            Some(row)
        } else {
            None
        }
    }

    fn ensure_visible(&mut self, idx: usize, ui: &dear_imgui_rs::Ui) {
        let y = idx as f32 * self.line_height;
        let scroll_y = ui.scroll_y();
        let visible_h = ui.window_size()[1];

        if y < scroll_y || y + self.line_height > scroll_y + visible_h {
            self.scroll_to = Some(idx);
        }
    }

    pub(super) fn copy_selected(&self, provider: &dyn DisasmDataProvider) {
        // Copy all selected instructions (or just cursor if nothing selected).
        let indices: Vec<usize> = if self.selection.is_empty() {
            self.cursor_idx.into_iter().collect()
        } else {
            self.selection.iter().copied().collect()
        };

        if indices.is_empty() {
            return;
        }

        let lines: Vec<String> = indices
            .iter()
            .filter_map(|&idx| {
                provider.instruction(idx).map(|instr| {
                    let addr = if self.config.address_width_64 {
                        format!("{:016X}", instr.address())
                    } else {
                        format!("{:08X}", instr.address())
                    };
                    let bytes_str: String = instr
                        .bytes()
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let comment = instr
                        .comment()
                        .map(|c| format!(" ; {}", c))
                        .unwrap_or_default();
                    format!(
                        "{}  {:16}  {} {}{}",
                        addr,
                        bytes_str,
                        instr.mnemonic(),
                        instr.operands(),
                        comment
                    )
                })
            })
            .collect();

        set_clipboard(&lines.join("\n"));
    }
}
