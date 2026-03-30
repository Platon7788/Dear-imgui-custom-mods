//! # HexViewer
//!
//! Standalone hex dump widget for raw memory / binary inspection.
//!
//! Classic 3-column layout: **offset | hex bytes | ASCII**.
//! Supports color regions (struct overlays), data inspector,
//! goto-address, byte search, selection with copy, and optional editing.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::hex_viewer::HexViewer;
//!
//! let data: Vec<u8> = vec![0x4D, 0x5A, 0x90, 0x00, 0x03];
//! let mut viewer = HexViewer::new("##hex");
//! viewer.set_data(&data);
//! // In render loop: viewer.render(ui);
//! ```

pub mod config;

pub use config::{
    ByteGrouping, BytesPerRow, ColorRegion, Endianness, HexViewerConfig,
};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

/// Convert `[r, g, b, a]` to packed u32 color.
fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── Selection ────────────────────────────────────────────────────────────────

/// Byte range selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
}

impl Selection {
    pub fn is_empty(&self) -> bool { self.start == self.end }
    pub fn contains(&self, offset: usize) -> bool {
        let (lo, hi) = self.ordered();
        offset >= lo && offset < hi
    }
    pub fn ordered(&self) -> (usize, usize) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
    pub fn len(&self) -> usize {
        let (lo, hi) = self.ordered();
        hi - lo
    }
}

// ── HexViewer ────────────────────────────────────────────────────────────────

/// Standalone hex dump widget.
pub struct HexViewer {
    id: String,
    data: Vec<u8>,
    /// Optional reference snapshot for diff highlighting.
    reference: Vec<u8>,
    /// Color regions for struct overlays.
    regions: Vec<ColorRegion>,
    /// Configuration.
    pub config: HexViewerConfig,

    // ── Interaction state ─────────────────────────────────
    /// Currently selected byte (cursor position).
    cursor: usize,
    /// Selection range.
    selection: Selection,
    /// Whether we are in hex edit mode.
    editing: bool,
    /// Partial hex digit during editing (first nibble typed).
    edit_nibble: Option<u8>,
    /// Goto address input buffer.
    goto_buf: String,
    /// Search hex pattern input buffer.
    search_buf: String,
    /// Search results (offsets of matches).
    search_results: Vec<usize>,
    /// Current search result index.
    search_idx: usize,
    /// Show goto popup.
    show_goto: bool,
    /// Show search popup.
    show_search: bool,
    /// Scroll target (row index) — set by goto/search.
    scroll_to_row: Option<usize>,
    /// Cached char advance for the monospace font.
    char_advance: f32,
    /// Cached line height.
    line_height: f32,
    /// Whether the widget is focused.
    focused: bool,
}

