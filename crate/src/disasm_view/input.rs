//! Input handling for [`super::DisasmView`] — keyboard navigation
//! (cursor moves, page-up/down, Home/End, Enter to follow branch, G for
//! goto, Ctrl+A select-all, F9 breakpoint, Alt+arrow nav history),
//! mouse interaction (click / shift / ctrl / drag-select / double-click
//! to edit / right-click context menu), and the inline edit commit path.

use super::provider::DisasmDataProvider;
use super::{DisasmView, EditColumn, EditState};
use crate::utils::clipboard::set_clipboard;
use crate::utils::hex::byte_hex;

/// How many frames the address-gutter "just copied" pill stays
/// painted after a double-click-to-copy. ~30 frames @ 60 fps ≈ 0.5 s
/// — same dwell as `hex_viewer::ADDRESS_FLASH_FRAMES`.
pub(super) const ADDRESS_FLASH_FRAMES: u32 = 30;

/// Visible-row count from window height. Guards against zero
/// `line_height` (degenerate font load) so a divide doesn't yield
/// `inf as usize` (UB-adjacent in debug, `0` in release on x86).
#[inline]
fn rows_in_window(window_h: f32, line_height: f32) -> usize {
    if line_height > 0.0 {
        (window_h / line_height) as usize
    } else {
        0
    }
}

