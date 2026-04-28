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

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;

mod draw;
mod input;
mod popup;
mod tokens;

pub use config::{
    BranchArrow, ColumnWidths, DisasmColors, DisasmDataProvider, DisasmViewConfig, FlowKind,
    Instruction, InstructionEntry, MAX_ARROW_DEPTH, VecDisasmProvider, compute_arrows,
};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

use std::collections::BTreeSet;

use crate::hex_viewer::NavHistory;

/// Convert `[r, g, b, a]` to packed u32 color.
fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── Edit State ──────────────────────────────────────────────────────────────

/// Which column is being edited inline. Currently only `Bytes` is
/// constructed by the UI — the `Mnemonic` variant is reserved for a future
/// "edit mnemonic / operands" feature whose commit path
/// ([`DisasmView::commit_edit`]) is already wired up. The
/// `#[allow(dead_code)]` documents the deliberate placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditColumn {
    Bytes,
    #[allow(dead_code)]
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
    /// Frames since edit started — drives auto-focus on frame 0 and the
    /// "lost-focus → cancel" guard from frame 2 onwards (frame 1 is still
    /// in the focus-grab transition).
    frames: u32,
}

// ── DisasmView ──────────────────────────────────────────────────────────────

/// Standalone disassembly viewer widget.
pub struct DisasmView {
    id: String,
    /// Configuration.
    pub config: DisasmViewConfig,

