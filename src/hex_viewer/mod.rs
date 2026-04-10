//! # HexViewer
//!
//! Standalone hex dump widget for raw memory / binary inspection.
//!
//! Classic 3-column layout: **offset | hex bytes | ASCII**.
//! Supports color regions (struct overlays), data inspector,
//! goto-address, wildcard byte search, selection with copy,
//! optional editing with undo/redo, navigation history,
//! semantic byte-category coloring, and auto-refresh for live data.

pub mod config;

pub use config::{
    ByteCategory, ByteGrouping, BytesPerRow, ColorRegion, CopyFormat, Endianness,
    HexDataProvider, HexSearchMode, HexViewerConfig, NavHistory, UndoEntry, UndoStack,
    VecDataProvider,
};

use crate::utils::clipboard::{
    self, set_clipboard, vk_down, VK_A, VK_C, VK_F, VK_G, VK_Y, VK_Z,
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

// ── Wildcard Search ─────────────────────────────────────────────────────────

/// A single byte in a search pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternByte {
    Exact(u8),
    Any,
}

fn parse_hex_pattern_masked(s: &str) -> Vec<PatternByte> {
    s.split_whitespace()
        .filter_map(|tok| {
            if tok == "??" || tok == "?" {
                Some(PatternByte::Any)
            } else {
                u8::from_str_radix(tok, 16).ok().map(PatternByte::Exact)
            }
        })
        .collect()
}

fn parse_ascii_pattern(s: &str) -> Vec<PatternByte> {
    s.bytes().map(|b| PatternByte::Exact(b)).collect()
}

fn find_pattern_masked(data: &[u8], pattern: &[PatternByte]) -> Vec<usize> {
    if pattern.is_empty() || pattern.len() > data.len() {
        return Vec::new();
    }
    let mut results = Vec::new();
    'outer: for i in 0..=data.len() - pattern.len() {
        for (j, pb) in pattern.iter().enumerate() {
            match pb {
                PatternByte::Any => {}
                PatternByte::Exact(b) => {
                    if data[i + j] != *b { continue 'outer; }
                }
            }
        }
        results.push(i);
    }
    results
}

fn format_bytes(bytes: &[u8], format: CopyFormat, uppercase: bool) -> String {
    match format {
        CopyFormat::HexSpaced => bytes.iter()
            .map(|b| if uppercase { format!("{:02X}", b) } else { format!("{:02x}", b) })
            .collect::<Vec<_>>().join(" "),
        CopyFormat::HexCompact => bytes.iter()
            .map(|b| if uppercase { format!("{:02X}", b) } else { format!("{:02x}", b) })
            .collect::<String>(),
        CopyFormat::CArray => {
            let inner: String = bytes.iter()
                .map(|b| if uppercase { format!("0x{:02X}", b) } else { format!("0x{:02x}", b) })
                .collect::<Vec<_>>().join(", ");
            format!("{{ {} }}", inner)
        }
        CopyFormat::RustArray => {
            let inner: String = bytes.iter()
                .map(|b| if uppercase { format!("0x{:02X}", b) } else { format!("0x{:02x}", b) })
                .collect::<Vec<_>>().join(", ");
            format!("[{}]", inner)
        }
        CopyFormat::Base64 => base64_encode(bytes),
        CopyFormat::Ascii => bytes.iter()
            .map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '.' })
            .collect(),
    }
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        } else { out.push('='); }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3F) as usize] as char);
        } else { out.push('='); }
    }
    out
}

// ── Edit Mode ───────────────────────────────────────────────────────────────

/// Which column is being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditColumn {
    /// Editing in the hex column (nibble input).
    Hex,
    /// Editing in the ASCII column (character input).
    Ascii,
}

// ── HexViewer ────────────────────────────────────────────────────────────────

/// Standalone hex dump widget.
pub struct HexViewer {
    id: String,
    data: Vec<u8>,
    reference: Vec<u8>,
    regions: Vec<ColorRegion>,
    pub config: HexViewerConfig,

    nav: NavHistory,
    undo: UndoStack,