/// Reusable space-separated hex byte formatter — single-allocation
/// `String` (3 chars/byte: two hex + one space, minus the trailing
/// gap). Mirrors the per-row pattern in `draw::draw_instruction_row`.
fn join_bytes_hex(bytes: &[u8], uppercase: bool) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(byte_hex(*b, uppercase));
    }
    s
}

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

        // Arrow keys. Gated on `!ctrl` because the Ctrl-modified
        // variants below trigger function-scope navigation
        // (Ctrl+Up = function start, Ctrl+Down = function end) and
        // would otherwise also fire single-row movement on the same
        // keystroke.
        if ui.is_key_pressed(Key::UpArrow) && !alt && !ctrl {
            let idx = self.cursor_idx.unwrap_or(0);
            if idx > 0 {
                let new = idx - 1;
                move_cursor(self, new);
                self.ensure_visible(new, ui);
            }
        }
        if ui.is_key_pressed(Key::DownArrow) && !alt && !ctrl {
            let idx = self.cursor_idx.unwrap_or(0);
            if idx + 1 < count {
                let new = idx + 1;
                move_cursor(self, new);
                self.ensure_visible(new, ui);
            }
        }

        // Page Up/Down. `line_height` is guarded against zero — a
        // degenerate font load would otherwise divide by zero and
        // produce `inf as usize` (`0` in release, UB-adjacent in
        // debug). One row fallback keeps the keystroke responsive
        // even on a window that briefly reports 0 height.
        let visible_rows = rows_in_window(ui.window_size()[1], self.line_height).max(1);
        // `count - 1` previously relied on the `count == 0 { return; }`
        // guard ~60 lines above; `saturating_sub(1)` removes that
        // long-range coupling so future refactors that re-entry-point
        // the function can't underflow.
        let last_idx = count.saturating_sub(1);
        if ui.is_key_pressed(Key::PageUp) {
            let new = self.cursor_idx.unwrap_or(0).saturating_sub(visible_rows);
            move_cursor(self, new);
            self.scroll_to = Some(new);
        }
        if ui.is_key_pressed(Key::PageDown) {
            let new = (self.cursor_idx.unwrap_or(0) + visible_rows).min(last_idx);
            move_cursor(self, new);
            self.scroll_to = Some(new);
        }

        // Home/End — buffer-wide. Gated `!ctrl` so future
        // Ctrl+Home/End additions don't double-fire.
        if ui.is_key_pressed(Key::Home) && !ctrl {
            move_cursor(self, 0);
            self.scroll_to = Some(0);
        }
        if ui.is_key_pressed(Key::End) && !ctrl {
            move_cursor(self, last_idx);
            self.scroll_to = Some(last_idx);
        }

        // ── Function-scope navigation (Ctrl+Up / Ctrl+Down / Ctrl+L) ──
        // Mirrors common reverse-engineering convention (IDA, x64dbg
        // style "go to func top / bottom / select procedure"). The
        // boundary heuristic is RET-based — see
        // [`super::find_function_start`] / [`super::find_function_end`]
        // for caveats (tail-call jumps not detected, padding folds
        // into either neighbour).
        if ctrl && ui.is_key_pressed(Key::UpArrow) {
            self.jump_to_function_start(provider);
        }
        if ctrl && ui.is_key_pressed(Key::DownArrow) {
            self.jump_to_function_end(provider);
        }
        if ctrl && ui.is_key_pressed(Key::L) {
            self.select_function(provider);
        }

        // Ctrl+A — select all. Layout-independence is provided by
        // `crate::input::keyboard::try_inject_ctrl_alt_shortcut` at the
        // host level (see app_window and demo_disasm_view).
        if ctrl && ui.is_key_pressed(Key::A) {
            for i in 0..count {
                self.selection.insert(i);
            }
        }

        // Enter / Space → "Cheat-Engine-style" follow at cursor.
        // Tries [`Instruction::branch_target`] first, then falls
        // back to scanning operand `Number` tokens for an
        // address known to the provider — see
        // [`super::DisasmView::follow_at_cursor`]. Both keys land
        // here so the user can pick whichever is comfortable
        // (Enter for keyboard-only flow, Space when one hand sits
        // on the arrows).
        if ui.is_key_pressed(Key::Enter) || ui.is_key_pressed(Key::Space) {
            self.follow_at_cursor(provider);
        }

        // G → goto address popup.
        if ui.is_key_pressed(Key::G) && !ctrl {
            self.show_goto = true;
            self.goto_buf.clear();
        }

        // Ctrl+F → search-bytes popup. Mirrors `hex_viewer`'s
        // Ctrl+F. Doesn't clear `search_buf` so the user can re-open
        // the popup and tweak the previous query without retyping.
        if ctrl && ui.is_key_pressed(Key::F) && !self.show_search {
            self.show_search = true;
            self.search_focus_pending = true;
        }

        // F3 / Shift+F3 → step through search matches (no-op when
        // no active search). Wraps around at both ends.
        if ui.is_key_pressed(Key::F3) && !self.search_match_starts.is_empty() {
            if shift {
                self.search_prev();
            } else {
                self.search_next();
            }
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

        // Ctrl+B → toggle bookmark on the cursor row. Editor-style
        // shortcut (VS Code / JetBrains / Sublime). Silently no-ops
        // at the 64-bookmark cap when adding; removal always works.
        if ctrl
            && ui.is_key_pressed(Key::B)
            && let Some(idx) = self.cursor_idx
            && let Some(instr) = provider.instruction(idx)
        {
            self.toggle_bookmark(instr.address());
        }

        // Alt+Left → nav back.
        if alt && ui.is_key_pressed(Key::LeftArrow) {
            self.nav_back(provider);
        }
        // Alt+Right → nav forward.
        if alt && ui.is_key_pressed(Key::RightArrow) {
            self.nav_forward(provider);
        }

        // Esc → clear navigation breadcrumb (origin highlight).
        // Decisive "I'm done with the trail" gesture. Edit-mode
        // Esc is handled separately inside `handle_edit_keyboard`
        // (cancels the edit instead) — this branch only runs
        // when no edit is active.
        if ui.is_key_pressed(Key::Escape) {
            self.origin_addr = None;
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
                EditColumn::Comment => {
                    // `set_comment` is a no-op on providers that
                    // don't override the trait default; the typed
                    // text is still consumed (no leak / panic).
                    // `VecDisasmProvider` writes through.
                    provider.set_comment(addr, &edit.buf);
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

        // Mouse wheel scroll. Compute in `f32` end-to-end — the
        // previous `as isize` round-trip silently truncated fractional
        // wheel deltas (high-resolution touchpads emit `0.10`-step
        // values), so a slow touchpad pan stayed at zero rows and
        // never scrolled. Three rows per notch matches Windows'
        // SystemParametersInfo(SPI_GETWHEELSCROLLLINES) default.
        let wheel = ui.io().mouse_wheel();
        if wheel != 0.0 {
            let scroll_y = ui.scroll_y();
            let delta_px = -wheel * 3.0 * self.line_height;
            let new_scroll = (scroll_y + delta_px).max(0.0);
            ui.set_scroll_y(new_scroll);
        }

        let ctrl = ui.io().key_ctrl();
        let shift = ui.io().key_shift();

        // ── Address-gutter double-click-to-copy ──────────────────
        // Hovering the address column shows a `Hand` cursor + tooltip
        // ("Double-click to copy 0x...") so the gesture is
        // discoverable. A bare double-click (no modifier) copies the
        // row's address as a hex literal and triggers a brief flash
        // pill behind the text. Single-click keeps its normal
        // select-row semantics — user reported single-click copy felt
        // accidental on 2026-04-30.
        if let Some(row) = self.mouse_to_address_row(ui, provider) {
            ui.set_mouse_cursor(Some(dear_imgui_rs::MouseCursor::Hand));
            if let Some(instr) = provider.instruction(row) {
                let addr = instr.address();
                let formatted = self.format_address_literal(addr);
                let s = self.strings();
                crate::utils::tooltip::themed_tooltip(ui, || {
                    ui.text(format!("{}: {}", s.tooltip_double_click_copy, formatted));
                });
                if !shift && !ctrl && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left) {
                    set_clipboard(&formatted);
                    self.address_flash = Some((row, ADDRESS_FLASH_FRAMES));
                    // Drop any drag-select origin a single click below
                    // may have set — otherwise the next mouse-drag
                    // after the copy uses the stale origin index and
                    // selects a phantom range (M7 from session 034
                    // audit).
                    self.drag_origin = None;
                    // Don't fall through into the row-edit double-click
                    // handler below — gesture was for the address gutter.
                    return;
                }
            }
        }

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
                    // Single-click moves the cursor + replaces
                    // the selection. Origin breadcrumb is *kept*
                    // — clicking around the disasm to read code
                    // shouldn't wipe the "where I jumped from"
                    // highlight. Explicit dismissal is `Esc` (or
                    // a new navigation, which overwrites origin
                    // anyway).
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

        // ── Double-click anywhere on a row → "Cheat-Engine-style"
        // follow at cursor ───────────────────────────────────────
        //
        // Pre-2026-05-05 this branch only fired in the
        // Mnemonic+Operands column, so a double-click on a wide
        // operand text that overflowed into the Comment area
        // (e.g. `jmp qword ptr [rip+0x12345678]`) silently fell
        // through into the edit-cell handler — the user reported
        // "double-click on jmp doesn't work". Now: any non-address-
        // gutter row hit attempts follow first; only when follow
        // declines (no branch target, no resolvable operand
        // pointer) does the edit-cell branch get a turn.
        //
        // Address-gutter double-click is handled separately above
        // (copy-to-clipboard with `return`), so we never run twice
        // on the same gesture.
        if ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
            && let Some(row) = self.mouse_to_instruction(ui, provider)
        {
            self.cursor_idx = Some(row);
            if self.follow_at_cursor(provider) {
                return;
            }
        }

        // ── Middle-click anywhere on a row → follow ──────────────
        //
        // IDA / Cheat-Engine convention. Middle-click is more
        // discoverable than double-click for users who aren't sure
        // whether the gesture is enabled; it also dodges the
        // double-click time threshold (`io.MouseDoubleClickTime`)
        // and same-position threshold (`MouseDoubleClickMaxDist`)
        // entirely — a single deliberate middle-click always
        // navigates if the row is followable.
        if ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Middle)
            && let Some(row) = self.mouse_to_instruction(ui, provider)
        {
            self.cursor_idx = Some(row);
            self.follow_at_cursor(provider);
            return;
        }

        // ── Double-click to edit (if editable) ───────────────────
        //
        // Reached only when follow declined (e.g. `mov rax, rbx`
        // with no number / branch target). Cell hit-test picks
        // the right column → buffer initialiser:
        //   Bytes    — pre-fill with current "AA BB CC" hex string
        //   Mnemonic — reserved for a future re-assemble flow (not
        //              wired to UI yet; commit path exists)
        //   Comment  — pre-fill with current comment text (empty
        //              string when the row has no comment yet —
        //              user types from scratch)
        if ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
            && self.config.editable
            && let Some((idx, column)) = self.mouse_to_cell(ui, provider)
            && let Some(instr) = provider.instruction(idx)
        {
            let buf = match column {
                EditColumn::Bytes => join_bytes_hex(instr.bytes(), true),
                EditColumn::Mnemonic => {
                    format!("{} {}", instr.mnemonic(), instr.operands())
                }
                EditColumn::Comment => instr.comment().unwrap_or("").to_string(),
            };
            self.edit = Some(EditState {
                idx,
                column,
                buf,
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
            // Capture click position so the menu spawns under the
            // cursor (default ImGui auto-position is `(0, 0)` when
            // BeginPopup runs outside any window context — see
            // hex_viewer::popup for the same pattern).
            self.popup_open_pos = Some(ui.io().mouse_pos());
        }
    }

    fn mouse_to_instruction(
        &self,
        ui: &dear_imgui_rs::Ui,
        provider: &dyn DisasmDataProvider,
    ) -> Option<usize> {
        // Zero-`line_height` guard: a degenerate font load would feed
        // `inf` into the `as usize` cast — `0` in release on x86, but
        // UB-adjacent in debug. Fail open with `None` instead.
        if self.line_height <= 0.0 {
            return None;
        }
        let [_mx, my] = ui.io().mouse_pos();
        let [_win_x, win_y] = ui.cursor_screen_pos();
        let header_h = if self.config.show_header {
            self.line_height
        } else {
            0.0
        };

        // Single combined division. The previous code did
        // `floor((my - win_y - scroll_y - header_h)/lh) + floor(scroll_y/lh)`
        // — two independent floors that don't compose at the row
        // boundary, so a click on the very last pixel of a row could
        // promote to the row above (audit M6). `cursor_screen_pos()`
        // already accounts for the scroll offset (it returns the
        // current draw position in screen pixels), so the `+scroll_y
        // -scroll_y` round-trip cancels out cleanly.
        let combined = (my - win_y - header_h) / self.line_height;
        if combined < 0.0 {
            return None;
        }
        let row = combined as usize;

        if row < provider.instruction_count() {
            Some(row)
        } else {
            None
        }
    }

    /// Hit-test the mouse against the address gutter: returns the
    /// instruction row index when the cursor X falls between the end
    /// of the arrow/breakpoint area and the start of the bytes
    /// column. Mirrors `hex_viewer::mouse_to_address_row`.
    pub(super) fn mouse_to_address_row(
        &self,
        ui: &dear_imgui_rs::Ui,
        provider: &dyn DisasmDataProvider,
    ) -> Option<usize> {
        let row = self.mouse_to_instruction(ui, provider)?;
        let [mx, _my] = ui.io().mouse_pos();
        let [win_x, _win_y] = ui.cursor_screen_pos();
        let cols = &self.config.columns;
        // Left edge of the address column = origin + breakpoint
        // gutter + arrows gutter (whichever are enabled).
        let mut addr_left = win_x + ui.scroll_x();
        if self.config.show_breakpoints {
            addr_left += cols.margin;
        }
        if self.config.show_arrows {
            addr_left += cols.arrows;
        }
        let addr_right = addr_left + cols.address;
        if mx >= addr_left && mx < addr_right {
            Some(row)
        } else {
            None
        }
    }

    /// Hit-test the mouse against the cell grid: returns the row
    /// index AND which editable column the cursor is over (Bytes /
    /// Mnemonic / Comment), or `None` when outside any editable
    /// region. Used by the double-click handler so each column has
    /// its own edit affordance — bytes ↦ hex patch, mnemonic ↦
    /// re-assemble, comment ↦ free-form text.
    ///
    /// Address column / breakpoint margin / arrow gutter return
    /// `None` even when hovered — those areas have their own
    /// non-edit affordances (click-to-select, breakpoint toggle).
    pub(super) fn mouse_to_cell(
        &self,
        ui: &dear_imgui_rs::Ui,
        provider: &dyn DisasmDataProvider,
    ) -> Option<(usize, EditColumn)> {
        let row = self.mouse_to_instruction(ui, provider)?;

        let [mx, _my] = ui.io().mouse_pos();
        let [win_x, _win_y] = ui.cursor_screen_pos();
        // Mirrors the X-layout in `draw::draw_instruction_row` —
        // accumulate column boundaries left-to-right.
        let cols = &self.config.columns;
        let mut x = win_x + ui.scroll_x();
        if self.config.show_breakpoints {
            x += cols.margin;
        }
        if self.config.show_arrows {
            x += cols.arrows;
        }
        x += cols.address;
        let bytes_x = x;
        let mnemonic_x = if self.config.show_bytes {
            bytes_x + cols.bytes
        } else {
            bytes_x
        };
        // Read the per-frame dynamic comment X computed by
        // `render()` last frame — when the instruction text grew
        // long enough to push the comment column right, the hit-
        // test boundary follows so the user double-clicks the
        // glyph they actually see. 1-frame stale on the very first
        // frame after such a shift; invisible for double-click.
        //
        // `Option<f32>` cell — `None` only on frame 0 before `render()`
        // populated the value (M1 from session 034 audit; the prior
        // `<= 0.0` sentinel could misfire when the host docked the
        // view at screen-space x = 0).
        let comment_x = self
            .frame_comment_x
            .get()
            .unwrap_or(mnemonic_x + cols.mnemonic + cols.operands);

        // Bytes cell — only when bytes column is visible.
        if self.config.show_bytes && mx >= bytes_x && mx < mnemonic_x {
            return Some((row, EditColumn::Bytes));
        }
        // Mnemonic + operands together (UI shows them as one
        // editable region for the future "edit instruction" path).
        // **GATED**: returns `None` until the assembler round-trip is
        // wired (`DisasmDataProvider::assemble` default-impl is no-op
        // → opening the editor here would be a UX leak: type +
        // Enter, nothing happens). The `EditColumn::Mnemonic` variant
        // and the matching `commit_edit` branch stay alive for the
        // future flow — flip this `None` back to
        // `Some((row, EditColumn::Mnemonic))` once `assemble` works.
        if mx >= mnemonic_x && mx < comment_x {
            return None;
        }
        // Comment cell — only when the comment column is visible.
        if self.config.show_comments && mx >= comment_x {
            return Some((row, EditColumn::Comment));
        }
        None
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

        let upper = self.config.uppercase;
        let lines: Vec<String> = indices
            .iter()
            .filter_map(|&idx| {
                provider.instruction(idx).map(|instr| {
                    // Honour both `address_width_64` and `uppercase`
                    // when assembling the clipboard payload — a host
                    // configured for x32 lowercase output should get
                    // exactly that on Ctrl+C.
                    let addr = match (upper, self.config.address_width_64) {
                        (true, true) => format!("{:016X}", instr.address()),
                        (false, true) => format!("{:016x}", instr.address()),
                        (true, false) => format!("{:08X}", instr.address()),
                        (false, false) => format!("{:08x}", instr.address()),
                    };
                    let bytes_str = join_bytes_hex(instr.bytes(), upper);
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
