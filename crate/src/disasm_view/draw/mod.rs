//! Drawing routines for [`super::DisasmView`] — header, instruction rows
//! (with selection / hover / breakpoint / tooltip), syntax-colored operands,
//! and L-shaped branch arrows.
//!
//! Split into cohesive sub-modules (each `use super::*;` + `impl DisasmView`):
//! - [`rows`] — per-row paint + the comprehensive hover tooltip.
//! - [`operands`] — syntax-colored operand run + the pure width helper.
//! - [`arrows`] — L-shaped branch-arrow geometry.
//!
//! This `mod.rs` carries the shared layout constants, the column-header
//! and column-divider painters, and the pure `split_operand_list`
//! scanner (with its unit tests) used by the tooltip's operand decoder.

use super::DisasmView;
use super::config::DisasmColors;
use crate::utils::color::col32;

mod arrows;
mod operands;
mod rows;
mod tooltip;

/// Pixel offset added to the X position of the comment column so
/// it doesn't sit flush against the operands column. User
/// requested ~5 px on 2026-04-29 — gives the comment text breathing
/// room without widening the comment column itself.
const COMMENT_LEFT_PAD: f32 = 5.0;

/// Inner left padding for the Bytes and Instruction columns —
/// keeps their data text from sitting flush against the column
/// divider on the left. ~1 char advance worth of breathing room.
/// Right edge has its own slack via the column-width slack
/// (typical bytes string < 180 px column, mnemonic < 70 px column).
pub(in crate::disasm_view) const COL_INNER_PAD: f32 = 6.0;

/// Cushion the dynamic comment X keeps from the longest visible
/// instruction text — when an overflowing operand string pushes the
/// comment column right, the divider lands `COMMENT_GAP` pixels past
/// the rightmost glyph so the divider never hugs the text.
pub(in crate::disasm_view) const COMMENT_GAP: f32 = 10.0;

/// Background tint of the inline edit cell (Bytes / Comment) — warm
/// brown so the user sees instantly which cell is being edited.
/// Theme-independent on purpose: this is a transient, modal-ish UI
/// affordance that needs to read consistently across all themes.
pub(super) const EDIT_CELL_BG: [f32; 4] = [0.20, 0.15, 0.08, 0.95];

/// Border colour of the inline edit cell — same warm-amber accent
/// the cursor / focus highlight uses.
pub(super) const EDIT_CELL_BORDER: [f32; 4] = [1.0, 0.7, 0.3, 0.80];

