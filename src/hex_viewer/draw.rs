//! Render pipeline: row drawing, byte color overrides, data inspector,
//! goto/search popups.
//!
//! `render` is the only entry point. It caches font metrics, computes
//! the visible row window from `scroll_y`, and dispatches to the
//! per-row, per-byte drawing helpers. All of these are split off as
//! `&self` / `&mut self` methods on `HexViewer` (defined here) so the
//! state lookup paths stay direct.

use super::HexViewer;
use super::config::{ByteCategory, Endianness};
use super::input::EditColumn;
use crate::utils::color::rgba_f32;
use crate::utils::hex::byte_hex;
use crate::utils::text::calc_text_size;

/// Convert `[r, g, b, a]` to packed u32 color.
pub(super) fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── HexViewer impl: render entry + virtualisation ────────────────────────────

impl HexViewer {
    pub fn render(&mut self, ui: &dear_imgui_rs::Ui) {
        if self.data.is_empty() {
            return;
        }

        if self.config.auto_refresh_frames > 0 {
            self.frame_count = self.frame_count.wrapping_add(1);
        }

        // Cache font metrics. Guard against the rare zero-glyph case (e.g.
        // before the font atlas is fully built or in test stubs) — division
        // by zero in the row math below would produce inf-cast UB.
        let [cw, ch] = calc_text_size("0");
        self.char_advance = cw.max(1.0);
        self.line_height = (ch + 2.0).max(1.0);

        let bpr = self.config.bytes_per_row.value();
        let total_rows = self.data.len().div_ceil(bpr);

        self.render_goto_popup(ui);
        self.render_search_popup(ui);

        let avail = ui.content_region_avail();
        let inspector_h = if self.config.show_inspector {
            self.line_height * 5.0
        } else {
            0.0
        };
        let child_h = avail[1] - inspector_h;

        let child_id = format!("##hv_child_{}", self.id);

        ui.child_window(&child_id)
            .size([avail[0], child_h])
            .build(ui, || {
                self.focused = ui.is_window_focused();

                // Scroll-to target.
                if let Some(row) = self.scroll_to_row.take() {
                    let y = row as f32 * self.line_height;
                    ui.set_scroll_y(y);
                }

                if self.focused {
                    self.handle_keyboard(ui);
                }
                self.handle_mouse(ui, avail[0]);

                let draw_list = ui.get_window_draw_list();
                let [win_x, win_y] = ui.cursor_screen_pos();
                let scroll_y = ui.scroll_y();
                let visible_h = ui.window_size()[1];

                // Correct virtualization: use scroll_y from ImGui scrollbar.
                let first_row = (scroll_y / self.line_height) as usize;
                let visible_count = (visible_h / self.line_height) as usize + 2;
                let last_row = (first_row + visible_count).min(total_rows);

                let header_offset = if self.config.show_column_headers {
                    1
                } else {
                    0
                };

                // Column header — draw at fixed position relative to window.
                if self.config.show_column_headers && first_row == 0 {
                    let hdr_y = win_y;
                    self.draw_column_header(&draw_list, win_x, hdr_y);
                }

                // Rows: position each row at its absolute scroll position.
                let mouse_pos = ui.io().mouse_pos();
                for row in first_row..last_row {
                    // Absolute Y position within the scrollable area.
                    let y = win_y + (row + header_offset) as f32 * self.line_height - scroll_y;
                    let offset = row * bpr;
                    let row_end = (offset + bpr).min(self.data.len());

                    self.draw_row(
                        ui,
                        &draw_list,
                        win_x,
                        y,
                        offset,
                        row_end,
                        bpr,
                        mouse_pos,
                        [win_x, win_y],
                        avail[0],
                    );
                }

                // Total content height for scrollbar.
                let total_h = (total_rows + header_offset) as f32 * self.line_height;
                ui.dummy([avail[0], total_h]);
            });

        if self.config.show_inspector {
            self.render_inspector(ui);
        }
    }
}

// ── HexViewer impl: layout helpers ───────────────────────────────────────────

impl HexViewer {
    pub(super) fn offset_col_width(&self) -> f32 {
        if self.config.show_offsets {
            self.char_advance * 10.0
        } else {
            0.0
        }
    }

    pub(super) fn hex_col_width(&self) -> f32 {
        let bpr = self.config.bytes_per_row.value();
        let group = self.config.grouping.value();
        let groups = if group > 0 { bpr.div_ceil(group) } else { 1 };
        let extra_spaces = groups.saturating_sub(1);
        (bpr * 3 + extra_spaces) as f32 * self.char_advance
    }

