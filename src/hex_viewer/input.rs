//! Keyboard + mouse handling, edit-mode state machine, hit-testing.
//!
//! All input paths route through `move_cursor_with_selection` so
//! shift-extends, selection clears and pending-nibble flushes are
//! consistent across arrow keys, Page/Home/End, and explicit `goto`.

use super::HexViewer;
use super::config::UndoEntry;
use super::search::{Selection, format_bytes};
use crate::utils::clipboard::{self, set_clipboard};

/// How many frames the address-gutter "just copied" highlight lingers
/// after a click-to-copy. ~30 frames @ 60 fps ≈ 0.5 s — long enough to
/// confirm the action visually without overstaying its welcome.
pub(super) const ADDRESS_FLASH_FRAMES: u32 = 30;

// ── EditColumn ───────────────────────────────────────────────────────────────

/// Which column is being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditColumn {
    /// Editing in the hex column (nibble input).
    Hex,
    /// Editing in the ASCII column (character input).
    Ascii,
}

// ── HexViewer impl: edit lifecycle, undo/redo, navigation helpers ────────────

impl HexViewer {
    pub(super) fn start_editing(&mut self, column: EditColumn) {
        self.edit_column = Some(column);
        self.edit_nibble = None;
        // Switch to English layout for hex input.
        if column == EditColumn::Hex && !self.layout_switched {
            clipboard::activate_english_layout();
            self.layout_switched = true;
        }
    }

    pub(super) fn stop_editing(&mut self) {
        self.edit_column = None;
        self.edit_nibble = None;
        if self.layout_switched {
            clipboard::restore_keyboard_layout();
            self.layout_switched = false;
        }
    }

    /// Apply a half-typed hex nibble before the user navigates away.
    ///
    /// In hex-edit mode each byte takes two keystrokes (high nibble +
    /// low nibble). If only the high nibble was typed, navigating to
    /// another byte historically silently dropped it. This commit-on-
    /// leave variant replaces the **upper** nibble of the current byte
    /// and keeps the lower nibble intact — matches HxD/010-style hex
    /// editors and gives the user a way to write a single nibble.
    ///
    /// Pushes a single-byte undo entry only when the new value really
    /// differs from the old (avoids polluting undo history with no-op
    /// "type X then leave with same X" sequences).
    ///
    /// To **discard** instead of committing, call `stop_editing` —
    /// it zeroes `edit_nibble` first.
    pub(super) fn commit_pending_edit(&mut self) {
        let Some(hi) = self.edit_nibble.take() else {
            return;
        };
        if self.cursor >= self.data.len() {
            return;
        }
        let old_byte = self.data[self.cursor];
        let new_byte = (hi << 4) | (old_byte & 0x0F);
        if new_byte == old_byte {
            return;
        }
        self.undo.push(UndoEntry {
            offset: self.cursor as u64,
            old_bytes: vec![old_byte],
            new_bytes: vec![new_byte],
        });
        self.data[self.cursor] = new_byte;
    }

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo.undo() {
            let off = entry.offset as usize;
            let old = entry.old_bytes.clone();
            if off + old.len() <= self.data.len() {
                self.data[off..off + old.len()].copy_from_slice(&old);
                self.cursor = off;
                self.scroll_to_cursor();
            }
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.undo.redo() {
            let off = entry.offset as usize;
            let new = entry.new_bytes.clone();
            if off + new.len() <= self.data.len() {
                self.data[off..off + new.len()].copy_from_slice(&new);
                self.cursor = off;
                self.scroll_to_cursor();
            }
        }
    }

    pub(super) fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.data.len().saturating_sub(1));
    }

    pub(super) fn scroll_to_cursor(&mut self) {
        let bpr = self.config.bytes_per_row.value();
        let row = self.cursor / bpr;
        self.scroll_to_row = Some(row);
    }

    /// Move the cursor to `new_cursor`, optionally extending the
    /// selection. When `shift` is true and selection is currently
    /// empty, anchors `selection.start` at the *previous* cursor
    /// position so the byte under the old cursor is included in the
    /// growing range. When `shift` is false, the selection is cleared.
    ///
    /// Commits any half-typed hex nibble on the *old* byte before
    /// moving — keyboard navigation should never silently drop user
    /// input. The edit-mode itself is preserved (caller is moving
    /// inside the same edit session); to leave edit mode, callers
    /// also invoke `stop_editing`.
    pub(super) fn move_cursor_with_selection(&mut self, new_cursor: usize, shift: bool) {
        self.commit_pending_edit();
        let old = self.cursor;
        self.cursor = new_cursor.min(self.data.len().saturating_sub(1));
        if shift {
            if self.selection.is_empty() {
                self.selection.start = old;
            }
            self.selection.end = self.cursor;
        } else {
            self.selection = Selection::default();
        }
        self.scroll_to_cursor();
    }

    pub(super) fn copy_selection(&self) {
        let bytes = self.selected_bytes();
        if bytes.is_empty() {
            if self.cursor < self.data.len() {
                let s = format_bytes(
                    &[self.data[self.cursor]],
                    self.config.copy_format,
                    self.config.uppercase,
                );
                set_clipboard(&s);
            }
            return;
        }
        let s = format_bytes(bytes, self.config.copy_format, self.config.uppercase);
        set_clipboard(&s);
    }
}