    // ── Interaction state ────────────────────────────────────
    cursor: usize,
    selection: Selection,
    /// Which column is being edited (None = not editing).
    edit_column: Option<EditColumn>,
    /// Partial hex digit during hex editing (first nibble typed).
    edit_nibble: Option<u8>,
    /// Whether keyboard layout was switched for editing.
    layout_switched: bool,
    goto_buf: String,
    search_buf: String,
    search_pattern: Vec<PatternByte>,
    search_results: Vec<usize>,
    search_idx: usize,
    show_goto: bool,
    show_search: bool,
    scroll_to_row: Option<usize>,
    char_advance: f32,
    line_height: f32,
    focused: bool,
    frame_count: u32,
}

impl HexViewer {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            data: Vec::new(),
            reference: Vec::new(),
            regions: Vec::new(),
            config: HexViewerConfig::default(),
            nav: NavHistory::new(64),
            undo: UndoStack::default(),
            cursor: 0,
            selection: Selection::default(),
            edit_column: None,
            edit_nibble: None,
            layout_switched: false,
            goto_buf: String::new(),
            search_buf: String::new(),
            search_pattern: Vec::new(),
            search_results: Vec::new(),
            search_idx: 0,
            show_goto: false,
            show_search: false,
            scroll_to_row: None,
            char_advance: 0.0,
            line_height: 0.0,
            focused: false,
            frame_count: 0,
        }
    }

    // ── Data management ─────────────────────────────────────────────

    pub fn set_data(&mut self, data: &[u8]) {
        self.data = data.to_vec();
        self.clamp_cursor();
        self.selection = Selection::default();
        self.stop_editing();
    }

    pub fn set_data_vec(&mut self, data: Vec<u8>) {
        self.data = data;
        self.clamp_cursor();
        self.selection = Selection::default();
    }

    pub fn data(&self) -> &[u8] { &self.data }
    pub fn data_mut(&mut self) -> &mut Vec<u8> { &mut self.data }
    pub fn data_len(&self) -> usize { self.data.len() }

    pub fn set_reference(&mut self, reference: &[u8]) {
        self.reference = reference.to_vec();
    }
    pub fn clear_reference(&mut self) { self.reference.clear(); }

    pub fn set_regions(&mut self, regions: Vec<ColorRegion>) { self.regions = regions; }
    pub fn add_region(&mut self, region: ColorRegion) { self.regions.push(region); }
    pub fn clear_regions(&mut self) { self.regions.clear(); }

    // ── Cursor & Selection ───────────────────────────────────────────

    pub fn cursor(&self) -> usize { self.cursor }

    pub fn set_cursor(&mut self, offset: usize) {
        let old = self.cursor;
        self.cursor = offset.min(self.data.len().saturating_sub(1));
        let bpr = self.config.bytes_per_row.value();
        if old.abs_diff(self.cursor) > bpr {
            self.nav.push(self.config.base_address + old as u64);
        }
        self.scroll_to_row = Some(self.cursor / bpr);
    }

    pub fn selection(&self) -> Selection { self.selection }

    pub fn selected_bytes(&self) -> &[u8] {
        if self.selection.is_empty() { return &[]; }
        let (lo, hi) = self.selection.ordered();
        let lo = lo.min(self.data.len());
        let hi = hi.min(self.data.len());
        &self.data[lo..hi]
    }

    pub fn goto(&mut self, offset: usize) { self.set_cursor(offset); }

    pub fn nav_back(&mut self) {
        let current = self.config.base_address + self.cursor as u64;
        if let Some(addr) = self.nav.go_back(current) {
            let offset = addr.saturating_sub(self.config.base_address) as usize;
            self.cursor = offset.min(self.data.len().saturating_sub(1));
            self.scroll_to_cursor();
        }
    }

    pub fn nav_forward(&mut self) {
        let current = self.config.base_address + self.cursor as u64;
        if let Some(addr) = self.nav.go_forward(current) {
            let offset = addr.saturating_sub(self.config.base_address) as usize;
            self.cursor = offset.min(self.data.len().saturating_sub(1));
            self.scroll_to_cursor();
        }
    }

    pub fn is_focused(&self) -> bool { self.focused }
    pub fn config(&self) -> &HexViewerConfig { &self.config }
    pub fn config_mut(&mut self) -> &mut HexViewerConfig { &mut self.config }
    pub fn undo_stack(&self) -> &UndoStack { &self.undo }
    pub fn nav_history(&self) -> &NavHistory { &self.nav }

    // ── Undo / Redo ──────────────────────────────────────────────────

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo.undo() {
            let off = entry.offset as usize;
            let old = entry.old_bytes.clone();
            if off + old.len() <= self.data.len() {
                self.data[off..off + old.len()].copy_from_slice(&old);
                self.cursor = off;
                self.scroll_to_cursor();
            }
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.undo.redo() {
            let off = entry.offset as usize;
            let new = entry.new_bytes.clone();
            if off + new.len() <= self.data.len() {
                self.data[off..off + new.len()].copy_from_slice(&new);
                self.cursor = off;
                self.scroll_to_cursor();
            }
        }
    }

    // ── Edit helpers ─────────────────────────────────────────────────

    fn start_editing(&mut self, column: EditColumn) {
        self.edit_column = Some(column);
        self.edit_nibble = None;
        // Switch to English layout for hex input.
        if column == EditColumn::Hex && !self.layout_switched {
            clipboard::activate_english_layout();
            self.layout_switched = true;
        }
    }

    fn stop_editing(&mut self) {
        self.edit_column = None;
        self.edit_nibble = None;
        if self.layout_switched {
            clipboard::restore_keyboard_layout();
            self.layout_switched = false;
        }
    }

    // ── Rendering ────────────────────────────────────────────────────

    pub fn render(&mut self, ui: &dear_imgui_rs::Ui) {
        if self.data.is_empty() { return; }

        if self.config.auto_refresh_frames > 0 {
            self.frame_count += 1;
        }

        let [cw, ch] = calc_text_size("0");
        self.char_advance = cw;
        self.line_height = ch + 2.0;

        let bpr = self.config.bytes_per_row.value();
        let total_rows = self.data.len().div_ceil(bpr);

        self.render_goto_popup(ui);
        self.render_search_popup(ui);

        let avail = ui.content_region_avail();
        let inspector_h = if self.config.show_inspector { self.line_height * 5.0 } else { 0.0 };
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

                let header_offset = if self.config.show_column_headers { 1 } else { 0 };

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
                        ui, &draw_list, win_x, y, offset, row_end, bpr,
                        mouse_pos, [win_x, win_y], avail[0],
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

    // ── Drawing helpers ──────────────────────────────────────────────

    fn offset_col_width(&self) -> f32 {
        if self.config.show_offsets { self.char_advance * 10.0 } else { 0.0 }
    }

    fn hex_col_width(&self) -> f32 {
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
                    draw_list.add_rect(
                        [x - 1.0, y],
                        [x + self.char_advance * 2.0 + 1.0, y + self.line_height],
                        bg_col,
                    ).filled(true).build();
                }

                // Foreground color.
                let fg = self.byte_fg_with_overrides(i, byte);

                // Show text: either the editing nibble or the byte value.
                if is_editing {
                    if let Some(hi_nibble) = self.edit_nibble {
                        // Show first nibble + underscore for second.
                        let txt = if cfg.uppercase {
                            format!("{:X}_", hi_nibble)
                        } else {
                            format!("{:x}_", hi_nibble)
                        };
                        draw_list.add_text([x, y], col32([1.0, 1.0, 0.5, 1.0]), &txt);
                    } else {
                        // Show current value with blinking underline.
                        let txt = if cfg.uppercase {
                            format!("{:02X}", byte)
                        } else {
                            format!("{:02x}", byte)
                        };
                        draw_list.add_text([x, y], col32([1.0, 1.0, 0.5, 1.0]), &txt);
                        // Underline to indicate edit mode.
                        draw_list.add_line(
                            [x, y + self.line_height - 1.0],
                            [x + self.char_advance * 2.0, y + self.line_height - 1.0],
                            col32([1.0, 0.8, 0.3, 1.0]),
                        ).thickness(1.5).build();
                    }
                } else {
                    let txt = if cfg.uppercase {
                        format!("{:02X}", byte)
                    } else {
                        format!("{:02x}", byte)
                    };
                    draw_list.add_text([x, y], fg, &txt);
                }

                // Hover tooltip.
                let byte_hovered = mouse_pos[0] >= x
                    && mouse_pos[0] < x + self.char_advance * 2.5
                    && mouse_pos[1] >= y
                    && mouse_pos[1] < y + self.line_height;
                if byte_hovered && !is_editing {
                    draw_list.add_rect(
                        [x - 1.0, y],
                        [x + self.char_advance * 2.0 + 1.0, y + self.line_height],
                        col32([0.4, 0.63, 0.88, 0.18]),
                    ).filled(true).build();
                    ui.tooltip(|| {
                        let addr = cfg.base_address + i as u64;
                        ui.text(format!("Offset: 0x{:08X} ({})", addr, i));
                        ui.text(format!("Hex: 0x{:02X}  Dec: {}  Oct: 0o{:03o}", byte, byte, byte));
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
            if group > 0 && (col_idx + 1)% group == 0 && col_idx + 1 < bpr {
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
                    draw_list.add_rect(
                        [ax, y],
                        [ax + self.char_advance, y + self.line_height],
                        col32([0.50, 0.30, 0.10, 0.85]),
                    ).filled(true).build();
                } else if is_cursor {
                    draw_list.add_rect(
                        [ax, y],
                        [ax + self.char_advance, y + self.line_height],
                        col32(cfg.color_cursor_bg),
                    ).filled(true).build();
                } else if is_selected {
                    draw_list.add_rect(
                        [ax, y],
                        [ax + self.char_advance, y + self.line_height],
                        col32(cfg.color_selection_bg),
                    ).filled(true).build();
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
                    draw_list.add_line(
                        [ax, y + self.line_height - 1.0],
                        [ax + self.char_advance, y + self.line_height - 1.0],
                        col32([1.0, 0.8, 0.3, 1.0]),
                    ).thickness(1.5).build();
                }

                ax += self.char_advance;
            }
        }
    }

    /// Check if offset is within a search match.
    fn is_search_match(&self, offset: usize) -> bool {
        if self.search_results.is_empty() || self.search_pattern.is_empty() {
            return false;
        }
        let plen = self.search_pattern.len();
        self.search_results.iter().any(|&start| offset >= start && offset < start + plen)
    }

    /// Get foreground color with diff/region overrides.
    fn byte_fg_with_overrides(&self, offset: usize, byte: u8) -> u32 {
        let cfg = &self.config;

        // Changed byte (diff).
        if cfg.highlight_changes && !self.reference.is_empty()
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

    fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.data.len().saturating_sub(1));
    }

    // ── Input handling ───────────────────────────────────────────────

    fn handle_keyboard(&mut self, ui: &dear_imgui_rs::Ui) {
        use dear_imgui_rs::Key;

        let bpr = self.config.bytes_per_row.value();
        let len = self.data.len();
        if len == 0 { return; }

        let shift = ui.io().key_shift();
        // Use physical Ctrl detection (works with any keyboard layout).
        let ctrl = ui.io().key_ctrl();

        // === Hotkeys ===
        // Combine ImGui logical key + physical VK fallback (for non-latin layouts).
        // `vk_pressed` returns true only on the transition frame (not while held).
        let vk_pressed = |vk: i32| -> bool {
            // ImGui tracks key repeat internally via is_key_pressed.
            // For VK fallback: we check VK is down AND the ImGui frame indicates
            // a new key event (io.KeysDown changed). Simple approach: use ImGui's
            // repeat detection via any key pressed + VK state.
            vk_down(vk)
        };

        // Ctrl+C — copy
        if ctrl && (ui.is_key_pressed(Key::C) || vk_pressed(VK_C)) {
            self.copy_selection();
        }

        // Ctrl+G — goto
        if ctrl && (ui.is_key_pressed(Key::G) || vk_pressed(VK_G)) {
            if !self.show_goto {
                self.show_goto = true;
                self.goto_buf.clear();
            }
        }

        // Ctrl+F — search
        if ctrl && (ui.is_key_pressed(Key::F) || vk_pressed(VK_F)) {
            if !self.show_search {
                self.show_search = true;
            }
        }

        // Ctrl+A — select all
        if ctrl && (ui.is_key_pressed(Key::A) || vk_pressed(VK_A)) {
            self.selection = Selection { start: 0, end: len };
        }

        // Ctrl+Z — undo (not shift)
        if ctrl && !shift && (ui.is_key_pressed(Key::Z) || vk_pressed(VK_Z)) {
            self.undo();
        }

        // Ctrl+Y — redo
        if ctrl && (ui.is_key_pressed(Key::Y) || vk_pressed(VK_Y)) {
            self.redo();
        }
        // Ctrl+Shift+Z — redo (alternative)
        if ctrl && shift && (ui.is_key_pressed(Key::Z) || vk_pressed(VK_Z)) {
            self.redo();
        }

        // F3 = next/prev search result.
        if ui.is_key_pressed(Key::F3) && !self.search_results.is_empty() {
            if shift {
                self.search_idx = self.search_idx
                    .checked_sub(1)
                    .unwrap_or(self.search_results.len() - 1);
            } else {
                self.search_idx = (self.search_idx + 1) % self.search_results.len();
            }
            self.cursor = self.search_results[self.search_idx];
            self.selection = Selection {
                start: self.cursor,
                end: self.cursor + self.search_pattern.len(),
            };
            self.scroll_to_cursor();
        }

        // Escape — stop editing.
        if ui.is_key_pressed(Key::Escape) {
            self.stop_editing();
        }

        // Alt+Left/Right — nav back/forward.
        let alt = ui.io().key_alt();
        if alt && ui.is_key_pressed(Key::LeftArrow) {
            self.nav_back();
            return;
        }
        if alt && ui.is_key_pressed(Key::RightArrow) {
            self.nav_forward();
            return;
        }

        // Navigation (not while in active popup).
        if !ctrl && !alt {
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
                self.cursor -= self.cursor % bpr;
                self.scroll_to_cursor();
            }
            if ui.is_key_pressed(Key::End) {
                self.cursor = ((self.cursor / bpr + 1) * bpr - 1).min(len - 1);
                self.scroll_to_cursor();
            }
        }

        // Ctrl+Home/End.
        if ctrl && ui.is_key_pressed(Key::Home) {
            self.cursor = 0;
            self.scroll_to_cursor();
        }
        if ctrl && ui.is_key_pressed(Key::End) {
            self.cursor = len - 1;
            self.scroll_to_cursor();
        }

        // Hex / ASCII editing input.
        if self.config.editable && !ctrl && !alt {
            match self.edit_column {
                Some(EditColumn::Hex) => self.handle_hex_input(),
                Some(EditColumn::Ascii) => self.handle_ascii_input(),
                None => {}
            }
        }
    }

    fn handle_hex_input(&mut self) {
        let chars = read_input_chars();
        for ch in chars {
            let nibble = match ch {
                '0'..='9' => ch as u8 - b'0',
                'a'..='f' => ch as u8 - b'a' + 10,
                'A'..='F' => ch as u8 - b'A' + 10,
                _ => continue,
            };
            if let Some(hi) = self.edit_nibble.take() {
                let new_byte = (hi << 4) | nibble;
                if self.cursor < self.data.len() {
                    let old_byte = self.data[self.cursor];
                    self.undo.push(UndoEntry {
                        offset: self.cursor as u64,
                        old_bytes: vec![old_byte],
                        new_bytes: vec![new_byte],
                    });
                    self.data[self.cursor] = new_byte;
                    if self.cursor < self.data.len() - 1 {
                        self.cursor += 1;
                    }
                }
            } else {
                self.edit_nibble = Some(nibble);
            }
        }
    }

    fn handle_ascii_input(&mut self) {
        let chars = read_input_chars();
        for ch in chars {
            // Accept any printable character (any language/case).
            if ch.is_control() { continue; }
            let mut buf = [0u8; 4];
            let encoded = ch.encode_utf8(&mut buf);
            // Only write the first byte (ASCII-compatible for most cases).
            let new_byte = encoded.as_bytes()[0];
            if self.cursor < self.data.len() {
                let old_byte = self.data[self.cursor];
                self.undo.push(UndoEntry {
                    offset: self.cursor as u64,
                    old_bytes: vec![old_byte],
                    new_bytes: vec![new_byte],
                });
                self.data[self.cursor] = new_byte;
                if self.cursor < self.data.len() - 1 {
                    self.cursor += 1;
                }
            }
        }
    }

    fn handle_mouse(&mut self, ui: &dear_imgui_rs::Ui, _win_w: f32) {
        if !ui.is_window_hovered() { return; }

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

        // Click to set cursor — detect which column was clicked.
        if ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Left) {
            if let Some((offset, column)) = self.mouse_to_offset(ui) {
                if shift {
                    // Shift+Click: extend selection.
                    self.selection.end = offset + 1;
                } else if ctrl {
                    // Ctrl+Click: toggle byte in selection (simple multi-select).
                    if self.selection.contains(offset) {
                        // Deselect (simplistic — just clear).
                        self.selection = Selection::default();
                    } else {
                        // If no selection, start one. Otherwise extend.
                        if self.selection.is_empty() {
                            self.selection = Selection { start: offset, end: offset + 1 };
                        } else {
                            // Extend to include this byte.
                            let (lo, hi) = self.selection.ordered();
                            let new_lo = lo.min(offset);
                            let new_hi = hi.max(offset + 1);
                            self.selection = Selection { start: new_lo, end: new_hi };
                        }
                    }
                    self.cursor = offset;
                } else {
                    self.cursor = offset;
                    self.selection = Selection { start: offset, end: offset };

                    // Start editing if editable.
                    if self.config.editable {
                        self.start_editing(column);
                    } else {
                        self.stop_editing();
                    }
                }
            }
        }

        // Drag to select.
        if ui.is_mouse_dragging(dear_imgui_rs::MouseButton::Left) {
            if let Some((offset, _)) = self.mouse_to_offset(ui) {
                self.selection.end = offset + 1;
            }
        }
    }

    /// Returns (byte_offset, which_column) from mouse position.
    fn mouse_to_offset(&self, ui: &dear_imgui_rs::Ui) -> Option<(usize, EditColumn)> {
        let [mx, my] = ui.io().mouse_pos();
        let [win_x, win_y] = ui.cursor_screen_pos();
        let scroll_y = ui.scroll_y();
        let header_offset = if self.config.show_column_headers { 1 } else { 0 };

        let rel_y = my - win_y + scroll_y;
        let row = (rel_y / self.line_height) as isize - header_offset as isize;
        if row < 0 { return None; }
        let row = row as usize;

        let bpr = self.config.bytes_per_row.value();
        let group = self.config.grouping.value();

        let hex_x = win_x + self.offset_col_width();
        let ascii_x = hex_x + self.hex_col_width() + self.char_advance;

        // Check ASCII column first.
        if self.config.show_ascii && mx >= ascii_x {
            let rel_x = mx - ascii_x;
            let col = (rel_x / self.char_advance) as usize;
            let offset = row * bpr + col.min(bpr - 1);
            if offset < self.data.len() {
                return Some((offset, EditColumn::Ascii));
            }
        }

        // Hex column.
        let rel_x = mx - hex_x;
        if rel_x < 0.0 { return None; }

        let mut col = 0usize;
        let mut x = 0.0f32;
        while col < bpr {
            let next_x = x + self.char_advance * 3.0;
            if rel_x < next_x { break; }
            x = next_x;
            col += 1;
            if group > 0 && col% group == 0 && col < bpr {
                x += self.char_advance;
            }
        }

        let offset = row * bpr + col;
        if offset < self.data.len() { Some((offset, EditColumn::Hex)) } else { None }
    }

    fn scroll_to_cursor(&mut self) {
        let bpr = self.config.bytes_per_row.value();
        let row = self.cursor / bpr;
        self.scroll_to_row = Some(row);
    }

    fn copy_selection(&self) {
        let bytes = self.selected_bytes();
        if bytes.is_empty() {
            if self.cursor < self.data.len() {
                let s = format_bytes(
                    &[self.data[self.cursor]], self.config.copy_format, self.config.uppercase,
                );
                set_clipboard(&s);
            }
            return;
        }
        let s = format_bytes(bytes, self.config.copy_format, self.config.uppercase);
        set_clipboard(&s);
    }

    // ── Goto popup ───────────────────────────────────────────────────

    fn render_goto_popup(&mut self, ui: &dear_imgui_rs::Ui) {
        if !self.show_goto { return; }

        let label = format!("##goto_{}", self.id);
        ui.open_popup(&label);
        self.show_goto = false;

        if let Some(_popup) = ui.begin_popup(&label) {
            ui.text("Goto offset (hex or decimal):");
            ui.input_text("##goto_input", &mut self.goto_buf).build();

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
            let mode_name = self.config.search_mode.display_name();
            if ui.button(mode_name) {
                self.config.search_mode = match self.config.search_mode {
                    HexSearchMode::Hex => HexSearchMode::Ascii,
                    HexSearchMode::Ascii => HexSearchMode::Hex,
                };
            }
            ui.same_line();

            let hint = match self.config.search_mode {
                HexSearchMode::Hex => "Hex (e.g. 4D 5A ?? 00):",
                HexSearchMode::Ascii => "ASCII string:",
            };
            ui.text(hint);
            ui.input_text("##search_input", &mut self.search_buf).build();

            if !self.search_results.is_empty() {
                ui.text(format!(
                    "Result {}/{}", self.search_idx + 1, self.search_results.len()
                ));
            }

            if ui.button("Find") || ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                self.do_search();
                if self.search_results.is_empty() {
                    ui.text("No matches found.");
                } else {
                    ui.close_current_popup();
                }
            }
            ui.same_line();
            if ui.button("Cancel") || ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                ui.close_current_popup();
            }
        }
    }

    fn do_search(&mut self) {
        self.search_pattern = match self.config.search_mode {
            HexSearchMode::Hex => parse_hex_pattern_masked(&self.search_buf),
            HexSearchMode::Ascii => parse_ascii_pattern(&self.search_buf),
        };
        if self.search_pattern.is_empty() { return; }

        self.search_results = find_pattern_masked(&self.data, &self.search_pattern);
        self.search_idx = 0;

        if !self.search_results.is_empty() {
            self.cursor = self.search_results[0];
            self.selection = Selection {
                start: self.cursor,
                end: self.cursor + self.search_pattern.len(),
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
            } else { ("u16", "\u{2014}".into()) },
            if remaining >= 4 {
                let arr = [bytes[0], bytes[1], bytes[2], bytes[3]];
                let v = if le { u32::from_le_bytes(arr) } else { u32::from_be_bytes(arr) };
                ("u32", format!("{}", v))
            } else { ("u32", "\u{2014}".into()) },
            if remaining >= 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[..8]);
                let v = if le { u64::from_le_bytes(arr) } else { u64::from_be_bytes(arr) };
                ("u64", format!("{}", v))
            } else { ("u64", "\u{2014}".into()) },
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
            } else { ("f32", "\u{2014}".into()) },
            if remaining >= 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[..8]);
                let v = if le { f64::from_le_bytes(arr) } else { f64::from_be_bytes(arr) };
                ("f64", format!("{:.6e}", v))
            } else { ("f64", "\u{2014}".into()) },
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
            format!("  Undo: {} / Redo: {}", self.undo.undo_count(), self.undo.redo_count())
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

