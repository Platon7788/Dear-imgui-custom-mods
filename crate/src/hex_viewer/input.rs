//! Keyboard handling + edit-column enum + the raw char-queue reader.
//!
//! The edit-lifecycle / undo / copy helpers live in [`super::edit`] and
//! the mouse / hit-test paths in [`super::mouse`]; both route through
//! `move_cursor_with_selection_with` so shift-extends, selection clears
//! and pending-nibble flushes stay consistent across arrow keys,
//! Page/Home/End, F3, and explicit `goto`.

use super::HexViewer;
use super::provider::HexDataProvider;
use super::search::Selection;
use super::undo::UndoEntry;

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

// ── HexViewer impl: keyboard ─────────────────────────────────────────────────

impl HexViewer {
    pub(super) fn handle_keyboard(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn HexDataProvider,
    ) {
        use dear_imgui_rs::Key;

        let bpr = self.config.bytes_per_row.value();
        let len = super::draw::provider_len_usize(provider);
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
            self.copy_selection_with(provider);
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
            // Move cursor to the end of the selection so a
            // subsequent shift+arrow re-anchors at `len-1` (the
            // current cursor) rather than re-anchoring at the
            // OLD cursor position and silently shrinking the
            // select-all to `[old_cursor..new]`.
            self.cursor = len.saturating_sub(1);
        }
        if ctrl && !shift && ui.is_key_pressed(Key::Z) {
            self.undo_with(provider);
        }
        if ctrl && ui.is_key_pressed(Key::Y) {
            self.redo_with(provider);
        }
        if ctrl && shift && ui.is_key_pressed(Key::Z) {
            self.redo_with(provider);
        }

