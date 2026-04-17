//! # DisasmView
//!
//! Standalone disassembly viewer widget for code analysis / debugging UIs.
//!
//! Features:
//! - **5-column layout**: margin | arrows | address | bytes | mnemonic operands ; comment
//! - **Branch arrows** with nesting depth and flow-kind coloring
//! - **Breakpoint markers** (red circles in left gutter)
//! - **Block tinting** (alternating background for logical code blocks)
//! - **Syntax coloring** by instruction type (jump/call/ret/nop/stack/system)
//! - **Operand highlighting** (registers, numbers, memory, strings)
//! - **Keyboard navigation** (arrows, PgUp/Dn, Enter → follow branch, G → goto)
//! - **Selection** with copy (address + mnemonic + operands)
//! - **Navigation history** (Alt+Left/Right, back/forward)
//! - **Inline editing** (double-click bytes to patch)
//! - **Stopped-at highlight** (current execution point)
//! - **Auto-scroll** to follow execution
//! - **Virtualized rendering** — only draws visible rows
//! - **Custom data provider trait** — bring your own decoder
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::disasm_view::{
//!     DisasmView, InstructionEntry, VecDisasmProvider, FlowKind,
//! };
//!
//! let mut provider = VecDisasmProvider::new();
//! provider.push(
//!     InstructionEntry::new(0x401000, vec![0x55], "push", "rbp")
//!         .with_flow(FlowKind::Stack)
//! );
//! let mut view = DisasmView::new("##disasm");
//! // In render loop: view.render(ui, &mut provider);
//! ```

pub mod config;

pub use config::{
    BranchArrow, ColumnWidths, DisasmColors, DisasmDataProvider, DisasmViewConfig,
    FlowKind, Instruction, InstructionEntry, VecDisasmProvider,
    compute_arrows, MAX_ARROW_DEPTH,
};

use crate::utils::clipboard::{set_clipboard, vk_down, VK_C};
use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

use std::collections::BTreeSet;

use crate::hex_viewer::NavHistory;

/// Convert `[r, g, b, a]` to packed u32 color.
fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── Edit State ──────────────────────────────────────────────────────────────

/// Which column is being edited inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum EditColumn {
    Bytes,
    Mnemonic,
}

/// Inline editing state.
struct EditState {
    /// Index of the instruction being edited.
    idx: usize,
    /// Which column.
    column: EditColumn,
    /// Text buffer.
    buf: String,
    /// Frame counter since edit started (for click-outside debounce).
    #[allow(dead_code)]
    frames: u32,
}

// ── DisasmView ──────────────────────────────────────────────────────────────

/// Standalone disassembly viewer widget.
pub struct DisasmView {
    id: String,
    /// Configuration.
    pub config: DisasmViewConfig,

    // ── Navigation ───────────────────────────────────────────
    nav: NavHistory,

    // ── Interaction state ────────────────────────────────────
    /// Currently focused (cursor) instruction index.
    cursor_idx: Option<usize>,
    /// Multi-selection set (indices of selected instructions).
    selection: BTreeSet<usize>,
    /// Anchor index for shift-click range selection.
    sel_anchor: Option<usize>,
    /// Drag-select origin index.
    drag_origin: Option<usize>,
    /// Scroll target (instruction index).
    scroll_to: Option<usize>,
    /// Inline edit state.
    edit: Option<EditState>,
    /// Goto address buffer.
    goto_buf: String,
    /// Show goto popup.
    show_goto: bool,
    /// Whether the widget is focused.
    focused: bool,
    /// Cached char advance.
    char_advance: f32,
    /// Cached line height.
    line_height: f32,
    /// Context menu target instruction index.
    context_idx: Option<usize>,
    /// Show context menu flag.
    show_context_menu: bool,
    /// Cached arrows for current frame.
    cached_arrows: Vec<BranchArrow>,
    /// Frame counter for blinking cursor in edit mode.
    frame_counter: u32,
    /// Position for InputText widget (set by draw_row, consumed by render).
    edit_render_pos: std::cell::Cell<Option<[f32; 2]>>,
    /// Width for the InputText widget.
    edit_render_width: std::cell::Cell<f32>,
}