    // ── Cached ImGui IDs (built once at construction) ─────────
    pub(super) edit_label: String,
    pub(super) goto_popup_id: String,
    pub(super) ctx_popup_id: String,

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
        let id: String = id.into();
        let edit_label = format!("##dv_edit_{id}");
        let goto_popup_id = format!("##dv_goto_{id}");
        let ctx_popup_id = format!("##dv_ctx_{id}");
        Self {
            id,
            config: DisasmViewConfig::default(),
            edit_label,
            goto_popup_id,
            ctx_popup_id,
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
    pub fn selected_index(&self) -> Option<usize> {
        self.cursor_idx
    }

    /// All selected instruction indices.
    pub fn selected_indices(&self) -> &BTreeSet<usize> {
        &self.selection
    }

    /// Number of selected instructions.
    pub fn selected_count(&self) -> usize {
        self.selection.len()
    }

    /// Whether a specific index is selected.
    pub fn is_selected(&self, idx: usize) -> bool {
        self.selection.contains(&idx)
    }

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
        let current_addr = self
            .cursor_idx
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
        let current_addr = self
            .cursor_idx
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
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    // ── Rendering ────────────────────────────────────────────────────

    /// Render the disassembly view widget.
    pub fn render(&mut self, ui: &dear_imgui_rs::Ui, provider: &mut dyn DisasmDataProvider) {
        let count = provider.instruction_count();
        if count == 0 {
            return;
        }

        self.frame_counter = self.frame_counter.wrapping_add(1);

        // Cache font metrics. Guard against the rare zero-glyph case (e.g.
        // before the font atlas is fully built or in test stubs) — division
        // by zero in the row math below would produce inf-cast UB.
        let [cw, ch] = calc_text_size("0");
        self.char_advance = cw.max(1.0);
        self.line_height = (ch + 4.0).max(1.0);

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
                    self.cached_arrows =
                        compute_arrows(&visible_instrs, first_row, last_row - first_row);
                    if self.cached_arrows.len() > self.config.max_arrows {
                        self.cached_arrows.truncate(self.config.max_arrows);
                    }
                }

                // ── Column header ─────────────────────────────
                if self.config.show_header {
                    self.draw_header(&draw_list, origin_x, origin_y);
                }

                let header_h = if self.config.show_header {
                    self.line_height
                } else {
                    0.0
                };

                // ── Draw rows ─────────────────────────────────
                let mouse_pos = ui.io().mouse_pos();
                for row in first_row..last_row {
                    if let Some(instr) = provider.instruction(row) {
                        let y = origin_y + header_h + (row - first_row) as f32 * self.line_height;
                        self.draw_instruction_row(
                            ui, &draw_list, origin_x, y, row, instr, mouse_pos, avail[0], first_row,
                        );
                    }
                }

                // ── Draw branch arrows on top ─────────────────
                if self.config.show_arrows && !self.cached_arrows.is_empty() {
                    self.draw_arrows(&draw_list, origin_x, origin_y + header_h, first_row);
                }

                // ── Render inline InputText for editing ──────────
                if let Some(pos) = self.edit_render_pos.take() {
                    let input_w = self.edit_render_width.get();
                    // Position the ImGui cursor at the edit cell.
                    ui.set_cursor_screen_pos(pos);
                    ui.set_next_item_width(input_w);
                    if let Some(edit) = &mut self.edit {
                        // Auto-focus on first frame.
                        if edit.frames == 0 {
                            ui.set_keyboard_focus_here();
                        }
                        edit.frames += 1;

                        let flags = dear_imgui_rs::InputTextFlags::CHARS_HEXADECIMAL
                            | dear_imgui_rs::InputTextFlags::CHARS_UPPERCASE
                            | dear_imgui_rs::InputTextFlags::AUTO_SELECT_ALL
                            | dear_imgui_rs::InputTextFlags::ENTER_RETURNS_TRUE;

                        let entered = ui
                            .input_text(&self.edit_label, &mut edit.buf)
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
}

// ── Free helpers ─────────────────────────────────────────────────────────────

/// Parse a goto-popup / API-supplied address string.
///
/// Acceptance order (first match wins):
/// 1. **`0x` / `0X` prefix** → unconditional hex parse.
/// 2. **Contains a hex letter (`a–f` / `A–F`)** → unambiguous hex, parse as base 16.
/// 3. Otherwise → parse as base 10.
///
/// The previous heuristic accepted hex on length alone (`s.len() > 4` with
/// any hex digits) which made `"4080"` (4 chars) decimal but `"40810"`
/// (5 chars) hex — a confusing length-cliff. Now decimal is the default
/// for all-digit input; hex requires either an explicit `0x` prefix or
/// at least one `a–f` letter.
fn parse_address(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    let has_hex_letter = s.chars().any(|c| matches!(c, 'a'..='f' | 'A'..='F'));
    if has_hex_letter && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return u64::from_str_radix(s, 16).ok();
    }
    s.parse::<u64>().ok()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use config::{InstructionEntry, VecDisasmProvider};
    use tokens::{OperandTokenizer, TokenKind, classify_operand_token};

    fn sample_provider() -> VecDisasmProvider {
        let mut p = VecDisasmProvider::new();
        p.push(
            InstructionEntry::new(0x401000, vec![0x55], "push", "rbp").with_flow(FlowKind::Stack),
        );
        p.push(InstructionEntry::new(
            0x401001,
            vec![0x48, 0x89, 0xE5],
            "mov",
            "rbp, rsp",
        ));
        p.push(
            InstructionEntry::new(0x401004, vec![0x48, 0x83, 0xEC, 0x20], "sub", "rsp, 0x20")
                .with_flow(FlowKind::Stack),
        );
        p.push(
            InstructionEntry::new(
                0x401008,
                vec![0xE8, 0x10, 0x00, 0x00, 0x00],
                "call",
                "0x40101D",
            )
            .with_flow(FlowKind::Call)
            .with_target(0x40101D)
            .with_comment("some_function"),
        );
        p.push(InstructionEntry::new(
            0x40100D,
            vec![0x48, 0x85, 0xC0],
            "test",
            "rax, rax",
        ));
        p.push(
            InstructionEntry::new(0x401010, vec![0x74, 0x05], "je", "0x401017")
                .with_flow(FlowKind::Jump)
                .with_target(0x401017),
        );
        p.push(InstructionEntry::new(0x401012, vec![0xC9], "leave", ""));
        p.push(InstructionEntry::new(0x401013, vec![0xC3], "ret", "").with_flow(FlowKind::Return));
        p
    }

    #[test]
    fn test_new_view() {
        let view = DisasmView::new("test");
        assert_eq!(view.selected_index(), None);
        assert!(!view.is_focused());
    }

    #[test]
    fn test_instruction_entry() {
        let entry = InstructionEntry::new(0x1000, vec![0x90], "nop", "");
        assert_eq!(entry.address(), 0x1000);
        assert_eq!(entry.bytes(), &[0x90]);
        assert_eq!(entry.mnemonic(), "nop");
        assert_eq!(entry.operands(), "");
        assert_eq!(entry.flow_kind(), FlowKind::Normal);

        let entry2 = InstructionEntry::new(0x2000, vec![0xEB, 0x10], "jmp", "0x2010")
            .with_flow(FlowKind::Jump)
            .with_target(0x2010)
            .with_comment("loop top");
        assert_eq!(entry2.flow_kind(), FlowKind::Jump);
        assert_eq!(entry2.branch_target(), Some(0x2010));
        assert_eq!(entry2.comment(), Some("loop top"));
    }

    #[test]
    fn test_vec_provider() {
        let p = sample_provider();
        assert_eq!(p.instruction_count(), 8);
        assert!(p.instruction(0).is_some());
        assert!(p.instruction(8).is_none());
        assert_eq!(p.index_of_address(0x401004), Some(2));
        assert_eq!(p.index_of_address(0xFF0000), None);
    }

    #[test]
    fn test_toggle_breakpoint() {
        let mut p = sample_provider();
        p.toggle_breakpoint(0x401000);
        assert!(p.instruction(0).unwrap().has_breakpoint());
        p.toggle_breakpoint(0x401000);
        assert!(!p.instruction(0).unwrap().has_breakpoint());
    }

    #[test]
    fn test_flow_kind_colors() {
        let colors = DisasmColors::default();
        // Different flow kinds should have visually distinct mnemonic colors.
        let normal = colors.mnemonic_color(FlowKind::Normal);
        let jump = colors.mnemonic_color(FlowKind::Jump);
        let call = colors.mnemonic_color(FlowKind::Call);
        let ret = colors.mnemonic_color(FlowKind::Return);

        assert_ne!(normal, jump);
        assert_ne!(jump, call);
        assert_ne!(call, ret);
    }

    #[test]
    fn test_arrow_color() {
        let colors = DisasmColors::default();
        let jump_color = colors.arrow_color(FlowKind::Jump);
        let call_color = colors.arrow_color(FlowKind::Call);
        // Should have different colors for different flow types.
        assert_ne!(jump_color, call_color);
    }

    #[test]
    fn test_block_tint() {
        let colors = DisasmColors::default();
        let tint0 = colors.block_tint(0);
        let tint1 = colors.block_tint(1);
        // Block tints should differ between adjacent blocks.
        assert!(tint0 != tint1 || tint0[3] == 0.0);
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
        assert!(
            arrows.is_empty() || arrows.len() <= 2,
            "Expected 0-2 arrows, got {}",
            arrows.len()
        );
    }

    #[test]
    fn test_operand_tokenizer_registers() {
        let tokens: Vec<_> = OperandTokenizer::new("rax, rbx").collect();
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Register));
    }

