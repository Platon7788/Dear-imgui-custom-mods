//! Input handling for [`super::DisasmView`], split into cohesive
//! siblings:
//!
//! - [`keyboard`] — cursor moves, page-up/down, Home/End, Enter/Space
//!   to follow, `G` goto, `Ctrl+A` select-all, `F9` breakpoint,
//!   `Alt+arrow` nav history, the inline-edit Esc path.
//! - [`mouse`] — click / shift / ctrl / drag-select / double-click to
//!   follow or edit / middle-click follow / right-click context menu,
//!   and the address-gutter double-click-to-copy gesture.
//!
//! This `mod.rs` keeps the cross-cutting pieces both siblings reach
//! for: the pure row/column hit-test math, the inline-edit commit
//! path, clipboard copy, and the small reusable byte/row helpers.

mod keyboard;
mod mouse;

use super::provider::DisasmDataProvider;
use super::{DisasmView, EditColumn, EditState};
use crate::utils::hex::byte_hex;

/// How many frames the address-gutter "just copied" pill stays
/// painted after a double-click-to-copy. ~30 frames @ 60 fps ≈ 0.5 s
/// — same dwell as `hex_viewer::ADDRESS_FLASH_FRAMES`.
pub(super) const ADDRESS_FLASH_FRAMES: u32 = 30;

/// Visible-row count from window height. Guards against zero
/// `line_height` (degenerate font load) so a divide doesn't yield
/// `inf as usize` (UB-adjacent in debug, `0` in release on x86).
#[inline]
#[must_use]
pub(super) fn rows_in_window(window_h: f32, line_height: f32) -> usize {
    if line_height > 0.0 {
        (window_h / line_height) as usize
    } else {
        0
    }
}

/// Pure row-from-pixel hit-test, factored out of
/// [`DisasmView::mouse_to_instruction`] so the boundary math can be
/// unit-tested without an ImGui context.
///
/// `my` is the mouse Y in screen space, `win_y` the current draw
/// origin Y (already scroll-adjusted by `cursor_screen_pos`), and
/// `header_h` the column-header band height (0 when hidden). Returns
/// `None` for a click above the first row, on/inside the header band,
/// when `line_height` is non-positive (degenerate font), or past the
/// last decoded instruction.
#[inline]
#[must_use]
pub(super) fn row_from_mouse_y(
    my: f32,
    win_y: f32,
    header_h: f32,
    line_height: f32,
    count: usize,
) -> Option<usize> {
    if line_height <= 0.0 {
        return None;
    }
    // Single combined division — the previous code did two
    // independent `floor`s that didn't compose at the row boundary,
    // so a click on the last pixel of a row could promote to the row
    // above (audit M6). `cursor_screen_pos()` already accounts for
    // scroll, so the offset cancels cleanly here.
    let combined = (my - win_y - header_h) / line_height;
    if combined < 0.0 {
        return None;
    }
    let row = combined as usize;
    (row < count).then_some(row)
}