// ── HexViewer impl: keyboard ─────────────────────────────────────────────────

impl HexViewer {
    pub(super) fn handle_keyboard(&mut self, ui: &dear_imgui_rs::Ui) {
        use dear_imgui_rs::Key;

        let bpr = self.config.bytes_per_row.value();
        let len = self.data.len();
        if len == 0 {
            return;
        }

        let shift = ui.io().key_shift();
        let ctrl = ui.io().key_ctrl();
        let alt = ui.io().key_alt();

        // Shortcuts rely on ImGui's `is_key_pressed` (front-edge with
        // repeat detection). For non-Latin layouts the host application
        // is expected to wire in `crate::input::keyboard::*` helpers
        // (`try_inject_ctrl_alt_shortcut` + `reinforce_physical_key_state`)
        // around `platform.handle_event` — the same pattern used by
        // `app_window`. With those installed, ImGui
        // sees the physical-key-derived `Key::C` regardless of the
        // active layout, so no per-widget VK fallback is needed here.

        // === Hotkeys ===
        if ctrl && ui.is_key_pressed(Key::C) {
            self.copy_selection();
        }
        if ctrl && ui.is_key_pressed(Key::G) && !self.show_goto {
            self.show_goto = true;
            self.goto_buf.clear();
            // Anchor the popup near the cursor — `mouse_pos` is the
            // screen-space coordinate of the cursor at keypress.
            self.popup_open_pos = ui.io().mouse_pos();
        }
        if ctrl && ui.is_key_pressed(Key::F) && !self.show_search {
            self.show_search = true;
            self.popup_open_pos = ui.io().mouse_pos();
        }
        if ctrl && ui.is_key_pressed(Key::A) {
            self.selection = Selection { start: 0, end: len };
        }
        if ctrl && !shift && ui.is_key_pressed(Key::Z) {
            self.undo();
        }
        if ctrl && ui.is_key_pressed(Key::Y) {
            self.redo();
        }
        if ctrl && shift && ui.is_key_pressed(Key::Z) {
            self.redo();
        }

        // F3 = next/prev search result.
        if ui.is_key_pressed(Key::F3) && !self.search_results.is_empty() {
            if shift {
                self.search_idx = self
                    .search_idx
                    .checked_sub(1)
                    .unwrap_or(self.search_results.len() - 1);
            } else {
                self.search_idx = (self.search_idx + 1) % self.search_results.len();
            }
            self.cursor = self.search_results[self.search_idx];
            self.selection = Selection {
                start: self.cursor,
                end: self.cursor + self.search_pattern.len(),
            };
            self.scroll_to_cursor();
        }

        // Escape — stop editing.
        if ui.is_key_pressed(Key::Escape) {
            self.stop_editing();
        }

        // Alt+Left/Right — nav back/forward.
        if alt && ui.is_key_pressed(Key::LeftArrow) {
            self.nav_back();
            return;
        }
        if alt && ui.is_key_pressed(Key::RightArrow) {
            self.nav_forward();
            return;
        }

        // Navigation (all paths share `move_cursor_with_selection` so
        // Shift-extends and selection-clear behave identically across
        // arrows / PageUp-Down / Home-End).
        if !ctrl && !alt {
            if ui.is_key_pressed(Key::LeftArrow) {
                let new = self.cursor.saturating_sub(1);
                self.move_cursor_with_selection(new, shift);
            }
            if ui.is_key_pressed(Key::RightArrow) {
                let new = (self.cursor + 1).min(len - 1);
                self.move_cursor_with_selection(new, shift);
            }
            if ui.is_key_pressed(Key::UpArrow) {
                let new = self.cursor.saturating_sub(bpr);
                self.move_cursor_with_selection(new, shift);
            }
            if ui.is_key_pressed(Key::DownArrow) {
                let new = (self.cursor + bpr).min(len - 1);
                self.move_cursor_with_selection(new, shift);
            }
            if ui.is_key_pressed(Key::PageUp) {
                let rows = (ui.window_size()[1] / self.line_height) as usize;
                let new = self.cursor.saturating_sub(bpr * rows);
                self.move_cursor_with_selection(new, shift);
            }
            if ui.is_key_pressed(Key::PageDown) {
                let rows = (ui.window_size()[1] / self.line_height) as usize;
                let new = (self.cursor + bpr * rows).min(len - 1);
                self.move_cursor_with_selection(new, shift);
            }
            if ui.is_key_pressed(Key::Home) {
                let new = self.cursor - self.cursor % bpr;
                self.move_cursor_with_selection(new, shift);
            }
            if ui.is_key_pressed(Key::End) {
                let new = ((self.cursor / bpr + 1) * bpr - 1).min(len - 1);
                self.move_cursor_with_selection(new, shift);
            }
        }

        // Ctrl+Home/End.
        if ctrl && ui.is_key_pressed(Key::Home) {
            self.move_cursor_with_selection(0, shift);
        }
        if ctrl && ui.is_key_pressed(Key::End) {
            self.move_cursor_with_selection(len - 1, shift);
        }

        // Hex / ASCII editing input.
        if self.config.editable && !ctrl && !alt {
            match self.edit_column {
                Some(EditColumn::Hex) => self.handle_hex_input(),
                Some(EditColumn::Ascii) => self.handle_ascii_input(),
                None => {}
            }
        }
    }