impl Drop for HexViewer {
    fn drop(&mut self) {
        // Ensure keyboard layout is restored if we're dropped while editing.
        if self.layout_switched {
            clipboard::restore_keyboard_layout();
        }
    }
}

// ── Free helpers ─────────────────────────────────────────────────────────────

fn read_input_chars() -> Vec<char> {
    let io = unsafe { &*dear_imgui_rs::sys::igGetIO_Nil() };
    let data = io.InputQueueCharacters.Data;
    let size = io.InputQueueCharacters.Size;
    if data.is_null() || size <= 0 { return Vec::new(); }
    let slice = unsafe { std::slice::from_raw_parts(data as *const u16, size as usize) };
    slice.iter().filter_map(|&c| char::from_u32(c as u32)).collect()
}

fn parse_address(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if s.chars().all(|c| c.is_ascii_hexdigit()) && s.len() > 4 {
        u64::from_str_radix(s, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
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
    fn test_parse_hex_pattern_masked() {
        let p = parse_hex_pattern_masked("4D 5A ?? 00");
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], PatternByte::Exact(0x4D));
        assert_eq!(p[2], PatternByte::Any);
    }

    #[test]
    fn test_find_pattern_masked() {
        let data = [0x00, 0x4D, 0x5A, 0xFF, 0x00, 0x4D, 0x5A, 0x90];
        let pattern = parse_hex_pattern_masked("4D 5A ??");
        let results = find_pattern_masked(&data, &pattern);
        assert_eq!(results, vec![1, 5]);
    }

    #[test]
    fn test_find_pattern_exact() {
        let data = [0x00, 0x4D, 0x5A, 0x00, 0x4D, 0x5A, 0x90];
        let pattern = parse_hex_pattern_masked("4D 5A");
        let results = find_pattern_masked(&data, &pattern);
        assert_eq!(results, vec![1, 4]);
    }

    #[test]
    fn test_search_ascii() {
        let mut v = HexViewer::new("test");
        v.set_data(b"Hello World Hello");
        v.search_buf = "Hello".to_string();
        v.config.search_mode = HexSearchMode::Ascii;
        v.do_search();
        assert_eq!(v.search_results, vec![0, 12]);
    }

    #[test]
    fn test_byte_category() {
        assert_eq!(ByteCategory::of(0x00), ByteCategory::Zero);
        assert_eq!(ByteCategory::of(0x01), ByteCategory::Control);
        assert_eq!(ByteCategory::of(0x41), ByteCategory::Printable);
        assert_eq!(ByteCategory::of(0x80), ByteCategory::High);
        assert_eq!(ByteCategory::of(0xFF), ByteCategory::Full);
    }

    #[test]
    fn test_undo_stack() {
        let mut stack = UndoStack::new(10);
        assert!(!stack.can_undo());
        stack.push(UndoEntry {
            offset: 0, old_bytes: vec![0xAA], new_bytes: vec![0xBB],
        });
        assert!(stack.can_undo());
        let entry = stack.undo().unwrap();
        assert_eq!(entry.old_bytes, vec![0xAA]);
        assert!(stack.can_redo());
    }

    #[test]
    fn test_nav_history() {
        let mut nav = NavHistory::new(10);
        nav.push(0x1000);
        let back = nav.go_back(0x2000);
        assert_eq!(back, Some(0x1000));
        let fwd = nav.go_forward(0x1000);
        assert_eq!(fwd, Some(0x2000));
    }

    #[test]
    fn test_format_bytes_hex_spaced() {
        assert_eq!(format_bytes(&[0x4D, 0x5A, 0x90], CopyFormat::HexSpaced, true), "4D 5A 90");
    }

    #[test]
    fn test_format_bytes_c_array() {
        assert_eq!(format_bytes(&[0x4D, 0x5A], CopyFormat::CArray, true), "{ 0x4D, 0x5A }");
    }

    #[test]
    fn test_format_bytes_base64() {
        assert_eq!(format_bytes(&[0x4D, 0x5A, 0x90], CopyFormat::Base64, true), "TVqQ");
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn test_bytes_per_row_new_values() {
        assert_eq!(BytesPerRow::EIGHT.value(), 8);
        assert_eq!(BytesPerRow::TWELVE.value(), 12);
        assert_eq!(BytesPerRow::SIXTEEN.value(), 16);
        assert_eq!(BytesPerRow::TWENTY.value(), 20);
        assert_eq!(BytesPerRow::TWENTY_FOUR.value(), 24);
        assert_eq!(BytesPerRow::TWENTY_EIGHT.value(), 28);
        assert_eq!(BytesPerRow::THIRTY_TWO.value(), 32);
        assert_eq!(BytesPerRow::ALL.len(), 7);
    }

    #[test]
    fn test_vec_data_provider() {
        let mut p = VecDataProvider::new(vec![0x10, 0x20, 0x30, 0x40]);
        assert_eq!(p.len(), 4);
        let mut buf = [0u8; 2];
        assert_eq!(p.read(1, &mut buf), 2);
        assert_eq!(buf, [0x20, 0x30]);
        assert!(p.write(2, &[0xFF]));
        assert_eq!(p.data()[2], 0xFF);
    }

    #[test]
    fn test_config_defaults() {
        let cfg = HexViewerConfig::default();
        assert_eq!(cfg.bytes_per_row, BytesPerRow::SIXTEEN);
        assert!(cfg.show_ascii);
        assert!(cfg.category_colors);
    }

    #[test]
    fn test_goto() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0; 1024]);
        v.goto(512);
        assert_eq!(v.cursor(), 512);
    }

    #[test]
    fn test_byte_colors_region() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0; 16]);
        v.config.category_colors = false;
        v.regions.push(ColorRegion::new(4, 4, [1.0, 0.0, 0.0, 1.0], "magic"));
        let fg = v.byte_fg_with_overrides(5, 0);
        assert_eq!(fg, col32([1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn test_byte_colors_changed() {
        let mut v = HexViewer::new("test");
        v.set_data(&[0xAA, 0xBB, 0xCC]);
        v.set_reference(&[0xAA, 0xCC, 0xCC]);
        v.config.highlight_changes = true;
        let fg1 = v.byte_fg_with_overrides(1, 0xBB);
        assert_eq!(fg1, col32(v.config.color_changed));
    }
}