    fn draw_column_header(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
    ) {
        let bpr = self.config.bytes_per_row.value();
        let group = self.config.grouping.value();
        let hdr_col = col32(self.config.color_header);

        if self.config.show_offsets {
            draw_list.add_text([origin_x, y], hdr_col, "Offset  ");
        }

        let hex_x = origin_x + self.offset_col_width();
        let mut x = hex_x;
        for i in 0..bpr {
            // Column index 0..63 always fits in u8 — safe cast.
            draw_list.add_text([x, y], hdr_col, byte_hex(i as u8, self.config.uppercase));
            x += self.char_advance * 3.0;
            if group > 0 && (i + 1) % group == 0 && i + 1 < bpr {
                x += self.char_advance;
            }
        }

        if self.config.show_ascii {
            let ascii_x = hex_x + self.hex_col_width() + self.char_advance;
            draw_list.add_text([ascii_x, y], hdr_col, "ASCII");
        }
    }

    /// Check if offset is within a search match.
    ///
    /// Hot — runs once per visible byte per frame. `search_results` is
    /// sorted by construction (`find_pattern_masked` walks the buffer
    /// left-to-right), so we binary-search for the candidate start
    /// instead of scanning all matches. Brings overall hex-render cost
    /// from O(visible_bytes × matches) down to O(visible_bytes × log
    /// matches), which matters when a permissive wildcard pattern
    /// produces thousands of hits.
    pub(super) fn is_search_match(&self, offset: usize) -> bool {
        if self.search_results.is_empty() || self.search_pattern.is_empty() {
            return false;
        }
        let plen = self.search_pattern.len();
        // We want the smallest match-start `s` with `s >= offset - (plen - 1)`,
        // then verify `s <= offset`. A match covers `[s, s + plen)`.
        let lower = offset.saturating_sub(plen.saturating_sub(1));
        let pos = self.search_results.partition_point(|&s| s < lower);
        self.search_results
            .get(pos)
            .is_some_and(|&s| s <= offset)
    }

    /// Get foreground color with diff/region overrides.
    pub(super) fn byte_fg_with_overrides(&self, offset: usize, byte: u8) -> u32 {
        let cfg = &self.config;

        // Changed byte (diff).
        if cfg.highlight_changes
            && !self.reference.is_empty()
            && offset < self.reference.len()
            && self.data[offset] != self.reference[offset]
        {
            return col32(cfg.color_changed);
        }

        // Color region.
        for region in &self.regions {
            if offset >= region.offset && offset < region.offset + region.len {
                return col32(region.color);
            }
        }

        // Category / default.
        col32(cfg.byte_fg_color(byte))
    }
}

// ── HexViewer impl: row drawing ──────────────────────────────────────────────

