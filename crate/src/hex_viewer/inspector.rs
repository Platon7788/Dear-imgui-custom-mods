//! Data inspector subview — numeric reinterpretations of the cursor byte.
//!
//! Three-column grid laid out by hand on the foreground draw list. Each
//! column is "label gap value" with shared `label_off` so the value
//! columns line up vertically. Mixing `add_text` with the live
//! `cursor_screen_pos` keeps the panel exactly four line-heights tall —
//! the most compact layout that still shows every numeric
//! reinterpretation simultaneously.
//!
//! Split out of `draw.rs` to keep that file under the 500-line ceiling.

use super::HexViewer;
use super::config::Endianness;
use super::draw::col32;
use super::input::EditColumn;
use super::provider::HexDataProvider;

/// Width of the typed-value grid's label cell ("u64 ", "char "), in glyphs.
/// Five fits both 3-letter labels (`u32`, `f64`, `hex`) and `char` with a
/// single trailing space.
const INSPECTOR_LABEL_GLYPHS: f32 = 5.0;

/// Horizontal offset of the second value column (start of `u64`), in glyphs.
/// Sized to clear `u32 4294967295` (label + max u32 = 14 chars) with two
/// glyphs of breathing room.
const INSPECTOR_COL2_GLYPHS: f32 = 16.0;

/// Horizontal offset of the third column (`char` + info row), in glyphs.
/// Sized to clear the longest middle-column value `u64 18446744073709551615`
/// (24 chars) with two glyphs of breathing room.
const INSPECTOR_COL3_GLYPHS: f32 = 42.0;

/// Total inspector height in line-heights — four data rows; the info row
/// piggy-backs on column 3 row 1, so it does not need its own line.
const INSPECTOR_ROWS: f32 = 4.0;

impl HexViewer {
    /// Pixel height required for the inspector subview. `0.0` when the
    /// inspector is hidden via config.
    pub(super) fn inspector_height(&self) -> f32 {
        if self.config.show_inspector {
            // INSPECTOR_ROWS lines + 1 px gap below the separator + ~2 px
            // breathing room so the bottom row never grazes the child border.
            self.line_height * INSPECTOR_ROWS + 3.0
        } else {
            0.0
        }
    }

    pub(super) fn render_inspector(
        &self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn HexDataProvider,
        data_len: usize,
    ) {
        if self.cursor >= data_len {
            return;
        }

        ui.separator();
        let offset = self.cursor;
        // Pull up to 8 bytes once from the active provider — that's
        // the largest reinterpretation the inspector renders (f64 /
        // u64). For a streaming provider this is the single read
        // that backs every numeric row below; if the provider returns
        // fewer than 8 bytes (live-memory hole, edge of mapped region)
        // the `remaining` value drops the unread rows to `—` exactly
        // like the legacy path did for buffers shorter than 8 bytes
        // at `cursor`.
        let mut inspector_buf = [0u8; 8];
        let want = 8usize.min(data_len.saturating_sub(offset));
        let got = provider.read(offset as u64, &mut inspector_buf[..want]);
        if got == 0 {
            // Provider couldn't satisfy the cursor read (live-memory
            // hole, unmapped page). Skip the inspector entirely
            // instead of rendering an all-'—' row — matches the
            // empty-buffer guard above.
            return;
        }
        let remaining = got;
        let bytes = &inspector_buf[..got];
        let le = matches!(self.config.endianness, Endianness::Little);

        let label_col = col32(self.config.color_inspector_label);
        let value_col = col32(self.config.color_inspector_value);

        let draw_list = ui.get_window_draw_list();
        let [x_raw, y_top] = ui.cursor_screen_pos();
        let cw = self.char_advance;
        let lh = self.line_height;

        // Mirror the one-glyph left padding the hex area uses so the
        // inspector's first column lines up visually with the offset
        // column above instead of hugging the window border.
        let x = x_raw + cw;
        // 1 px gap below the separator so the first row doesn't sit
        // flush against the divider line.
        let y0 = y_top + 1.0;

        let label_off = INSPECTOR_LABEL_GLYPHS * cw;
        let col2_x = x + INSPECTOR_COL2_GLYPHS * cw;
        let col3_x = x + INSPECTOR_COL3_GLYPHS * cw;

        // ── Numeric reinterpretations (decoded once, reused) ────────────
        let u16_v = (remaining >= 2).then(|| {
            if le {
                u16::from_le_bytes([bytes[0], bytes[1]])
            } else {
                u16::from_be_bytes([bytes[0], bytes[1]])
            }
        });
        let arr4 = (remaining >= 4).then(|| [bytes[0], bytes[1], bytes[2], bytes[3]]);
        let u32_v = arr4.map(|a| {
            if le {
                u32::from_le_bytes(a)
            } else {
                u32::from_be_bytes(a)
            }
        });
        let f32_v = arr4.map(|a| {
            if le {
                f32::from_le_bytes(a)
            } else {
                f32::from_be_bytes(a)
            }
        });
        let arr8 = (remaining >= 8).then(|| {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes[..8]);
            arr
        });
        let u64_v = arr8.map(|a| {
            if le {
                u64::from_le_bytes(a)
            } else {
                u64::from_be_bytes(a)
            }
        });
        let f64_v = arr8.map(|a| {
            if le {
                f64::from_le_bytes(a)
            } else {
                f64::from_be_bytes(a)
            }
        });
        let char_str = {
            let b = bytes[0];
            if (0x20..0x7F).contains(&b) {
                format!("'{}'", b as char)
            } else {
                format!("\\x{b:02X}")
            }
        };
        let em_dash = || "\u{2014}".to_string();

