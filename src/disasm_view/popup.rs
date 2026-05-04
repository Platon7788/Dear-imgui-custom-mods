//! Goto-address popup and right-click context menu for [`super::DisasmView`].
//!
//! Both popups share the crate-wide `themed_popup_style` look (same
//! padding / rounding as `hex_viewer`'s) and the centred-anchor
//! positioning so the goto popup always lands at the visual middle
//! of the viewer regardless of where the user pressed `G`.

use super::provider::DisasmDataProvider;
use super::{DisasmView, parse_address};
use crate::utils::clipboard::set_clipboard;
use crate::utils::popup::{action_row, compact_popup_body, themed_popup_style};

/// Reference width for the goto popup body (input + action row).
/// Matches `hex_viewer::popup::POPUP_INPUT_WIDTH` so both modules
/// host visually-identical goto dialogs.
const POPUP_INPUT_WIDTH: f32 = 360.0;

/// Anchor the next ImGui window (a popup) at `pos` in screen space
/// using the given `pivot`. See `hex_viewer::popup` for full
/// rationale — same `igSetNextWindowPos(.., Cond_Always, pivot)`
/// FFI call (no `set_next_window_pos` builder in dear-imgui-rs 0.11).
fn anchor_next_popup_at(pos: [f32; 2], pivot: [f32; 2]) {
    // SAFETY: side-effect-only ImGui call; just records the
    // requested next-window position in the shared context.
    unsafe {
        #[allow(clippy::unnecessary_cast)]
        // `ImGuiCond_Always` is `u32` on Linux, `i32` on Windows.
        dear_imgui_rs::sys::igSetNextWindowPos(
            dear_imgui_rs::sys::ImVec2 {
                x: pos[0],
                y: pos[1],
            },
            dear_imgui_rs::sys::ImGuiCond_Always as i32,
            dear_imgui_rs::sys::ImVec2 {
                x: pivot[0],
                y: pivot[1],
            },
        );
    }
}

/// Top-left anchor — for the right-click context menu (spawns
/// where the click happened).
fn anchor_next_popup_topleft(pos: [f32; 2]) {
    anchor_next_popup_at(pos, [0.0, 0.0]);
}

