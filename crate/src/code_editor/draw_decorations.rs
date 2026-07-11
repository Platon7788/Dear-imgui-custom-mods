//! Line-decoration draw passes for [`CodeEditor`].
//!
//! Five per-sub-row passes that consume [`crate::code_editor::LineAnnotation`]
//! data and render it via `DrawList`. Called from
//! [`crate::code_editor::CodeEditor::draw_visible_lines`] at fixed z-order
//! slots — the `y` here is the top of the current visual sub-row.
//!
//! # Z-order within the sub-row
//!
//! ```text
//!   [ selection ]  [ find highlight ]      ← handled elsewhere
//!   [   Wash    ]                          ← this file, BEHIND tokens
//!   [  tokens   ]                          ← handled elsewhere
//!   [   Rule    ]                          ← this file, ON tokens
//!   [   Ghost   ]                          ← this file, dim text
//!   [ bracket   ]                          ← handled elsewhere
//!   [ EndPill   ]                          ← this file, last sub-row only
//!   [ captions  ]                          ← this file, into strip band
//!   [  cursor   ]                          ← always on top
//! ```
//!
//! All position math uses **char columns** (not bytes), same unit as
//! `CursorPos.col`. `col_to_x` in `helpers.rs` is tab-aware — decorations
//! stay aligned with tokens even in tab-indented lines.
//!
//! Passes are no-ops when the line is not in `annotated_lines` (checked
//! by the caller). They also silently skip decorations that clip outside
//! the sub-row column range, so word-wrap doesn't need special handling
//! at the call site — every pass is invoked per sub-row and consumes
//! only the portion it should show.

use super::col32;
use super::decoration::{Decoration, clip_range_to_sub_row, last_non_whitespace_col};
use super::helpers::col_to_x;

use crate::utils::color::with_alpha;

