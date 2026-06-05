//! Mouse handling + hit-testing for `HexViewer`.
//!
//! Split out of `input.rs` to keep both files under the 500-line
//! ceiling. `handle_mouse` and the `mouse_to_*` hit-test helpers are
//! `pub(super)` so the render loop in `draw.rs` and the keyboard
//! handler in `input.rs` can reach them.

use super::HexViewer;
use super::input::{ADDRESS_FLASH_FRAMES, EditColumn};
use super::provider::HexDataProvider;
use super::search::Selection;
use crate::utils::clipboard::set_clipboard;

// ── HexViewer impl: mouse + hit-testing ──────────────────────────────────────

impl HexViewer {
    pub(super) fn handle_mouse(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        _win_w: f32,
        provider: &mut dyn HexDataProvider,
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

        // ── Address-gutter affordance ────────────────────────────────
        // Hovering the address column shows a `Hand` cursor + tooltip
        // ("Double-click to copy 0x...") so the gesture is
        // discoverable. A bare double-click (no modifier) copies the
        // row's address as a hex literal to the clipboard and triggers
        // a brief background flash. Single-click was the historic
        // gesture but the user reported it felt accidental on
        // 2026-04-30 — promoted to double-click for parity with
        // `disasm_view`'s address-gutter copy. Modifier-held clicks
        // still fall through so shift-extend / ctrl-toggle work in
        // the data area when the cursor happens to be over the
        // address column edge.
        if let Some(row) = self.mouse_to_address_row(ui) {
            ui.set_mouse_cursor(Some(dear_imgui_rs::MouseCursor::Hand));
            let bpr = self.config.bytes_per_row.value();
            let addr = self.config.base_address + (row * bpr) as u64;
            let formatted = self.format_address_literal(addr);
            let s = self.strings();
            crate::utils::tooltip::themed_tooltip(ui, || {
                ui.text(format!("{}: {}", s.tooltip_double_click_copy, formatted));
            });
            if !shift && !ctrl && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left) {
                set_clipboard(&formatted);
                self.address_flash = Some((row, ADDRESS_FLASH_FRAMES));
                // Don't fall through into the hex/ASCII click handler:
                // the gesture was intended for the address gutter.
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
                self.commit_pending_edit_with(provider);
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
        // Match the offset-gutter digit count via `effective_data_len`
        // — streaming providers report 64-bit windows while
        // `self.data` may be tiny / empty; the gutter shows 16-digit
        // addresses in that case, so the copy literal must too.
        let digits = self.config.address_width.hex_digits(
            self.config.base_address,
            self.data.len().max(self.effective_data_len),
        );
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
        // Use the cached `effective_data_len` (set by `render_impl`)
        // so streaming providers reporting `u64::MAX` clamp through
        // `PROVIDER_LEN_CAP` instead of overflowing the row count.
        let total_rows = self.effective_data_len.div_ceil(bpr);
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
            if offset < self.effective_data_len {
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
        if offset < self.effective_data_len {
            Some((offset, EditColumn::Hex))
        } else {
            None
        }
    }
}