        // ── Column 1: u8 / i8 / u16 / u32 ───────────────────────────────
        let col1: [(&str, String); 4] = [
            ("u8", format!("{}", bytes[0])),
            ("i8", format!("{}", bytes[0] as i8)),
            ("u16", u16_v.map_or_else(em_dash, |v| v.to_string())),
            ("u32", u32_v.map_or_else(em_dash, |v| v.to_string())),
        ];
        for (i, (label, value)) in col1.iter().enumerate() {
            let row_y = y0 + i as f32 * lh;
            draw_list.add_text([x, row_y], label_col, *label);
            draw_list.add_text([x + label_off, row_y], value_col, value);
        }

        // ── Column 2: u64 / f32 / f64 / hex ─────────────────────────────
        let col2: [(&str, String); 4] = [
            ("u64", u64_v.map_or_else(em_dash, |v| v.to_string())),
            ("f32", f32_v.map_or_else(em_dash, |v| format!("{v:.6e}"))),
            ("f64", f64_v.map_or_else(em_dash, |v| format!("{v:.6e}"))),
            ("hex", format!("0x{:02X}", bytes[0])),
        ];
        for (i, (label, value)) in col2.iter().enumerate() {
            let row_y = y0 + i as f32 * lh;
            draw_list.add_text([col2_x, row_y], label_col, *label);
            draw_list.add_text([col2_x + label_off, row_y], value_col, value);
        }

        // ── Column 3: char (row 0) + info (row 1) ───────────────────────
        // Single typed cell on top, single full-info line right under.
        // Rows 2-3 stay empty by design — the panel is anchored to four
        // lines tall so it lines up with the longest left/middle column.
        draw_list.add_text([col3_x, y0], label_col, "char");
        draw_list.add_text([col3_x + label_off, y0], value_col, &char_str);

        let undo_info = if self.undo.can_undo() || self.undo.can_redo() {
            format!(
                "   Undo {} / Redo {}",
                self.undo.undo_count(),
                self.undo.redo_count()
            )
        } else {
            String::new()
        };
        let edit_info = match self.edit_column {
            Some(EditColumn::Hex) => "   [EDITING HEX]",
            Some(EditColumn::Ascii) => "   [EDITING ASCII]",
            None => "",
        };
        // Address column width tracks the same `AddressWidth` policy as
        // the offset gutter — keeps file-dump panels compact while
        // letting 64-bit memory dumps breathe.
        let digits = self
            .config
            .address_width
            .hex_digits(self.config.base_address, data_len);
        let abs_addr = self.config.base_address + offset as u64;
        let addr_str = if digits == 16 {
            format!("0x{abs_addr:016X} ({offset})")
        } else {
            format!("0x{abs_addr:08X} ({offset})")
        };
        let info = format!(
            "{}   {}-endian   {} bytes{}{}",
            addr_str,
            self.config.endianness.display_name().to_lowercase(),
            data_len,
            undo_info,
            edit_info,
        );
        draw_list.add_text([col3_x, y0 + lh], label_col, &info);

        // Reserve exactly the painted area so the host child-window
        // scrollbar (if any) computes the right thumb size and no
        // dead space appears at the bottom.
        ui.dummy([0.0, INSPECTOR_ROWS * lh + 1.0]);
    }
}