    fn handle_hex_input(&mut self) {
        let chars = read_input_chars();
        for ch in chars {
            let nibble = match ch {
                '0'..='9' => ch as u8 - b'0',
                'a'..='f' => ch as u8 - b'a' + 10,
                'A'..='F' => ch as u8 - b'A' + 10,
                _ => continue,
            };
            if let Some(hi) = self.edit_nibble.take() {
                let new_byte = (hi << 4) | nibble;
                if self.cursor < self.data.len() {
                    let old_byte = self.data[self.cursor];
                    self.undo.push(UndoEntry {
                        offset: self.cursor as u64,
                        old_bytes: vec![old_byte],
                        new_bytes: vec![new_byte],
                    });
                    self.data[self.cursor] = new_byte;
                    if self.cursor < self.data.len() - 1 {
                        self.cursor += 1;
                    }
                }
            } else {
                self.edit_nibble = Some(nibble);
            }
        }
    }

    fn handle_ascii_input(&mut self) {
        let chars = read_input_chars();
        for ch in chars {
            // Each cursor cell is exactly one byte. Multi-byte UTF-8 (e.g.
            // Cyrillic 'А' = 0xD0 0x90) would only have its leading byte
            // written here, silently corrupting the buffer with a stray
            // 0xD0. Reject anything outside printable ASCII — the user
            // should switch the input column to hex for non-ASCII writes.
            if !ch.is_ascii() || ch.is_control() {
                continue;
            }
            let new_byte = ch as u8;
            if self.cursor < self.data.len() {
                let old_byte = self.data[self.cursor];
                self.undo.push(UndoEntry {
                    offset: self.cursor as u64,
                    old_bytes: vec![old_byte],
                    new_bytes: vec![new_byte],
                });
                self.data[self.cursor] = new_byte;
                if self.cursor < self.data.len() - 1 {
                    self.cursor += 1;
                }
            }
        }
    }
}

// ── HexViewer impl: mouse + hit-testing ──────────────────────────────────────

impl HexViewer {
    pub(super) fn handle_mouse(&mut self, ui: &dear_imgui_rs::Ui, _win_w: f32) {
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

        // ── Address-gutter affordance ────────────────────────────────
        // Hovering the address column shows a `Hand` cursor + tooltip
        // ("Click to copy 0x...") so the click-to-copy behaviour is
        // discoverable. A bare left click (no modifier) copies the
        // row's address as a hex literal to the clipboard and triggers
        // a brief background flash on the address text. Modifier-held
        // clicks fall through to the normal hex/ASCII handler so
        // shift-extend / ctrl-toggle still work in the data area.
        if let Some(row) = self.mouse_to_address_row(ui) {
            ui.set_mouse_cursor(Some(dear_imgui_rs::MouseCursor::Hand));
            let bpr = self.config.bytes_per_row.value();
            let addr = self.config.base_address + (row * bpr) as u64;
            let formatted = self.format_address_literal(addr);
            crate::utils::tooltip::themed_tooltip(ui, || {
                ui.text(format!("Click to copy: {}", formatted));
            });
            if !shift && !ctrl && ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Left) {
                set_clipboard(&formatted);
                self.address_flash = Some((row, ADDRESS_FLASH_FRAMES));
                // Don't fall through into the hex/ASCII click handler:
                // the click was intended for the address gutter.
                return;
            }
        }