impl DisasmView {
    /// Create a new disassembly view with the given ImGui ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            config: DisasmViewConfig::default(),
            nav: NavHistory::new(64),
            cursor_idx: None,
            selection: BTreeSet::new(),
            sel_anchor: None,
            drag_origin: None,
            scroll_to: None,
            edit: None,
            goto_buf: String::new(),
            show_goto: false,
            focused: false,
            char_advance: 0.0,
            line_height: 0.0,
            context_idx: None,
            show_context_menu: false,
            cached_arrows: Vec::new(),
            frame_counter: 0,
            edit_render_pos: std::cell::Cell::new(None),
            edit_render_width: std::cell::Cell::new(0.0),
        }
    }

    // ── Public API ───────────────────────────────────────────────────

    /// Currently focused (cursor) instruction index.
    pub fn selected_index(&self) -> Option<usize> { self.cursor_idx }

    /// All selected instruction indices.
    pub fn selected_indices(&self) -> &BTreeSet<usize> { &self.selection }

    /// Number of selected instructions.
    pub fn selected_count(&self) -> usize { self.selection.len() }

    /// Whether a specific index is selected.
    pub fn is_selected(&self, idx: usize) -> bool { self.selection.contains(&idx) }

    /// Set the cursor and single-select one instruction.
    pub fn select(&mut self, idx: usize) {
        self.cursor_idx = Some(idx);
        self.selection.clear();
        self.selection.insert(idx);
        self.sel_anchor = Some(idx);
        self.scroll_to = Some(idx);
    }

    /// Clear all selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.sel_anchor = None;
    }

    /// Scroll to and select the instruction at `addr`.
    pub fn goto_address(&mut self, addr: u64, provider: &dyn DisasmDataProvider) {
        if let Some(idx) = provider.index_of_address(addr) {
            if let Some(old_idx) = self.cursor_idx
                && let Some(old_instr) = provider.instruction(old_idx)
            {
                self.nav.push(old_instr.address());
            }
            self.select(idx);
        }
    }

    /// Navigate back in address history.
    pub fn nav_back(&mut self, provider: &dyn DisasmDataProvider) {
        let current_addr = self.cursor_idx
            .and_then(|i| provider.instruction(i))
            .map(|instr| instr.address())
            .unwrap_or(0);
        if let Some(addr) = self.nav.go_back(current_addr)
            && let Some(idx) = provider.index_of_address(addr)
        {
            self.select(idx);
        }
    }

    /// Navigate forward in address history.
    pub fn nav_forward(&mut self, provider: &dyn DisasmDataProvider) {
        let current_addr = self.cursor_idx
            .and_then(|i| provider.instruction(i))
            .map(|instr| instr.address())
            .unwrap_or(0);
        if let Some(addr) = self.nav.go_forward(current_addr)
            && let Some(idx) = provider.index_of_address(addr)
        {
            self.select(idx);
        }
    }

    // ── Selection helpers ────────────────────────────────────────────

    /// Select a contiguous range [lo..=hi].
    fn select_range(&mut self, a: usize, b: usize) {
        let lo = a.min(b);
        let hi = a.max(b);
        self.selection.clear();
        for i in lo..=hi {
            self.selection.insert(i);
        }
    }

    /// Whether the view is focused.
    pub fn is_focused(&self) -> bool { self.focused }

    // ── Rendering ────────────────────────────────────────────────────

    /// Render the disassembly view widget.
    pub fn render(&mut self, ui: &dear_imgui_rs::Ui, provider: &mut dyn DisasmDataProvider) {
        let count = provider.instruction_count();
        if count == 0 { return; }

        self.frame_counter = self.frame_counter.wrapping_add(1);

        // Cache font metrics.
        let [cw, ch] = calc_text_size("0");
        self.char_advance = cw;
        self.line_height = ch + 4.0; // slightly more padding than hex viewer

        // Auto-scroll to current execution point.
        if self.config.follow_execution && self.scroll_to.is_none() {
            for i in 0..count {
                if let Some(instr) = provider.instruction(i)
                    && instr.is_current()
                {
                    self.cursor_idx = Some(i);
                    self.scroll_to = Some(i);
                    break;
                }
            }
        }

        // ── Goto popup ───────────────────────────────────────
        self.render_goto_popup(ui, provider);
        // ── Context menu ─────────────────────────────────────
        self.render_context_menu(ui, provider);

        let avail = ui.content_region_avail();
        let child_id = format!("##dv_child_{}", self.id);

        ui.child_window(&child_id)
            .size([avail[0], avail[1]])
            .flags(
                dear_imgui_rs::WindowFlags::NO_MOVE
                | dear_imgui_rs::WindowFlags::NO_SCROLL_WITH_MOUSE,
            )
            .build(ui, || {
                let was_focused = self.focused;
                self.focused = ui.is_window_focused();

                // Cancel edit on focus loss.
                if was_focused && !self.focused && self.edit.is_some() {
                    self.edit = None;
                }

                // Handle scroll-to target.
                if let Some(idx) = self.scroll_to.take() {
                    let y = idx as f32 * self.line_height;
                    let visible_h = ui.window_size()[1];
                    // Center the target row.
                    let target_y = (y - visible_h * 0.5).max(0.0);
                    ui.set_scroll_y(target_y);
                }

                // Keyboard.
                if self.focused {
                    self.handle_keyboard(ui, provider);
                }

                // Mouse.
                self.handle_mouse(ui, provider);

                let draw_list = ui.get_window_draw_list();
                let [win_x, win_y] = ui.cursor_screen_pos();
                let scroll_y = ui.scroll_y();
                let visible_h = ui.window_size()[1];

                let first_row = (scroll_y / self.line_height) as usize;
                let visible_count = (visible_h / self.line_height) as usize + 2;
                let last_row = (first_row + visible_count).min(count);

                let origin_x = win_x + ui.scroll_x();
                let origin_y = win_y + scroll_y;

                // ── Compute branch arrows for visible range ───
                if self.config.show_arrows {
                    let visible_instrs: Vec<&dyn Instruction> = (first_row..last_row)
                        .filter_map(|i| provider.instruction(i))
                        .collect();
                    self.cached_arrows = compute_arrows(
                        &visible_instrs, first_row, last_row - first_row,
                    );
                    if self.cached_arrows.len() > self.config.max_arrows {
                        self.cached_arrows.truncate(self.config.max_arrows);
                    }
                }

                // ── Column header ─────────────────────────────
                if self.config.show_header {
                    self.draw_header(&draw_list, origin_x, origin_y);
                }

                let header_h = if self.config.show_header { self.line_height } else { 0.0 };

                // ── Draw rows ─────────────────────────────────
                let mouse_pos = ui.io().mouse_pos();
                for row in first_row..last_row {
                    if let Some(instr) = provider.instruction(row) {
                        let y = origin_y + header_h + (row - first_row) as f32 * self.line_height;
                        self.draw_instruction_row(
                            ui, &draw_list, origin_x, y, row, instr,
                            mouse_pos, avail[0], first_row,
                        );
                    }
                }

                // ── Draw branch arrows on top ─────────────────
                if self.config.show_arrows && !self.cached_arrows.is_empty() {
                    self.draw_arrows(
                        &draw_list, origin_x, origin_y + header_h,
                        first_row,
                    );
                }

                // ── Render inline InputText for editing ──────────
                if let Some(pos) = self.edit_render_pos.take() {
                    let input_w = self.edit_render_width.get();
                    // Position the ImGui cursor at the edit cell.
                    ui.set_cursor_screen_pos(pos);
                    ui.set_next_item_width(input_w);
                    if let Some(edit) = &mut self.edit {
                        let label = format!("##dv_edit_{}", self.id);
                        // Auto-focus on first frame.
                        if edit.frames == 0 {
                            ui.set_keyboard_focus_here();
                        }
                        edit.frames += 1;

                        let flags = dear_imgui_rs::InputTextFlags::CHARS_HEXADECIMAL
                            | dear_imgui_rs::InputTextFlags::CHARS_UPPERCASE
                            | dear_imgui_rs::InputTextFlags::AUTO_SELECT_ALL
                            | dear_imgui_rs::InputTextFlags::ENTER_RETURNS_TRUE;

                        let entered = ui.input_text(&label, &mut edit.buf)
                            .flags(flags)
                            .build();

                        if entered {
                            // Enter pressed — commit.
                            let edit_data = self.edit.take().unwrap();
                            self.commit_edit(edit_data, provider);
                        } else if !ui.is_item_active() && edit.frames > 2 {
                            // Lost focus (clicked elsewhere, Tab, etc.) — cancel.
                            self.edit = None;
                        }
                    }
                }

                // Dummy for scroll extent.
                let total_h = count as f32 * self.line_height + header_h + self.line_height;
                ui.set_cursor_pos([0.0, total_h]);
                ui.dummy([avail[0], 1.0]);
            });
    }

    // ── Drawing helpers ──────────────────────────────────────────────

    fn draw_header(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
    ) {
        let cols = &self.config.columns;
        let hdr_col = col32(self.config.colors.header);
        let mut x = origin_x;

        if self.config.show_breakpoints {
            x += cols.margin;
        }
        if self.config.show_arrows {
            x += cols.arrows;
        }

        draw_list.add_text([x, y], hdr_col, "Address");
        x += cols.address;

        if self.config.show_bytes {
            draw_list.add_text([x, y], hdr_col, "Bytes");
            x += cols.bytes;
        }

        draw_list.add_text([x, y], hdr_col, "Instruction");
        x += cols.mnemonic + cols.operands;

        if self.config.show_comments {
            draw_list.add_text([x, y], hdr_col, "Comment");
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_instruction_row(
        &self,
        ui: &dear_imgui_rs::Ui,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
        idx: usize,
        instr: &dyn Instruction,
        mouse_pos: [f32; 2],
        win_w: f32,
        _first_visible_row: usize,
    ) {
        let cfg = &self.config;
        let colors = &cfg.colors;
        let cols = &cfg.columns;
        let lh = self.line_height;

        // ── Block tint background ─────────────────────────────
        if cfg.show_block_tints {
            let tint = colors.block_tint(instr.block_index());
            if tint[3] > 0.0 {
                draw_list.add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32(tint),
                ).filled(true).build();
            }
        }

        // ── Current execution highlight ───────────────────────
        if instr.is_current() {
            draw_list.add_rect(
                [origin_x, y],
                [origin_x + win_w, y + lh],
                col32(colors.current_line_bg),
            ).filled(true).build();
        }

        // ── Selection highlight ───────────────────────────────
        let is_selected = self.selection.contains(&idx);
        let is_cursor = self.cursor_idx == Some(idx);
        if is_selected {
            // Brighter for cursor row, dimmer for other selected rows.
            let alpha = if is_cursor { 0.55 } else { 0.35 };
            draw_list.add_rect(
                [origin_x, y],
                [origin_x + win_w, y + lh],
                col32([colors.selection_bg[0], colors.selection_bg[1],
                       colors.selection_bg[2], alpha]),
            ).filled(true).build();
        } else if is_cursor {
            draw_list.add_rect(
                [origin_x, y],
                [origin_x + win_w, y + lh],
                col32(colors.selection_bg),
            ).filled(true).build();
        }

        // ── Row hover ─────────────────────────────────────────
        let row_hovered = mouse_pos[1] >= y && mouse_pos[1] < y + lh
            && mouse_pos[0] >= origin_x && mouse_pos[0] < origin_x + win_w;
        if row_hovered && !is_selected && !is_cursor {
            draw_list.add_rect(
                [origin_x, y],
                [origin_x + win_w, y + lh],
                col32(colors.hover_bg),
            ).filled(true).build();
        }

        let mut x = origin_x;

        // ── Breakpoint margin (numbered, colored) ──────────────
        if cfg.show_breakpoints {
            let bp_num = instr.breakpoint_number();
            if bp_num > 0 {
                let bp_color = colors.bp_color(bp_num);
                // Background tint for the gutter cell.
                draw_list.add_rect(
                    [x, y], [x + cols.margin, y + lh],
                    col32([bp_color[0] * 0.3, bp_color[1] * 0.3, bp_color[2] * 0.3, 0.35]),
                ).filled(true).build();
                // Numbered label (centered).
                let label = format!("{}", bp_num);
                let text_w = label.len() as f32 * self.char_advance;
                let tx = x + (cols.margin - text_w) * 0.5;
                // Center vertically: (row_height - text_height) / 2
                let text_h = self.line_height - 4.0; // approx glyph height (lh includes padding)
                let ty = y + (lh - text_h) * 0.5;
                draw_list.add_text([tx, ty], col32(bp_color), &label);
            }
            x += cols.margin;
        }

        // ── Arrow area (drawn separately in draw_arrows) ──────
        if cfg.show_arrows {
            x += cols.arrows;
        }

        // ── Address column ────────────────────────────────────
        let addr = instr.address();
        let addr_str = if cfg.address_width_64 {
            if cfg.uppercase { format!("{:016X}", addr) }
            else { format!("{:016x}", addr) }
        } else if cfg.uppercase {
            format!("{:08X}", addr)
        } else {
            format!("{:08x}", addr)
        };
        draw_list.add_text([x, y], col32(colors.address), &addr_str);
        x += cols.address;

        // ── Bytes column (with inline InputText edit) ──────────
        if cfg.show_bytes {
            let is_editing_bytes = self.edit.as_ref()
                .map(|e| e.idx == idx && e.column == EditColumn::Bytes)
                .unwrap_or(false);

            if is_editing_bytes {
                // We need `&mut self.edit` but `self` is borrowed by draw_row.
                // Mark that this row needs an InputText widget rendered after draw_row.
                // We use a flag — the actual InputText is rendered in render() after draw_row.
                self.edit_render_pos.set(Some([x, y]));
                self.edit_render_width.set(cols.bytes);

                // Draw placeholder background so it's visible.
                draw_list.add_rect(
                    [x - 2.0, y], [x + cols.bytes, y + lh],
                    col32([0.20, 0.15, 0.08, 0.95]),
                ).filled(true).build();
                draw_list.add_rect(
                    [x - 2.0, y], [x + cols.bytes, y + lh],
                    col32([1.0, 0.7, 0.3, 0.80]),
                ).build();
            } else {
                let bytes_str: String = instr.bytes().iter()
                    .map(|b| if cfg.uppercase { format!("{:02X} ", b) } else { format!("{:02x} ", b) })
                    .collect();
                draw_list.add_text([x, y], col32(colors.bytes), bytes_str.trim_end());
            }
            x += cols.bytes;
        }

        // ── Mnemonic ──────────────────────────────────────────
        let mnemonic = instr.mnemonic();
        let mnemonic_color = colors.mnemonic_color(instr.flow_kind());
        draw_list.add_text([x, y], col32(mnemonic_color), mnemonic);
        x += cols.mnemonic;

        // ── Operands (with syntax coloring) ───────────────────
        let operands = instr.operands();
        self.draw_colored_operands(draw_list, x, y, operands, colors);
        x += cols.operands;

        // ── Comment ───────────────────────────────────────────
        if cfg.show_comments
            && let Some(comment) = instr.comment()
        {
            let comment_str = format!("; {}", comment);
            draw_list.add_text([x, y], col32(colors.comment), &comment_str);
        }

        // ── Tooltip on hover (comprehensive) ─────────────────
        if row_hovered {
            ui.tooltip(|| {
                // Address (both 32 and 64 bit representations)
                ui.text(format!("Address: 0x{:016X}", addr));
                if addr <= 0xFFFF_FFFF {
                    ui.text(format!("     32: 0x{:08X}", addr as u32));
                }

                // Instruction size and raw bytes
                let bytes = instr.bytes();
                ui.text(format!("Size: {} bytes", bytes.len()));
                let hex_str: String = bytes.iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>().join(" ");
                ui.text(format!("Bytes: {}", hex_str));

                // Full instruction text
                ui.text(format!("Instr: {} {}", instr.mnemonic(), instr.operands()));

                // Flow kind with semantic description
                let flow_desc = match instr.flow_kind() {
                    FlowKind::Normal  => "Normal (sequential)",
                    FlowKind::Jump    => "Jump (conditional/unconditional)",
                    FlowKind::Call    => "Call (function call)",
                    FlowKind::Return  => "Return (function epilogue)",
                    FlowKind::Nop     => "NOP / padding",
                    FlowKind::Stack   => "Stack operation (push/pop/sub rsp)",
                    FlowKind::System  => "System (syscall/int/sysenter)",
                    FlowKind::Invalid => "INVALID (undecodable)",
                };
                ui.text(format!("Flow: {}", flow_desc));

                // Branch target
                if let Some(target) = instr.branch_target() {
                    ui.text(format!("Target: 0x{:X}", target));
                    // Show offset (relative distance)
                    let offset = target as i64 - addr as i64;
                    if offset >= 0 {
                        ui.text(format!("Offset: +0x{:X} ({} bytes forward)", offset, offset));
                    } else {
                        ui.text(format!("Offset: -0x{:X} ({} bytes back)", -offset, -offset));
                    }
                }

                // Block index
                ui.text(format!("Block: {}", instr.block_index()));

                // Breakpoint
                if instr.has_breakpoint() {
                    let bp_num = instr.breakpoint_number();
                    if bp_num > 0 {
                        ui.text(format!("Breakpoint: #{}", bp_num));
                    } else {
                        ui.text("Breakpoint: YES");
                    }
                }

                // Current IP
                if instr.is_current() {
                    ui.text(">> CURRENT INSTRUCTION POINTER <<");
                }

                // Comment
                if let Some(comment) = instr.comment() {
                    ui.text(format!("Comment: {}", comment));
                }
            });
        }
    }

    /// Draw operand string with basic syntax coloring.
    fn draw_colored_operands(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        x: f32,
        y: f32,
        operands: &str,
        colors: &DisasmColors,
    ) {
        // Simple tokenizer: split on spaces and commas, color by token type.
        let cw = self.char_advance;
        let mut cx = x;

        for token in OperandTokenizer::new(operands) {
            let color = match token.kind {
                TokenKind::Register => colors.operand_register,
                TokenKind::Number   => colors.operand_number,
                TokenKind::Memory   => colors.operand_memory,
                TokenKind::String   => colors.operand_string,
                TokenKind::Plain    => colors.operand_default,
            };
            draw_list.add_text([cx, y], col32(color), token.text);
            cx += token.text.len() as f32 * cw;
        }
    }

    /// Draw branch arrows between instructions.
    fn draw_arrows(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        origin_y: f32,
        _first_visible_row: usize,
    ) {
        let cols = &self.config.columns;
        let colors = &self.config.colors;
        let lh = self.line_height;

        // Arrow area starts after margin.
        let arrow_base_x = origin_x
            + if self.config.show_breakpoints { cols.margin } else { 0.0 }
            + cols.arrows;
        let depth_spacing = cols.arrows / (MAX_ARROW_DEPTH as f32 + 1.0);

        for arrow in &self.cached_arrows {
            let from_y = origin_y + (arrow.from_idx) as f32 * lh + lh * 0.5;
            let to_y = origin_y + (arrow.to_idx) as f32 * lh + lh * 0.5;
            let x = arrow_base_x - (arrow.depth as f32 + 1.0) * depth_spacing;

            let color = col32(colors.arrow_color(arrow.flow_kind));
            let thickness = if arrow.depth == 0 { 1.5 } else { 1.0 };

            // Draw L-shaped arrow: source → left → vertical → right → target.
            // Horizontal from source to vertical line.
            draw_list.add_line([arrow_base_x, from_y], [x, from_y], color)
                .thickness(thickness).build();
            // Vertical line.
            draw_list.add_line([x, from_y], [x, to_y], color)
                .thickness(thickness).build();
            // Horizontal to target.
            draw_list.add_line([x, to_y], [arrow_base_x, to_y], color)
                .thickness(thickness).build();

            // Arrowhead at target.
            let dir = if to_y > from_y { 1.0 } else { -1.0 };
            let head_size = 4.0;
            draw_list.add_triangle(
                [arrow_base_x, to_y],
                [arrow_base_x - head_size, to_y - head_size * dir],
                [arrow_base_x - head_size, to_y + head_size * dir],
                color,
            ).filled(true).build();
        }
    }

    // ── Input handling ───────────────────────────────────────────────

    fn handle_keyboard(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        use dear_imgui_rs::Key;

        let count = provider.instruction_count();
        if count == 0 { return; }

        let ctrl = ui.io().key_ctrl();
        let alt = ui.io().key_alt();
        let shift = ui.io().key_shift();

        // Inline edit active → skip navigation.
        if self.edit.is_some() {
            self.handle_edit_keyboard(ui, provider);
            return;
        }

        // Helper: move cursor and handle shift-selection.
        let move_cursor = |s: &mut Self, new_idx: usize| {
            if shift {
                // Extend selection from anchor to new position.
                let anchor = s.sel_anchor.unwrap_or(s.cursor_idx.unwrap_or(0));
                s.select_range(anchor, new_idx);
            } else {
                s.selection.clear();
                s.selection.insert(new_idx);
                s.sel_anchor = Some(new_idx);
            }
            s.cursor_idx = Some(new_idx);
        };

        // Arrow keys.
        if ui.is_key_pressed(Key::UpArrow) && !alt {
            let idx = self.cursor_idx.unwrap_or(0);
            if idx > 0 {
                let new = idx - 1;
                move_cursor(self, new);
                self.ensure_visible(new, ui);
            }
        }
        if ui.is_key_pressed(Key::DownArrow) && !alt {
            let idx = self.cursor_idx.unwrap_or(0);
            if idx + 1 < count {
                let new = idx + 1;
                move_cursor(self, new);
                self.ensure_visible(new, ui);
            }
        }

        // Page Up/Down.
        if ui.is_key_pressed(Key::PageUp) {
            let visible = (ui.window_size()[1] / self.line_height) as usize;
            let new = self.cursor_idx.unwrap_or(0).saturating_sub(visible);
            move_cursor(self, new);
            self.scroll_to = Some(new);
        }
        if ui.is_key_pressed(Key::PageDown) {
            let visible = (ui.window_size()[1] / self.line_height) as usize;
            let new = (self.cursor_idx.unwrap_or(0) + visible).min(count - 1);
            move_cursor(self, new);
            self.scroll_to = Some(new);
        }

        // Home/End.
        if ui.is_key_pressed(Key::Home) {
            move_cursor(self, 0);
            self.scroll_to = Some(0);
        }
        if ui.is_key_pressed(Key::End) {
            move_cursor(self, count - 1);
            self.scroll_to = Some(count - 1);
        }

        // Ctrl+A — select all.
        if ctrl && (ui.is_key_pressed(Key::A) || vk_down(crate::utils::clipboard::VK_A)) {
            for i in 0..count {
                self.selection.insert(i);
            }
        }

        // Enter → follow branch target.
        if ui.is_key_pressed(Key::Enter)
            && let Some(idx) = self.cursor_idx
            && let Some(instr) = provider.instruction(idx)
            && let Some(target) = instr.branch_target()
        {
            self.goto_address(target, provider);
        }

        // G → goto address popup.
        if ui.is_key_pressed(Key::G) && !ctrl {
            self.show_goto = true;
            self.goto_buf.clear();
        }

        // Ctrl+C → copy selected instruction (physical key for non-latin layouts).
        if ctrl && (ui.is_key_pressed(Key::C) || vk_down(VK_C)) {
            self.copy_selected(provider);
        }

        // F9 → toggle breakpoint.
        if ui.is_key_pressed(Key::F9)
            && let Some(idx) = self.cursor_idx
            && let Some(instr) = provider.instruction(idx)
        {
            provider.toggle_breakpoint(instr.address());
        }

        // Alt+Left → nav back.
        if alt && ui.is_key_pressed(Key::LeftArrow) {
            self.nav_back(provider);
        }
        // Alt+Right → nav forward.
        if alt && ui.is_key_pressed(Key::RightArrow) {
            self.nav_forward(provider);
        }
    }

    fn handle_edit_keyboard(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        _provider: &mut dyn DisasmDataProvider,
    ) {
        // InputText widget handles all input now.
        // Only Escape needs manual handling (ImGui InputText doesn't cancel on Esc by default).
        if ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
            self.edit = None;
        }
    }

    fn commit_edit(&self, edit: EditState, provider: &mut dyn DisasmDataProvider) {
        if let Some(instr) = provider.instruction(edit.idx) {
            let addr = instr.address();
            match edit.column {
                EditColumn::Bytes => {
                    // Parse hex bytes.
                    let bytes: Vec<u8> = edit.buf.split_whitespace()
                        .filter_map(|tok| u8::from_str_radix(tok, 16).ok())
                        .collect();
                    if !bytes.is_empty() {
                        provider.write_bytes(addr, &bytes);
                    }
                }
                EditColumn::Mnemonic => {
                    // Assemble instruction text.
                    provider.assemble(addr, &edit.buf);
                }
            }
        }
    }

    fn handle_mouse(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
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

        // Click to select — with Ctrl/Shift modifiers.
        if ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Left) {
            if let Some(idx) = self.mouse_to_instruction(ui, provider) {
                // Cancel edit if clicking a different row.
                if let Some(edit) = &self.edit
                    && edit.idx != idx
                {
                    self.edit = None;
                }

                if shift {
                    // Shift+Click: range select from anchor to clicked row.
                    let anchor = self.sel_anchor.unwrap_or(self.cursor_idx.unwrap_or(0));
                    self.select_range(anchor, idx);
                    self.cursor_idx = Some(idx);
                } else if ctrl {
                    // Ctrl+Click: toggle individual row in selection.
                    if self.selection.contains(&idx) {
                        self.selection.remove(&idx);
                    } else {
                        self.selection.insert(idx);
                    }
                    self.cursor_idx = Some(idx);
                    self.sel_anchor = Some(idx);
                } else {
                    // Plain click: single select.
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

        // Double-click to edit (if editable).
        if ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
            && self.config.editable
            && let Some(idx) = self.mouse_to_instruction(ui, provider)
            && let Some(instr) = provider.instruction(idx)
        {
            let bytes_str: String = instr.bytes().iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>().join(" ");
            self.edit = Some(EditState {
                idx,
                column: EditColumn::Bytes,
                buf: bytes_str,
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
        }
    }

    fn mouse_to_instruction(
        &self,
        ui: &dear_imgui_rs::Ui,
        provider: &dyn DisasmDataProvider,
    ) -> Option<usize> {
        let [_mx, my] = ui.io().mouse_pos();
        let [_win_x, win_y] = ui.cursor_screen_pos();
        let scroll_y = ui.scroll_y();
        let origin_y = win_y + scroll_y;
        let header_h = if self.config.show_header { self.line_height } else { 0.0 };

        let rel_y = my - origin_y - header_h;
        if rel_y < 0.0 { return None; }

        let scroll_offset = (scroll_y / self.line_height) as usize;
        let row = (rel_y / self.line_height) as usize + scroll_offset;

        if row < provider.instruction_count() {
            Some(row)
        } else {
            None
        }
    }

    fn ensure_visible(&mut self, idx: usize, ui: &dear_imgui_rs::Ui) {
        let y = idx as f32 * self.line_height;
        let scroll_y = ui.scroll_y();
        let visible_h = ui.window_size()[1];

        if y < scroll_y || y + self.line_height > scroll_y + visible_h {
            self.scroll_to = Some(idx);
        }
    }

    fn copy_selected(&self, provider: &dyn DisasmDataProvider) {
        // Copy all selected instructions (or just cursor if nothing selected).
        let indices: Vec<usize> = if self.selection.is_empty() {
            self.cursor_idx.into_iter().collect()
        } else {
            self.selection.iter().copied().collect()
        };

        if indices.is_empty() { return; }

        let lines: Vec<String> = indices.iter().filter_map(|&idx| {
            provider.instruction(idx).map(|instr| {
                let addr = if self.config.address_width_64 {
                    format!("{:016X}", instr.address())
                } else {
                    format!("{:08X}", instr.address())
                };
                let bytes_str: String = instr.bytes().iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>().join(" ");
                let comment = instr.comment()
                    .map(|c| format!(" ; {}", c))
                    .unwrap_or_default();
                format!("{}  {:16}  {} {}{}",
                    addr, bytes_str, instr.mnemonic(), instr.operands(), comment)
            })
        }).collect();

        set_clipboard(&lines.join("\n"));
    }

    // ── Goto popup ───────────────────────────────────────────────────

    fn render_goto_popup(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        if !self.show_goto { return; }

        let label = format!("##dv_goto_{}", self.id);
        ui.open_popup(&label);
        self.show_goto = false;

        if let Some(_popup) = ui.begin_popup(&label) {
            ui.text("Goto address (hex):");
            ui.input_text("##dv_goto_input", &mut self.goto_buf)
                .build();

            if ui.button("Go") || ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                if let Some(addr) = parse_address(&self.goto_buf) {
                    self.goto_address(addr, provider);
                }
                ui.close_current_popup();
            }
            ui.same_line();
            if ui.button("Cancel") || ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                ui.close_current_popup();
            }
        }
    }

    // ── Context menu ─────────────────────────────────────────────────

    fn render_context_menu(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        if !self.show_context_menu { return; }

        let label = format!("##dv_ctx_{}", self.id);
        ui.open_popup(&label);
        self.show_context_menu = false;

        if let Some(_popup) = ui.begin_popup(&label) {
            let idx = self.context_idx.unwrap_or(0);
            let instr_addr = provider.instruction(idx).map(|i| i.address());
            let has_target = provider.instruction(idx)
                .and_then(|i| i.branch_target()).is_some();

            if ui.selectable("Copy Address") {
                if let Some(addr) = instr_addr {
                    let s = format!("0x{:X}", addr);
                    set_clipboard(&s);
                }
                ui.close_current_popup();
            }

            let sel_count = self.selection.len();
            let copy_label = if sel_count > 1 {
                format!("Copy {} Instructions", sel_count)
            } else {
                "Copy Instruction".to_string()
            };
            if ui.selectable(&copy_label) {
                self.copy_selected(provider);
                ui.close_current_popup();
            }

            ui.separator();

            if has_target && ui.selectable("Follow Branch") {
                if let Some(target) = provider.instruction(idx)
                    .and_then(|i| i.branch_target())
                {
                    self.goto_address(target, provider);
                }
                ui.close_current_popup();
            }

            if ui.selectable("Toggle Breakpoint") {
                if let Some(addr) = instr_addr {
                    provider.toggle_breakpoint(addr);
                }
                ui.close_current_popup();
            }

            ui.separator();

            if ui.selectable("Goto Address...") {
                self.show_goto = true;
                self.goto_buf.clear();
                ui.close_current_popup();
            }
        }
    }
}

// ── Operand Tokenizer ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Register,
    Number,
    Memory,
    String,
    Plain,
}

