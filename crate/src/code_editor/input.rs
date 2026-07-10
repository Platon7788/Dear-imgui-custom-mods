//! Keyboard + mouse input handling for [`CodeEditor`].
//!
//! Split out of mod.rs. `handle_keyboard` / `handle_mouse` mutate the
//! buffer, scroll state, and find/replace state in response to ImGui input.

use super::*;

impl CodeEditor {
    // ── Input handling ───────────────────────────────────────────────

    pub(super) fn handle_keyboard(&mut self, ui: &Ui) {
        let io = ui.io();
        // Accept ⌘ (Super) as the command modifier on macOS so shortcuts work
        // natively; on other platforms Super is the Win/Meta key and must not
        // trigger editor shortcuts.
        let ctrl = io.key_ctrl() || (cfg!(target_os = "macos") && io.key_super());
        let shift = io.key_shift();
        let alt = io.key_alt();

        // ── Navigation keys ─────────────────────────────────────────

        // Helper macro for nav keys that do NOT collapse selection (Up, Down,
        // Home, End, word movement, doc start/end).
        macro_rules! nav_key {
            ($key:ident, $action:expr) => {
                if ui.is_key_pressed(Key::$key) {
                    let anchor = if shift {
                        Some(
                            self.buffer
                                .selection()
                                .map_or(self.buffer.cursor(), |s| s.anchor),
                        )
                    } else {
                        None
                    };
                    if !shift {
                        self.buffer.clear_selection();
                    }
                    $action;
                    if let Some(a) = anchor {
                        self.buffer.set_selection(a, self.buffer.cursor());
                    }
                    self.reset_blink();
                    self.ensure_cursor_visible();
                }
            };
        }

        // Left/Right arrows: collapse selection to start/end when selection
        // is active and Shift is NOT held (standard editor behaviour).
        // Without this, pressing Left with a selection would clear_selection
        // then move_left, landing one char *before* the selection start.
        macro_rules! nav_lr {
            ($key:ident, $move_action:expr, $collapse_end:expr) => {
                if ui.is_key_pressed(Key::$key) {
                    if shift {
                        let anchor = self
                            .buffer
                            .selection()
                            .map_or(self.buffer.cursor(), |s| s.anchor);
                        $move_action;
                        self.buffer.set_selection(anchor, self.buffer.cursor());
                    } else if let Some(sel) = self.buffer.selection().filter(|s| !s.is_empty()) {
                        // Collapse to the appropriate end of the selection
                        let (start, end) = sel.ordered();
                        let target = $collapse_end(start, end);
                        self.buffer.set_cursor_clear_sel(target);
                    } else {
                        self.buffer.clear_selection();
                        $move_action;
                    }
                    self.reset_blink();
                    self.ensure_cursor_visible();
                }
            };
        }

        if ctrl {
            nav_lr!(
                LeftArrow,
                self.buffer.move_word_left(),
                |start: CursorPos, _end: CursorPos| start
            );
            nav_lr!(
                RightArrow,
                self.buffer.move_word_right(),
                |_start: CursorPos, end: CursorPos| end
            );
            nav_key!(Home, self.buffer.move_doc_start());
            nav_key!(End, self.buffer.move_doc_end());
        } else {
            nav_lr!(
                LeftArrow,
                self.buffer.move_left(),
                |start: CursorPos, _end: CursorPos| start
            );
            nav_lr!(
                RightArrow,
                self.buffer.move_right(),
                |_start: CursorPos, end: CursorPos| end
            );
            if !alt {
                nav_key!(UpArrow, self.buffer.move_up());
                nav_key!(DownArrow, self.buffer.move_down());
            }
            nav_key!(Home, self.buffer.move_home());
            nav_key!(End, self.buffer.move_end());
        }

        // PageUp / PageDown
        for (key, sign) in [(Key::PageUp, -1isize), (Key::PageDown, 1isize)] {
            if ui.is_key_pressed(key) {
                let lines = sign * self.visible_lines() as isize;
                let anchor = if shift {
                    Some(
                        self.buffer
                            .selection()
                            .map_or(self.buffer.cursor(), |s| s.anchor),
                    )
                } else {
                    None
                };
                if !shift {
                    self.buffer.clear_selection();
                }
                self.buffer.move_page(lines);
                if let Some(a) = anchor {
                    self.buffer.set_selection(a, self.buffer.cursor());
                }
                self.reset_blink();
                self.ensure_cursor_visible();
            }
        }

        // ── Ctrl shortcuts ──────────────────────────────────────────
        // One-shot commands use the no-repeat variant so holding the key
        // (e.g. Ctrl+V, Ctrl+Z) fires once instead of auto-repeating paste/undo.
        if ctrl && ui.is_key_pressed_with_repeat(Key::A, false) {
            self.buffer.select_all();
            return;
        }

        if ctrl && ui.is_key_pressed_with_repeat(Key::C, false) {
            let text = self.buffer.selected_text();
            if !text.is_empty() {
                set_clipboard(&text);
            }
            return;
        }

        if ctrl && ui.is_key_pressed_with_repeat(Key::X, false) && !self.config.read_only {
            let text = self.buffer.selected_text();
            if !text.is_empty() {
                set_clipboard(&text);
                self.snapshot_undo(true);
                self.buffer.backspace();
                self.invalidate_token_cache_from(self.buffer.cursor().line);
                self.reset_blink();
            }
            return;
        }

        if ctrl && ui.is_key_pressed_with_repeat(Key::V, false) && !self.config.read_only {
            if let Some(clip) = get_clipboard()
                && !clip.is_empty()
            {
                // Truncate pasted text to respect max_lines / max_line_length.
                let clip = self.truncate_paste(&clip);
                if !clip.is_empty() {
                    self.snapshot_undo(true);
                    let paste_line = self.buffer.cursor().line;
                    self.buffer.insert_text(&clip);
                    self.invalidate_token_cache_from(paste_line);
                    self.reset_blink();
                    self.ensure_cursor_visible();
                }
            }
            return;
        }

        if ctrl && ui.is_key_pressed_with_repeat(Key::Z, false) && !self.config.read_only {
            self.undo();
            return;
        }

        if ctrl && ui.is_key_pressed_with_repeat(Key::Y, false) && !self.config.read_only {
            self.redo();
            return;
        }

        // ── Find/Replace shortcuts ──────────────────────────────────
        if ctrl && ui.is_key_pressed(Key::F) {
            // Pre-fill with selection if any
            let sel = self.buffer.selected_text();
            if !sel.is_empty() && !sel.contains('\n') {
                self.find_replace.query = sel;
            }
            self.find_replace.open = true;
            self.find_replace.show_replace = false;
            self.find_replace.just_opened = true;
            self.update_find_matches();
            return;
        }

        if ctrl && ui.is_key_pressed(Key::H) && !self.config.read_only {
            let sel = self.buffer.selected_text();
            if !sel.is_empty() && !sel.contains('\n') {
                self.find_replace.query = sel;
            }
            self.find_replace.open = true;
            self.find_replace.show_replace = true;
            self.find_replace.just_opened = true;
            self.update_find_matches();
            return;
        }

        // Escape closes find panel. Alt is excluded — same reasoning as the
        // Tab guard above: Alt+Escape is a legacy Windows window-switch
        // shortcut and must not be swallowed as "close the find bar".
        if !alt && ui.is_key_pressed(Key::Escape) && self.find_replace.open {
            self.find_replace.open = false;
            return;
        }

        // F3 / Ctrl+G: next match;  Shift+F3: previous match
        if shift && ui.is_key_pressed(Key::F3) {
            self.find_prev();
            return;
        }
        if ui.is_key_pressed(Key::F3) || (ctrl && ui.is_key_pressed(Key::G)) {
            self.find_next();
            return;
        }

        // ── Comment toggling (Ctrl+/) ───────────────────────────────
        if ctrl && ui.is_key_pressed(Key::Slash) && !self.config.read_only {
            self.snapshot_undo(true);
            let (start, end) = if let Some(sel) = self.buffer.selection() {
                let (s, e) = sel.ordered();
                (s.line, e.line)
            } else {
                let l = self.buffer.cursor().line;
                (l, l)
            };
            self.buffer.toggle_line_comment(start..end + 1);
            self.invalidate_token_cache_all();
            return;
        }

        // ── Line operations ─────────────────────────────────────────
        if !self.config.read_only {
            // Alt+Up: move line up
            if alt && ui.is_key_pressed(Key::UpArrow) {
                self.snapshot_undo(true);
                self.buffer.move_line_up();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                return;
            }

            // Alt+Down: move line down
            if alt && ui.is_key_pressed(Key::DownArrow) {
                self.snapshot_undo(true);
                self.buffer.move_line_down();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                return;
            }

            // Ctrl+Shift+D: duplicate line
            if ctrl && shift && ui.is_key_pressed(Key::D) {
                self.snapshot_undo(true);
                self.buffer.duplicate_line();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                return;
            }

            // Alt+Shift+DownArrow: duplicate line (VSCode convention,
            // alternative to Ctrl+Shift+D which conflicts with "select
            // next occurrence" on some keymaps).
            if alt && shift && ui.is_key_pressed(Key::DownArrow) {
                self.snapshot_undo(true);
                self.buffer.duplicate_line();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                return;
            }

            // Ctrl+Shift+K: delete line
            if ctrl && shift && ui.is_key_pressed(Key::K) {
                self.snapshot_undo(true);
                self.buffer.delete_line();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                return;
            }

            // Ctrl+L: select current line (VSCode / IntelliJ convention).
            // Extends selection to include the line break if one exists.
            if ctrl && !shift && !alt && ui.is_key_pressed(Key::L) {
                self.buffer.select_line();
                return;
            }

            // Ctrl+D: select next occurrence (add cursor)
            if ctrl && !shift && ui.is_key_pressed(Key::D) {
                // Get current word under cursor or selected text
                let needle = {
                    let sel_text = self.buffer.selected_text();
                    if sel_text.is_empty() {
                        // Select word under cursor first
                        self.buffer.select_word_at_cursor();
                        self.buffer.selected_text()
                    } else {
                        sel_text
                    }
                };

                if !needle.is_empty() {
                    // Find next occurrence after the last cursor
                    let all = self.buffer.all_cursors_sorted();
                    let search_from = all.last().copied().unwrap_or(self.buffer.cursor());
                    if let Some((start, end)) =
                        self.buffer.find_next_occurrence(&needle, search_from)
                    {
                        let sel = Selection {
                            anchor: start,
                            cursor: end,
                        };
                        self.buffer.add_cursor_with_selection(end, sel);
                    }
                }
                self.reset_blink();
                return;
            }

            // Escape: clear extra cursors (if any) before other Escape
            // behavior. Alt excluded for the same Alt+Escape reason as above.
            if !alt && ui.is_key_pressed(Key::Escape) && self.buffer.has_extra_cursors() {
                self.buffer.clear_extra_cursors();
                return;
            }
        }

        // ── Editing keys ────────────────────────────────────────────
        if !self.config.read_only {
            if ui.is_key_pressed(Key::Enter) || ui.is_key_pressed(Key::KeypadEnter) {
                // Enforce max_lines limit
                if self.config.max_lines > 0 && self.buffer.line_count() >= self.config.max_lines {
                    return;
                }
                self.snapshot_undo(true);
                let split_line = self.buffer.cursor().line;
                self.buffer
                    .insert_newline(self.config.auto_indent, self.config.tab_size);
                self.invalidate_token_cache_from(split_line);
                self.reset_blink();
                self.ensure_cursor_visible();
                return;
            }

            if ui.is_key_pressed(Key::Backspace) {
                self.snapshot_undo(self.buffer.has_extra_cursors() || ctrl);
                if self.buffer.has_extra_cursors() && !ctrl {
                    self.buffer.multi_backspace();
                    self.invalidate_token_cache_all();
                } else if ctrl {
                    self.buffer.delete_word_left();
                    self.invalidate_token_cache_from(self.buffer.cursor().line);
                } else {
                    self.buffer.backspace();
                    self.invalidate_token_cache_from(self.buffer.cursor().line);
                }
                self.reset_blink();
                self.ensure_cursor_visible();
                return;
            }

            if ui.is_key_pressed(Key::Delete) {
                self.snapshot_undo(self.buffer.has_extra_cursors() || ctrl);
                if self.buffer.has_extra_cursors() && !ctrl {
                    self.buffer.multi_delete();
                    self.invalidate_token_cache_all();
                } else if ctrl {
                    self.buffer.delete_word_right();
                    self.invalidate_token_cache_from(self.buffer.cursor().line);
                } else {
                    self.buffer.delete();
                    self.invalidate_token_cache_from(self.buffer.cursor().line);
                }
                self.reset_blink();
                self.ensure_cursor_visible();
                return;
            }

            // Alt is excluded: Windows delivers WM_SYSKEYDOWN(VK_TAB) to the
            // still-focused window *before* the OS task-switcher actually
            // moves focus away, so for at least one frame `io.key_alt()` and
            // "Tab just pressed" are both true here even though the user is
            // invoking Alt+Tab, not asking for an indent. Let it fall
            // through unhandled so the OS shortcut isn't shadowed by a
            // spurious tab/space insertion (mirrors the `alt` guard already
            // used above for Alt+Up/Down and below for Ctrl+Alt+L).
            if !alt && ui.is_key_pressed(Key::Tab) {
                if let Some(sel) = self.buffer.selection() {
                    let (start, end) = sel.ordered();
                    if start.line != end.line {
                        self.snapshot_undo(true);
                        if shift {
                            self.buffer
                                .unindent_lines(start.line..end.line + 1, self.config.tab_size);
                        } else {
                            self.buffer.indent_lines(
                                start.line..end.line + 1,
                                self.config.tab_size,
                                self.config.insert_spaces,
                            );
                        }
                        self.invalidate_token_cache_from(start.line);
                        return;
                    }
                }
                // Single-line tab insert
                self.snapshot_undo(false);
                if self.config.insert_spaces {
                    let cur_col = self.buffer.cursor().col;
                    let spaces = tab_stop_spaces(self.config.tab_size, cur_col);
                    self.buffer.insert_text(&" ".repeat(spaces));
                } else {
                    self.buffer.insert_char('\t');
                }
                self.invalidate_token_cache_at(self.buffer.cursor().line);
                self.reset_blink();
                return;
            }

            // ── Text input (typed characters) ───────────────────────
            self.handle_text_input();
        }
    }
}