impl HexViewer {
    /// Create a new hex viewer with the given ImGui ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            data: Vec::new(),
            reference: Vec::new(),
            regions: Vec::new(),
            config: HexViewerConfig::default(),
            cursor: 0,
            selection: Selection::default(),
            editing: false,
            edit_nibble: None,
            goto_buf: String::new(),
            search_buf: String::new(),
            search_results: Vec::new(),
            search_idx: 0,
            show_goto: false,
            show_search: false,
            scroll_to_row: None,
            char_advance: 0.0,
            line_height: 0.0,
            focused: false,
        }
    }

    // ── Data management ──────────────────────────────────────────────

    /// Set the data buffer to display.
    pub fn set_data(&mut self, data: &[u8]) {
        self.data = data.to_vec();
        self.cursor = self.cursor.min(self.data.len().saturating_sub(1));
        self.selection = Selection::default();
        self.editing = false;
        self.edit_nibble = None;
    }

    /// Set data from a `Vec<u8>` (zero-copy).
    pub fn set_data_vec(&mut self, data: Vec<u8>) {
        self.data = data;
        self.cursor = self.cursor.min(self.data.len().saturating_sub(1));
        self.selection = Selection::default();
    }

    /// Get a reference to the current data.
    pub fn data(&self) -> &[u8] { &self.data }

    /// Get mutable access to the data (for external edits).
    pub fn data_mut(&mut self) -> &mut Vec<u8> { &mut self.data }

    /// Set a reference snapshot for diff highlighting.
    pub fn set_reference(&mut self, reference: &[u8]) {
        self.reference = reference.to_vec();
    }

    /// Clear the reference snapshot.
    pub fn clear_reference(&mut self) {
        self.reference.clear();
    }

    /// Set color regions for struct overlays.
    pub fn set_regions(&mut self, regions: Vec<ColorRegion>) {
        self.regions = regions;
    }

    /// Add a single color region.
    pub fn add_region(&mut self, region: ColorRegion) {
        self.regions.push(region);
    }

    /// Clear all color regions.
    pub fn clear_regions(&mut self) {
        self.regions.clear();
    }

    /// Current cursor position (byte offset).
    pub fn cursor(&self) -> usize { self.cursor }

    /// Set cursor position.
    pub fn set_cursor(&mut self, offset: usize) {
        self.cursor = offset.min(self.data.len().saturating_sub(1));
        self.scroll_to_row = Some(self.cursor / self.config.bytes_per_row.value());
    }

    /// Current selection.
    pub fn selection(&self) -> Selection { self.selection }

    /// Selected bytes as a slice.
    pub fn selected_bytes(&self) -> &[u8] {
        if self.selection.is_empty() { return &[]; }
        let (lo, hi) = self.selection.ordered();
        let lo = lo.min(self.data.len());
        let hi = hi.min(self.data.len());
        &self.data[lo..hi]
    }

    /// Goto an address (scrolls and sets cursor).
    pub fn goto(&mut self, offset: usize) {
        self.set_cursor(offset);
    }

    /// Whether the viewer is focused.
    pub fn is_focused(&self) -> bool { self.focused }

    /// Get immutable config.
    pub fn config(&self) -> &HexViewerConfig { &self.config }

    /// Get mutable config.
    pub fn config_mut(&mut self) -> &mut HexViewerConfig { &mut self.config }

    // ── Rendering ────────────────────────────────────────────────────

    /// Render the hex viewer widget.
    pub fn render(&mut self, ui: &dear_imgui_rs::Ui) {
        if self.data.is_empty() { return; }

        // Cache font metrics.
        let [cw, ch] = calc_text_size("0");
        self.char_advance = cw;
        self.line_height = ch + 2.0;

        let bpr = self.config.bytes_per_row.value();
        let total_rows = self.data.len().div_ceil(bpr);

        // ── Goto / Search popups ──────────────────────────────────
        self.render_goto_popup(ui);
        self.render_search_popup(ui);

        let avail = ui.content_region_avail();
        let inspector_h = if self.config.show_inspector { self.line_height * 5.0 } else { 0.0 };
        let child_h = avail[1] - inspector_h;

        let child_id = format!("##hv_child_{}", self.id);
        ui.child_window(&child_id)
            .size([avail[0], child_h])
            .flags(
                dear_imgui_rs::WindowFlags::NO_MOVE
                | dear_imgui_rs::WindowFlags::NO_SCROLL_WITH_MOUSE,
            )
            .build(ui, || {
                self.focused = ui.is_window_focused();

                // Handle scroll-to target.
                if let Some(row) = self.scroll_to_row.take() {
                    let y = row as f32 * self.line_height;
                    ui.set_scroll_y(y);
                }

                // Handle keyboard input.
                if self.focused {
                    self.handle_keyboard(ui);
                }

                // Handle mouse.
                self.handle_mouse(ui);

                let draw_list = ui.get_window_draw_list();
                let [win_x, win_y] = ui.cursor_screen_pos();
                let scroll_y = ui.scroll_y();
                let visible_h = ui.window_size()[1];

                let first_row = (scroll_y / self.line_height) as usize;
                let visible_count = (visible_h / self.line_height) as usize + 2;
                let last_row = (first_row + visible_count).min(total_rows);

                let origin_x = win_x + ui.scroll_x();
                let origin_y = win_y + scroll_y;

                // ── Column header ─────────────────────────────────
                if self.config.show_column_headers && first_row == 0 {
                    self.draw_column_header(&draw_list, origin_x, origin_y);
                }

                let header_offset = if self.config.show_column_headers { 1 } else { 0 };

                // ── Rows ──────────────────────────────────────────
                let mouse_pos = ui.io().mouse_pos();
                let win_pos = [origin_x, origin_y];
                let win_w = avail[0];

                for row in first_row..last_row {
                    let y = origin_y + (row + header_offset) as f32 * self.line_height;
                    let offset = row * bpr;
                    let row_end = (offset + bpr).min(self.data.len());

                    self.draw_row(
                        ui, &draw_list, origin_x, y, offset, row_end, bpr,
                        mouse_pos, win_pos, win_w,
                    );
                }

                // Dummy for scroll extent.
                let total_h = (total_rows + header_offset) as f32 * self.line_height
                    + self.line_height;
                ui.set_cursor_pos([0.0, total_h]);
                ui.dummy([avail[0], 1.0]);
            });

        // ── Data inspector ────────────────────────────────────────
        if self.config.show_inspector {
            self.render_inspector(ui);
        }
    }

    // ── Drawing helpers ──────────────────────────────────────────────

    fn offset_col_width(&self) -> f32 {
        // 8 hex digits + ": " = 10 chars
        if self.config.show_offsets { self.char_advance * 10.0 } else { 0.0 }
    }

    fn hex_col_width(&self) -> f32 {
        let bpr = self.config.bytes_per_row.value();
        let group = self.config.grouping.value();
        // Each byte = "XX " (3 chars), extra space per group boundary.
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
            let txt = if self.config.uppercase {
                format!("{:02X}", i)
            } else {
                format!("{:02x}", i)
            };
            draw_list.add_text([x, y], hdr_col, &txt);
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
        win_pos: [f32; 2],
        win_w: f32,
    ) {
        let cfg = &self.config;
        let group = cfg.grouping.value();

        // ── Row hover highlight ───────────────────────────────
        let row_hovered = mouse_pos[1] >= y && mouse_pos[1] < y + self.line_height
            && mouse_pos[0] >= win_pos[0] && mouse_pos[0] < win_pos[0] + win_w;
        if row_hovered {
            draw_list.add_rect(
                [win_pos[0], y],
                [win_pos[0] + win_w, y + self.line_height],
                col32([1.0, 1.0, 1.0, 0.03]),
            ).filled(true).build();
        }

        // ── Offset column ─────────────────────────────────────
        if cfg.show_offsets {
            let addr = cfg.base_address + offset as u64;
            let txt = if cfg.uppercase {
                format!("{:08X}: ", addr)
            } else {
                format!("{:08x}: ", addr)
            };
            draw_list.add_text(
                [origin_x, y],
                col32(cfg.color_offset),
                &txt,
            );
        }

        let hex_x = origin_x + self.offset_col_width();
        let mut x = hex_x;

        // ── Hex bytes ─────────────────────────────────────────
        for i in offset..offset + bpr {
            if i < row_end {
                let byte = self.data[i];
                let (fg, bg) = self.byte_colors(i, byte);

                // Background highlight.
                if let Some(bg_col) = bg {
                    draw_list.add_rect(
                        [x - 1.0, y],
                        [x + self.char_advance * 2.0 + 1.0, y + self.line_height],
                        bg_col,
                    ).filled(true).build();
                }

                // ── Byte hover highlight + tooltip ────────────
                let byte_hovered = mouse_pos[0] >= x
                    && mouse_pos[0] < x + self.char_advance * 2.5
                    && mouse_pos[1] >= y
                    && mouse_pos[1] < y + self.line_height;
                if byte_hovered {
                    draw_list.add_rect(
                        [x - 1.0, y],
                        [x + self.char_advance * 2.0 + 1.0, y + self.line_height],
                        col32([0.4, 0.63, 0.88, 0.25]),
                    ).filled(true).build();
                    let byte_val = self.data[i];
                    ui.tooltip(|| {
                        ui.text(format!(
                            "Offset: 0x{:08X} ({})", i, i
                        ));
                        ui.text(format!(
                            "Hex: 0x{:02X}  Dec: {}  Oct: 0o{:03o}",
                            byte_val, byte_val, byte_val
                        ));
                        ui.text(format!("Bin: {:08b}", byte_val));
                        if byte_val.is_ascii_graphic() || byte_val == b' ' {
                            ui.text(format!(
                                "Char: '{}'", byte_val as char
                            ));
                        }
                    });
                }

                let txt = if cfg.uppercase {
                    format!("{:02X}", byte)
                } else {
                    format!("{:02x}", byte)
                };
                draw_list.add_text([x, y], fg, &txt);
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
                let ch = if (0x20..0x7F).contains(&byte) {
                    byte as char
                } else {
                    '.'
                };
                let color = if ch == '.' {
                    col32(cfg.color_ascii_dot)
                } else {
                    col32(cfg.color_ascii)
                };

                // Selection/cursor bg in ASCII column too.
                if self.selection.contains(i) || i == self.cursor {
                    let bg = if i == self.cursor {
                        col32(cfg.color_cursor_bg)
                    } else {
                        col32(cfg.color_selection_bg)
                    };
                    draw_list.add_rect(
                        [ax, y],
                        [ax + self.char_advance, y + self.line_height],
                        bg,
                    ).filled(true).build();
                }

                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                draw_list.add_text([ax, y], color, s);
                ax += self.char_advance;
            }
        }
    }

    /// Determine foreground and optional background color for a byte.
    fn byte_colors(&self, offset: usize, byte: u8) -> (u32, Option<u32>) {
        let cfg = &self.config;

        // Cursor.
        if offset == self.cursor {
            let fg = col32(cfg.color_hex);
            return (fg, Some(col32(cfg.color_cursor_bg)));
        }

        // Selection.
        if self.selection.contains(offset) {
            let fg = col32(cfg.color_hex);
            return (fg, Some(col32(cfg.color_selection_bg)));
        }

        // Changed byte (diff).
        if cfg.highlight_changes
            && offset < self.reference.len()
            && self.data[offset] != self.reference[offset]
        {
            return (col32(cfg.color_changed), None);
        }

        // Color region.
        for region in &self.regions {
            if offset >= region.offset && offset < region.offset + region.len {
                return (col32(region.color), None);
            }
        }

        // Zero byte.
        if byte == 0 && cfg.dim_zeros {
            return (col32(cfg.color_zero), None);
        }

        (col32(cfg.color_hex), None)
    }

    // ── Input handling ───────────────────────────────────────────────

    fn handle_keyboard(&mut self, ui: &dear_imgui_rs::Ui) {
        use dear_imgui_rs::Key;

        let bpr = self.config.bytes_per_row.value();
        let len = self.data.len();
        if len == 0 { return; }

        let shift = ui.io().key_shift();
        let ctrl = ui.io().key_ctrl();

        // Navigation.
        if ui.is_key_pressed(Key::LeftArrow) {
            if self.cursor > 0 { self.cursor -= 1; }
            if shift { self.selection.end = self.cursor; }
            else { self.selection = Selection::default(); }
            self.scroll_to_cursor();
        }
        if ui.is_key_pressed(Key::RightArrow) {
            if self.cursor < len - 1 { self.cursor += 1; }
            if shift { self.selection.end = self.cursor; }
            else { self.selection = Selection::default(); }
            self.scroll_to_cursor();
        }
        if ui.is_key_pressed(Key::UpArrow) {
            if self.cursor >= bpr { self.cursor -= bpr; }
            if shift { self.selection.end = self.cursor; }
            else { self.selection = Selection::default(); }
            self.scroll_to_cursor();
        }
        if ui.is_key_pressed(Key::DownArrow) {
            if self.cursor + bpr < len { self.cursor += bpr; }
            if shift { self.selection.end = self.cursor; }
            else { self.selection = Selection::default(); }
            self.scroll_to_cursor();
        }
        if ui.is_key_pressed(Key::PageUp) {
            let rows = (ui.window_size()[1] / self.line_height) as usize;
            self.cursor = self.cursor.saturating_sub(bpr * rows);
            self.scroll_to_cursor();
        }
        if ui.is_key_pressed(Key::PageDown) {
            let rows = (ui.window_size()[1] / self.line_height) as usize;
            self.cursor = (self.cursor + bpr * rows).min(len - 1);
            self.scroll_to_cursor();
        }
        if ui.is_key_pressed(Key::Home) {
            if ctrl { self.cursor = 0; }
            else { self.cursor -= self.cursor % bpr; }
            self.scroll_to_cursor();
        }
        if ui.is_key_pressed(Key::End) {
            if ctrl { self.cursor = len - 1; }
            else { self.cursor = ((self.cursor / bpr + 1) * bpr - 1).min(len - 1); }
            self.scroll_to_cursor();
        }

        // Select all.
        if ctrl && ui.is_key_pressed(Key::A) {
            self.selection = Selection { start: 0, end: len };
        }

        // Copy as hex string.
        if ctrl && ui.is_key_pressed(Key::C) {
            self.copy_selection_hex();
        }

        // Goto (Ctrl+G).
        if ctrl && ui.is_key_pressed(Key::G) {
            self.show_goto = true;
            self.goto_buf.clear();
        }

        // Search (Ctrl+F).
        if ctrl && ui.is_key_pressed(Key::F) {
            self.show_search = true;
            self.search_buf.clear();
        }

        // F3 = next search result.
        if ui.is_key_pressed(Key::F3)
            && !self.search_results.is_empty() {
                if shift {
                    self.search_idx = self.search_idx
                        .checked_sub(1)
                        .unwrap_or(self.search_results.len() - 1);
                } else {
                    self.search_idx = (self.search_idx + 1) % self.search_results.len();
                }
                self.cursor = self.search_results[self.search_idx];
                self.scroll_to_cursor();
            }

        // Hex editing input.
        if self.config.editable && !ctrl {
            self.handle_hex_input(ui);
        }
    }

    fn handle_hex_input(&mut self, _ui: &dear_imgui_rs::Ui) {
        let chars = read_input_chars();
        for ch in chars {
            let nibble = match ch {
                '0'..='9' => ch as u8 - b'0',
                'a'..='f' => ch as u8 - b'a' + 10,
                'A'..='F' => ch as u8 - b'A' + 10,
                _ => continue,
            };
            if let Some(hi) = self.edit_nibble.take() {
                // Second nibble → commit byte.
                let byte = (hi << 4) | nibble;
                if self.cursor < self.data.len() {
                    self.data[self.cursor] = byte;
                    if self.cursor < self.data.len() - 1 {
                        self.cursor += 1;
                    }
                }
            } else {
                // First nibble → store.
                self.edit_nibble = Some(nibble);
            }
        }
    }

    fn handle_mouse(&mut self, ui: &dear_imgui_rs::Ui) {
        if !ui.is_window_hovered() { return; }

        // Mouse wheel scroll.
        let wheel = ui.io().mouse_wheel();
        if wheel != 0.0 {
            let bpr = self.config.bytes_per_row.value();
            let rows = (-wheel * self.config.bytes_per_row.value() as f32 * 0.5) as isize;
            let scroll_y = ui.scroll_y();
            let new_scroll = (scroll_y + rows as f32 * self.line_height).max(0.0);
            ui.set_scroll_y(new_scroll);
            let _ = bpr; // suppress warning
        }

        // Click to set cursor.
        if ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Left)
            && let Some(offset) = self.mouse_to_offset(ui) {
                self.cursor = offset;
                self.selection = Selection { start: offset, end: offset };
                self.editing = true;
                self.edit_nibble = None;
            }

        // Drag to select.
        if ui.is_mouse_dragging(dear_imgui_rs::MouseButton::Left)
            && let Some(offset) = self.mouse_to_offset(ui) {
                self.selection.end = offset + 1;
            }
    }

    fn mouse_to_offset(&self, ui: &dear_imgui_rs::Ui) -> Option<usize> {
        let [mx, my] = ui.io().mouse_pos();
        let [win_x, win_y] = ui.cursor_screen_pos();
        let scroll_y = ui.scroll_y();
        let origin_x = win_x + ui.scroll_x();
        let origin_y = win_y + scroll_y;
        let header_offset = if self.config.show_column_headers { 1 } else { 0 };

        let rel_y = my - origin_y;
        let row = (rel_y / self.line_height) as isize - header_offset as isize;
        if row < 0 { return None; }
        let row = row as usize;

        let hex_x = origin_x + self.offset_col_width();
        let rel_x = mx - hex_x;
        if rel_x < 0.0 { return None; }

        // Approximate column from x position.
        let bpr = self.config.bytes_per_row.value();
        let group = self.config.grouping.value();

        // Each byte takes 3 char widths, plus 1 extra at group boundaries.
        let mut col = 0usize;
        let mut x = 0.0f32;
        while col < bpr {
            let next_x = x + self.char_advance * 3.0;
            if rel_x < next_x { break; }
            x = next_x;
            col += 1;
            if group > 0 && col.is_multiple_of(group) && col < bpr {
                x += self.char_advance;
            }
        }

        let offset = row * bpr + col;
        if offset < self.data.len() { Some(offset) } else { None }
    }

    fn scroll_to_cursor(&mut self) {
        let bpr = self.config.bytes_per_row.value();
        let row = self.cursor / bpr;
        self.scroll_to_row = Some(row);
    }

    fn copy_selection_hex(&self) {
        let bytes = self.selected_bytes();
        if bytes.is_empty() {
            // Copy single byte at cursor.
            if self.cursor < self.data.len() {
                let s = format!("{:02X}", self.data[self.cursor]);
                set_clipboard(&s);
            }
            return;
        }
        let hex: String = bytes.iter()
            .map(|b| format!("{:02X} ", b))
            .collect();
        set_clipboard(hex.trim_end());
    }

    // ── Goto popup ───────────────────────────────────────────────────

    fn render_goto_popup(&mut self, ui: &dear_imgui_rs::Ui) {
        if !self.show_goto { return; }

        let label = format!("##goto_{}", self.id);
        ui.open_popup(&label);
        self.show_goto = false;

        if let Some(_popup) = ui.begin_popup(&label) {
            ui.text("Goto offset (hex or decimal):");
            ui.input_text("##goto_input", &mut self.goto_buf)
                .build();

            if ui.button("Go") || ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                if let Some(addr) = parse_address(&self.goto_buf) {
                    let offset = addr.saturating_sub(self.config.base_address) as usize;
                    self.goto(offset);
                }
                ui.close_current_popup();
            }
            ui.same_line();
            if ui.button("Cancel") || ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                ui.close_current_popup();
            }
        }
    }

    // ── Search popup ─────────────────────────────────────────────────

    fn render_search_popup(&mut self, ui: &dear_imgui_rs::Ui) {
        if !self.show_search { return; }

        let label = format!("##search_{}", self.id);
        ui.open_popup(&label);
        self.show_search = false;

        if let Some(_popup) = ui.begin_popup(&label) {
            ui.text("Hex pattern (e.g. 4D 5A 90):");
            ui.input_text("##search_input", &mut self.search_buf)
                .build();

            if ui.button("Find") || ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                self.do_search();
                ui.close_current_popup();
            }
            ui.same_line();
            if ui.button("Cancel") || ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                ui.close_current_popup();
            }
        }
    }

    fn do_search(&mut self) {
        let pattern = parse_hex_pattern(&self.search_buf);
        if pattern.is_empty() { return; }

        self.search_results.clear();
        self.search_idx = 0;

        let data = &self.data;
        let plen = pattern.len();
        if plen > data.len() { return; }

        for i in 0..=data.len() - plen {
            if data[i..i + plen] == pattern[..] {
                self.search_results.push(i);
            }
        }

        if !self.search_results.is_empty() {
            self.cursor = self.search_results[0];
            self.selection = Selection {
                start: self.cursor,
                end: self.cursor + plen,
            };
            self.scroll_to_cursor();
        }
    }

    // ── Data inspector ───────────────────────────────────────────────

    fn render_inspector(&self, ui: &dear_imgui_rs::Ui) {
        if self.cursor >= self.data.len() { return; }

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
                let v = if le { u16::from_le_bytes([bytes[0], bytes[1]]) }
                        else  { u16::from_be_bytes([bytes[0], bytes[1]]) };
                ("u16", format!("{}", v))
            } else { ("u16", "—".into()) },
            if remaining >= 4 {
                let arr = [bytes[0], bytes[1], bytes[2], bytes[3]];
                let v = if le { u32::from_le_bytes(arr) } else { u32::from_be_bytes(arr) };
                ("u32", format!("{}", v))
            } else { ("u32", "—".into()) },
            if remaining >= 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[..8]);
                let v = if le { u64::from_le_bytes(arr) } else { u64::from_be_bytes(arr) };
                ("u64", format!("{}", v))
            } else { ("u64", "—".into()) },
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
                let v = if le { f32::from_le_bytes(arr) } else { f32::from_be_bytes(arr) };
                ("f32", format!("{:.6e}", v))
            } else { ("f32", "—".into()) },
            if remaining >= 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[..8]);
                let v = if le { f64::from_le_bytes(arr) } else { f64::from_be_bytes(arr) };
                ("f64", format!("{:.6e}", v))
            } else { ("f64", "—".into()) },
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

        // Row 3: offset info + endianness
        let y3 = y2 + lh;
        let info = format!(
            "Offset: 0x{:08X} ({})  Endian: {}  Data: {} bytes",
            self.config.base_address + offset as u64,
            offset,
            self.config.endianness.display_name(),
            self.data.len(),
        );
        draw_list.add_text([x, y3], label_col, &info);

        // Reserve space so ImGui knows about the inspector height.
        ui.dummy([0.0, lh * 4.0]);
    }
}