        // F3 = next/prev search result. Commit any half-typed nibble
        // first so jumping between matches mid-edit never silently
        // drops the user's pending input — consistent with every other
        // navigation path (arrows / Page / Home / End all flush via
        // `move_cursor_with_selection_with`).
        if ui.is_key_pressed(Key::F3) && !self.search_results.is_empty() {
            self.commit_pending_edit_with(provider);
            if shift {
                self.search_idx = self
                    .search_idx
                    .checked_sub(1)
                    .unwrap_or(self.search_results.len() - 1);
            } else {
                self.search_idx = (self.search_idx + 1) % self.search_results.len();
            }
            self.cursor = self.search_results[self.search_idx].min(len.saturating_sub(1));
            // `find_pattern_masked` guarantees `start + pattern.len() <= len`,
            // but clamp the selection end defensively in case the buffer
            // shrank between the search and this F3 press (streaming host
            // re-anchored a smaller window).
            self.selection = Selection {
                start: self.cursor,
                end: (self.cursor + self.search_pattern.len()).min(len),
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

        // Navigation (all paths share `move_cursor_with_selection_with`
        // so Shift-extends and selection-clear behave identically
        // across arrows / PageUp-Down / Home-End, and the active
        // provider sees the pending-nibble commit).
        if !ctrl && !alt {
            if ui.is_key_pressed(Key::LeftArrow) {
                let new = self.cursor.saturating_sub(1);
                self.move_cursor_with_selection_with(new, shift, provider);
            }
            if ui.is_key_pressed(Key::RightArrow) {
                let new = (self.cursor + 1).min(len - 1);
                self.move_cursor_with_selection_with(new, shift, provider);
            }
            if ui.is_key_pressed(Key::UpArrow) {
                let new = self.cursor.saturating_sub(bpr);
                self.move_cursor_with_selection_with(new, shift, provider);
            }
            if ui.is_key_pressed(Key::DownArrow) {
                let new = (self.cursor + bpr).min(len - 1);
                self.move_cursor_with_selection_with(new, shift, provider);
            }
            if ui.is_key_pressed(Key::PageUp) {
                let rows = (ui.window_size()[1] / self.line_height) as usize;
                let new = self.cursor.saturating_sub(bpr * rows);
                self.move_cursor_with_selection_with(new, shift, provider);
            }
            if ui.is_key_pressed(Key::PageDown) {
                let rows = (ui.window_size()[1] / self.line_height) as usize;
                let new = (self.cursor + bpr * rows).min(len - 1);
                self.move_cursor_with_selection_with(new, shift, provider);
            }
            if ui.is_key_pressed(Key::Home) {
                let new = self.cursor - self.cursor % bpr;
                self.move_cursor_with_selection_with(new, shift, provider);
            }
            if ui.is_key_pressed(Key::End) {
                let new = ((self.cursor / bpr + 1) * bpr - 1).min(len - 1);
                self.move_cursor_with_selection_with(new, shift, provider);
            }
        }

        // Ctrl+Home/End.
        if ctrl && ui.is_key_pressed(Key::Home) {
            self.move_cursor_with_selection_with(0, shift, provider);
        }
        if ctrl && ui.is_key_pressed(Key::End) {
            self.move_cursor_with_selection_with(len - 1, shift, provider);
        }

        // Hex / ASCII editing input — provider participates in the
        // write path (host's debug-target memory gets the new byte
        // before the user types the next one).
        if self.config.editable && !ctrl && !alt {
            match self.edit_column {
                Some(EditColumn::Hex) => self.handle_hex_input(provider),
                Some(EditColumn::Ascii) => self.handle_ascii_input(provider),
                None => {}
            }
        }
    }

    fn handle_hex_input(&mut self, provider: &mut dyn HexDataProvider) {
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
                let data_len = super::draw::provider_len_usize(provider);
                if self.cursor < data_len {
                    // Read the OLD byte through the provider so the
                    // undo entry / `byte_edit_callback` carry the
                    // live value the host actually had on disk /
                    // in memory (matters for streaming providers
                    // where `self.data` is empty or stale).
                    let mut rb = [0u8; 1];
                    let n = provider.read(self.cursor as u64, &mut rb);
                    let old_byte = if n > 0 {
                        rb[0]
                    } else if self.cursor < self.data.len() {
                        self.data[self.cursor]
                    } else {
                        0
                    };
                    self.undo.push(UndoEntry {
                        offset: self.cursor as u64,
                        old_bytes: vec![old_byte],
                        new_bytes: vec![new_byte],
                    });
                    // Same dual-write rationale as
                    // `commit_pending_edit_with` — provider keeps
                    // current-frame visuals coherent; `self.data`
                    // patch keeps next-frame / `data()` getter
                    // consistent.
                    let _ = provider.write(self.cursor as u64, &[new_byte]);
                    if self.cursor < self.data.len() {
                        let cur = self.cursor;
                        std::sync::Arc::make_mut(&mut self.data)[cur] = new_byte;
                    }
                    // Fire host callback after the mutation so the
                    // host can chain its own propagation (drives a
                    // debug-target memory write for streaming
                    // providers; here the provider may have already
                    // queued the write inside `write()`).
                    if let Some(cb) = self.byte_edit_callback.as_mut() {
                        let va = self.config.base_address + self.cursor as u64;
                        cb(va, old_byte, new_byte);
                    }
                    if self.cursor + 1 < data_len {
                        self.cursor += 1;
                    }
                }
            } else {
                self.edit_nibble = Some(nibble);
            }
        }
    }

    fn handle_ascii_input(&mut self, provider: &mut dyn HexDataProvider) {
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
            let data_len = super::draw::provider_len_usize(provider);
            if self.cursor < data_len {
                let mut rb = [0u8; 1];
                let n = provider.read(self.cursor as u64, &mut rb);
                let old_byte = if n > 0 {
                    rb[0]
                } else if self.cursor < self.data.len() {
                    self.data[self.cursor]
                } else {
                    0
                };
                self.undo.push(UndoEntry {
                    offset: self.cursor as u64,
                    old_bytes: vec![old_byte],
                    new_bytes: vec![new_byte],
                });
                // Dual-write — see `commit_pending_edit_with`.
                let _ = provider.write(self.cursor as u64, &[new_byte]);
                if self.cursor < self.data.len() {
                    let cur = self.cursor;
                    std::sync::Arc::make_mut(&mut self.data)[cur] = new_byte;
                }
                if let Some(cb) = self.byte_edit_callback.as_mut() {
                    let va = self.config.base_address + self.cursor as u64;
                    cb(va, old_byte, new_byte);
                }
                if self.cursor + 1 < data_len {
                    self.cursor += 1;
                }
            }
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