impl HexViewer {
    #[allow(clippy::too_many_arguments)]
    fn draw_row(
        &self,
        ui: &dear_imgui_rs::Ui,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
        offset: usize,
        row_end: usize,
        bpr: usize,
        mouse_pos: [f32; 2],
        _win_pos: [f32; 2],
        _win_w: f32,
    ) {
        let cfg = &self.config;
        let group = cfg.grouping.value();

        // ── Offset column ─────────────────────────────────────
        if cfg.show_offsets {
            let addr = cfg.base_address + offset as u64;
            let txt = if cfg.uppercase {
                format!("{:08X}: ", addr)
            } else {
                format!("{:08x}: ", addr)
            };
            draw_list.add_text([origin_x, y], col32(cfg.color_offset), &txt);
        }

        let hex_x = origin_x + self.offset_col_width();
        let mut x = hex_x;

        // ── Hex bytes ─────────────────────────────────────────
        for i in offset..offset + bpr {
            if i < row_end {
                let byte = self.data[i];
                let is_cursor = i == self.cursor;
                let is_selected = self.selection.contains(i);
                let is_editing = is_cursor && self.edit_column == Some(EditColumn::Hex);

                // Background: cursor > selection > search > changed > region.
                let bg = if is_editing {
                    // Editing highlight — bright, distinctive.
                    Some(col32([0.50, 0.30, 0.10, 0.85]))
                } else if is_cursor {
                    Some(col32(cfg.color_cursor_bg))
                } else if is_selected {
                    Some(col32(cfg.color_selection_bg))
                } else if self.is_search_match(i) {
                    Some(col32(cfg.color_search_match))
                } else {
                    None
                };

                if let Some(bg_col) = bg {
                    draw_list
                        .add_rect(
                            [x - 1.0, y],
                            [x + self.char_advance * 2.0 + 1.0, y + self.line_height],
                            bg_col,
                        )
                        .filled(true)
                        .build();
                }

                // Foreground color.
                let fg = self.byte_fg_with_overrides(i, byte);

                // Show text: either the editing nibble or the byte value.
                if is_editing {
                    if let Some(hi_nibble) = self.edit_nibble {
                        // Show first nibble + underscore for second. Two-byte
                        // staticbuffer avoids a per-byte heap alloc.
                        let mut buf = [b'_'; 2];
                        buf[0] = if cfg.uppercase {
                            b"0123456789ABCDEF"[hi_nibble as usize & 0xF]
                        } else {
                            b"0123456789abcdef"[hi_nibble as usize & 0xF]
                        };
                        // SAFETY: `buf` is two ASCII bytes from the hex
                        // alphabet (or `_`), valid UTF-8 by construction.
                        let txt = unsafe { std::str::from_utf8_unchecked(&buf) };
                        draw_list.add_text([x, y], col32([1.0, 1.0, 0.5, 1.0]), txt);
                    } else {
                        // Show current value with blinking underline.
                        let txt = byte_hex(byte, cfg.uppercase);
                        draw_list.add_text([x, y], col32([1.0, 1.0, 0.5, 1.0]), txt);
                        draw_list
                            .add_line(
                                [x, y + self.line_height - 1.0],
                                [x + self.char_advance * 2.0, y + self.line_height - 1.0],
                                col32([1.0, 0.8, 0.3, 1.0]),
                            )
                            .thickness(1.5)
                            .build();
                    }
                } else {
                    draw_list.add_text([x, y], fg, byte_hex(byte, cfg.uppercase));
                }

                // Hover tooltip.
                let byte_hovered = mouse_pos[0] >= x
                    && mouse_pos[0] < x + self.char_advance * 2.5
                    && mouse_pos[1] >= y
                    && mouse_pos[1] < y + self.line_height;
                if byte_hovered && !is_editing {
                    draw_list
                        .add_rect(
                            [x - 1.0, y],
                            [x + self.char_advance * 2.0 + 1.0, y + self.line_height],
                            col32([0.4, 0.63, 0.88, 0.18]),
                        )
                        .filled(true)
                        .build();
                    ui.tooltip(|| {
                        let addr = cfg.base_address + i as u64;
                        ui.text(format!("Offset: 0x{:08X} ({})", addr, i));
                        ui.text(format!(
                            "Hex: 0x{:02X}  Dec: {}  Oct: 0o{:03o}",
                            byte, byte, byte
                        ));
                        ui.text(format!("Bin: {:08b}", byte));
                        ui.text(format!("Category: {:?}", ByteCategory::of(byte)));
                        if byte.is_ascii_graphic() || byte == b' ' {
                            ui.text(format!("Char: '{}'", byte as char));
                        }
                    });
                }
            }

            x += self.char_advance * 3.0;
            let col_idx = i - offset;
            if group > 0 && (col_idx + 1).is_multiple_of(group) && col_idx + 1 < bpr {
                x += self.char_advance;
            }
        }

        // ── ASCII column ──────────────────────────────────────
        if cfg.show_ascii {
            let ascii_x = hex_x + self.hex_col_width() + self.char_advance;
            let mut ax = ascii_x;
            for i in offset..row_end {
                let byte = self.data[i];
                let is_cursor = i == self.cursor;
                let is_selected = self.selection.contains(i);
                let is_ascii_editing = is_cursor && self.edit_column == Some(EditColumn::Ascii);

                let ch = if (0x20..0x7F).contains(&byte) {
                    byte as char
                } else {
                    '.'
                };

                // Background highlight.
                if is_ascii_editing {
                    draw_list
                        .add_rect(
                            [ax, y],
                            [ax + self.char_advance, y + self.line_height],
                            col32([0.50, 0.30, 0.10, 0.85]),
                        )
                        .filled(true)
                        .build();
                } else if is_cursor {
                    draw_list
                        .add_rect(
                            [ax, y],
                            [ax + self.char_advance, y + self.line_height],
                            col32(cfg.color_cursor_bg),
                        )
                        .filled(true)
                        .build();
                } else if is_selected {
                    draw_list
                        .add_rect(
                            [ax, y],
                            [ax + self.char_advance, y + self.line_height],
                            col32(cfg.color_selection_bg),
                        )
                        .filled(true)
                        .build();
                }

                let color = if is_ascii_editing {
                    col32([1.0, 1.0, 0.5, 1.0])
                } else if ch == '.' {
                    col32(cfg.color_ascii_dot)
                } else {
                    col32(cfg.color_ascii)
                };

                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                draw_list.add_text([ax, y], color, s);

                // Underline for ASCII edit mode.
                if is_ascii_editing {
                    draw_list
                        .add_line(
                            [ax, y + self.line_height - 1.0],
                            [ax + self.char_advance, y + self.line_height - 1.0],
                            col32([1.0, 0.8, 0.3, 1.0]),
                        )
                        .thickness(1.5)
                        .build();
                }

                ax += self.char_advance;
            }
        }
    }
}

