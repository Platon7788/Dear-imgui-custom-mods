//! Syntax-colored operand run for [`super::super::DisasmView`].
//!
//! Maps each [`OperandTokenizer`] token to a theme colour and lays the
//! tokens out left-to-right at the monospace advance. The per-token
//! horizontal advance is factored into the pure [`token_advance`]
//! helper so the glyph-count math (UTF-8 safe) is unit-testable
//! without an ImGui context.

use super::*;
use crate::disasm_view::tokens::{OperandTokenizer, TokenKind};

/// Horizontal advance (px) for a monospace token: glyph (codepoint)
/// count × char advance.
///
/// Audit fix: the renderer previously advanced by `text.len()` which
/// is the **byte** length, not the glyph count. For ASCII operands
/// (x86 / x86-64 / ARM register + immediate text) the two agree, but a
/// `TokenKind::String` literal can carry multi-byte UTF-8 (e.g. a
/// referenced wide-string fragment), which over-advanced the cursor by
/// the extra continuation bytes and left a visible gap before the next
/// token. `chars().count()` is the correct monospace measure.
pub(super) fn token_advance(text: &str, char_advance: f32) -> f32 {
    text.chars().count() as f32 * char_advance
}

impl DisasmView {
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
            cx += token_advance(token.text, cw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::token_advance;

    #[test]
    fn ascii_advance_is_byte_len() {
        // "rax" — 3 ASCII glyphs.
        assert_eq!(token_advance("rax", 8.0), 24.0);
    }

    #[test]
    fn utf8_advance_counts_glyphs_not_bytes() {
        // "é" is 2 bytes but 1 glyph — advance must be ONE char width,
        // not two. This is the exact bug the helper fixes.
        assert_eq!(token_advance("é", 8.0), 8.0);
        // "café" — 4 glyphs, 5 bytes.
        assert_eq!(token_advance("café", 8.0), 32.0);
    }

    #[test]
    fn empty_token_zero_advance() {
        assert_eq!(token_advance("", 8.0), 0.0);
    }
}