        // Right click — opens the context menu (Go to Address /
        // back / forward / Settings) at the cursor. The popup body
        // lives in `popup::render_context_menu`; this branch just
        // raises the one-shot flag so the popup-open call runs on
        // the next frame, and captures the click position so the
        // popup spawns under the cursor (not at `(0, 0)`, which is
        // what ImGui defaults to when `BeginPopup` is called from
        // outside any window context — see render flow in `draw.rs`).
        if ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Right) {
            self.show_context_menu = true;
            self.popup_open_pos = ui.io().mouse_pos();
        }

        // Single click — moves the cursor / extends selection. **Does
        // not** enter edit mode (would be too easy to nudge a byte by
        // accident); double-click below is the explicit edit gesture.
        // Any half-typed nibble on the previous byte is committed first,
        // then edit mode is exited if we land on a different byte.
        if ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Left)
            && let Some((offset, _)) = self.mouse_to_offset(ui)
        {
            if shift {
                // Shift+Click: extend selection.
                self.selection.end = offset + 1;
            } else if ctrl {
                // Ctrl+Click: toggle byte in selection (simple multi-select).
                if self.selection.contains(offset) {
                    self.selection = Selection::default();
                } else if self.selection.is_empty() {
                    self.selection = Selection {
                        start: offset,
                        end: offset + 1,
                    };
                } else {
                    let (lo, hi) = self.selection.ordered();
                    let new_lo = lo.min(offset);
                    let new_hi = hi.max(offset + 1);
                    self.selection = Selection {
                        start: new_lo,
                        end: new_hi,
                    };
                }
                self.cursor = offset;
            } else {
                let was_editing = self.edit_column.is_some();
                let target_changed = self.cursor != offset;
                // Always flush the pending nibble so a deliberate "type
                // F then click somewhere else" persists the F as the
                // upper nibble. If the user wanted to discard, Esc is
                // the documented gesture.
                self.commit_pending_edit();
                self.cursor = offset;
                self.selection = Selection {
                    start: offset,
                    end: offset,
                };
                // Single click on a different byte exits edit mode (the
                // user "moved away"). Single click on the same byte
                // does nothing extra — re-enter is via double-click.
                if was_editing && target_changed {
                    self.stop_editing();
                }
            }
        }

        // Double-click — explicit "edit this byte" gesture. Only
        // meaningful when `editable` is set; otherwise it's a no-op.
        if self.config.editable
            && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
            && let Some((_, column)) = self.mouse_to_offset(ui)
        {
            self.start_editing(column);
        }

        // Drag to select. Suppressed while editing — drag inside an
        // active edit cell would otherwise hijack the gesture and
        // start a selection the user did not ask for.
        if self.edit_column.is_none()
            && ui.is_mouse_dragging(dear_imgui_rs::MouseButton::Left)
            && let Some((offset, _)) = self.mouse_to_offset(ui)
        {
            self.selection.end = offset + 1;
        }
    }

    /// Format `addr` as a copy-friendly hex literal (`0x...`) honouring
    /// the configured `address_width` and `uppercase` flags. Used by
    /// the click-to-copy path for the address gutter.
    pub(super) fn format_address_literal(&self, addr: u64) -> String {
        let digits = self
            .config
            .address_width
            .hex_digits(self.config.base_address, self.data.len());
        match (self.config.uppercase, digits) {
            (true, 16) => format!("0x{:016X}", addr),
            (false, 16) => format!("0x{:016x}", addr),
            (true, _) => format!("0x{:08X}", addr),
            (false, _) => format!("0x{:08x}", addr),
        }
    }

    /// Hit-test: returns the row index (0-based) when `mouse_pos` is
    /// inside the address gutter, otherwise `None`.
    ///
    /// The horizontal range is `[origin_x, hex_x)`; the vertical math
    /// mirrors [`Self::mouse_to_offset`] exactly so a row reported here
    /// is guaranteed to line up with the row that
    /// [`Self::mouse_to_offset`] would report for the same `my` in the
    /// data area.
    pub(super) fn mouse_to_address_row(&self, ui: &dear_imgui_rs::Ui) -> Option<usize> {
        if !self.config.show_offsets {
            return None;
        }
        let [mx, my] = ui.io().mouse_pos();
        let [win_x, win_y] = ui.cursor_screen_pos();
        let scroll_y = ui.scroll_y();
        let header_offset = if self.config.show_column_headers {
            1
        } else {
            0
        };

        // Mirror the one-glyph left padding applied in `draw::render`.
        let origin_x = win_x + self.char_advance;
        let hex_x = origin_x + self.offset_col_width();

        if mx < origin_x || mx >= hex_x {
            return None;
        }

        let rel_y = my - win_y + scroll_y;
        let row = (rel_y / self.line_height) as isize - header_offset as isize;
        if row < 0 {
            return None;
        }
        let row = row as usize;

        let bpr = self.config.bytes_per_row.value();
        let total_rows = self.data.len().div_ceil(bpr);
        if row >= total_rows {
            return None;
        }
        Some(row)
    }

    /// Returns (byte_offset, which_column) from mouse position.
    pub(super) fn mouse_to_offset(&self, ui: &dear_imgui_rs::Ui) -> Option<(usize, EditColumn)> {
        let [mx, my] = ui.io().mouse_pos();
        let [win_x, win_y] = ui.cursor_screen_pos();
        let scroll_y = ui.scroll_y();
        let header_offset = if self.config.show_column_headers {
            1
        } else {
            0
        };

        let rel_y = my - win_y + scroll_y;
        let row = (rel_y / self.line_height) as isize - header_offset as isize;
        if row < 0 {
            return None;
        }
        let row = row as usize;

        let bpr = self.config.bytes_per_row.value();
        let group = self.config.grouping.value();

        // Mirror the one-glyph left padding applied in `draw::render`.
        let origin_x = win_x + self.char_advance;
        let hex_x = origin_x + self.offset_col_width();
        // ASCII is right-anchored to the inner content edge (see
        // `HexViewer::ascii_col_x`); hit-test must use the same
        // position or clicks land on the wrong cell.
        let ascii_x = self.ascii_col_x(win_x);

        // Check ASCII column first.
        if self.config.show_ascii && mx >= ascii_x {
            let rel_x = mx - ascii_x;
            let col = (rel_x / self.char_advance) as usize;
            let offset = row * bpr + col.min(bpr - 1);
            if offset < self.data.len() {
                return Some((offset, EditColumn::Ascii));
            }
        }

        // Hex column.
        let rel_x = mx - hex_x;
        if rel_x < 0.0 {
            return None;
        }

        let mut col = 0usize;
        let mut x = 0.0f32;
        while col < bpr {
            let next_x = x + self.char_advance * 3.0;
            if rel_x < next_x {
                break;
            }
            x = next_x;
            col += 1;
            if group > 0 && col.is_multiple_of(group) && col < bpr {
                x += self.char_advance;
            }
        }

        let offset = row * bpr + col;
        if offset < self.data.len() {
            Some((offset, EditColumn::Hex))
        } else {
            None
        }
    }
}

// ── Free helpers ─────────────────────────────────────────────────────────────

pub(super) fn read_input_chars() -> Vec<char> {
    // SAFETY: `igGetIO_Nil` returns a pointer to ImGui's process-wide IO
    // singleton, valid for the entire ImGui context lifetime. This
    // helper is only called from `handle_*_input`, which themselves run
    // inside `HexViewer::render` — so a frame is active and the IO
    // struct (and its `InputQueueCharacters` ImVector) are live.
    let io = unsafe { &*dear_imgui_rs::sys::igGetIO_Nil() };
    let data = io.InputQueueCharacters.Data;
    let size = io.InputQueueCharacters.Size;
    if data.is_null() || size <= 0 {
        return Vec::new();
    }
    // SAFETY: `ImWchar` is `u16` in the cimgui build we link against
    // (no `IMGUI_USE_WCHAR32` define). `Data` + `Size` describe a
    // contiguous ImGui-owned buffer that stays alive at least until
    // the end of the current frame, so the slice is valid here.
    let slice = unsafe { std::slice::from_raw_parts(data as *const u16, size as usize) };
    slice
        .iter()
        .filter_map(|&c| char::from_u32(c as u32))
        .collect()
}
