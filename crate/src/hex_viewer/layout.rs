//! Column-geometry math, header drawing, and per-byte colour resolution.
//!
//! Split out of `draw.rs` (which kept growing past the 500-line ceiling)
//! so the render-entry / virtualisation code and the layout math live in
//! separate files. Every helper here is `pub(super)` so the row drawer
//! (`row.rs`), the hit-test path (`input.rs`), and the render loop
//! (`draw.rs`) can all reach them.

use super::HexViewer;
use super::draw::{SPLITTER_THICKNESS, col32};
use crate::utils::hex::byte_hex;

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
            // Use the cached `effective_data_len` rather than
            // `self.data.len()`. Phase 2: provider-driven renders set
            // this to the provider's clamped `len()` so a streaming
            // host gets a 16-digit gutter when its window spans a
            // 64-bit address range, even though `self.data` may be
            // small or empty. Legacy `set_data*` callers also write
            // the field, so non-provider hosts behave exactly as
            // before.
            let digits = self
                .config
                .address_width
                .hex_digits(self.config.base_address, self.effective_data_len);
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

    pub(super) fn draw_column_header(
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
    ///
    /// `byte` is the live byte value at `offset` (read by the caller
    /// from the active provider / row buffer). Using the caller-supplied
    /// `byte` for the reference comparison — rather than re-indexing
    /// `self.data[offset]` — keeps the diff highlight aligned with what
    /// the row drawer actually painted. For provider-driven hosts the
    /// internal `self.data` may not match the live byte at all (streaming
    /// memory pane), so the parameter is the only correct source.
    pub(super) fn byte_fg_with_overrides(&self, offset: usize, byte: u8) -> u32 {
        let cfg = &self.config;

        // Changed byte (diff).
        if cfg.highlight_changes
            && !self.reference.is_empty()
            && offset < self.reference.len()
            && byte != self.reference[offset]
        {
            return col32(cfg.color_changed);
        }

        // Color region.
        for region in &self.regions {
            if offset >= region.offset && offset < region.offset + region.len {
                return col32(region.color);
            }
        }

        // Category / default — uses pre-computed palette (M2).
        // Equivalent to `col32(cfg.byte_fg_color(byte))` but the lookup
        // tables are built once per frame in `render()`.
        self.byte_palette[byte as usize]
    }

    /// Draggable horizontal splitter between the hex child-window and
    /// the inspector subview. Draws a 1 px line centred in a
    /// `SPLITTER_THICKNESS`-tall hit zone; the cursor turns into
    /// `ResizeNS` on hover, and dragging adjusts `self.inspector_h`
    /// (clamped against `min_h` / `max_h`).
    pub(super) fn render_splitter(
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
