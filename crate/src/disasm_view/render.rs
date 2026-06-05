//! Per-frame render path for `DisasmView`.
//!
//! Split out of `mod.rs` (audit session 043). The `DisasmView`
//! struct + fields stay in `mod.rs`; this file only carries the
//! `render` method.

use super::*;

impl DisasmView {
    // ── Rendering ────────────────────────────────────────────────────

    /// Render the disassembly view widget.
    pub fn render(&mut self, ui: &dear_imgui_rs::Ui, provider: &mut dyn DisasmDataProvider) {
        let count = provider.instruction_count();
        if count == 0 {
            return;
        }

        // Tick down the address-gutter "just copied" flash. Counter
        // hits zero → state cleared so the pill stops painting.
        // Mirrors `hex_viewer::render`.
        if let Some((row, frames)) = self.address_flash {
            if frames > 1 {
                self.address_flash = Some((row, frames - 1));
            } else {
                self.address_flash = None;
            }
        }

        // Cache font metrics. Guard against the rare zero-glyph case (e.g.
        // before the font atlas is fully built or in test stubs) — division
        // by zero in the row math below would produce inf-cast UB.
        let [cw, ch] = calc_text_size("0");
        self.char_advance = cw.max(1.0);
        self.line_height = (ch + 4.0).max(1.0);

        // Auto-scroll to current execution point.
        if self.config.follow_execution && self.scroll_to.is_none() {
            for i in 0..count {
                if let Some(instr) = provider.instruction(i)
                    && instr.is_current()
                {
                    self.cursor_idx = Some(i);
                    self.scroll_to = Some(i);
                    break;
                }
            }
        }

        // ── Goto popup ───────────────────────────────────────
        self.render_goto_popup(ui, provider);
        // ── Context menu ─────────────────────────────────────
        self.render_context_menu(ui, provider);
        // ── Settings popup ───────────────────────────────────
        self.render_settings_popup(ui);
        // ── Search popup ─────────────────────────────────────
        self.render_search_popup(ui, provider);

        let avail = ui.content_region_avail();
        let child_id = self.child_id.clone();

        ui.child_window(&child_id)
            .size([avail[0], avail[1]])
            .flags(
                dear_imgui_rs::WindowFlags::NO_MOVE
                    | dear_imgui_rs::WindowFlags::NO_SCROLL_WITH_MOUSE,
            )
            .build(ui, || {
                let was_focused = self.focused;
                self.focused = ui.is_window_focused();

                // Cancel edit on focus loss.
                if was_focused && !self.focused && self.edit.is_some() {
                    self.edit = None;
                }

                // Cache child window centre — used by modal popups
                // (Goto / Settings) to anchor at the visual middle.
                // Mirrors `hex_viewer::draw::render`.
                let wp = ui.window_pos();
                let ws = ui.window_size();
                self.component_center = [wp[0] + ws[0] * 0.5, wp[1] + ws[1] * 0.5];

                // Handle scroll-to target.
                if let Some(idx) = self.scroll_to.take() {
                    let y = idx as f32 * self.line_height;
                    let visible_h = ui.window_size()[1];
                    // Center the target row.
                    let target_y = (y - visible_h * 0.5).max(0.0);
                    ui.set_scroll_y(target_y);
                }

                // Keyboard.
                if self.focused {
                    self.handle_keyboard(ui, provider);
                }

                // Mouse.
                self.handle_mouse(ui, provider);

                let draw_list = ui.get_window_draw_list();
                let [win_x, win_y] = ui.cursor_screen_pos();
                let scroll_y = ui.scroll_y();
                let visible_h = ui.window_size()[1];

                // Clamp `first_row` to `count` defensively: when the
                // provider shrinks between frames (live disasm with
                // re-decoding), `scroll_y / line_height` can land
                // past the new tail and `last_row - first_row` would
                // underflow when fed to `compute_arrows_clipped`.
                let first_row = ((scroll_y / self.line_height) as usize).min(count);
                let visible_count = (visible_h / self.line_height) as usize + 2;
                let last_row = (first_row + visible_count).min(count);

                let origin_x = win_x + ui.scroll_x();
                let origin_y = win_y + scroll_y;

                // ── Compute branch arrows for visible range ───
                // Cross-window scan ([`compute_arrows_clipped`]):
                // walks ALL provider instructions, retains arrows
                // where AT LEAST ONE endpoint is in the visible
                // window, clamps off-window endpoints to the
                // window edge with `clipped_*` flags so the
                // renderer can suppress the arrowhead there.
                // Replaces the older visible-only scan that
                // dropped long-range jumps when the source or
                // target scrolled offscreen.
                if self.config.show_arrows {
                    let key = (first_row, last_row, count, self.config.max_arrows);
                    if self.cached_arrows_key != Some(key) {
                        self.cached_arrows = compute_arrows_clipped(
                            provider as &dyn DisasmDataProvider,
                            first_row,
                            last_row - first_row,
                        );
                        if self.cached_arrows.len() > self.config.max_arrows {
                            self.cached_arrows.truncate(self.config.max_arrows);
                        }
                        self.cached_arrows_key = Some(key);
                    }
                } else if !self.cached_arrows.is_empty() {
                    // Arrows toggled off — drop the cache so a
                    // subsequent flip back doesn't paint stale data.
                    self.cached_arrows.clear();
                    self.cached_arrows_key = None;
                }

                // ── Dynamic comment X ─────────────────────────
                // Pre-pass: walk the visible rows once to find the
                // rightmost glyph any instruction text would draw to.
                // If it overflows the default `operand_end`, push the
                // comment column (header + text + edit + divider 3)
                // right by exactly that overflow + COMMENT_GAP. Cell
                // is read by `mouse_to_cell` for the next frame's
                // double-click hit-test.
                let cols = &self.config.columns;
                let bytes_col_x = origin_x
                    + if self.config.show_breakpoints {
                        cols.margin
                    } else {
                        0.0
                    }
                    + if self.config.show_arrows {
                        cols.arrows
                    } else {
                        0.0
                    }
                    + cols.address;
                let mnemonic_col_x = if self.config.show_bytes {
                    bytes_col_x + cols.bytes
                } else {
                    bytes_col_x
                };
                let default_comment_x = mnemonic_col_x + cols.mnemonic + cols.operands;
                let instr_data_x = mnemonic_col_x + draw::COL_INNER_PAD;
                // Pre-pass runs unconditionally — even when the
                // comment column is hidden, the value is consumed
                // by `draw_header` for "Instruction" centring and by
                // `mouse_to_cell` to bound the Mnemonic hit-zone.
                // The Comment-column draw branches are individually
                // gated on `show_comments`.
                let mut max_instr_right = default_comment_x;
                for row in first_row..last_row {
                    if let Some(instr) = provider.instruction(row) {
                        // Monospace assumption: width = char count
                        // × char_advance (mnemonic + space + operands).
                        // Mnemonic / operands are always ASCII for x86 /
                        // x86-64 / ARM, so byte length == codepoint count
                        // and `len()` is O(1) where `chars().count()`
                        // walks the string per row per frame.
                        let mn = instr.mnemonic().len();
                        let op = instr.operands().len();
                        let chars = mn + 1 + op;
                        let row_right =
                            instr_data_x + chars as f32 * self.char_advance + draw::COMMENT_GAP;
                        if row_right > max_instr_right {
                            max_instr_right = row_right;
                        }
                    }
                }
                let comment_x = max_instr_right;
                self.frame_comment_x.set(Some(comment_x));

                // Comment column stretches to fill remaining
                // host-window width (per user request 2026-04-30:
                // Comment has lowest layout priority). Floor at
                // `cols.comment` so the column never collapses
                // smaller than its configured min — prevents the
                // edit cell from becoming unusably narrow when the
                // host window is shrunk below the layout total.
                let comment_w = (origin_x + avail[0] - comment_x).max(cols.comment);
                self.frame_comment_w.set(Some(comment_w));

                // ── Column header ─────────────────────────────
                if self.config.show_header {
                    self.draw_header(&draw_list, origin_x, origin_y, comment_x);
                }

                let header_h = if self.config.show_header {
                    self.line_height
                } else {
                    0.0
                };

                // ── Draw rows ─────────────────────────────────
                // Gate the mouse position on `is_window_hovered`
                // so hover-based row tooltips don't leak through
                // popups that overlap this widget (e.g. another
                // module's Settings dialog rendered on top of the
                // disasm view). Without this gate the row hit-test
                // is pure coordinate math that fires regardless of
                // whether ImGui has handed mouse focus to a popup —
                // user reported the tooltip ghosting through a
                // hex-viewer Settings popup on 2026-04-29. Same
                // pattern as `hex_viewer::draw::render`.
                let mouse_pos = if ui.is_window_hovered() {
                    ui.io().mouse_pos()
                } else {
                    [f32::NEG_INFINITY, f32::NEG_INFINITY]
                };
                for row in first_row..last_row {
                    if let Some(instr) = provider.instruction(row) {
                        let y = origin_y + header_h + (row - first_row) as f32 * self.line_height;
                        // Pull the immediate neighbours so the tooltip's
                        // idiom detector can recognise multi-instruction
                        // patterns (prologue / cmp+Jcc / get-IP / etc).
                        // `prev` / `next` may be `None` near edges or
                        // when the provider returns a sparse hole.
                        let prev_instr = row.checked_sub(1).and_then(|i| provider.instruction(i));
                        let next_instr = provider.instruction(row + 1);
                        self.draw_instruction_row(
                            ui, &draw_list, origin_x, y, row, instr, prev_instr, next_instr,
                            mouse_pos, avail[0], comment_x, provider,
                        );
                    }
                }

                // ── Draw branch arrows on top ─────────────────
                if self.config.show_arrows && !self.cached_arrows.is_empty() {
                    self.draw_arrows(&draw_list, origin_x, origin_y + header_h);
                }

                // ── Vertical column dividers ──────────────────
                if self.config.show_column_dividers {
                    let visible_h = ui.window_size()[1];
                    self.draw_column_dividers(
                        &draw_list,
                        origin_x,
                        origin_y,
                        origin_y + visible_h,
                        comment_x,
                    );
                }

                // ── Render inline InputText for editing ──────────
                if let Some(pos) = self.edit_render_pos.take() {
                    let input_w = self.edit_render_width.get();
                    // Position the ImGui cursor at the edit cell.
                    ui.set_cursor_screen_pos(pos);
                    ui.set_next_item_width(input_w);
                    if let Some(edit) = &mut self.edit {
                        // Auto-focus on first frame.
                        if edit.frames == 0 {
                            ui.set_keyboard_focus_here();
                        }
                        edit.frames += 1;

                        // Per-column input flags:
                        // - Bytes: hex-only + uppercase (raw `AA BB`
                        //   hex patch).
                        // - Mnemonic: free text (assembly source —
                        //   `mov rax, rbx`); no character restriction.
                        // - Comment: free text — same as mnemonic.
                        // AUTO_SELECT_ALL + ENTER_RETURNS_TRUE apply
                        // to all three so the open-then-overwrite
                        // gesture and Enter-to-commit shortcut behave
                        // uniformly.
                        let mut flags = dear_imgui_rs::InputTextFlags::AUTO_SELECT_ALL
                            | dear_imgui_rs::InputTextFlags::ENTER_RETURNS_TRUE;
                        if edit.column == EditColumn::Bytes {
                            flags |= dear_imgui_rs::InputTextFlags::CHARS_HEXADECIMAL
                                | dear_imgui_rs::InputTextFlags::CHARS_UPPERCASE;
                        }

                        let entered = ui
                            .input_text(&self.edit_label, &mut edit.buf)
                            .flags(flags)
                            .build();

                        if entered {
                            // Enter pressed — commit. The `take()` /
                            // `if let` pattern (instead of `take().unwrap()`)
                            // is intentional: a sibling module
                            // (`input::handle_keyboard`, focus-loss guard
                            // above) can clear `self.edit` in the same frame
                            // by future refactors. The outer `if let
                            // Some(edit) = &mut self.edit` borrow keeps
                            // that safe today, but the `unwrap` was the
                            // only production-code panic surface in the
                            // module — removing it removes the footgun.
                            if let Some(edit_data) = self.edit.take() {
                                self.commit_edit(edit_data, provider);
                            }
                        } else if !ui.is_item_active() && edit.frames > 2 {
                            // Lost focus (clicked elsewhere, Tab, etc.) — cancel.
                            self.edit = None;
                        }
                    }
                }

                // Dummy for scroll extent.
                let total_h = count as f32 * self.line_height + header_h + self.line_height;
                ui.set_cursor_pos([0.0, total_h]);
                ui.dummy([avail[0], 1.0]);
            });
    }
}