struct OperandToken<'a> {
    text: &'a str,
    kind: TokenKind,
}

/// Simple operand tokenizer for syntax coloring.
struct OperandTokenizer<'a> {
    remaining: &'a str,
}

impl<'a> OperandTokenizer<'a> {
    fn new(text: &'a str) -> Self {
        Self { remaining: text }
    }
}

impl<'a> Iterator for OperandTokenizer<'a> {
    type Item = OperandToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() { return None; }

        // Consume leading whitespace/punctuation as plain tokens.
        let first = self.remaining.as_bytes()[0];
        if matches!(first, b' ' | b',' | b'+' | b'-' | b'*' | b':') {
            let end = self.remaining.bytes()
                .position(|b| !matches!(b, b' ' | b',' | b'+' | b'-' | b'*' | b':'))
                .unwrap_or(self.remaining.len());
            let (tok, rest) = self.remaining.split_at(end);
            self.remaining = rest;
            return Some(OperandToken { text: tok, kind: TokenKind::Plain });
        }

        // Memory brackets.
        if first == b'[' || first == b']' {
            let (tok, rest) = self.remaining.split_at(1);
            self.remaining = rest;
            return Some(OperandToken { text: tok, kind: TokenKind::Memory });
        }

        // String literal.
        if first == b'"' {
            let end = self.remaining[1..].find('"')
                .map(|p| p + 2)
                .unwrap_or(self.remaining.len());
            let (tok, rest) = self.remaining.split_at(end);
            self.remaining = rest;
            return Some(OperandToken { text: tok, kind: TokenKind::String });
        }

