//! Single-row byte drawing: offset gutter, hex column, ASCII column.
//!
//! Split out of `draw.rs` so the per-row hot path lives in its own file
//! away from the render-entry / virtualisation code. `draw_row` is
//! `pub(super)` so the render loop in `draw.rs` can call it.

use super::HexViewer;
use super::draw::col32;
use super::input::EditColumn;
use super::provider::ByteCategory;
use crate::utils::hex::byte_hex;

impl HexViewer {
    /// Render a single row of bytes.
    ///
    /// `row_bytes` is the already-read slice of bytes for this row,
    /// length ≤ `bpr` (the partial last row has `< bpr` bytes). The
    /// caller (`render_impl`) reads these once per row via the active
    /// provider — `draw_row` itself never touches `self.data` for the
    /// byte values, only for downstream queries like
    /// `byte_fg_with_overrides` (which now also takes the live byte).
    ///
    /// `data_len` is the provider's `usize`-projected length, used for
    /// the `AddressWidth` digit selection so 8 vs 16 digit gutters
    /// match the underlying buffer (legacy behaviour: digits depend on
    /// the buffer's last address).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_row(
        &self,
        ui: &dear_imgui_rs::Ui,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
        offset: usize,
        row_bytes: &[u8],
        bpr: usize,
        mouse_pos: [f32; 2],
        _win_pos: [f32; 2],
        _win_w: f32,
        data_len: usize,
    ) {
        let cfg = &self.config;
        let group = cfg.grouping.value();
        let row_end = offset + row_bytes.len();

        // ── Offset column ─────────────────────────────────────
        if cfg.show_offsets {
            let row = offset / bpr;
            let addr = cfg.base_address + offset as u64;
            let digits = cfg.address_width.hex_digits(cfg.base_address, data_len);

            // "Just copied" flash — paint a translucent accent-coloured
            // pill behind the address text whenever the user has just
            // clicked the gutter. Tracks the row index, not the byte
            // offset, so the pill stays put even if the cursor wanders
            // off into the data area mid-flash.
            if let Some((flash_row, frames)) = self.address_flash
                && flash_row == row
                && frames > 0
            {
                // Fade out as the counter ticks down — clearer "this
                // happened just now, but it's about to disappear".
                let fade =
                    (frames as f32 / super::input::ADDRESS_FLASH_FRAMES as f32).clamp(0.0, 1.0);
                let c = cfg.color_cursor_bg;
                // Span = 0.5 ca left padding + the visible address
                // glyphs (`{:0NX}`) + 0.5 ca right padding — leaves
                // the trailing format-space unhighlighted so the
                // pill doesn't visually fuse into the column
                // divider. Was `digits + 1` while the format string
                // appended `:`; dropped to `digits` on 2026-04-30
                // when the colon was removed.
                let pad_x = self.char_advance * 0.5;
                let bg_left = origin_x - pad_x;
                let bg_right = origin_x + self.char_advance * digits as f32 + pad_x;
                draw_list
                    .add_rect(
                        [bg_left, y],
                        [bg_right, y + self.line_height],
                        col32([c[0], c[1], c[2], c[3] * 0.65 * fade]),
                    )
                    .filled(true)
                    .rounding(2.0)
                    .build();
            }

            // 16-digit path covers x86_64 / kernel-space dumps; 8-digit
            // is the compact default for files / 32-bit memory.
            // No trailing `:` — the column divider already separates
            // the address from the hex content (user request,
            // 2026-04-30). The single trailing space keeps a 1-ca
            // gap before the divider so the address text never
            // grazes the line.
            //
            // Host can override the displayed address via the
            // `address_formatter` closure (see `set_address_formatter`)
            // — typically used to show "module+offset" strings for
            // mapped memory. The widget itself stays domain-agnostic.
            // When the host returns `Some(s)` we pay one allocation
            // per row for the host's string; the default `None` path
            // routes through the thread-local zero-alloc buffer below.
            let host_str = self.address_formatter.as_ref().and_then(|f| f(addr));
            if let Some(s) = host_str {
                draw_list.add_text([origin_x, y], col32(cfg.color_offset), &s);
            } else {
                // Per-row formatting reuses a thread-local scratch
                // `String` so the address gutter pays zero allocations
                // per frame (the previous `format!("{:016X} ", addr)`
                // built a fresh `String` ~50 rows × 60 fps = ~3000
                // alloc/sec). `draw_row` runs on `&self`, so the
                // mutable scratch buffer can't live on the struct —
                // thread-local is the right scope: each render thread
                // gets its own reused buffer.
                use std::cell::RefCell;
                use std::fmt::Write as _;
                thread_local! {
                    static ADDR_BUF: RefCell<String> = RefCell::new(String::with_capacity(18));
                }
                ADDR_BUF.with(|cell| {
                    let mut buf = cell.borrow_mut();
                    buf.clear();
                    let _ = match (cfg.uppercase, digits) {
                        (true, 16) => write!(&mut *buf, "{addr:016X} "),
                        (false, 16) => write!(&mut *buf, "{addr:016x} "),
                        (true, _) => write!(&mut *buf, "{addr:08X} "),
                        (false, _) => write!(&mut *buf, "{addr:08x} "),
                    };
                    draw_list.add_text([origin_x, y], col32(cfg.color_offset), buf.as_str());
                });
            }
        }

        let hex_x = origin_x + self.offset_col_width();

        // ── Hex bytes ─────────────────────────────────────────
        //
        // Pre-compute x position of each byte column. Mirrors the
        // grouping-aware advance so visual layout is pixel-identical
        // to the column header.
        let mut x_at = [0.0f32; 64]; // bpr ≤ 64 per `BytesPerRow`.
        {
            let mut xv = hex_x;
            for (col_idx, slot) in x_at.iter_mut().take(bpr).enumerate() {
                *slot = xv;
                xv += self.char_advance * 3.0;
                if group > 0 && (col_idx + 1).is_multiple_of(group) && col_idx + 1 < bpr {
                    xv += self.char_advance;
                }
            }
        }

        // PASS 1 — per-byte backgrounds + locate editing byte (if any).
        let mut editing_col: Option<usize> = None;
        for (col_idx, &xb) in x_at.iter().take(bpr).enumerate() {
            let i = offset + col_idx;
            if i >= row_end {
                break;
            }
            let is_cursor = i == self.cursor;
            let is_selected = self.selection.contains(i);
            let is_editing = is_cursor && self.edit_column == Some(EditColumn::Hex);
            if is_editing {
                editing_col = Some(col_idx);
            }

            // Background: cursor > selection > search.
            let bg = if is_editing {
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
                        [xb - 1.0, y],
                        [xb + self.char_advance * 2.0 + 1.0, y + self.line_height],
                        bg_col,
                    )
                    .filled(true)
                    .build();
            }
        }

        // PASS 2 — per-byte hex text rendering.
        //
        // Each byte is emitted as its own `add_text` call at the
        // exact x-coordinate pre-computed in `x_at`. This is the
        // hex-dump invariant the user expects: bytes anchored to a
        // pixel-perfect grid, independent of font metrics or font
        // changes. (An earlier per-colour-run concatenation produced
        // sub-pixel column drift on wide rows; reverting to per-byte
        // keeps data columns in lock-step with the header.)
        let last_col = row_bytes.len(); // exclusive; ≤ bpr
        for col_idx in 0..last_col {
            let i = offset + col_idx;
            let byte = row_bytes[col_idx];
            let xb = x_at[col_idx];

            if Some(col_idx) == editing_col {
                if let Some(hi_nibble) = self.edit_nibble {
                    let mut buf = [b'_'; 2];
                    buf[0] = if cfg.uppercase {
                        b"0123456789ABCDEF"[hi_nibble as usize & 0xF]
                    } else {
                        b"0123456789abcdef"[hi_nibble as usize & 0xF]
                    };
                    // SAFETY: `buf` is two ASCII bytes from the
                    // hex alphabet (or `_`), valid UTF-8 by
                    // construction.
                    let txt = unsafe { std::str::from_utf8_unchecked(&buf) };
                    draw_list.add_text([xb, y], col32([1.0, 1.0, 0.5, 1.0]), txt);
                } else {
                    let txt = byte_hex(byte, cfg.uppercase);
                    draw_list.add_text([xb, y], col32([1.0, 1.0, 0.5, 1.0]), txt);
                    draw_list
                        .add_line(
                            [xb, y + self.line_height - 1.0],
                            [xb + self.char_advance * 2.0, y + self.line_height - 1.0],
                            col32([1.0, 0.8, 0.3, 1.0]),
                        )
                        .thickness(1.5)
                        .build();
                }
                continue;
            }

            let fg = self.byte_fg_with_overrides(i, byte);
            draw_list.add_text([xb, y], fg, byte_hex(byte, cfg.uppercase));
        }

        // PASS 3 — per-byte hover hit-test + tooltip. Kept per-byte
        // because tooltip is rare (≤ 1 hit per frame).
        for col_idx in 0..bpr {
            let i = offset + col_idx;
            if i >= row_end {
                break;
            }
            let xb = x_at[col_idx];
            let is_editing = Some(col_idx) == editing_col;
            let byte_hovered = mouse_pos[0] >= xb
                && mouse_pos[0] < xb + self.char_advance * 2.5
                && mouse_pos[1] >= y
                && mouse_pos[1] < y + self.line_height;
            if byte_hovered && !is_editing {
                let byte = row_bytes[col_idx];
                draw_list
                    .add_rect(
                        [xb - 1.0, y],
                        [xb + self.char_advance * 2.0 + 1.0, y + self.line_height],
                        col32([0.4, 0.63, 0.88, 0.18]),
                    )
                    .filled(true)
                    .build();
                crate::utils::themed_tooltip(ui, || {
                    let addr = cfg.base_address + i as u64;
                    let digits = cfg.address_width.hex_digits(cfg.base_address, data_len);
                    let addr_str = if digits == 16 {
                        format!("Address: 0x{addr:016X} ({i})")
                    } else {
                        format!("Address: 0x{addr:08X} ({i})")
                    };
                    ui.text(addr_str);
                    ui.text(format!("Hex: 0x{byte:02X}  Dec: {byte}  Oct: 0o{byte:03o}"));
                    ui.text(format!("Bin: {byte:08b}"));
                    ui.text(format!("Category: {:?}", ByteCategory::of(byte)));
                    if byte.is_ascii_graphic() || byte == b' ' {
                        ui.text(format!("Char: '{}'", byte as char));
                    }
                });
            }
        }

        // ── ASCII column ──────────────────────────────────────
        //
        // Per-colour-run text spans: ASCII has only two normal
        // categories (printable vs. dot for non-printable), so most
        // rows fold into 1–2 `add_text` calls.
        if cfg.show_ascii {
            let win_x = origin_x - self.char_advance;
            let ascii_x = self.ascii_col_x(win_x);
            let printable_col = col32(cfg.color_ascii);
            let dot_col = col32(cfg.color_ascii_dot);
            let editing_col_rgb = col32([1.0, 1.0, 0.5, 1.0]);

            // PASS 1 — per-byte backgrounds + identify editing byte.
            let mut ascii_editing_col: Option<usize> = None;
            for col_idx in 0..row_bytes.len() {
                let i = offset + col_idx;
                let is_cursor = i == self.cursor;
                let is_selected = self.selection.contains(i);
                let is_ascii_editing = is_cursor && self.edit_column == Some(EditColumn::Ascii);
                let ax = ascii_x + col_idx as f32 * self.char_advance;

                if is_ascii_editing {
                    ascii_editing_col = Some(col_idx);
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
            }

            // PASS 2 — per-colour-run text spans.
            use std::cell::RefCell;
            thread_local! {
                static ASCII_SPAN_BUF: RefCell<String> =
                    RefCell::new(String::with_capacity(96));
            }
            ASCII_SPAN_BUF.with(|cell| {
                let mut span_buf = cell.borrow_mut();
                span_buf.clear();
                let mut span_x = ascii_x;
                let mut span_color: u32 = 0;
                let mut span_open = false;
                for (col_idx, &byte) in row_bytes.iter().enumerate() {
                    let ax = ascii_x + col_idx as f32 * self.char_advance;
                    // Editing byte — flush, then draw + underline
                    // individually so the blink underline lines up.
                    if Some(col_idx) == ascii_editing_col {
                        if span_open && !span_buf.is_empty() {
                            draw_list.add_text([span_x, y], span_color, span_buf.as_str());
                        }
                        span_open = false;
                        span_buf.clear();

                        let ch = if (0x20..0x7F).contains(&byte) {
                            byte as char
                        } else {
                            '.'
                        };
                        let mut buf = [0u8; 4];
                        let s = ch.encode_utf8(&mut buf);
                        draw_list.add_text([ax, y], editing_col_rgb, s);
                        draw_list
                            .add_line(
                                [ax, y + self.line_height - 1.0],
                                [ax + self.char_advance, y + self.line_height - 1.0],
                                col32([1.0, 0.8, 0.3, 1.0]),
                            )
                            .thickness(1.5)
                            .build();
                        continue;
                    }

                    let (ch, fg) = if (0x20..0x7F).contains(&byte) {
                        (byte as char, printable_col)
                    } else {
                        ('.', dot_col)
                    };

                    if !span_open || fg != span_color {
                        if span_open && !span_buf.is_empty() {
                            draw_list.add_text([span_x, y], span_color, span_buf.as_str());
                        }
                        span_buf.clear();
                        span_x = ax;
                        span_color = fg;
                        span_open = true;
                    }
                    span_buf.push(ch);
                }
                if span_open && !span_buf.is_empty() {
                    draw_list.add_text([span_x, y], span_color, span_buf.as_str());
                }
            });
        }
    }
}