/// Centred anchor — for modal popups (Goto). Pivot `(0.5, 0.5)`
/// means `pos` is the popup's CENTRE, so the body is centred no
/// matter what its size is.
fn anchor_next_popup_centred(pos: [f32; 2]) {
    anchor_next_popup_at(pos, [0.5, 0.5]);
}

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

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.goto_popup_id) {
                compact_popup_body(ui, || {
                    ui.text("Goto address (hex):");

                    if self.goto_focus_pending {
                        ui.set_keyboard_focus_here();
                        self.goto_focus_pending = false;
                    }
                    ui.set_next_item_width(POPUP_INPUT_WIDTH);
                    ui.input_text("##dv_goto_input", &mut self.goto_buf)
                        .build();

                    let (go_clicked, cancel_clicked) =
                        action_row(ui, POPUP_INPUT_WIDTH, "Go");
                    if cancel_clicked {
                        ui.close_current_popup();
                    }
                    if go_clicked {
                        if let Some(addr) = parse_address(&self.goto_buf) {
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
            anchor_next_popup_topleft(self.popup_open_pos);
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

                {
                    let _c =
                        ui.push_style_color(dear_imgui_rs::StyleColor::Text, nav_col);
                    if ui.menu_item("\u{00BB}  Goto Address...\tG") {
                        self.show_goto = true;
                        self.goto_buf.clear();
                        self.goto_focus_pending = true;
                        ui.close_current_popup();
                    }
                    if ui.menu_item("\u{00BB}  Search bytes...\tCtrl+F") {
                        self.show_search = true;
                        self.search_focus_pending = true;
                        ui.close_current_popup();
                    }
                }

                {
                    let _c =
                        ui.push_style_color(dear_imgui_rs::StyleColor::Text, nav_col);
                    if ui.menu_item("\u{00BB}  Copy Address") {
                        if let Some(addr) = instr_addr {
                            set_clipboard(&self.format_address_literal(addr));
                        }
                        ui.close_current_popup();
                    }
                    let sel_count = self.selection.len();
                    let copy_label = if sel_count > 1 {
                        format!("\u{00BB}  Copy {} Instructions\tCtrl+C", sel_count)
                    } else {
                        "\u{00BB}  Copy Instruction\tCtrl+C".to_string()
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
                    let _c = ui
                        .push_style_color(dear_imgui_rs::StyleColor::Text, call_col);
                    if ui.menu_item("\u{2192}  Follow\tEnter / Space") {
                        self.cursor_idx = Some(idx);
                        self.follow_at_cursor(provider);
                        ui.close_current_popup();
                    }
                    drop(_follow_dim);
                }

                // Function navigation — amber.
                {
                    let _c = ui
                        .push_style_color(dear_imgui_rs::StyleColor::Text, jump_col);
                    if ui.menu_item("\u{2191}  Jump to function start\tCtrl+Up") {
                        self.cursor_idx = Some(idx);
                        self.jump_to_function_start(provider);
                        ui.close_current_popup();
                    }
                    if ui.menu_item("\u{2193}  Jump to function end\tCtrl+Down") {
                        self.cursor_idx = Some(idx);
                        self.jump_to_function_end(provider);
                        ui.close_current_popup();
                    }
                    if ui.menu_item("\u{00BB}  Select function\tCtrl+L") {
                        self.cursor_idx = Some(idx);
                        self.select_function(provider);
                        ui.close_current_popup();
                    }
                }

                // Breakpoint — red.
                {
                    let _c =
                        ui.push_style_color(dear_imgui_rs::StyleColor::Text, bp_col);
                    if ui.menu_item("\u{25CF}  Toggle Breakpoint\tF9") {
                        if let Some(addr) = instr_addr {
                            provider.toggle_breakpoint(addr);
                        }
                        ui.close_current_popup();
                    }
                }
                // Watchpoint — orange. Same `\u{25CF}` filled
                // circle as Toggle Breakpoint so both "things
                // that pause the running process" (BP / RW) read
                // as one visual class — colour carries the kind.
                // Single entry instead of separate R / W rows;
                // hosts that distinguish read-only vs write-only
                // data breakpoints sort that out on the engine
                // side and report the union via
                // `Instruction::has_watchpoint`.
                {
                    let _c = ui
                        .push_style_color(dear_imgui_rs::StyleColor::Text, watch_col);
                    if ui.menu_item("\u{25CF}  Toggle Watchpoint") {
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
                    let _c = ui.push_style_color(
                        dear_imgui_rs::StyleColor::Text,
                        bookmark_col,
                    );
                    let bookmarked = instr_addr.is_some_and(|a| self.is_bookmarked(a));
                    let glyph = if self.config.icons_available {
                        crate::icons::BOOKMARK_CHECK_OUTLINE
                    } else {
                        "\u{25CB}"
                    };
                    let bookmark_label = if bookmarked {
                        format!("{glyph}  Remove from bookmarks\tCtrl+B")
                    } else {
                        format!("{glyph}  Add to bookmarks\tCtrl+B")
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
                let settings_label = if self.config.icons_available {
                    "\u{F1B91}  Settings..."
                } else {
                    "\u{2026}  Settings..."
                };
                if ui.menu_item(settings_label) {
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

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.settings_popup_id) {
                compact_popup_body(ui, || {
                    let header = if self.config.icons_available {
                        "\u{F1B91}  Disassembly Settings"
                    } else {
                        "\u{2026}  Disassembly Settings"
                    };
                    ui.text(header);
                    ui.separator();

                    ui.text("Display:");
                    ui.checkbox("Show bytes", &mut self.config.show_bytes);
                    ui.checkbox("Show comments", &mut self.config.show_comments);
                    ui.checkbox("Show branch arrows", &mut self.config.show_arrows);
                    ui.checkbox("Show breakpoints", &mut self.config.show_breakpoints);
                    ui.checkbox("Show bookmarks", &mut self.config.show_bookmarks);
                    ui.checkbox("Show block tints", &mut self.config.show_block_tints);
                    ui.checkbox("Show header", &mut self.config.show_header);
                    ui.checkbox(
                        "Show column dividers",
                        &mut self.config.show_column_dividers,
                    );

                    ui.separator();
                    ui.text("Format:");
                    ui.checkbox("Uppercase hex", &mut self.config.uppercase);
                    ui.checkbox("64-bit address width", &mut self.config.address_width_64);
                    ui.checkbox(
                        "Byte category colors",
                        &mut self.config.byte_category_colors,
                    );

                    ui.separator();
                    ui.text("Behavior:");
                    ui.checkbox("Editable (double-click to patch)", &mut self.config.editable);
                    ui.checkbox(
                        "Follow execution",
                        &mut self.config.follow_execution,
                    );

                    ui.separator();

                    let total_w = ui.content_region_avail()[0];
                    let close_w = 64.0_f32;
                    let close_x = ui.cursor_pos()[0] + (total_w - close_w - 2.0).max(0.0);
                    ui.set_cursor_pos_x(close_x);
                    if ui.button_with_size("Close", [close_w, 0.0])
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

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.search_popup_id) {
                compact_popup_body(ui, || {
                    ui.text(format!(
                        "Search bytes (min {} bytes; ?? wildcard):",
                        super::SEARCH_MIN_BYTES,
                    ));

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
                        crate::hex_viewer::search::parse_hex_pattern_masked(&self.search_buf)
                            .len();
                    if !self.search_buf.trim().is_empty() {
                        if parsed_len < super::SEARCH_MIN_BYTES {
                            ui.text(format!(
                                "Pattern too short: {} / {} bytes",
                                parsed_len,
                                super::SEARCH_MIN_BYTES,
                            ));
                        } else if !self.search_match_starts.is_empty() {
                            ui.text(format!(
                                "Result {}/{}  (F3 / Shift+F3 to step)",
                                self.search_idx + 1,
                                self.search_match_starts.len(),
                            ));
                        } else if !self.search_pattern.is_empty() {
                            ui.text("No matches.");
                        }
                    }

                    let (find_clicked, cancel_clicked) =
                        action_row(ui, POPUP_INPUT_WIDTH, "Find");
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
