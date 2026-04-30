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

/// Pixel thickness of the draggable splitter strip between the hex
/// child-window and the inspector. The visual line is 1 px; the rest
/// is invisible hit-area so the user has a comfortable grab zone.
pub(super) const SPLITTER_THICKNESS: f32 = 5.0;

// ── HexViewer impl: render entry + virtualisation ────────────────────────────

impl HexViewer {
    pub fn render(&mut self, ui: &dear_imgui_rs::Ui) {
        if self.data.is_empty() {
            return;
        }

        if self.config.auto_refresh_frames > 0 {
            self.frame_count = self.frame_count.wrapping_add(1);
        }

        // Tick down the address-gutter "just copied" flash. Counter
        // hits zero → state cleared so the rect-pill stops painting.
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
        self.line_height = (ch + 2.0).max(1.0);

        let bpr = self.config.bytes_per_row.value();
        let total_rows = self.data.len().div_ceil(bpr);

        self.render_goto_popup(ui);
        self.render_search_popup(ui);
        self.render_context_menu(ui);
        self.render_settings_popup(ui);

        let avail = ui.content_region_avail();
        // Inspector height — auto by default, overridden by user when
        // they drag the splitter. The clamp is gated on `show_inspector`
        // so toggling the panel off/on later doesn't try to fit a
        // user-sized value into a zero-height envelope (`f32::clamp`
        // panics in debug when `min > max`).
        let auto_h = self.inspector_height();
        let splitter_h = if self.config.show_inspector && self.config.show_splitter {
            SPLITTER_THICKNESS
        } else {
            0.0
        };
        let (inspector_h, min_inspector_h, max_inspector_h) = if self.config.show_inspector {
            let min = auto_h.max(self.line_height * 2.0);
            let max = (avail[1] - splitter_h - self.line_height * 5.0).max(min);
            let h = if self.inspector_h > 0.0 {
                self.inspector_h.clamp(min, max)
            } else {
                auto_h
            };
            // Persist the clamp result so the next frame starts from
            // the already-bounded value (avoids creep when the parent
            // shrinks).
            if self.inspector_h > 0.0 {
                self.inspector_h = h;
            }
            (h, min, max)
        } else {
            (0.0, 0.0, 0.0)
        };
        let child_h = avail[1] - inspector_h - splitter_h;

        // `child_id` pre-built in `HexViewer::new` — clone-once
        // here unblocks the closure's `&mut self` borrow while
        // still saving the formatter overhead that the previous
        // `format!("##hv_child_{}", self.id)` per-frame call paid.
        // The clone is a small byte-copy; `format!` was alloc +
        // Display trait dispatch + write_str dance.
        let child_id = self.child_id.clone();
        ui.child_window(&child_id)
            .size([avail[0], child_h])
            .build(ui, || {
                self.focused = ui.is_window_focused();

                // Cache inner content width — used by `ascii_col_x` to
                // right-anchor the ASCII column. `content_region_avail`
                // here returns the post-scrollbar inner width because
                // the cursor sits at the top-left of the body before
                // any widgets have advanced it.
                self.inner_content_w = ui.content_region_avail()[0];

                // Cache the screen-space centre of the child window —
                // used by the modal popups (Goto / Search / Settings)
                // to anchor at the viewer's visual middle regardless
                // of where the trigger came from. `window_pos` returns
                // the absolute screen position of *this* child, and
                // `window_size` is its outer rect, so `pos + size*0.5`
                // is the centre point we want the popup to land at
                // with a `(0.5, 0.5)` pivot.
                let wp = ui.window_pos();
                let ws = ui.window_size();
                self.component_center = [wp[0] + ws[0] * 0.5, wp[1] + ws[1] * 0.5];

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

                // Origin nudged right by one glyph so the leftmost
                // column doesn't sit flush against the child-window
                // border. Mirrors the right-side scrollbar gap and is
                // applied uniformly to header, rows, and hit-testing
                // (see `mouse_to_offset`).
                let origin_x = win_x + self.char_advance;

                // Column header — draw at fixed position relative to window.
                if self.config.show_column_headers && first_row == 0 {
                    let hdr_y = win_y;
                    self.draw_column_header(&draw_list, origin_x, hdr_y);
                }

                // Vertical dividers between offset / hex / ASCII columns.
                // Drawn first so byte text paints on top of them. Same
                // colour treatment as the horizontal header separator
                // for visual consistency.
                if self.config.show_column_dividers {
                    let c = self.config.color_header;
                    let div_col = col32([c[0], c[1], c[2], c[3] * 0.40]);
                    let div_top = win_y;
                    let div_bot = win_y + visible_h;
                    let hex_x_local = origin_x + self.offset_col_width();
                    // Offset side: visible address ends at
                    // `hex_x_local - ca`, hex content starts at
                    // `hex_x_local` — the visible gap is exactly 1 ca
                    // wide (the trailing space of `"{addr} "` lives
                    // inside `offset_col_width`). Centre the divider in
                    // it at `hex_x_local - 0.5 ca`.
                    if self.config.show_offsets {
                        let dx = hex_x_local - self.char_advance * 0.5;
                        draw_list
                            .add_line([dx, div_top], [dx, div_bot], div_col)
                            .thickness(1.0)
                            .build();
                    }
                    // ASCII side: with right-anchored ASCII the gap
                    // between hex and ASCII is no longer fixed (it
                    // grows with window width). Pinning the divider to
                    // hex's right edge (with a small breathing offset)
                    // keeps it acting as a "this column has ended"
                    // marker rather than floating in dead space.
                    if self.config.show_ascii {
                        let dx = hex_x_local + self.hex_col_width() + self.char_advance * 0.5;
                        draw_list
                            .add_line([dx, div_top], [dx, div_bot], div_col)
                            .thickness(1.0)
                            .build();
                    }
                }

                // Rows: position each row at its absolute scroll position.
                // Hover-tooltip is gated on `is_window_hovered` so that
                // moving the cursor down into the inspector subview
                // (or any sibling widget below) immediately suppresses
                // the byte tooltip — without this, the last visible
                // row's hit-rect can still match `mouse_pos.y` and the
                // tooltip ghosts through the inspector.
                let mouse_pos = if ui.is_window_hovered() {
                    ui.io().mouse_pos()
                } else {
                    [f32::NEG_INFINITY, f32::NEG_INFINITY]
                };
                for row in first_row..last_row {
                    // Absolute Y position within the scrollable area.
                    let y = win_y + (row + header_offset) as f32 * self.line_height - scroll_y;
                    let offset = row * bpr;
                    let row_end = (offset + bpr).min(self.data.len());

                    self.draw_row(
                        ui,
                        &draw_list,
                        origin_x,
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
            if self.config.show_splitter {
                self.render_splitter(ui, avail[0], inspector_h, min_inspector_h, max_inspector_h);
            }
            self.render_inspector(ui);
        }
    }

    /// Draggable horizontal splitter between the hex child-window and
    /// the inspector subview. Draws a 1 px line centred in a
    /// `SPLITTER_THICKNESS`-tall hit zone; the cursor turns into
    /// `ResizeNS` on hover, and dragging adjusts `self.inspector_h`
    /// (clamped against `min_h` / `max_h`).
    fn render_splitter(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        width: f32,
        current_h: f32,
        min_h: f32,
        max_h: f32,
    ) {
        let [start_x, start_y] = ui.cursor_screen_pos();
        // `splitter_id` pre-built in `HexViewer::new` — same
        // motivation as `child_id`: zero per-frame allocation.
        ui.invisible_button(&self.splitter_id, [width, SPLITTER_THICKNESS]);
        let hovered = ui.is_item_hovered();
        let active = ui.is_item_active();
        if hovered || active {
            ui.set_mouse_cursor(Some(dear_imgui_rs::MouseCursor::ResizeNS));
        }
        if active {
            // Drag down → shrink the inspector (push hex/splitter down).
            // The clamp uses `current_h - dy` as the baseline rather than
            // `self.inspector_h` so the very first active frame works
            // even when no manual height was stored yet (`inspector_h`
            // is still `0.0` = auto). Mere hover does NOT prime the
            // value — only an actual drag.
            let dy = ui.io().mouse_delta()[1];
            if dy != 0.0 {
                let next = (current_h - dy).clamp(min_h, max_h);
                if next != self.inspector_h {
                    self.inspector_h = next;
                }
            } else if self.inspector_h == 0.0 {
                // Drag started without movement yet — pin the baseline
                // so a sub-pixel jiggle doesn't snap back to auto.
                self.inspector_h = current_h;
            }
        }

        // Visual: 1 px line centred in the hit zone, brighter when the
        // user is actively grabbing it for clear feedback.
        let line_y = start_y + (SPLITTER_THICKNESS * 0.5).floor();
        let alpha = if active {
            0.85
        } else if hovered {
            0.55
        } else {
            0.30
        };
        let c = self.config.color_header;
        let line_col = col32([c[0], c[1], c[2], c[3] * alpha]);
        let draw_list = ui.get_window_draw_list();
        draw_list
            .add_line([start_x, line_y], [start_x + width, line_y], line_col)
            .thickness(1.0)
            .build();
    }
}

// ── HexViewer impl: layout helpers ───────────────────────────────────────────

impl HexViewer {
    pub(super) fn offset_col_width(&self) -> f32 {
        if self.config.show_offsets {
            // address digits + 1 trailing space (no colon — the
            // column divider already separates this gutter from the
            // hex content; the `:` was redundant signal). The 1-ca
            // trailing space stays so the address text doesn't graze
            // the divider line (centred at column-edge − 0.5 ca).
            //
            // Width history:
            //   pre 2026-04-29 → `digits + 3` (over-padded gutter)
            //   2026-04-29     → `digits + 2` (tightened, kept `:`)
            //   2026-04-30     → `digits + 1` (dropped `:`)
            let digits = self
                .config
                .address_width
                .hex_digits(self.config.base_address, self.data.len());
            self.char_advance * (digits + 1) as f32
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

    /// X-coordinate of the ASCII column's left edge, in screen space.
    ///
    /// Right-anchors the ASCII content to the child window's inner
    /// right edge (with a 1-char trailing margin so it doesn't graze
    /// the scrollbar) when there's room — keeps the ASCII column on
    /// the page boundary the way HxD / 010 Editor / Ghidra render
    /// hex dumps. When the window is too narrow, falls back to the
    /// "natural" position one char advance after the hex column so
    /// the layout never overlaps.
    pub(super) fn ascii_col_x(&self, win_x: f32) -> f32 {
        let bpr = self.config.bytes_per_row.value();
        let ascii_w = bpr as f32 * self.char_advance;
        let origin_x = win_x + self.char_advance;

        // "Natural" position — one char advance after the hex column.
        let natural = origin_x + self.offset_col_width() + self.hex_col_width() + self.char_advance;

        // "Anchored" position — right-aligned with one char of
        // breathing room so it doesn't sit under the scrollbar.
        let anchored = win_x + self.inner_content_w - ascii_w - self.char_advance;

        anchored.max(natural)
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

        // Helper: paint `label` centred inside `[col_x, col_x + col_w)`.
        // `label.len()` works as a proxy for visual width here because
        // the hex viewer's font is monospaced (`char_advance == width
        // of any glyph`). Falls back to left-aligned when the label is
        // wider than the column (no negative padding).
        let centred_label = |label: &str, col_x: f32, col_w: f32| {
            let label_w = label.len() as f32 * self.char_advance;
            let pad = ((col_w - label_w) * 0.5).max(0.0);
            draw_list.add_text([col_x + pad, y], hdr_col, label);
        };

        if self.config.show_offsets {
            // Header reads as "Address" (not "Offset") — even when the
            // viewer is bound to a `base_address == 0` file dump, the
            // gutter content is still a *virtual address* in the user's
            // mental model (the offset of the byte from the start of
            // the buffer is also its address). Centred over the gutter
            // — left-aligned looked uneven against the centred ASCII
            // label and the right-edge divider.
            centred_label("Address", origin_x, self.offset_col_width());
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

        // Right-edge of the painted content — used to size the
        // separator line so it tracks header / hex / ASCII columns
        // exactly instead of running off into empty padding. Uses the
        // hex column's right edge as the baseline; gets bumped to the
        // ASCII column's right edge below if the ASCII column is
        // visible.
        let win_x = origin_x - self.char_advance;
        let mut right_edge = hex_x + self.hex_col_width();

        if self.config.show_ascii {
            let ascii_x = self.ascii_col_x(win_x);
            let ascii_w = bpr as f32 * self.char_advance;
            centred_label("ASCII", ascii_x, ascii_w);
            right_edge = ascii_x + ascii_w;
        }

        // ── Separator line under the header row ─────────────────────────
        // Visual cue that the top row is column labels, not data. Uses
        // the header color with a 0.55 alpha multiplier so it reads as
        // a gentle divider rather than competing with the labels above.
        let c = self.config.color_header;
        let sep_col = col32([c[0], c[1], c[2], c[3] * 0.55]);
        let sep_y = y + self.line_height - 1.0;
        draw_list
            .add_line([origin_x, sep_y], [right_edge, sep_y], sep_col)
            .thickness(1.0)
            .build();
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
        self.search_results.get(pos).is_some_and(|&s| s <= offset)
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
            let row = offset / bpr;
            let addr = cfg.base_address + offset as u64;
            let digits = cfg
                .address_width
                .hex_digits(cfg.base_address, self.data.len());

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
                    (true, 16) => write!(&mut *buf, "{:016X} ", addr),
                    (false, 16) => write!(&mut *buf, "{:016x} ", addr),
                    (true, _) => write!(&mut *buf, "{:08X} ", addr),
                    (false, _) => write!(&mut *buf, "{:08x} ", addr),
                };
                draw_list.add_text([origin_x, y], col32(cfg.color_offset), buf.as_str());
            });
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
                    // Crate-wide tooltip styling lives in
                    // `crate::utils::tooltip::themed_tooltip` so every
                    // widget gets identical padding / spacing / rounding
                    // out of the box.
                    crate::utils::themed_tooltip(ui, || {
                        let addr = cfg.base_address + i as u64;
                        let digits = cfg
                            .address_width
                            .hex_digits(cfg.base_address, self.data.len());
                        // Tooltip label kept in sync with the column
                        // header rename ("Offset" → "Address"). The
                        // parenthesised number remains the raw byte
                        // index inside the buffer for callers used to
                        // 0-based offsets.
                        let addr_str = if digits == 16 {
                            format!("Address: 0x{:016X} ({})", addr, i)
                        } else {
                            format!("Address: 0x{:08X} ({})", addr, i)
                        };
                        ui.text(addr_str);
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
            let win_x = origin_x - self.char_advance;
            let ascii_x = self.ascii_col_x(win_x);
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
//
// Three-column grid laid out by hand on the foreground draw list. Each
// column is "label gap value" with shared `label_off` so the value
// columns line up vertically. Mixing `add_text` with the live
// `cursor_screen_pos` keeps the panel exactly four line-heights tall —
// the most compact layout that still shows every numeric
// reinterpretation simultaneously.

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
                format!("\\x{:02X}", b)
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
            ("f32", f32_v.map_or_else(em_dash, |v| format!("{:.6e}", v))),
            ("f64", f64_v.map_or_else(em_dash, |v| format!("{:.6e}", v))),
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
            .hex_digits(self.config.base_address, self.data.len());
        let addr_str = if digits == 16 {
            format!(
                "0x{:016X} ({})",
                self.config.base_address + offset as u64,
                offset
            )
        } else {
            format!(
                "0x{:08X} ({})",
                self.config.base_address + offset as u64,
                offset
            )
        };
        let info = format!(
            "{}   {}-endian   {} bytes{}{}",
            addr_str,
            self.config.endianness.display_name().to_lowercase(),
            self.data.len(),
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