    #[test]
    fn test_operand_tokenizer_numbers() {
        let tokens: Vec<_> = OperandTokenizer::new("0x1234").collect();
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Number));
    }

    #[test]
    fn test_operand_tokenizer_memory() {
        let tokens: Vec<_> = OperandTokenizer::new("[rsp+8]").collect();
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Memory));
    }

    #[test]
    fn test_classify_operand_register() {
        assert_eq!(classify_operand_token("rax"), TokenKind::Register);
        assert_eq!(classify_operand_token("xmm0"), TokenKind::Register);
        assert_eq!(classify_operand_token("RAX"), TokenKind::Register);
    }

    #[test]
    fn test_classify_operand_number() {
        assert_eq!(classify_operand_token("0x1234"), TokenKind::Number);
        assert_eq!(classify_operand_token("100"), TokenKind::Number);
        assert_eq!(classify_operand_token("FFh"), TokenKind::Number);
    }

    #[test]
    fn test_classify_operand_size() {
        assert_eq!(classify_operand_token("qword"), TokenKind::Memory);
        assert_eq!(classify_operand_token("ptr"), TokenKind::Memory);
    }

    #[test]
    fn test_column_widths_default() {
        let cols = ColumnWidths::default();
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
        // Explicit 0x prefix → hex.
        assert_eq!(parse_address("0x401000"), Some(0x401000));
        assert_eq!(parse_address("0X401000"), Some(0x401000));
        // No prefix, no hex letters → decimal.
        assert_eq!(parse_address("256"), Some(256));
        assert_eq!(parse_address("4080"), Some(4080));
        assert_eq!(parse_address("401000"), Some(401000));
        // No prefix, contains a hex letter → hex.
        assert_eq!(parse_address("4abc"), Some(0x4abc));
        assert_eq!(parse_address("DEAD"), Some(0xDEAD));
        assert_eq!(parse_address("cafef00d"), Some(0xcafef00d));
        // Whitespace is trimmed.
        assert_eq!(parse_address("  0xff  "), Some(0xff));
        // Garbage → None.
        assert_eq!(parse_address("hello"), None);
        assert_eq!(parse_address(""), None);
    }

    #[test]
    fn test_arrow_depth_assignment() {
        // Create instructions with nested branches.
        let mut p = VecDisasmProvider::new();
        for i in 0..10 {
            let mut entry = InstructionEntry::new(0x1000 + i * 2, vec![0x90], "nop", "");
            entry.flow_kind = FlowKind::Normal;
            p.push(entry);
        }
        // Add two overlapping jumps.
        p.instructions_mut()[2] = InstructionEntry::new(0x1004, vec![0xEB, 0x08], "jmp", "0x100E")
            .with_flow(FlowKind::Jump)
            .with_target(0x100E);
        p.instructions_mut()[1] = InstructionEntry::new(0x1002, vec![0x74, 0x0C], "je", "0x1010")
            .with_flow(FlowKind::Jump)
            .with_target(0x1010);

        let instrs: Vec<&dyn Instruction> = (0..p.instruction_count())
            .filter_map(|i| p.instruction(i))
            .collect();
        let arrows = compute_arrows(&instrs, 0, instrs.len());

        // If both targets are in range, should have different depths.
        if arrows.len() >= 2 {
            assert_ne!(
                arrows[0].depth, arrows[1].depth,
                "Overlapping arrows should have different depths"
            );
        }
    }

    // ── Property-based tests ─────────────────────────────────────────────

    use proptest::prelude::*;

    proptest! {
        /// `parse_address` accepts arbitrary strings without panicking.
        #[test]
        fn prop_parse_address_never_panics(s in ".{0,32}") {
            let _ = parse_address(&s);
        }

        /// Hex-prefixed addresses round-trip cleanly.
        #[test]
        fn prop_parse_address_hex_roundtrips(value in any::<u64>()) {
            let s = format!("0x{value:X}");
            prop_assert_eq!(parse_address(&s), Some(value));
        }
    }

    // ── OperandTokenizer edge cases ──────────────────────────────────────

    fn tokens_of(s: &str) -> Vec<(String, TokenKind)> {
        OperandTokenizer::new(s)
            .map(|t| (t.text.to_string(), t.kind))
            .collect()
    }

    #[test]
    fn tokenizer_empty_input() {
        assert!(tokens_of("").is_empty());
    }

    #[test]
    fn tokenizer_only_punctuation_collapses() {
        // Run of `, +-*: ` is consumed as a single Plain token.
        let toks = tokens_of(",,, ");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].0, ",,, ");
        assert_eq!(toks[0].1, TokenKind::Plain);
    }

    #[test]
    fn tokenizer_trailing_comma() {
        // `rax,` → Register("rax"), Plain(",").
        let toks = tokens_of("rax,");
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[0].0, "rax");
        assert_eq!(toks[0].1, TokenKind::Register);
        assert_eq!(toks[1].0, ",");
        assert_eq!(toks[1].1, TokenKind::Plain);
    }

    #[test]
    fn tokenizer_two_operands() {
        // `rax, rbx` splits into reg, plain (", "), reg.
        let toks = tokens_of("rax, rbx");
        assert_eq!(toks.len(), 3);
        assert_eq!(toks[0].1, TokenKind::Register);
        assert_eq!(toks[1].1, TokenKind::Plain);
        assert_eq!(toks[2].1, TokenKind::Register);
        assert_eq!(toks[2].0, "rbx");
    }

    #[test]
    fn tokenizer_memory_brackets() {
        // `[rsp+8]` → `[`, `rsp`, `+`, `8`, `]`.
        let toks = tokens_of("[rsp+8]");
        let kinds: Vec<TokenKind> = toks.iter().map(|t| t.1).collect();
        let texts: Vec<&str> = toks.iter().map(|t| t.0.as_str()).collect();
        assert_eq!(texts, vec!["[", "rsp", "+", "8", "]"]);
        assert_eq!(
            kinds,
            vec![
                TokenKind::Memory,
                TokenKind::Register,
                TokenKind::Plain,
                TokenKind::Number,
                TokenKind::Memory,
            ]
        );
    }

    #[test]
    fn tokenizer_nested_brackets_size_keyword() {
        // `qword ptr [rax + 0x10]` exercises size keywords + memory + register + hex.
        let toks = tokens_of("qword ptr [rax + 0x10]");
        let kinds: Vec<TokenKind> = toks.iter().map(|t| t.1).collect();
        // qword/ptr classify as Memory; rax Register; 0x10 Number; brackets Memory.
        assert!(kinds.contains(&TokenKind::Memory));
        assert!(kinds.contains(&TokenKind::Register));
        assert!(kinds.contains(&TokenKind::Number));
        assert_eq!(toks.first().unwrap().0, "qword");
        assert_eq!(toks.last().unwrap().0, "]");
        assert_eq!(toks.last().unwrap().1, TokenKind::Memory);
    }

    #[test]
    fn tokenizer_unterminated_string() {
        // Missing closing quote: tokenizer must consume to end-of-input, not panic.
        let toks = tokens_of("\"hello world");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].1, TokenKind::String);
        assert_eq!(toks[0].0, "\"hello world");
    }

    #[test]
    fn tokenizer_hex_suffix_h() {
        // MASM-style `1Fh` is classified as a number.
        let toks = tokens_of("1Fh");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].1, TokenKind::Number);
    }

    #[test]
    fn tokenizer_unknown_word_is_plain() {
        // `gibberish` is not a register, not a number → Plain.
        let toks = tokens_of("gibberish");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].1, TokenKind::Plain);
    }
}
