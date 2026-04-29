//! Drawing routines for [`super::DisasmView`] — header, instruction rows
//! (with selection / hover / breakpoint / tooltip), syntax-colored operands,
//! and L-shaped branch arrows.

use super::config::{DisasmColors, FlowKind, Instruction, MAX_ARROW_DEPTH};
use super::tokens::{OperandTokenizer, TokenKind};
use super::{DisasmView, EditColumn, col32};
use crate::utils::hex::byte_hex;

impl DisasmView {
    pub(super) fn draw_header(
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
    pub(super) fn draw_instruction_row(
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
                draw_list
                    .add_rect([origin_x, y], [origin_x + win_w, y + lh], col32(tint))
                    .filled(true)
                    .build();
            }
        }

        // ── Current execution highlight ───────────────────────
        if instr.is_current() {
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32(colors.current_line_bg),
                )
                .filled(true)
                .build();
        }

        // ── Selection highlight ───────────────────────────────
        let is_selected = self.selection.contains(&idx);
        let is_cursor = self.cursor_idx == Some(idx);
        if is_selected {
            // Brighter for cursor row, dimmer for other selected rows.
            let alpha = if is_cursor { 0.55 } else { 0.35 };
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32([
                        colors.selection_bg[0],
                        colors.selection_bg[1],
                        colors.selection_bg[2],
                        alpha,
                    ]),
                )
                .filled(true)
                .build();
        } else if is_cursor {
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32(colors.selection_bg),
                )
                .filled(true)
                .build();
        }

        // ── Row hover ─────────────────────────────────────────
        let row_hovered = mouse_pos[1] >= y
            && mouse_pos[1] < y + lh
            && mouse_pos[0] >= origin_x
            && mouse_pos[0] < origin_x + win_w;
        if row_hovered && !is_selected && !is_cursor {
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32(colors.hover_bg),
                )
                .filled(true)
                .build();
        }

        let mut x = origin_x;

        // ── Breakpoint margin (numbered, colored) ──────────────
        if cfg.show_breakpoints {
            let bp_num = instr.breakpoint_number();
            if bp_num > 0 {
                let bp_color = colors.bp_color(bp_num);
                // Background tint for the gutter cell.
                draw_list
                    .add_rect(
                        [x, y],
                        [x + cols.margin, y + lh],
                        col32([
                            bp_color[0] * 0.3,
                            bp_color[1] * 0.3,
                            bp_color[2] * 0.3,
                            0.35,
                        ]),
                    )
                    .filled(true)
                    .build();
                // Numbered label (centered).
                let label = format!("{}", bp_num);
                let text_w = label.len() as f32 * self.char_advance;
                let tx = x + (cols.margin - text_w) * 0.5;
                let text_h = self.line_height - 4.0;
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
            if cfg.uppercase {
                format!("{:016X}", addr)
            } else {
                format!("{:016x}", addr)
            }
        } else if cfg.uppercase {
            format!("{:08X}", addr)
        } else {
            format!("{:08x}", addr)
        };
        draw_list.add_text([x, y], col32(colors.address), &addr_str);
        x += cols.address;

        // ── Bytes column (with inline InputText edit) ──────────
        if cfg.show_bytes {
            let is_editing_bytes = self
                .edit
                .as_ref()
                .map(|e| e.idx == idx && e.column == EditColumn::Bytes)
                .unwrap_or(false);

            if is_editing_bytes {
                self.edit_render_pos.set(Some([x, y]));
                self.edit_render_width.set(cols.bytes);

                // Draw placeholder background so it's visible.
                draw_list
                    .add_rect(
                        [x - 2.0, y],
                        [x + cols.bytes, y + lh],
                        col32([0.20, 0.15, 0.08, 0.95]),
                    )
                    .filled(true)
                    .build();
                draw_list
                    .add_rect(
                        [x - 2.0, y],
                        [x + cols.bytes, y + lh],
                        col32([1.0, 0.7, 0.3, 0.80]),
                    )
                    .build();
            } else {
                // Reuse a single buffer (3 chars/byte) instead of N per-byte
                // String allocations from `format!` in a `map().collect()`.
                let bytes = instr.bytes();
                let mut bytes_str = String::with_capacity(bytes.len() * 3);
                for (i, b) in bytes.iter().enumerate() {
                    if i > 0 {
                        bytes_str.push(' ');
                    }
                    bytes_str.push_str(byte_hex(*b, cfg.uppercase));
                }
                draw_list.add_text([x, y], col32(colors.bytes), &bytes_str);
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
            crate::utils::themed_tooltip(ui, || {
                ui.text(format!("Address: 0x{:016X}", addr));
                if addr <= 0xFFFF_FFFF {
                    ui.text(format!("     32: 0x{:08X}", addr as u32));
                }

                let bytes = instr.bytes();
                ui.text(format!("Size: {} bytes", bytes.len()));
                let hex_str: String = bytes
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                ui.text(format!("Bytes: {}", hex_str));

                ui.text(format!("Instr: {} {}", instr.mnemonic(), instr.operands()));

                let flow_desc = match instr.flow_kind() {
                    FlowKind::Normal => "Normal (sequential)",
                    FlowKind::Jump => "Jump (conditional/unconditional)",
                    FlowKind::Call => "Call (function call)",
                    FlowKind::Return => "Return (function epilogue)",
                    FlowKind::Nop => "NOP / padding",
                    FlowKind::Stack => "Stack operation (push/pop/sub rsp)",
                    FlowKind::System => "System (syscall/int/sysenter)",
                    FlowKind::Invalid => "INVALID (undecodable)",
                };
                ui.text(format!("Flow: {}", flow_desc));

                if let Some(target) = instr.branch_target() {
                    ui.text(format!("Target: 0x{:X}", target));
                    let offset = target as i64 - addr as i64;
                    if offset >= 0 {
                        ui.text(format!(
                            "Offset: +0x{:X} ({} bytes forward)",
                            offset, offset
                        ));
                    } else {
                        ui.text(format!("Offset: -0x{:X} ({} bytes back)", -offset, -offset));
                    }
                }

                ui.text(format!("Block: {}", instr.block_index()));

                if instr.has_breakpoint() {
                    let bp_num = instr.breakpoint_number();
                    if bp_num > 0 {
                        ui.text(format!("Breakpoint: #{}", bp_num));
                    } else {
                        ui.text("Breakpoint: YES");
                    }
                }

                if instr.is_current() {
                    ui.text(">> CURRENT INSTRUCTION POINTER <<");
                }

                if let Some(comment) = instr.comment() {
                    ui.text(format!("Comment: {}", comment));
                }
            });
        }
    }

    /// Draw operand string with basic syntax coloring.
    pub(super) fn draw_colored_operands(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        x: f32,
        y: f32,
        operands: &str,
        colors: &DisasmColors,
    ) {
        let cw = self.char_advance;
        let mut cx = x;

        for token in OperandTokenizer::new(operands) {
            let color = match token.kind {
                TokenKind::Register => colors.operand_register,
                TokenKind::Number => colors.operand_number,
                TokenKind::Memory => colors.operand_memory,
                TokenKind::String => colors.operand_string,
                TokenKind::Plain => colors.operand_default,
            };
            draw_list.add_text([cx, y], col32(color), token.text);
            cx += token.text.len() as f32 * cw;
        }
    }

    /// Draw branch arrows between instructions.
    pub(super) fn draw_arrows(
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
            + if self.config.show_breakpoints {
                cols.margin
            } else {
                0.0
            }
            + cols.arrows;
        let depth_spacing = cols.arrows / (MAX_ARROW_DEPTH as f32 + 1.0);

        for arrow in &self.cached_arrows {
            let from_y = origin_y + (arrow.from_idx) as f32 * lh + lh * 0.5;
            let to_y = origin_y + (arrow.to_idx) as f32 * lh + lh * 0.5;
            let x = arrow_base_x - (arrow.depth as f32 + 1.0) * depth_spacing;

            let color = col32(colors.arrow_color(arrow.flow_kind));
            let thickness = if arrow.depth == 0 { 1.5 } else { 1.0 };

            // Horizontal from source to vertical line.
            draw_list
                .add_line([arrow_base_x, from_y], [x, from_y], color)
                .thickness(thickness)
                .build();
            // Vertical line.
            draw_list
                .add_line([x, from_y], [x, to_y], color)
                .thickness(thickness)
                .build();
            // Horizontal to target.
            draw_list
                .add_line([x, to_y], [arrow_base_x, to_y], color)
                .thickness(thickness)
                .build();

            // Arrowhead at target.
            let dir = if to_y > from_y { 1.0 } else { -1.0 };
            let head_size = 4.0;
            draw_list
                .add_triangle(
                    [arrow_base_x, to_y],
                    [arrow_base_x - head_size, to_y - head_size * dir],
                    [arrow_base_x - head_size, to_y + head_size * dir],
                    color,
                )
                .filled(true)
                .build();
        }
    }
}