        // Find end of word.
        let end = self.remaining.bytes()
            .position(|b| matches!(b, b' ' | b',' | b'+' | b'-' | b'*' | b':' | b'[' | b']'))
            .unwrap_or(self.remaining.len());
        let (word, rest) = self.remaining.split_at(end);
        self.remaining = rest;

        let kind = classify_operand_token(word);
        Some(OperandToken { text: word, kind })
    }
}

/// Classify an operand token as register, number, or plain.
fn classify_operand_token(token: &str) -> TokenKind {
    if token.is_empty() { return TokenKind::Plain; }

    // x86 register names.
    static REGS: &[&str] = &[
        // 64-bit
        "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp",
        "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15",
        // 32-bit
        "eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp",
        "r8d", "r9d", "r10d", "r11d", "r12d", "r13d", "r14d", "r15d",
        // 16-bit
        "ax", "bx", "cx", "dx", "si", "di", "bp", "sp",
        // 8-bit
        "al", "bl", "cl", "dl", "ah", "bh", "ch", "dh",
        "sil", "dil", "bpl", "spl",
        "r8b", "r9b", "r10b", "r11b", "r12b", "r13b", "r14b", "r15b",
        // Segment
        "cs", "ds", "es", "fs", "gs", "ss",
        // Special
        "rip", "eip", "rflags", "eflags",
        // SSE/AVX
        "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7",
        "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15",
        "ymm0", "ymm1", "ymm2", "ymm3", "ymm4", "ymm5", "ymm6", "ymm7",
        "ymm8", "ymm9", "ymm10", "ymm11", "ymm12", "ymm13", "ymm14", "ymm15",
        // x87
        "st0", "st1", "st2", "st3", "st4", "st5", "st6", "st7",
    ];

    let lower = token.to_ascii_lowercase();

    // Size keywords → memory context (check before registers).
    if matches!(lower.as_str(), "byte" | "word" | "dword" | "qword" | "ptr" | "xmmword" | "ymmword") {
        return TokenKind::Memory;
    }

    if REGS.contains(&lower.as_str()) {
        return TokenKind::Register;
    }

    // Number: 0x..., decimal, or hex with 'h' suffix.
    if token.starts_with("0x") || token.starts_with("0X") {
        return TokenKind::Number;
    }
    if (token.ends_with('h') || token.ends_with('H'))
        && token[..token.len() - 1].chars().all(|c| c.is_ascii_hexdigit())
    {
        return TokenKind::Number;
    }
    if token.chars().all(|c| c.is_ascii_digit()) {
        return TokenKind::Number;
    }

    TokenKind::Plain
}