impl super::CodeEditor {
    /// Draw `Wash` rectangles on this sub-row. Semi-transparent, painted
    /// BEHIND tokens — call from `draw_visible_lines` after selection
    /// / find highlights, before token text.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_wash_pass(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        let Some(annotations) = self.annotations_for(line_idx) else {
            return;
        };
        let base_x = col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
        for deco in &annotations.decorations {
            let Decoration::Wash {
                col_start: dc,
                col_len,
                color,
                ..
            } = deco
            else {
                continue;
            };
            let Some((cs, cl)) = clip_range_to_sub_row(*dc, *col_len, col_start, col_end) else {
                continue;
            };
            let x1 = text_start_x + col_to_x(line_str, cs, self.char_advance, self.config.tab_size)
                - base_x;
            let x2 = text_start_x
                + col_to_x(line_str, cs + cl, self.char_advance, self.config.tab_size)
                - base_x;
            // Force translucent alpha regardless of the source colour —
            // a Wash rendered opaque would blot out the tokens it's meant
            // to background.
            let resolved = color.resolve(&self.config.colors);
            let wash = with_alpha(resolved, 0.14);
            draw_list
                .add_rect(
                    [x1, y + self.text_baseline_dy],
                    [x2, y + self.line_height],
                    col32(wash),
                )
                .filled(true)
                .build();
        }
    }

    /// Draw `Rule` bars on this sub-row — thin colored lines drawn ON
    /// the text baseline (highlighter-style). Called after token text
    /// so the rule sits above the glyphs.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_rule_pass(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        let Some(annotations) = self.annotations_for(line_idx) else {
            return;
        };
        let base_x = col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
        // Rule thickness scales with text-line height so it stays visible
        // at larger font zoom without dominating small-font rendering.
        let rule_h = (self.text_line_height * 0.10).clamp(2.0, 3.0);
        // Sit the rule at the very bottom of the text portion of the row.
        let rule_y = y + self.text_baseline_dy + self.text_line_height - rule_h;
        for deco in &annotations.decorations {
            let Decoration::Rule {
                col_start: dc,
                col_len,
                color,
                ..
            } = deco
            else {
                continue;
            };
            let Some((cs, cl)) = clip_range_to_sub_row(*dc, *col_len, col_start, col_end) else {
                continue;
            };
            let x1 = text_start_x + col_to_x(line_str, cs, self.char_advance, self.config.tab_size)
                - base_x;
            let x2 = text_start_x
                + col_to_x(line_str, cs + cl, self.char_advance, self.config.tab_size)
                - base_x;
            let resolved = color.resolve(&self.config.colors);
            draw_list
                .add_rect([x1, rule_y], [x2, rule_y + rule_h], col32(resolved))
                .filled(true)
                .build();
        }
    }

    /// Draw `Ghost` pseudo-text on this sub-row — dimmed hint drawn
    /// AFTER `col_start`. Only rendered on the sub-row that contains
    /// `col_start`; other sub-rows skip.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_ghost_pass(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        let Some(annotations) = self.annotations_for(line_idx) else {
            return;
        };
        let base_x = col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
        for deco in &annotations.decorations {
            let Decoration::Ghost {
                col_start: gc,
                text,
                color,
            } = deco
            else {
                continue;
            };
            if *gc < col_start || *gc >= col_end {
                continue;
            }
            let x = text_start_x + col_to_x(line_str, *gc, self.char_advance, self.config.tab_size)
                - base_x;
            // Ghost is by definition semi-transparent — even a bright
            // source colour must read as a suggestion, not a real token.
            let ghost_color = with_alpha(color.resolve(&self.config.colors), 0.55);
            draw_list.add_text(
                [x, y + self.text_baseline_dy],
                col32(ghost_color),
                text.as_str(),
            );
        }
    }

    /// Draw `EndPill`s at the tail of the LAST visual sub-row of the line.
    /// Positioned two `char_advance`s to the right of the last non-whitespace
    /// glyph, so the pill hugs the visible text.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_end_pill_pass(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
    ) {
        let Some(annotations) = self.annotations_for(line_idx) else {
            return;
        };
        // Anchor after the last visible glyph on the full line, not on
        // the sub-row (a wrapped line still has one pill, positioned at
        // the end of its last sub-row).
        let last_col = last_non_whitespace_col(line_str);
        if last_col == 0
            && !annotations
                .decorations
                .iter()
                .any(|d| matches!(d, Decoration::EndPill { .. }))
        {
            // Empty line + no pill → nothing to do (fast path).
            return;
        }
        let base_x = col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
        let mut anchor_x = text_start_x
            + col_to_x(line_str, last_col, self.char_advance, self.config.tab_size)
            - base_x
            + self.char_advance * 2.0;

        for deco in &annotations.decorations {
            let Decoration::EndPill {
                text,
                fg,
                bg,
                border,
                ..
            } = deco
            else {
                continue;
            };
            let text_w = text.chars().count() as f32 * self.char_advance;
            // Pill hugs the text portion vertically — leaves the annotation
            // strip clean and never eats into the caption band.
            let pad_x = self.char_advance * 0.6;
            let pill_top = y + self.text_baseline_dy + 1.0;
            let pill_bot = y + self.line_height - 1.0;
            let x1 = anchor_x;
            let x2 = anchor_x + text_w + pad_x * 2.0;
            let radius = (pill_bot - pill_top) * 0.5;
            let bg_c = bg.resolve(&self.config.colors);
            let border_c = border.resolve(&self.config.colors);
            let fg_c = fg.resolve(&self.config.colors);
            draw_list
                .add_rect([x1, pill_top], [x2, pill_bot], col32(bg_c))
                .filled(true)
                .rounding(radius)
                .build();
            draw_list
                .add_rect([x1, pill_top], [x2, pill_bot], col32(border_c))
                .rounding(radius)
                .build();
            draw_list.add_text(
                [x1 + pad_x, y + self.text_baseline_dy],
                col32(fg_c),
                text.as_str(),
            );
            // Stack multiple pills side-by-side when a line carries more
            // than one — a small gap keeps them visually distinct.
            anchor_x = x2 + self.char_advance * 0.6;
        }
    }

    /// Render `Rule` / `Wash` captions inside the annotation strip band.
    /// No-op when the strip is [`crate::code_editor::AnnotationStrip::Off`]
    /// or when the caption is `None`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_captions_pass(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        use crate::code_editor::AnnotationStrip;
        let strip = self.config.annotation_strip;
        if matches!(strip, AnnotationStrip::Off) {
            return;
        }
        let Some(annotations) = self.annotations_for(line_idx) else {
            return;
        };
        // Caption band Y — either the top strip (Above) or the bottom
        // strip (Below) — always the same height as the reserved band.
        let caption_y = match strip {
            AnnotationStrip::Above(_) => y + 1.0,
            AnnotationStrip::Below(_) => y + self.text_baseline_dy + self.text_line_height + 1.0,
            AnnotationStrip::Off => return,
        };
        let base_x = col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);

        for deco in &annotations.decorations {
            // Only Wash/Rule carry captions.
            let (dc, dlen, caption, color) = match deco {
                Decoration::Wash {
                    col_start: dc,
                    col_len,
                    caption: Some(caption),
                    color,
                    ..
                }
                | Decoration::Rule {
                    col_start: dc,
                    col_len,
                    caption: Some(caption),
                    color,
                    ..
                } => (*dc, *col_len, caption.as_str(), *color),
                _ => continue,
            };
            let Some((cs, cl)) = clip_range_to_sub_row(dc, dlen, col_start, col_end) else {
                continue;
            };
            let x1 = text_start_x + col_to_x(line_str, cs, self.char_advance, self.config.tab_size)
                - base_x;
            let x2 = text_start_x
                + col_to_x(line_str, cs + cl, self.char_advance, self.config.tab_size)
                - base_x;
            // Centre the caption horizontally over the segment; truncate
            // via clip when it overflows.
            let text_w = caption.chars().count() as f32 * self.char_advance;
            let cx = if text_w >= x2 - x1 {
                x1
            } else {
                x1 + ((x2 - x1) - text_w) * 0.5
            };
            let caption_color = with_alpha(color.resolve(&self.config.colors), 0.90);
            draw_list.add_text([cx, caption_y], col32(caption_color), caption);
        }
    }

    /// Fast lookup — returns the [`LineAnnotation`] entry for `line_idx`,
    /// or `None` when the line has no decorations.
    #[inline]
    fn annotations_for(&self, line_idx: usize) -> Option<&super::decoration::LineAnnotation> {
        if !self.annotated_lines.contains(&line_idx) {
            return None;
        }
        self.line_annotations.iter().find(|a| a.line == line_idx)
    }
}