/// Reusable space-separated hex byte formatter — single-allocation
/// `String` (3 chars/byte: two hex + one space, minus the trailing
/// gap). Mirrors the per-row pattern in `draw::draw_instruction_row`.
#[must_use]
pub(super) fn join_bytes_hex(bytes: &[u8], uppercase: bool) -> String {
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
    pub(in crate::disasm_view) fn commit_edit(
        &self,
        edit: EditState,
        provider: &mut dyn DisasmDataProvider,
    ) {
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

    /// Hit-test the mouse against the row grid: returns the
    /// instruction index under the cursor (Y only), or `None` when
    /// above the first row / inside the header / past the last row.
    pub(super) fn mouse_to_instruction(
        &self,
        ui: &dear_imgui_rs::Ui,
        provider: &dyn DisasmDataProvider,
    ) -> Option<usize> {
        let [_mx, my] = ui.io().mouse_pos();
        let [_win_x, win_y] = ui.cursor_screen_pos();
        let header_h = if self.config.show_header {
            self.line_height
        } else {
            0.0
        };
        row_from_mouse_y(
            my,
            win_y,
            header_h,
            self.line_height,
            provider.instruction_count(),
        )
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
        (mx >= addr_left && mx < addr_right).then_some(row)
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
        let x0 = win_x + ui.scroll_x();
        column_at_x(mx, x0, &self.config, self.frame_comment_x.get()).map(|col| (row, col))
    }

    pub(super) fn ensure_visible(&mut self, idx: usize, ui: &dear_imgui_rs::Ui) {
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
                        .map(|c| format!(" ; {c}"))
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

        crate::utils::clipboard::set_clipboard(&lines.join("\n"));
    }
}

/// Pure column hit-test, factored out of [`DisasmView::mouse_to_cell`]
/// so the X-boundary math can be unit-tested without an ImGui context.
///
/// `mx` is the mouse X (screen space), `x0` the row draw origin
/// (`win_x + scroll_x`), and `frame_comment_x` the per-frame dynamic
/// comment-column X computed by `render()` last frame (`None` only on
/// frame 0, before `render()` populated it).
///
/// Mirrors the X-layout in `draw::draw_instruction_row` —
/// margin → arrows → address → bytes → mnemonic+operands → comment.
/// The Mnemonic+operands region returns `None` on purpose: the
/// re-assemble flow is gated until `DisasmDataProvider::assemble`
/// works, so opening an editor there would be a UX leak.
#[must_use]
pub(super) fn column_at_x(
    mx: f32,
    x0: f32,
    config: &super::config::DisasmViewConfig,
    frame_comment_x: Option<f32>,
) -> Option<EditColumn> {
    let cols = &config.columns;
    let mut x = x0;
    if config.show_breakpoints {
        x += cols.margin;
    }
    if config.show_arrows {
        x += cols.arrows;
    }
    x += cols.address;
    let bytes_x = x;
    let mnemonic_x = if config.show_bytes {
        bytes_x + cols.bytes
    } else {
        bytes_x
    };
    // 1-frame-stale dynamic comment X (invisible for double-click);
    // falls back to the static column total on frame 0.
    let comment_x = frame_comment_x.unwrap_or(mnemonic_x + cols.mnemonic + cols.operands);

    // Bytes cell — only when bytes column is visible.
    if config.show_bytes && mx >= bytes_x && mx < mnemonic_x {
        return Some(EditColumn::Bytes);
    }
    // Mnemonic + operands region is GATED to `None` until the
    // assembler round-trip is wired (`assemble` default-impl is a
    // no-op). Flip back to `Some(EditColumn::Mnemonic)` once it works.
    if mx >= mnemonic_x && mx < comment_x {
        return None;
    }
    // Comment cell — only when the comment column is visible.
    if config.show_comments && mx >= comment_x {
        return Some(EditColumn::Comment);
    }
    None
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disasm_view::config::DisasmViewConfig;

    // ── rows_in_window ───────────────────────────────────────────
    #[test]
    fn rows_in_window_basic() {
        assert_eq!(rows_in_window(100.0, 10.0), 10);
        assert_eq!(rows_in_window(105.0, 10.0), 10); // floor
        assert_eq!(rows_in_window(9.0, 10.0), 0);
    }

    #[test]
    fn rows_in_window_zero_line_height_is_zero_not_inf() {
        // Degenerate font: must NOT produce `inf as usize`.
        assert_eq!(rows_in_window(100.0, 0.0), 0);
        assert_eq!(rows_in_window(100.0, -1.0), 0);
    }

    // ── row_from_mouse_y ─────────────────────────────────────────
    #[test]
    fn row_from_mouse_y_maps_each_row() {
        // win_y = 0, no header, line_height 10, 5 rows.
        assert_eq!(row_from_mouse_y(0.0, 0.0, 0.0, 10.0, 5), Some(0));
        assert_eq!(row_from_mouse_y(9.9, 0.0, 0.0, 10.0, 5), Some(0));
        assert_eq!(row_from_mouse_y(10.0, 0.0, 0.0, 10.0, 5), Some(1));
        assert_eq!(row_from_mouse_y(49.9, 0.0, 0.0, 10.0, 5), Some(4));
    }

    #[test]
    fn row_from_mouse_y_above_first_row_is_none() {
        // Mouse above the draw origin.
        assert_eq!(row_from_mouse_y(-1.0, 0.0, 0.0, 10.0, 5), None);
    }

    #[test]
    fn row_from_mouse_y_header_offsets_rows() {
        // header_h = 10 → first data row starts at y=10.
        assert_eq!(row_from_mouse_y(5.0, 0.0, 10.0, 10.0, 5), None);
        assert_eq!(row_from_mouse_y(10.0, 0.0, 10.0, 10.0, 5), Some(0));
        assert_eq!(row_from_mouse_y(21.0, 0.0, 10.0, 10.0, 5), Some(1));
    }

    #[test]
    fn row_from_mouse_y_past_last_row_is_none() {
        // 5 rows, click at row 5 → out of range.
        assert_eq!(row_from_mouse_y(50.0, 0.0, 0.0, 10.0, 5), None);
    }

    #[test]
    fn row_from_mouse_y_zero_line_height_is_none() {
        assert_eq!(row_from_mouse_y(50.0, 0.0, 0.0, 0.0, 5), None);
        assert_eq!(row_from_mouse_y(50.0, 0.0, 0.0, -2.0, 5), None);
    }

    #[test]
    fn row_from_mouse_y_empty_provider_is_none() {
        assert_eq!(row_from_mouse_y(5.0, 0.0, 0.0, 10.0, 0), None);
    }

    // ── join_bytes_hex ───────────────────────────────────────────
    #[test]
    fn join_bytes_hex_formats_uppercase_and_lowercase() {
        assert_eq!(join_bytes_hex(&[0x48, 0x89, 0xE5], true), "48 89 E5");
        assert_eq!(join_bytes_hex(&[0x48, 0x89, 0xE5], false), "48 89 e5");
        assert_eq!(join_bytes_hex(&[], true), "");
        assert_eq!(join_bytes_hex(&[0xFF], true), "FF");
    }

    // ── column_at_x ──────────────────────────────────────────────
    // Build a config with the default column geometry so the
    // boundaries are deterministic.
    fn cfg_all_columns() -> DisasmViewConfig {
        DisasmViewConfig {
            show_breakpoints: true,
            show_arrows: true,
            show_bytes: true,
            show_comments: true,
            ..Default::default()
        }
    }

    #[test]
    fn column_at_x_bytes_region() {
        let c = cfg_all_columns();
        let cols = &c.columns;
        let bytes_x = cols.margin + cols.arrows + cols.address;
        // Just inside the bytes region.
        assert_eq!(
            column_at_x(bytes_x + 1.0, 0.0, &c, None),
            Some(EditColumn::Bytes)
        );
    }

    #[test]
    fn column_at_x_mnemonic_region_is_gated_none() {
        let c = cfg_all_columns();
        let cols = &c.columns;
        let mnemonic_x = cols.margin + cols.arrows + cols.address + cols.bytes;
        // Inside mnemonic/operands → intentionally None (gated).
        assert_eq!(column_at_x(mnemonic_x + 1.0, 0.0, &c, None), None);
    }

    #[test]
    fn column_at_x_comment_region() {
        let c = cfg_all_columns();
        let cols = &c.columns;
        let static_comment_x =
            cols.margin + cols.arrows + cols.address + cols.bytes + cols.mnemonic + cols.operands;
        assert_eq!(
            column_at_x(static_comment_x + 1.0, 0.0, &c, None),
            Some(EditColumn::Comment)
        );
    }

    #[test]
    fn column_at_x_dynamic_comment_x_overrides_static() {
        let c = cfg_all_columns();
        // Push the comment column far right; a click at the static
        // location now lands in the (gated) mnemonic region → None.
        let dynamic = 5000.0;
        assert_eq!(
            column_at_x(dynamic + 1.0, 0.0, &c, Some(dynamic)),
            Some(EditColumn::Comment)
        );
        // A click left of the dynamic comment X is mnemonic → None.
        assert_eq!(column_at_x(dynamic - 1.0, 0.0, &c, Some(dynamic)), None);
    }

    #[test]
    fn column_at_x_left_of_bytes_is_none() {
        let c = cfg_all_columns();
        // Inside the address/arrow/margin gutter → None (non-edit).
        assert_eq!(column_at_x(1.0, 0.0, &c, None), None);
    }

    #[test]
    fn column_at_x_hidden_bytes_shifts_mnemonic_left() {
        let mut c = cfg_all_columns();
        c.show_bytes = false;
        let cols = &c.columns;
        // With bytes hidden, mnemonic starts right after address, so
        // the old bytes region is now the (gated) mnemonic → None.
        let old_bytes_x = cols.margin + cols.arrows + cols.address;
        assert_eq!(column_at_x(old_bytes_x + 1.0, 0.0, &c, None), None);
    }
}