// ── Free helpers ─────────────────────────────────────────────────────────────

fn read_input_chars() -> Vec<char> {
    // SAFETY: igGetIO returns a valid pointer to the global ImGuiIO struct.
    let io = unsafe { &*dear_imgui_rs::sys::igGetIO_Nil() };
    let data = io.InputQueueCharacters.Data;
    let size = io.InputQueueCharacters.Size;
    if data.is_null() || size <= 0 { return Vec::new(); }
    let slice = unsafe { std::slice::from_raw_parts(data as *const u16, size as usize) };
    slice.iter().filter_map(|&c| char::from_u32(c as u32)).collect()
}

fn set_clipboard(text: &str) {
    let c_str = std::ffi::CString::new(text).unwrap_or_default();
    unsafe { dear_imgui_rs::sys::igSetClipboardText(c_str.as_ptr()); }
}

fn parse_address(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if s.chars().all(|c| c.is_ascii_hexdigit()) && s.len() > 4 {
        // Likely hex without prefix for long strings.
        u64::from_str_radix(s, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

fn parse_hex_pattern(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .filter_map(|tok| u8::from_str_radix(tok, 16).ok())
        .collect()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_viewer() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0x41, 0x42, 0x43, 0x44]);
        assert_eq!(v.data().len(), 4);
        assert_eq!(v.cursor(), 0);
    }

    #[test]
    fn test_set_cursor() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0; 256]);
        v.set_cursor(100);
        assert_eq!(v.cursor(), 100);
        // Clamp to max.
        v.set_cursor(9999);
        assert_eq!(v.cursor(), 255);
    }

    #[test]
    fn test_selection() {
        let sel = Selection { start: 5, end: 10 };
        assert!(!sel.is_empty());
        assert_eq!(sel.len(), 5);
        assert!(sel.contains(5));
        assert!(sel.contains(9));
        assert!(!sel.contains(10));
    }

    #[test]
    fn test_selection_reverse() {
        let sel = Selection { start: 10, end: 5 };
        assert_eq!(sel.ordered(), (5, 10));
        assert_eq!(sel.len(), 5);
        assert!(sel.contains(7));
    }

    #[test]
    fn test_selected_bytes() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0x10, 0x20, 0x30, 0x40, 0x50]);
        v.selection = Selection { start: 1, end: 4 };
        assert_eq!(v.selected_bytes(), &[0x20, 0x30, 0x40]);
    }

    #[test]
    fn test_parse_address_hex() {
        assert_eq!(parse_address("0x100"), Some(0x100));
        assert_eq!(parse_address("0xFF"), Some(0xFF));
        assert_eq!(parse_address("0X1A2B"), Some(0x1A2B));
    }

    #[test]
    fn test_parse_address_decimal() {
        assert_eq!(parse_address("256"), Some(256));
        assert_eq!(parse_address("0"), Some(0));
    }

    #[test]
    fn test_parse_hex_pattern() {
        assert_eq!(parse_hex_pattern("4D 5A 90"), vec![0x4D, 0x5A, 0x90]);
        assert_eq!(parse_hex_pattern("FF"), vec![0xFF]);
        assert_eq!(parse_hex_pattern(""), Vec::<u8>::new());
    }

    #[test]
    fn test_byte_colors_zero_dimmed() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0x00, 0xFF, 0x00, 0xFF]);
        v.config.dim_zeros = true;
        // Move cursor away so it doesn't affect color checks.
        v.cursor = 3;
        let (fg0, _) = v.byte_colors(0, 0x00);
        let (fgff, _) = v.byte_colors(1, 0xFF);
        // Zero should use dim color, non-zero should use normal.
        assert_ne!(fg0, fgff);
    }

    #[test]
    fn test_byte_colors_region() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0; 16]);
        v.regions.push(ColorRegion::new(4, 4, [1.0, 0.0, 0.0, 1.0], "magic"));
        let (fg, _) = v.byte_colors(5, 0);
        // Should be red from region.
        assert_eq!(fg, col32([1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn test_byte_colors_changed() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0xAA, 0xBB, 0xCC]);
        v.set_reference(&[0xAA, 0xCC, 0xCC]);
        v.config.highlight_changes = true;
        // Move cursor away from tested offsets.
        v.cursor = 2;
        let (_, bg0) = v.byte_colors(0, 0xAA);
        let (fg1, _) = v.byte_colors(1, 0xBB);
        assert!(bg0.is_none()); // unchanged
        assert_eq!(fg1, col32(v.config.color_changed)); // changed
    }

    #[test]
    fn test_config_defaults() {
        let cfg = HexViewerConfig::default();
        assert_eq!(cfg.bytes_per_row, BytesPerRow::Sixteen);
        assert!(cfg.show_ascii);
        assert!(cfg.show_inspector);
        assert!(cfg.show_offsets);
        assert!(cfg.uppercase);
        assert!(!cfg.editable);
    }

    #[test]
    fn test_search_pattern() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0x00, 0x4D, 0x5A, 0x00, 0x4D, 0x5A, 0x90]);
        v.search_buf = "4D 5A".to_string();
        v.do_search();
        assert_eq!(v.search_results, vec![1, 4]);
        assert_eq!(v.cursor, 1);
    }

    #[test]
    fn test_goto() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0; 1024]);
        v.goto(512);
        assert_eq!(v.cursor(), 512);
    }

    #[test]
    fn test_endianness() {
        assert_eq!(Endianness::Little.display_name(), "LE");
        assert_eq!(Endianness::Big.display_name(), "BE");
    }

    #[test]
    fn test_bytes_per_row() {
        assert_eq!(BytesPerRow::Eight.value(), 8);
        assert_eq!(BytesPerRow::Sixteen.value(), 16);
        assert_eq!(BytesPerRow::ThirtyTwo.value(), 32);
    }
}