// ── HexViewer impl: data inspector ───────────────────────────────────────────

impl HexViewer {
    fn render_inspector(&self, ui: &dear_imgui_rs::Ui) {
        if self.cursor >= self.data.len() {
            return;
        }

        ui.separator();
        let offset = self.cursor;
        let remaining = self.data.len() - offset;
        let bytes = &self.data[offset..];
        let le = matches!(self.config.endianness, Endianness::Little);

        let label_col = col32(self.config.color_inspector_label);
        let value_col = col32(self.config.color_inspector_value);

        let draw_list = ui.get_window_draw_list();
        let [x, y] = ui.cursor_screen_pos();
        let cw = self.char_advance;
        let lh = self.line_height;

        // Row 1: integers
        let mut cx = x;
        let items_r1: Vec<(&str, String)> = vec![
            ("u8", format!("{}", bytes[0])),
            ("i8", format!("{}", bytes[0] as i8)),
            if remaining >= 2 {
                let v = if le {
                    u16::from_le_bytes([bytes[0], bytes[1]])
                } else {
                    u16::from_be_bytes([bytes[0], bytes[1]])
                };
                ("u16", format!("{}", v))
            } else {
                ("u16", "\u{2014}".into())
            },
            if remaining >= 4 {
                let arr = [bytes[0], bytes[1], bytes[2], bytes[3]];
                let v = if le {
                    u32::from_le_bytes(arr)
                } else {
                    u32::from_be_bytes(arr)
                };
                ("u32", format!("{}", v))
            } else {
                ("u32", "\u{2014}".into())
            },
            if remaining >= 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[..8]);
                let v = if le {
                    u64::from_le_bytes(arr)
                } else {
                    u64::from_be_bytes(arr)
                };
                ("u64", format!("{}", v))
            } else {
                ("u64", "\u{2014}".into())
            },
        ];

        for (label, value) in &items_r1 {
            draw_list.add_text([cx, y], label_col, format!("{}=", label));
            cx += (label.len() + 1) as f32 * cw;
            draw_list.add_text([cx, y], value_col, value);
            cx += (value.len() + 2) as f32 * cw;
        }

        // Row 2: floats + hex + char
        cx = x;
        let y2 = y + lh;
        let items_r2: Vec<(&str, String)> = vec![
            if remaining >= 4 {
                let arr = [bytes[0], bytes[1], bytes[2], bytes[3]];
                let v = if le {
                    f32::from_le_bytes(arr)
                } else {
                    f32::from_be_bytes(arr)
                };
                ("f32", format!("{:.6e}", v))
            } else {
                ("f32", "\u{2014}".into())
            },
            if remaining >= 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[..8]);
                let v = if le {
                    f64::from_le_bytes(arr)
                } else {
                    f64::from_be_bytes(arr)
                };
                ("f64", format!("{:.6e}", v))
            } else {
                ("f64", "\u{2014}".into())
            },
            ("hex", format!("0x{:02X}", bytes[0])),
            ("char", {
                let ch = bytes[0];
                if (0x20..0x7F).contains(&ch) {
                    format!("'{}'", ch as char)
                } else {
                    format!("\\x{:02X}", ch)
                }
            }),
        ];

        for (label, value) in &items_r2 {
            draw_list.add_text([cx, y2], label_col, format!("{}=", label));
            cx += (label.len() + 1) as f32 * cw;
            draw_list.add_text([cx, y2], value_col, value);
            cx += (value.len() + 2) as f32 * cw;
        }

        // Row 3: offset info
        let y3 = y2 + lh;
        let undo_info = if self.undo.can_undo() || self.undo.can_redo() {
            format!(
                "  Undo: {} / Redo: {}",
                self.undo.undo_count(),
                self.undo.redo_count()
            )
        } else {
            String::new()
        };
        let edit_info = match self.edit_column {
            Some(EditColumn::Hex) => "  [EDITING HEX]",
            Some(EditColumn::Ascii) => "  [EDITING ASCII]",
            None => "",
        };
        let info = format!(
            "Offset: 0x{:08X} ({})  Endian: {}  Data: {} bytes{}{}",
            self.config.base_address + offset as u64,
            offset,
            self.config.endianness.display_name(),
            self.data.len(),
            undo_info,
            edit_info,
        );
        draw_list.add_text([x, y3], label_col, &info);

        ui.dummy([0.0, lh * 4.0]);
    }
}