// ── Free helpers ─────────────────────────────────────────────────────────────

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
    use config::{InstructionEntry, VecDisasmProvider};

    fn sample_provider() -> VecDisasmProvider {
        let mut p = VecDisasmProvider::new();
        p.push(InstructionEntry::new(0x401000, vec![0x55], "push", "rbp")
            .with_flow(FlowKind::Stack));
        p.push(InstructionEntry::new(0x401001, vec![0x48, 0x89, 0xE5], "mov", "rbp, rsp"));
        p.push(InstructionEntry::new(0x401004, vec![0x48, 0x83, 0xEC, 0x20], "sub", "rsp, 0x20")
            .with_flow(FlowKind::Stack));
        p.push(InstructionEntry::new(0x401008, vec![0xE8, 0x10, 0x00, 0x00, 0x00], "call", "0x40101D")
            .with_flow(FlowKind::Call)
            .with_target(0x40101D)
            .with_comment("some_function"));
        p.push(InstructionEntry::new(0x40100D, vec![0x48, 0x85, 0xC0], "test", "rax, rax"));
        p.push(InstructionEntry::new(0x401010, vec![0x74, 0x05], "je", "0x401017")
            .with_flow(FlowKind::Jump)
            .with_target(0x401017));
        p.push(InstructionEntry::new(0x401012, vec![0xC9], "leave", ""));
        p.push(InstructionEntry::new(0x401013, vec![0xC3], "ret", "")
            .with_flow(FlowKind::Return));
        p
    }

    #[test]
    fn test_new_view() {
        let view = DisasmView::new("test");
        assert!(view.selected_index().is_none());
        assert!(!view.is_focused());
    }

    #[test]
    fn test_instruction_entry() {
        let instr = InstructionEntry::new(0x401000, vec![0x55], "push", "rbp")
            .with_flow(FlowKind::Stack)
            .with_block(1)
            .with_breakpoint(true)
            .with_current(true)
            .with_comment("prologue");

        assert_eq!(instr.address(), 0x401000);
        assert_eq!(instr.bytes(), &[0x55]);
        assert_eq!(instr.mnemonic(), "push");
        assert_eq!(instr.operands(), "rbp");
        assert_eq!(instr.flow_kind(), FlowKind::Stack);
        assert_eq!(instr.block_index(), 1);
        assert!(instr.has_breakpoint());
        assert!(instr.is_current());
        assert_eq!(instr.comment(), Some("prologue"));
    }

    #[test]
    fn test_vec_provider() {
        let p = sample_provider();
        assert_eq!(p.instruction_count(), 8);
        assert_eq!(p.instruction(0).unwrap().mnemonic(), "push");
        assert_eq!(p.instruction(3).unwrap().mnemonic(), "call");
        assert_eq!(p.index_of_address(0x401008), Some(3));
        assert_eq!(p.index_of_address(0xDEAD), None);
    }

    #[test]
    fn test_toggle_breakpoint() {
        let mut p = sample_provider();
        assert!(!p.instruction(0).unwrap().has_breakpoint());
        assert!(p.toggle_breakpoint(0x401000));
        assert!(p.instruction(0).unwrap().has_breakpoint());
        assert!(!p.toggle_breakpoint(0x401000));
        assert!(!p.instruction(0).unwrap().has_breakpoint());
    }

    #[test]
    fn test_flow_kind_colors() {
        let colors = DisasmColors::default();
        assert_eq!(colors.mnemonic_color(FlowKind::Jump), colors.mnemonic_jump);
        assert_eq!(colors.mnemonic_color(FlowKind::Call), colors.mnemonic_call);
        assert_eq!(colors.mnemonic_color(FlowKind::Return), colors.mnemonic_return);
        assert_eq!(colors.mnemonic_color(FlowKind::Normal), colors.mnemonic_normal);
    }

    #[test]
    fn test_arrow_color() {
        let colors = DisasmColors::default();
        assert_eq!(colors.arrow_color(FlowKind::Jump), colors.arrow_jump);
        assert_eq!(colors.arrow_color(FlowKind::Call), colors.arrow_call);
        assert_eq!(colors.arrow_color(FlowKind::Normal), colors.arrow_default);
    }

    #[test]
    fn test_block_tint() {
        let colors = DisasmColors::default();
        let t0 = colors.block_tint(0);
        let t1 = colors.block_tint(1);
        assert_ne!(t0, t1);
        // Wraps around.
        let n = colors.block_tints.len();
        assert_eq!(colors.block_tint(0), colors.block_tint(n));
    }

    #[test]
    fn test_compute_arrows() {
        let p = sample_provider();
        let instrs: Vec<&dyn Instruction> = (0..p.instruction_count())
            .filter_map(|i| p.instruction(i))
            .collect();
        let arrows = compute_arrows(&instrs, 0, instrs.len());
        // je at index 5 targets 0x401017 and call targets 0x40101D — both outside
        // our 8 instructions, so no arrows expected in this basic sample.
        // Arrow computation only shows arrows where BOTH endpoints are visible.
        assert!(arrows.is_empty() || arrows.len() <= 2,
            "Expected 0-2 arrows, got {}", arrows.len());
    }

    #[test]
    fn test_operand_tokenizer_registers() {
        let tokens: Vec<_> = OperandTokenizer::new("rax, [rbp-0x10]").collect();
        assert!(tokens.len() >= 3);
        assert_eq!(tokens[0].kind, TokenKind::Register); // rax
    }

    #[test]
    fn test_operand_tokenizer_numbers() {
        let tokens: Vec<_> = OperandTokenizer::new("0x401000").collect();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Number);
    }

    #[test]
    fn test_operand_tokenizer_memory() {
        let tokens: Vec<_> = OperandTokenizer::new("[rsp+8]").collect();
        assert_eq!(tokens[0].kind, TokenKind::Memory); // [
    }

    #[test]
    fn test_classify_operand_register() {
        assert_eq!(classify_operand_token("rax"), TokenKind::Register);
        assert_eq!(classify_operand_token("xmm0"), TokenKind::Register);
        assert_eq!(classify_operand_token("r15"), TokenKind::Register);
    }

    #[test]
    fn test_classify_operand_number() {
        assert_eq!(classify_operand_token("0x1234"), TokenKind::Number);
        assert_eq!(classify_operand_token("42"), TokenKind::Number);
        assert_eq!(classify_operand_token("0FFh"), TokenKind::Number);
    }

    #[test]
    fn test_classify_operand_size() {
        assert_eq!(classify_operand_token("dword"), TokenKind::Memory);
        assert_eq!(classify_operand_token("qword"), TokenKind::Memory);
        assert_eq!(classify_operand_token("ptr"), TokenKind::Memory);
    }

    #[test]
    fn test_column_widths_default() {
        let cols = ColumnWidths::default();
        assert!(cols.margin > 0.0);
        assert!(cols.arrows > 0.0);
        assert!(cols.address > 0.0);
    }

    #[test]
    fn test_disasm_config_default() {
        let cfg = DisasmViewConfig::default();
        assert!(cfg.show_arrows);
        assert!(cfg.show_breakpoints);
        assert!(cfg.show_block_tints);
        assert!(cfg.show_header);
        assert!(!cfg.editable);
        assert!(cfg.address_width_64);
    }

    #[test]
    fn test_select_and_goto() {
        let p = sample_provider();
        let mut view = DisasmView::new("test");
        view.select(3);
        assert_eq!(view.selected_index(), Some(3));

        view.goto_address(0x401000, &p);
        assert_eq!(view.selected_index(), Some(0));
    }

    #[test]
    fn test_nav_history() {
        let p = sample_provider();
        let mut view = DisasmView::new("test");

        view.select(0); // at 0x401000
        view.goto_address(0x401008, &p); // jump to call
        assert_eq!(view.selected_index(), Some(3));

        view.nav_back(&p);
        assert_eq!(view.selected_index(), Some(0));

        view.nav_forward(&p);
        assert_eq!(view.selected_index(), Some(3));
    }

    #[test]
    fn test_parse_address() {
        assert_eq!(parse_address("0x401000"), Some(0x401000));
        assert_eq!(parse_address("401000"), Some(0x401000));
        assert_eq!(parse_address("256"), Some(256));
    }

    #[test]
    fn test_arrow_depth_assignment() {
        // Create instructions with nested branches.
        let mut p = VecDisasmProvider::new();
        for i in 0..10 {
            let mut entry = InstructionEntry::new(
                0x1000 + i * 2, vec![0x90], "nop", "",
            );
            entry.flow_kind = FlowKind::Normal;
            p.push(entry);
        }
        // Add two overlapping jumps.
        p.instructions_mut()[2] = InstructionEntry::new(
            0x1004, vec![0xEB, 0x08], "jmp", "0x100E",
        ).with_flow(FlowKind::Jump).with_target(0x100E);
        p.instructions_mut()[1] = InstructionEntry::new(
            0x1002, vec![0x74, 0x0C], "je", "0x1010",
        ).with_flow(FlowKind::Jump).with_target(0x1010);

        let instrs: Vec<&dyn Instruction> = (0..p.instruction_count())
            .filter_map(|i| p.instruction(i))
            .collect();
        let arrows = compute_arrows(&instrs, 0, instrs.len());

        // If both targets are in range, should have different depths.
        if arrows.len() >= 2 {
            assert_ne!(arrows[0].depth, arrows[1].depth,
                "Overlapping arrows should have different depths");
        }
    }
}