/// Split a disassembler-style operand list `"reg, [base+idx*8], imm"`
/// into trimmed top-level slices. Commas inside `[...]` are part of
/// the inner expression and **must not** trigger a split — the
/// scanner tracks bracket depth.
///
/// Pure (no ImGui context) — covered by the `split_*` unit tests at
/// the bottom of this module.
fn split_operand_list(operands: &str) -> Vec<&str> {
    let mut depth: i32 = 0;
    let mut start = 0usize;
    let bytes = operands.as_bytes();
    let mut out: Vec<&str> = Vec::new();
    for (idx, &b) in bytes.iter().enumerate() {
        match b {
            b'[' => depth += 1,
            b']' => depth -= 1,
            b',' if depth == 0 => {
                let slice = operands[start..idx].trim();
                if !slice.is_empty() {
                    out.push(slice);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let tail = operands[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

/// Horizontal span (px) of the visual "Instruction" column for header
/// centring — the gap between the start of the mnemonic and the
/// comment column, floored at the configured `mnemonic + operands`
/// widths so the centring stays sane when the comment column hasn't
/// slid right. Pure → unit-tested.
fn instruction_span(start_x: f32, comment_x: f32, min_span: f32) -> f32 {
    (comment_x - start_x).max(min_span)
}

impl DisasmView {
    pub(in crate::disasm_view) fn draw_header(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
        comment_x: f32,
    ) {
        let cols = &self.config.columns;
        let hdr_col = col32(self.config.colors.header);
        let cw = self.char_advance;
        let mut x = origin_x;

        if self.config.show_breakpoints {
            x += cols.margin;
        }
        if self.config.show_arrows {
            x += cols.arrows;
        }

        // Address — left-aligned (matches the addresses below it).
        draw_list.add_text([x, y], hdr_col, "Address");
        x += cols.address;

        // Bytes — centred within the column for visual balance with
        // the now-spaced data. `centred_x` clamps to the inner-pad
        // boundary so the header never overlaps the left divider.
        if self.config.show_bytes {
            let label = "Bytes";
            let text_w = label.len() as f32 * cw;
            let cx = x + ((cols.bytes - text_w) * 0.5).max(COL_INNER_PAD);
            draw_list.add_text([cx, y], hdr_col, label);
            x += cols.bytes;
        }

        // Instruction — centred across the combined mnemonic + operands
        // span. Stops at `comment_x` (which may have slid right beyond
        // the default span) so the centring still tracks the visible
        // column width.
        let span = instruction_span(x, comment_x, cols.mnemonic + cols.operands);
        let label = "Instruction";
        let text_w = label.len() as f32 * cw;
        let cx = x + ((span - text_w) * 0.5).max(COL_INNER_PAD);
        draw_list.add_text([cx, y], hdr_col, label);

        if self.config.show_comments {
            draw_list.add_text([comment_x + COMMENT_LEFT_PAD, y], hdr_col, "Comment");
        }
    }

    /// Draw thin vertical dividers between the address / bytes /
    /// instruction / comment columns. Same `colors.separator` with
    /// alpha 0.40 treatment as `hex_viewer`'s column dividers — the
    /// lines read as a gentle visual cue, not heavy borders.
    ///
    /// Skipped for the leftmost (margin / arrows) gutters because
    /// those areas already have their own visual identity (numbered
    /// breakpoint cells, branch arrows). The first divider sits
    /// between the address column and the bytes column.
    pub(in crate::disasm_view) fn draw_column_dividers(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        div_top: f32,
        div_bot: f32,
        comment_x: f32,
    ) {
        let cols = &self.config.columns;
        let c = self.config.colors.separator;
        let div_col = col32([c[0], c[1], c[2], c[3] * 0.40]);

        let mut x = origin_x;
        if self.config.show_breakpoints {
            x += cols.margin;
        }
        if self.config.show_arrows {
            x += cols.arrows;
        }
        x += cols.address;

        let emit = |dx: f32| {
            draw_list
                .add_line([dx, div_top], [dx, div_bot], div_col)
                .thickness(1.0)
                .build();
        };

        // Divider 1 — between address and bytes (only when bytes
        // column is visible; otherwise the next visible column starts
        // right after address).
        if self.config.show_bytes {
            emit(x);
            x += cols.bytes;
        }

        // Divider 2 — between bytes and instruction (mnemonic).
        emit(x);

        // Divider 3 — between instruction and comment, only when
        // comment column is visible. Sits at the per-frame dynamic
        // `comment_x` so the divider follows whenever the
        // instruction text would have collided with the default
        // column boundary.
        if self.config.show_comments {
            emit(comment_x);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_simple_two_operands() {
        let out = split_operand_list("rax, rbx");
        assert_eq!(out, vec!["rax", "rbx"]);
    }

    #[test]
    fn split_keeps_commas_inside_brackets() {
        // The comma inside `[base+idx*8]` must NOT split.
        let out = split_operand_list("rax, [rbx+rcx*8], 0x10");
        assert_eq!(out, vec!["rax", "[rbx+rcx*8]", "0x10"]);
    }

    #[test]
    fn split_trims_surrounding_whitespace() {
        let out = split_operand_list("  rax ,  rbx  ");
        assert_eq!(out, vec!["rax", "rbx"]);
    }

    #[test]
    fn split_empty_yields_no_slices() {
        assert!(split_operand_list("").is_empty());
        assert!(split_operand_list("   ").is_empty());
    }

    #[test]
    fn split_drops_empty_between_commas() {
        // `a,,b` — the empty middle slice is dropped, not emitted.
        let out = split_operand_list("a,,b");
        assert_eq!(out, vec!["a", "b"]);
    }

    #[test]
    fn split_single_operand_no_comma() {
        let out = split_operand_list("qword ptr [rsp+0x10]");
        assert_eq!(out, vec!["qword ptr [rsp+0x10]"]);
    }

    #[test]
    fn split_nested_brackets_balance() {
        // Two top-level operands, each with bracketed inner commas.
        let out = split_operand_list("[a, b], [c, d]");
        assert_eq!(out, vec!["[a, b]", "[c, d]"]);
    }

    #[test]
    fn instruction_span_floors_at_min() {
        // comment_x left of start → span clamps to the configured min.
        assert_eq!(instruction_span(100.0, 50.0, 300.0), 300.0);
    }

    #[test]
    fn instruction_span_tracks_slid_comment() {
        // comment_x slid right beyond the default span → use the gap.
        assert_eq!(instruction_span(100.0, 600.0, 300.0), 500.0);
    }
}
