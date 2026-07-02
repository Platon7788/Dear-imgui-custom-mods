//! Find bar + right-click context-menu UI for [`CodeEditor`].
//! Split out of find_glue.rs (500-line rule); the search/replace logic
//! stays in find_glue.rs, the chrome lives here.

use super::*;

impl CodeEditor {
    pub(super) fn render_context_menu(&mut self, ui: &Ui) {
        // Apply popup padding/spacing/rounding consistent with other themed
        // popups in the crate. ImGui defaults are tight (text touches the
        // border); these match `utils::themed_popup_style`.
        // Guard order matters: the popup must drop BEFORE the style guards
        // so the popup body sees the pushed style.
        let _pad = ui.push_style_var(StyleVar::WindowPadding([12.0, 10.0]));
        let _spc = ui.push_style_var(StyleVar::ItemSpacing([10.0, 6.0]));
        let _frame_pad = ui.push_style_var(StyleVar::FramePadding([10.0, 5.0]));
        let _round = ui.push_style_var(StyleVar::WindowRounding(6.0));
        let _frame_round = ui.push_style_var(StyleVar::FrameRounding(4.0));
        let Some(_popup) = ui.begin_popup("##editor_ctx") else {
            return;
        };
        let has_sel = self.buffer.selection().is_some();
        let ro = self.config.read_only;
        let cm = self.config.context_menu.clone();
        let s = crate::i18n::code_editor::strings(self.config.locale);
        let locale = self.config.locale;

        // ── Clipboard ────────────────────────────────────────────────────────
        if cm.show_clipboard {
            if ui.menu_item_enabled_selected_with_shortcut(
                s.menu_cut,
                "Ctrl+X",
                false,
                has_sel && !ro,
            ) {
                let text = self.buffer.selected_text();
                if !text.is_empty() {
                    set_clipboard(&text);
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.invalidate_token_cache_all();
                    self.reset_blink();
                }
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(s.menu_copy, "Ctrl+C", false, has_sel) {
                let text = self.buffer.selected_text();
                if !text.is_empty() {
                    set_clipboard(&text);
                }
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(s.menu_paste, "Ctrl+V", false, !ro) {
                if let Some(clip) = get_clipboard()
                    && !clip.is_empty()
                {
                    self.snapshot_undo(true);
                    self.buffer.insert_text(&clip);
                    self.invalidate_token_cache_all();
                    self.reset_blink();
                    self.ensure_cursor_visible();
                }
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── Select All ───────────────────────────────────────────────────────
        if cm.show_select_all {
            if ui.menu_item_with_shortcut(s.menu_select_all, "Ctrl+A") {
                self.buffer.select_all();
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── Undo / Redo ──────────────────────────────────────────────────────
        if cm.show_undo_redo {
            if ui.menu_item_enabled_selected_with_shortcut(
                s.menu_undo,
                "Ctrl+Z",
                false,
                !ro && self.undo_stack.can_undo(),
            ) {
                self.undo();
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(
                s.menu_redo,
                "Ctrl+Y",
                false,
                !ro && self.undo_stack.can_redo(),
            ) {
                self.redo();
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── Code actions ─────────────────────────────────────────────────────
        if cm.show_code_actions {
            if ui.menu_item_enabled_selected_with_shortcut(
                s.menu_toggle_comment,
                "Ctrl+/",
                false,
                !ro,
            ) {
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
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(
                s.menu_duplicate_line,
                "Ctrl+Shift+D",
                false,
                !ro,
            ) {
                self.snapshot_undo(true);
                self.buffer.duplicate_line();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(
                s.menu_delete_line,
                "Ctrl+Shift+K",
                false,
                !ro,
            ) {
                self.snapshot_undo(true);
                self.buffer.delete_line();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── Transform submenu ────────────────────────────────────────────────
        if cm.show_transform && !ro && has_sel {
            if let Some(_m) = ui.begin_menu(s.submenu_transform) {
                if ui.menu_item(s.menu_uppercase) {
                    let t = self.buffer.selected_text().to_uppercase();
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.buffer.insert_text(&t);
                    self.invalidate_token_cache_all();
                    ui.close_current_popup();
                }
                if ui.menu_item(s.menu_lowercase) {
                    let t = self.buffer.selected_text().to_lowercase();
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.buffer.insert_text(&t);
                    self.invalidate_token_cache_all();
                    ui.close_current_popup();
                }
                if ui.menu_item(s.menu_title_case) {
                    let t = title_case(&self.buffer.selected_text());
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.buffer.insert_text(&t);
                    self.invalidate_token_cache_all();
                    ui.close_current_popup();
                }
                if ui.menu_item(s.menu_trim_whitespace) {
                    let t = self
                        .buffer
                        .selected_text()
                        .lines()
                        .map(str::trim_end)
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.buffer.insert_text(&t);
                    self.invalidate_token_cache_all();
                    ui.close_current_popup();
                }
            }
            ui.separator();
        }

        // ── Find ─────────────────────────────────────────────────────────────
        if cm.show_find {
            if ui.menu_item_enabled_selected_with_shortcut(s.menu_find, "Ctrl+F", false, true) {
                let sel = self.buffer.selected_text();
                if !sel.is_empty() && !sel.contains('\n') {
                    self.find_replace.query = sel;
                }
                self.find_replace.open = true;
                self.find_replace.show_replace = false;
                self.find_replace.just_opened = true;
                self.update_find_matches();
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── View submenu ─────────────────────────────────────────────────────
        if cm.show_view_toggles
            && let Some(_m) = ui.begin_menu(s.submenu_view)
        {
            macro_rules! toggle {
                ($label:expr, $field:expr) => {
                    if ui.menu_item_enabled_selected_no_shortcut($label, $field, true) {
                        $field = !$field;
                    }
                };
            }
            toggle!(s.view_word_wrap, self.config.word_wrap);
            toggle!(s.view_line_numbers, self.config.show_line_numbers);
            toggle!(
                s.view_highlight_current_line,
                self.config.highlight_current_line
            );
            toggle!(s.view_show_whitespace, self.config.show_whitespace);
            toggle!(s.view_color_swatches, self.config.show_color_swatches);
            toggle!(s.view_smooth_scrolling, self.config.smooth_scrolling);
            toggle!(s.view_english_on_focus, self.config.force_english_on_focus);
        }

        // ── Language submenu ─────────────────────────────────────────────────
        if cm.show_language_selector
            && let Some(_m) = ui.begin_menu(s.submenu_language)
        {
            // Programming-language identifiers (Rust / RON / JSON / TOML / …)
            // stay untranslated — they're proper nouns. Only the
            // catch-all "Plain Text" entry follows the locale.
            for (lang, name) in [
                (Language::Rust, "Rust"),
                (Language::Rhai, "Rhai"),
                (Language::Toml, "TOML"),
                (Language::Ron, "RON"),
                (Language::Json, "JSON"),
                (Language::Yaml, "YAML"),
                (Language::Xml, "XML / HTML"),
                (Language::Asm, "Assembly (x86)"),
                (Language::Hex, "Hex Bytes"),
                (Language::Sql, "SQL"),
                (Language::Diff, "Diff / Patch"),
                (Language::Ini, "INI"),
                (Language::Dockerfile, "Dockerfile"),
                (Language::Markdown, "Markdown"),
                (Language::None, s.language_plain_text),
            ] {
                let selected = self.config.language == lang;
                if ui.menu_item_enabled_selected_no_shortcut(name, selected, true) {
                    self.config.language = lang;
                    self.invalidate_token_cache_all();
                }
            }
            // Show custom language name (read-only — can't switch away via menu)
            if let Language::Custom(ref def) = self.config.language.clone() {
                ui.separator();
                ui.text_disabled(format!("{}{}", s.custom_language_prefix, def.name()));
            }
        }

        // ── Theme submenu ─────────────────────────────────────────────────────
        if cm.show_theme_selector
            && let Some(_m) = ui.begin_menu(s.submenu_theme)
        {
            for &theme in EditorTheme::ALL {
                let selected = self.config.theme == theme;
                if ui.menu_item_enabled_selected_no_shortcut(theme.display_name(), selected, true) {
                    self.config.set_theme(theme);
                    self.invalidate_token_cache_all();
                }
            }
            ui.separator();
        }

        // ── Font size ±────────────────────────────────────────────────────────
        if cm.show_font_size {
            ui.text(s.font_scale_label);
            ui.same_line();
            let dec_lbl = format!("{}##fsd", icons::FORMAT_FONT_SIZE_DECREASE);
            if ui.small_button(&dec_lbl) {
                self.config.font_size_scale = (self.config.font_size_scale - 0.1).clamp(0.4, 4.0);
            }
            if ui.is_item_hovered() {
                crate::utils::themed_tooltip(ui, || ui.text(s.tip_decrease_font));
            }
            ui.same_line();
            ui.text(format!("{:.0}%", self.config.font_size_scale * 100.0));
            ui.same_line();
            let inc_lbl = format!("{}##fsi", icons::FORMAT_FONT_SIZE_INCREASE);
            if ui.small_button(&inc_lbl) {
                self.config.font_size_scale = (self.config.font_size_scale + 0.1).clamp(0.4, 4.0);
            }
            if ui.is_item_hovered() {
                crate::utils::themed_tooltip(ui, || ui.text(s.tip_increase_font));
            }
            ui.separator();
        }

        // ── Cursor info ───────────────────────────────────────────────────────
        if cm.show_cursor_info {
            let cur = self.buffer.cursor();
            let total = self.buffer.line_count();
            ui.text_disabled(crate::i18n::code_editor::cursor_info(
                locale,
                cur.line + 1,
                cur.col + 1,
                total,
            ));
        }
    }

    pub(super) fn render_find_replace_bar(&mut self, ui: &Ui) {
        let s = crate::i18n::code_editor::strings(self.config.locale);
        let avail_w = ui.content_region_avail()[0];
        // Row height: search row + optional replace row + 2px separator
        let row_h = self.line_height + 8.0;
        let bar_h = if self.find_replace.show_replace && !self.config.read_only {
            row_h * 2.0 + 4.0
        } else {
            row_h
        };

        // Toolbar background from the theme gutter panel colour (was a
        // hardcoded dark box that clashed on light themes).
        let _bg = ui.push_style_color(StyleColor::ChildBg, self.config.colors.gutter_bg);

        ui.child_window("##find_bar")
            .size([avail_w, bar_h])
            .build(ui, || {
                // ── Row 1: Find ──────────────────────────────────────────
                ui.spacing();

                // Auto-focus the input field the frame the bar opens
                if self.find_replace.just_opened {
                    // SAFETY: igSetKeyboardFocusHere sets focus on the next item.
                    unsafe {
                        dear_imgui_rs::sys::igSetKeyboardFocusHere(0);
                    }
                    self.find_replace.just_opened = false;
                }

                // Search icon + input
                ui.text_disabled(icons::MAGNIFY);
                ui.same_line();
                let query_w = (avail_w * 0.38).clamp(140.0, 360.0);
                ui.set_next_item_width(query_w);
                let changed = ui
                    .input_text("##find_query", &mut self.find_replace.query)
                    .hint(s.find_hint)
                    .build();
                if changed {
                    self.update_find_matches();
                    self.find_replace.current_match = 0;
                }

                // Navigate with Enter / Shift+Enter in the search field
                if ui.is_item_focused() {
                    let io = ui.io();
                    // Escape closes the bar even while the query field owns
                    // focus (its default state after Ctrl+F) — handle_keyboard
                    // only runs when the editor child is focused, so it can't.
                    if ui.is_key_pressed(Key::Escape) {
                        self.find_replace.open = false;
                    }
                    if ui.is_key_pressed(Key::Enter) || ui.is_key_pressed(Key::DownArrow) {
                        self.find_next();
                    }
                    if (io.key_shift() && ui.is_key_pressed(Key::Enter))
                        || ui.is_key_pressed(Key::UpArrow)
                    {
                        self.find_prev();
                    }
                }

                ui.same_line();

                // Match counter  "3 / 17"  or "No matches" in red
                if self.find_replace.query.is_empty() {
                    ui.text_disabled("…");
                } else if self.find_replace.matches.is_empty() {
                    ui.text_colored(self.config.colors.error_underline, s.no_matches);
                } else {
                    ui.text_colored(
                        self.config.colors.line_number_active,
                        format!(
                            "{} / {}",
                            self.find_replace.current_match + 1,
                            self.find_replace.matches.len()
                        ),
                    );
                }

                ui.same_line();

                // Prev / Next buttons
                let prev_lbl = format!("{}##fp", icons::ARROW_UP_BOLD);
                if ui.small_button(&prev_lbl) {
                    self.find_prev();
                }
                if ui.is_item_hovered() {
                    crate::utils::themed_tooltip(ui, || ui.text(s.tip_prev_match));
                }
                ui.same_line();
                let next_lbl = format!("{}##fn", icons::ARROW_DOWN_BOLD);
                if ui.small_button(&next_lbl) {
                    self.find_next();
                }
                if ui.is_item_hovered() {
                    crate::utils::themed_tooltip(ui, || ui.text(s.tip_next_match));
                }

                ui.same_line();

                // ── Toggle: case-sensitive ───────────────────────────────
                let cs_col = if self.find_replace.case_sensitive {
                    [0.24, 0.52, 0.88, 0.90]
                } else {
                    [0.28, 0.30, 0.36, 0.70]
                };
                let _c = ui.push_style_color(StyleColor::Button, cs_col);
                let cs_lbl = format!("{}##fcs", icons::FORMAT_LETTER_CASE);
                if ui.small_button(&cs_lbl) {
                    self.find_replace.case_sensitive = !self.find_replace.case_sensitive;
                    // Lowercase cache becomes meaningless in case-sensitive
                    // mode; invalidate so we don't hand out stale strings the
                    // next time the user toggles back.
                    self.find_replace.invalidate_lowercase_cache();
                    self.update_find_matches();
                }
                drop(_c);
                if ui.is_item_hovered() {
                    crate::utils::themed_tooltip(ui, || ui.text(s.tip_case_sensitive));
                }

                ui.same_line();

                // ── Toggle: whole word ───────────────────────────────────
                let ww_col = if self.find_replace.whole_word {
                    [0.24, 0.52, 0.88, 0.90]
                } else {
                    [0.28, 0.30, 0.36, 0.70]
                };
                let _w = ui.push_style_color(StyleColor::Button, ww_col);
                let ww_lbl = format!("{}##fww", icons::FORMAT_LETTER_MATCHES);
                if ui.small_button(&ww_lbl) {
                    self.find_replace.whole_word = !self.find_replace.whole_word;
                    self.update_find_matches();
                }
                drop(_w);
                if ui.is_item_hovered() {
                    crate::utils::themed_tooltip(ui, || ui.text(s.tip_whole_word));
                }

                if !self.config.read_only {
                    ui.same_line();
                    // Toggle replace row
                    let rep_lbl = format!("{}##frep", icons::FIND_REPLACE);
                    if ui.small_button(&rep_lbl) {
                        self.find_replace.show_replace = !self.find_replace.show_replace;
                    }
                    if ui.is_item_hovered() {
                        crate::utils::themed_tooltip(ui, || ui.text(s.tip_toggle_replace));
                    }
                }

                ui.same_line();

                // Close button
                let close_lbl = format!("{}##fc", icons::CLOSE_THICK);
                if ui.small_button(&close_lbl) {
                    self.find_replace.open = false;
                }
                if ui.is_item_hovered() {
                    crate::utils::themed_tooltip(ui, || ui.text(s.tip_close));
                }

                // ── Row 2: Replace (only in writable editors) ────────────
                if self.find_replace.show_replace && !self.config.read_only {
                    ui.text_disabled(icons::FIND_REPLACE);
                    ui.same_line();
                    let rep_w = (avail_w * 0.38).clamp(140.0, 360.0);
                    ui.set_next_item_width(rep_w);
                    ui.input_text("##find_rep", &mut self.find_replace.replacement)
                        .hint(s.replace_hint)
                        .build();
                    ui.same_line();
                    let replace_lbl = format!("{}##r1", s.btn_replace);
                    if ui.small_button(&replace_lbl) {
                        self.replace_current();
                    }
                    ui.same_line();
                    let all_lbl = format!("{}##ra", s.btn_replace_all);
                    if ui.small_button(&all_lbl) {
                        self.replace_all();
                    }
                }
            });
    }
}
