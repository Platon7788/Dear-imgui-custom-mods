//! Mouse interaction for [`super::DisasmView`] — wheel scroll, the
//! address-gutter double-click-to-copy gesture, click/shift/ctrl
//! selection, drag-select, double-click / middle-click follow,
//! double-click-to-edit, and the right-click context menu.

use super::super::provider::DisasmDataProvider;
use super::super::{DisasmView, EditColumn, EditState};
use super::{ADDRESS_FLASH_FRAMES, join_bytes_hex};
use crate::utils::clipboard::set_clipboard;

impl DisasmView {
    pub(in crate::disasm_view) fn handle_mouse(
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
        // values), so a slow touchpad pan stayed at zero rows and never
        // scrolled. Three rows per notch matches Windows'
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
        // select-row semantics.
        if let Some(row) = self.mouse_to_address_row(ui, provider)
            && let Some(instr) = provider.instruction(row)
        {
            ui.set_mouse_cursor(Some(dear_imgui_rs::MouseCursor::Hand));
            let addr = instr.address();
            let formatted = self.format_address_literal(addr);
            let s = self.strings();
            crate::utils::tooltip::themed_tooltip(ui, || {
                ui.text(format!("{}: {}", s.tooltip_double_click_copy, formatted));
            });
            if !shift && !ctrl && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left) {
                set_clipboard(&formatted);
                self.address_flash = Some((row, ADDRESS_FLASH_FRAMES));
                // Drop any drag-select origin a single click below may
                // have set — otherwise the next mouse-drag after the
                // copy uses the stale origin index and selects a
                // phantom range (M7 from session 034 audit).
                self.drag_origin = None;
                // Don't fall through into the row-edit double-click
                // handler below — gesture was for the address gutter.
                return;
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
                    if !self.selection.remove(&idx) {
                        self.selection.insert(idx);
                    }
                    self.cursor_idx = Some(idx);
                    self.sel_anchor = Some(idx);
                } else {
                    // Single-click moves the cursor + replaces the
                    // selection. Origin breadcrumb is *kept* — clicking
                    // around the disasm to read code shouldn't wipe the
                    // "where I jumped from" highlight. Explicit
                    // dismissal is `Esc` (or a new navigation, which
                    // overwrites origin anyway).
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
        // Any non-address-gutter row hit attempts follow first; only
        // when follow declines (no branch target, no resolvable
        // operand pointer) does the edit-cell branch below get a turn.
        // Address-gutter double-click is handled separately above
        // (copy-to-clipboard with `return`), so we never run twice on
        // the same gesture.
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
        // double-click time / position thresholds entirely — a single
        // deliberate middle-click always navigates if the row is
        // followable.
        if ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Middle)
            && let Some(row) = self.mouse_to_instruction(ui, provider)
        {
            self.cursor_idx = Some(row);
            self.follow_at_cursor(provider);
            return;
        }

        // ── Double-click to edit (if editable) ───────────────────
        //
        // Reached only when follow declined (e.g. `mov rax, rbx` with
        // no number / branch target). Cell hit-test picks the right
        // column → buffer initialiser:
        //   Bytes    — pre-fill with current "AA BB CC" hex string
        //   Mnemonic — reserved for a future re-assemble flow (not
        //              wired to UI yet; commit path exists)
        //   Comment  — pre-fill with current comment text
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
}
