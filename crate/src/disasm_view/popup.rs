//! Goto-address popup and right-click context menu for [`super::DisasmView`].
//!
//! Both popups share the crate-wide `themed_popup_style` look (same
//! padding / rounding as `hex_viewer`'s) and the centred-anchor
//! positioning so the goto popup always lands at the visual middle
//! of the viewer regardless of where the user pressed `G`.

use super::provider::DisasmDataProvider;
use super::{DisasmView, parse_address};
use crate::i18n;
use crate::utils::clipboard::set_clipboard;
use crate::utils::popup::{
    action_row_labeled, anchor_next_popup_centred, anchor_next_popup_topleft, compact_popup_body,
    themed_popup_style,
};

/// Reference width for the goto popup body (input + action row).
/// Matches `hex_viewer::popup::POPUP_INPUT_WIDTH` so both modules
/// host visually-identical goto dialogs.
const POPUP_INPUT_WIDTH: f32 = 360.0;

// `anchor_next_popup_centred` / `anchor_next_popup_topleft` moved to
// `crate::utils::popup` on 2026-05-04 so both `hex_viewer` and
// `disasm_view` share the same implementation. Imported above.

impl DisasmView {
    pub(super) fn render_goto_popup(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        // CRITICAL: `BeginPopup` runs every frame the popup is open,
        // not only on the trigger frame — see `hex_viewer::popup` for
        // the bug history. `OpenPopup` stays gated on the flag.
        if self.show_goto {
            anchor_next_popup_centred(self.component_center);
            ui.open_popup(&self.goto_popup_id);
            self.show_goto = false;
            self.goto_focus_pending = true;
        }

        let s = self.strings();
        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.goto_popup_id) {
                compact_popup_body(ui, || {
                    ui.text(s.goto_title);

                    if self.goto_focus_pending {
                        ui.set_keyboard_focus_here();
                        self.goto_focus_pending = false;
                    }
                    ui.set_next_item_width(POPUP_INPUT_WIDTH);
                    ui.input_text("##dv_goto_input", &mut self.goto_buf).build();

                    let (go_clicked, cancel_clicked) =
                        action_row_labeled(ui, POPUP_INPUT_WIDTH, s.action_go, s.action_cancel);
                    if cancel_clicked {
                        ui.close_current_popup();
                    }
                    if go_clicked {
                        if let Some(addr) = parse_address(&self.goto_buf) {
                            // Always notify the host — it owns the
                            // backing buffer and is the only place that
                            // can re-anchor + ReadMem if `addr` is
                            // outside the currently decoded range.
                            // `goto_address` here is best-effort: if
                            // `addr` happens to be in-range we get
                            // immediate scroll on the same frame; if
                            // not, the host's `forward_goto` will
                            // re-anchor and a later frame will land on
                            // the new instruction.
                            self.pending_goto_request = Some(addr);
                            self.goto_address(addr, provider);
                        }
                        ui.close_current_popup();
                    }
                });
            }
        });
    }

    /// Right-click context menu — Goto / Copy / Follow / Toggle bp /
    /// Watchpoints / Bookmark / Settings actions. Most icons follow
    /// atlas-safe glyph rules (Latin-1 + Arrows + General
    /// Punctuation); Settings + bookmark use MDI glyphs gated by
    /// `config.icons_available` (fallback to ASCII / Latin-1 when
    /// the host hasn't loaded the MDI atlas).
    ///
    /// Order (per user requests, last revised 2026-04-30):
    ///  1. Goto Address
    ///  2. Search bytes
    ///  3. Copy Address
    ///  4. Copy Instruction
    ///  5. — separator —
    ///  6. Follow (Enter / Space) — branch target or operand pointer
    ///  7. Jump to function start (Ctrl+Up)
    ///  8. Jump to function end (Ctrl+Down)
    ///  9. Select function (Ctrl+L)
    /// 10. Toggle Breakpoint (F9)
    /// 11. Toggle Watchpoint — single entry; host engine sorts
    ///     read-only / write-only on its side and reports the
    ///     union back via `Instruction::has_watchpoint`
    /// 12. Add / Remove Bookmark (Ctrl+B)
    /// 13. — separator —
    /// 14. Settings
    pub(super) fn render_context_menu(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        if self.show_context_menu {
            // Anchor only when the right-click handler captured a
            // position this session — `None` means the popup was
            // triggered without a mouse anchor (toolbar / keyboard
            // shortcut, hypothetical) and ImGui's default cursor
            // anchor is the right fallback (audit M7).
            if let Some(pos) = self.popup_open_pos {
                anchor_next_popup_topleft(pos);
            }
            ui.open_popup(&self.ctx_popup_id);
            self.show_context_menu = false;
        }

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.ctx_popup_id) {
                let idx = self.context_idx.unwrap_or(0);
                let instr_addr = provider.instruction(idx).map(|i| i.address());
                let has_target = provider
                    .instruction(idx)
                    .and_then(|i| i.branch_target())
                    .is_some();
                let palette = &self.config.colors;

                // Colour-coded entries — each `menu_item` runs inside
                // a `push_style_color(StyleColor::Text, ...)` block so
                // the icon + label + shortcut all carry the same
                // semantic hue:
                //   - navigation/copy → `address` (accent blue)
                //   - follow / call   → `mnemonic_call` (green)
                //   - function nav    → `mnemonic_jump` (amber)
                //   - breakpoint      → `breakpoint` (red)
                //   - watchpoint      → `operand_memory` (orange)
                //   - bookmark        → `bookmark` (accent)
                //   - settings        → default text
                // dear_imgui_rs's `menu_item` paints the entire row
                // a single colour, so this gives us per-action
                // colour coding without dropping to manual
                // `selectable + text_colored` layout.

                let nav_col = palette.address;
                let call_col = palette.mnemonic_call;
                let jump_col = palette.mnemonic_jump;
                let bp_col = palette.breakpoint;
                let watch_col = palette.operand_memory;
                let bookmark_col = palette.bookmark;
                let s = self.strings();
                let locale = self.config.locale;

                {
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, nav_col);
                    if ui.menu_item(s.menu_goto_address) {
                        self.show_goto = true;
                        self.goto_buf.clear();
                        self.goto_focus_pending = true;
                        ui.close_current_popup();
                    }
                    if ui.menu_item(s.menu_search_bytes) {
                        self.show_search = true;
                        self.search_focus_pending = true;
                        ui.close_current_popup();
                    }
                }

                {
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, nav_col);
                    if ui.menu_item(s.menu_copy_address) {
                        if let Some(addr) = instr_addr {
                            set_clipboard(&self.format_address_literal(addr));
                        }
                        ui.close_current_popup();
                    }
                    let sel_count = self.selection.len();
                    let copy_label = if sel_count > 1 {
                        i18n::disasm_view::copy_n_instructions(locale, sel_count)
                    } else {
                        s.menu_copy_instruction.to_string()
                    };
                    if ui.menu_item(&copy_label) {
                        self.copy_selected(provider);
                        ui.close_current_popup();
                    }
                }

                ui.separator();

                // Follow — green, dimmed when no branch target.
                {
                    let _follow_dim = if !has_target {
                        Some(ui.push_style_var(dear_imgui_rs::StyleVar::Alpha(0.40)))
                    } else {
                        None
                    };
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, call_col);
                    if ui.menu_item(s.menu_follow) {
                        self.cursor_idx = Some(idx);
                        self.follow_at_cursor(provider);
                        ui.close_current_popup();
                    }
                    drop(_follow_dim);
                }

                // Function navigation — amber.
                {
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, jump_col);
                    if ui.menu_item(s.menu_jump_func_start) {
                        self.cursor_idx = Some(idx);
                        self.jump_to_function_start(provider);
                        ui.close_current_popup();
                    }
                    if ui.menu_item(s.menu_jump_func_end) {
                        self.cursor_idx = Some(idx);
                        self.jump_to_function_end(provider);
                        ui.close_current_popup();
                    }
                    if ui.menu_item(s.menu_select_function) {
                        self.cursor_idx = Some(idx);
                        self.select_function(provider);
                        ui.close_current_popup();
                    }
                }

                // Breakpoint — red.
                {
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, bp_col);
                    if ui.menu_item(s.menu_toggle_breakpoint) {
                        if let Some(addr) = instr_addr {
                            provider.toggle_breakpoint(addr);
                        }
                        ui.close_current_popup();
                    }
                }
                {
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, watch_col);
                    if ui.menu_item(s.menu_toggle_watchpoint) {
                        if let Some(addr) = instr_addr {
                            provider.toggle_watchpoint(addr);
                        }
                        ui.close_current_popup();
                    }
                }

                // Bookmark — accent. State-aware label. Glyph
                // matches the gutter renderer: MDI
                // `BOOKMARK_CHECK_OUTLINE` when `icons_available`,
                // `\u{25CB}` ring fallback otherwise. Keeps the
                // popup ↔ gutter visual breadcrumb consistent.
                {
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, bookmark_col);
                    let bookmarked = instr_addr.is_some_and(|a| self.is_bookmarked(a));
                    let glyph = if self.config.icons_available {
                        crate::icons::BOOKMARK_CHECK_OUTLINE
                    } else {
                        "\u{25CB}"
                    };
                    let bookmark_label = if bookmarked {
                        format!("{glyph}  {}", s.menu_remove_bookmark)
                    } else {
                        format!("{glyph}  {}", s.menu_add_bookmark)
                    };
                    if ui.menu_item(&bookmark_label) {
                        if let Some(addr) = instr_addr {
                            self.toggle_bookmark(addr);
                        }
                        ui.close_current_popup();
                    }
                }

                ui.separator();

                // Settings — default text colour. MDI `wrench-cog`
                // (U+F1B91) when `icons_available`, ellipsis
                // fallback otherwise so the entry reads sanely on
                // hosts without the MDI atlas.
                let icon = if self.config.icons_available {
                    "\u{F1B91}  "
                } else {
                    "\u{2026}  "
                };
                let settings_label = format!("{icon}{}", s.menu_settings);
                if ui.menu_item(&settings_label) {
                    self.show_settings = true;
                    ui.close_current_popup();
                }
            }
        });
    }

    /// Settings popup — exposes the most-tweaked layout / colour
    /// toggles inline so the user can adjust the disassembly without
    /// the host wiring its own panel. Mirrors `hex_viewer`'s
    /// `render_settings_popup` for visual consistency (centred at
    /// component middle, themed background, compact body, Close
    /// button right-anchored with the same 2-px edge gap).
    pub(super) fn render_settings_popup(&mut self, ui: &dear_imgui_rs::Ui) {
        if self.show_settings {
            anchor_next_popup_centred(self.component_center);
            ui.open_popup(&self.settings_popup_id);
            self.show_settings = false;
        }

        let s = self.strings();
        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.settings_popup_id) {
                compact_popup_body(ui, || {
                    let icon = if self.config.icons_available {
                        "\u{F1B91}  "
                    } else {
                        "\u{2026}  "
                    };
                    let header = format!("{icon}{}", s.settings_title);
                    ui.text(&header);
                    ui.separator();

                    ui.text(s.settings_display);
                    ui.checkbox(s.settings_show_bytes, &mut self.config.show_bytes);
                    ui.checkbox(s.settings_show_comments, &mut self.config.show_comments);
                    ui.checkbox(s.settings_show_branch_arrows, &mut self.config.show_arrows);
                    ui.checkbox(
                        s.settings_show_breakpoints,
                        &mut self.config.show_breakpoints,
                    );
                    ui.checkbox(s.settings_show_bookmarks, &mut self.config.show_bookmarks);
                    ui.checkbox(
                        s.settings_show_block_tints,
                        &mut self.config.show_block_tints,
                    );
                    ui.checkbox(s.settings_show_header, &mut self.config.show_header);
                    ui.checkbox(
                        s.settings_show_column_dividers,
                        &mut self.config.show_column_dividers,
                    );

                    ui.separator();
                    ui.text(s.settings_format);
                    ui.checkbox(s.settings_uppercase, &mut self.config.uppercase);
                    ui.checkbox(
                        s.settings_address_width_64,
                        &mut self.config.address_width_64,
                    );
                    ui.checkbox(
                        s.settings_byte_category_colors,
                        &mut self.config.byte_category_colors,
                    );

                    ui.separator();
                    ui.text(s.settings_behavior);
                    ui.checkbox(s.settings_editable, &mut self.config.editable);
                    ui.checkbox(
                        s.settings_follow_execution,
                        &mut self.config.follow_execution,
                    );
                    ui.checkbox(
                        s.settings_show_explanation,
                        &mut self.config.show_explanation,
                    );
                    ui.checkbox(s.settings_show_idiom, &mut self.config.show_idiom);
                    ui.checkbox(s.settings_show_gotcha, &mut self.config.show_gotcha);
                    ui.checkbox(
                        s.settings_show_operand_hint,
                        &mut self.config.show_operand_hint,
                    );
                    ui.checkbox(
                        s.settings_show_compiler_pattern,
                        &mut self.config.show_compiler_pattern,
                    );
                    ui.checkbox(s.settings_show_antidisasm, &mut self.config.show_antidisasm);
                    ui.checkbox(s.settings_show_boundary, &mut self.config.show_boundary);
                    ui.checkbox(
                        s.settings_show_branch_direction,
                        &mut self.config.show_branch_direction,
                    );

                    ui.separator();

                    let total_w = ui.content_region_avail()[0];
                    let close_w = 64.0_f32;
                    let close_x = ui.cursor_pos()[0] + (total_w - close_w - 2.0).max(0.0);
                    ui.set_cursor_pos_x(close_x);
                    if ui.button_with_size(s.action_close, [close_w, 0.0])
                        || ui.is_key_pressed(dear_imgui_rs::Key::Escape)
                    {
                        ui.close_current_popup();
                    }
                });
            }
        });
    }

    /// Search popup — wildcard-aware byte search across the
    /// concatenated instruction-byte stream. Mirrors `hex_viewer`'s
    /// search popup geometry (centred at component middle, themed
    /// background, compact body, hex/wildcard input + Find / Cancel
    /// action row + "Result N/M" status line) so users get the same
    /// layout in both widgets.
    ///
    /// Pattern syntax: whitespace-separated hex bytes with `??` /
    /// `?` wildcards (`4D 5A ?? 00 ?? 89`). Minimum
    /// [`super::SEARCH_MIN_BYTES`] (5) bytes — anything shorter
    /// would produce too many spurious hits in typical x86
    /// disassembly.
    pub(super) fn render_search_popup(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        if self.show_search {
            anchor_next_popup_centred(self.component_center);
            ui.open_popup(&self.search_popup_id);
            self.show_search = false;
            self.search_focus_pending = true;
        }

        let s = self.strings();
        let locale = self.config.locale;
        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.search_popup_id) {
                compact_popup_body(ui, || {
                    ui.text(s.search_hint);

                    if self.search_focus_pending {
                        ui.set_keyboard_focus_here();
                        self.search_focus_pending = false;
                    }
                    ui.set_next_item_width(POPUP_INPUT_WIDTH);
                    ui.input_text("##dv_search_input", &mut self.search_buf)
                        .build();

                    // Status line — counts the parsed token length so
                    // the user knows whether they're under the
                    // 5-byte threshold without having to hit Find
                    // first. Empty when buffer is empty (no need to
                    // prod the user as they start typing).
                    let parsed_len =
                        crate::hex_viewer::search::parse_hex_pattern_masked(&self.search_buf).len();
                    if !self.search_buf.trim().is_empty() {
                        if parsed_len < super::SEARCH_MIN_BYTES {
                            ui.text(i18n::disasm_view::pattern_too_short(
                                locale,
                                parsed_len,
                                super::SEARCH_MIN_BYTES,
                            ));
                        } else if !self.search_match_starts.is_empty() {
                            let mut line = i18n::disasm_view::result_n_of_m(
                                locale,
                                self.search_idx + 1,
                                self.search_match_starts.len(),
                            );
                            line.push_str(s.result_step_hint);
                            ui.text(line);
                        } else if !self.search_pattern.is_empty() {
                            ui.text(s.no_matches);
                        }
                    }

                    let (find_clicked, cancel_clicked) =
                        action_row_labeled(ui, POPUP_INPUT_WIDTH, s.action_find, s.action_cancel);
                    if cancel_clicked {
                        ui.close_current_popup();
                    }
                    if find_clicked {
                        self.do_search(provider);
                        if !self.search_match_starts.is_empty() {
                            ui.close_current_popup();
                        }
                    }
                });
            }
        });
    }
}
