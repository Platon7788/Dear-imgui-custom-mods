//! Renderer + measurer for [`RichHoverPayload`] tooltips.
//!
//! Used by [`super::render`] when the hovered decoration is a
//! [`Decoration::HoverZoneRich`](super::decoration::Decoration::HoverZoneRich).
//! Measurement runs first (via [`measure_rich_payload`]) so
//! `smart_positioned_tooltip` can pick a flip direction that fits the
//! full body without a scrollbar; the draw pass ([`draw_rich_payload`])
//! then paints header + focus + structure with real ImGui widgets.
//!
//! All measurements assume the current ImGui font — call inside the
//! same font stack as the draw (both callers already are).

use dear_imgui_rs::Ui;

use super::decoration::{
    RichHoverAlign, RichHoverFocus, RichHoverHeader, RichHoverPayload, RichHoverStructure,
};

/// Vertical gap between adjacent lines of body text inside the tooltip.
/// Matches `TOOLTIP_ITEM_SPACING.y` pushed by `themed_tooltip`.
const LINE_GAP: f32 = 4.0;

/// Extra vertical padding around a `ui.separator()`.
const SEP_GAP: f32 = 8.0;

/// Horizontal gap between adjacent columns in the structure table.
const COL_GAP: f32 = 18.0;

/// Left-side indent of the focus block's `key: value` rows.
const FOCUS_INDENT: f32 = 14.0;

/// Width of the fixed-width `key` column inside the focus block.
const FOCUS_KEY_W: f32 = 60.0;

/// Chip separator glyph between title / chips / trailing on the header.
const CHIP_SEP: &str = "·";

/// Header row spacing between title, chips, and trailing.
const HDR_GAP: f32 = 12.0;

/// Approximate ImGui `ItemSpacing.x` between adjacent inline items —
/// used by measurement passes to estimate widget widths without
/// materialising a formatted string just to hand to `calc_text_size`.
const ITEM_SPACING_X: f32 = 8.0;

/// Muted / dim text color used for column headers, focus keys, `unknown`.
const DIM: [f32; 4] = [0.52, 0.56, 0.63, 1.0];

/// Accent color for the focus arrow and focused-row index.
const ACCENT: [f32; 4] = [0.81, 1.0, 0.02, 1.0];

/// Background tint painted behind the focused row in the structure
/// table. Semi-transparent so the popup bg still shows through.
const FOCUS_BG: [f32; 4] = [0.36, 0.61, 0.83, 0.16];

/// Compute the total content height (in pixels) and the required
/// minimum width for a rich tooltip. Height feeds the `smart_positioned_tooltip`
/// flip decision; width becomes the tooltip's `min_size[0]` so the
/// title + chips row isn't visually cramped.
pub(super) fn measure_rich_payload(ui: &Ui, p: &RichHoverPayload) -> (f32, f32) {
    let line_h = ui.text_line_height() + LINE_GAP;
    let mut h = 0.0f32;
    let mut w = 0.0f32;

    // ── Header row ─────────────────────────────────────────────────
    if header_visible(&p.header) {
        h += ui.text_line_height();
        let hdr_w = measure_header_width(&p.header);
        w = w.max(hdr_w);
    }

    // ── Focus block ────────────────────────────────────────────────
    if focus_visible(&p.focus) {
        if header_visible(&p.header) {
            h += SEP_GAP;
        }
        h += line_h; // label row
        h += p.focus.rows.len() as f32 * line_h;
        let focus_w = measure_focus_width(&p.focus);
        w = w.max(FOCUS_INDENT + focus_w);
    }

    // ── Structure table ───────────────────────────────────────────
    if structure_visible(&p.structure) {
        if header_visible(&p.header) || focus_visible(&p.focus) {
            h += SEP_GAP;
        }
        if !p.structure.title.is_empty() {
            h += line_h;
        }
        // Column-header row + one row per data row.
        h += line_h; // column header
        h += p.structure.rows.len() as f32 * line_h;
        let struct_w = measure_structure_width(&p.structure);
        w = w.max(struct_w);
    }

    (h, w)
}

/// Paint the payload inside an already-open tooltip. Callers must
/// arrange for the tooltip window itself (font, popup, padding); this
/// function just walks the sections and emits widgets.
pub(super) fn draw_rich_payload(ui: &Ui, p: &RichHoverPayload) {
    let mut needs_sep = false;

    // ── Header ────────────────────────────────────────────────────
    if header_visible(&p.header) {
        draw_header(ui, &p.header);
        needs_sep = true;
    }

    // ── Focus block ───────────────────────────────────────────────
    if focus_visible(&p.focus) {
        if needs_sep {
            ui.separator();
        }
        draw_focus(ui, &p.focus);
        needs_sep = true;
    }

    // ── Structure table ───────────────────────────────────────────
    if structure_visible(&p.structure) {
        if needs_sep {
            ui.separator();
        }
        draw_structure(ui, &p.structure);
    }
}

// ─── Visibility helpers ─────────────────────────────────────────────

fn header_visible(h: &RichHoverHeader) -> bool {
    !h.title.is_empty() || !h.chips.is_empty() || !h.trailing.is_empty()
}
fn focus_visible(f: &RichHoverFocus) -> bool {
    !f.label.is_empty() || !f.rows.is_empty()
}
fn structure_visible(s: &RichHoverStructure) -> bool {
    !s.rows.is_empty()
}

// ─── Header ─────────────────────────────────────────────────────────

fn measure_header_width(h: &RichHoverHeader) -> f32 {
    let mut w = 0.0f32;
    if !h.title.is_empty() {
        w += crate::utils::text::calc_text_size(&h.title)[0];
    }
    for (k, v) in &h.chips {
        w += HDR_GAP;
        w += crate::utils::text::calc_text_size(CHIP_SEP)[0];
        w += HDR_GAP;
        w += crate::utils::text::calc_text_size(k)[0];
        w += crate::utils::text::calc_text_size(" ")[0];
        w += crate::utils::text::calc_text_size(v)[0];
    }
    if !h.trailing.is_empty() {
        w += HDR_GAP * 2.0;
        w += crate::utils::text::calc_text_size(&h.trailing)[0];
    }
    w
}

fn draw_header(ui: &Ui, h: &RichHoverHeader) {
    // Layout: title | [chip pairs] | ... spacer ... | trailing
    // Trailing is right-aligned via SameLine + explicit x.
    let mut needs_sep_glyph = false;
    if !h.title.is_empty() {
        ui.text(&h.title);
        needs_sep_glyph = true;
    }
    for (k, v) in &h.chips {
        if needs_sep_glyph {
            ui.same_line();
            ui.text_colored(DIM, CHIP_SEP);
        }
        ui.same_line();
        ui.text_colored(DIM, k);
        ui.same_line();
        ui.text(v);
        needs_sep_glyph = true;
    }
    if !h.trailing.is_empty() {
        // Right-align: place trailing text so its RIGHT edge sits at
        // the tooltip's inner content-region right edge.
        let trailing_w = crate::utils::text::calc_text_size(&h.trailing)[0];
        let content_w = ui.content_region_avail()[0];
        let cur_x = ui.cursor_pos_x();
        let target_x = (cur_x + content_w - trailing_w).max(cur_x + HDR_GAP);
        ui.same_line();
        ui.set_cursor_pos_x(target_x);
        ui.text_colored(DIM, &h.trailing);
    }
}

// ─── Focus ──────────────────────────────────────────────────────────

fn measure_focus_width(f: &RichHoverFocus) -> f32 {
    // Measure the arrow and label separately — draw_focus emits them
    // as two widgets joined by `same_line()`, so this matches the
    // real rendered width without allocating a `format!()` string
    // every hover frame.
    let arrow_w = crate::utils::text::calc_text_size("→")[0];
    let label_w = arrow_w + ITEM_SPACING_X + crate::utils::text::calc_text_size(&f.label)[0];
    let mut kv_w = 0.0f32;
    for (_, v) in &f.rows {
        let v_w = crate::utils::text::calc_text_size(v)[0];
        kv_w = kv_w.max(FOCUS_KEY_W + v_w);
    }
    label_w.max(kv_w)
}

fn draw_focus(ui: &Ui, f: &RichHoverFocus) {
    if !f.label.is_empty() {
        ui.text_colored(ACCENT, "→");
        ui.same_line();
        ui.text(&f.label);
    }
    for (k, v) in &f.rows {
        ui.set_cursor_pos_x(ui.cursor_pos_x() + FOCUS_INDENT);
        ui.text_colored(DIM, k);
        ui.same_line();
        ui.set_cursor_pos_x(ui.cursor_pos_x().max(FOCUS_INDENT + FOCUS_KEY_W));
        ui.text(v);
    }
}

// ─── Structure ──────────────────────────────────────────────────────

fn measure_structure_width(s: &RichHoverStructure) -> f32 {
    let ncols = s.columns.len();
    if ncols == 0 {
        return 0.0;
    }
    let mut col_w = vec![0.0f32; ncols];
    for (i, c) in s.columns.iter().enumerate() {
        col_w[i] = col_w[i].max(crate::utils::text::calc_text_size(&c.name)[0]);
    }
    for row in &s.rows {
        for (i, cell) in row.cells.iter().enumerate().take(ncols) {
            col_w[i] = col_w[i].max(crate::utils::text::calc_text_size(&cell.text)[0]);
        }
    }
    let sum: f32 = col_w.iter().sum();
    // ncols >= 1 is guaranteed by the early-return above.
    sum + (ncols - 1) as f32 * COL_GAP
}

fn draw_structure(ui: &Ui, s: &RichHoverStructure) {
    if !s.title.is_empty() {
        ui.text_colored(DIM, &s.title);
    }
    let ncols = s.columns.len();
    if ncols == 0 || s.rows.is_empty() {
        return;
    }

    // Pre-compute per-column widths so cells align without a real
    // ui.table() (which paints its own bg + borders we don't want).
    let mut col_w = vec![0.0f32; ncols];
    for (i, c) in s.columns.iter().enumerate() {
        col_w[i] = col_w[i].max(crate::utils::text::calc_text_size(&c.name)[0]);
    }
    for row in &s.rows {
        for (i, cell) in row.cells.iter().enumerate().take(ncols) {
            col_w[i] = col_w[i].max(crate::utils::text::calc_text_size(&cell.text)[0]);
        }
    }
    // Per-column x-offset from the row's left edge.
    let mut col_x = vec![0.0f32; ncols];
    for i in 1..ncols {
        col_x[i] = col_x[i - 1] + col_w[i - 1] + COL_GAP;
    }
    let row_left_x = ui.cursor_pos_x();

    // Column header row.
    for (i, c) in s.columns.iter().enumerate() {
        if i > 0 {
            ui.same_line();
        }
        ui.set_cursor_pos_x(row_left_x + col_x[i]);
        match c.align {
            RichHoverAlign::Left => ui.text_colored(DIM, &c.name),
            RichHoverAlign::Right => {
                let hw = crate::utils::text::calc_text_size(&c.name)[0];
                ui.set_cursor_pos_x(row_left_x + col_x[i] + col_w[i] - hw);
                ui.text_colored(DIM, &c.name);
            }
        }
    }

    // Data rows. For a focused row we paint the tinted backdrop
    // FIRST so the cell text renders on top of it (the fill has
    // alpha < 1, but drawing text over the fill preserves crisp
    // glyph contrast). ncols >= 1 is guaranteed by the early-return
    // above, so `(ncols - 1) as f32 * COL_GAP` is non-negative.
    let total_row_w: f32 = col_w.iter().sum::<f32>() + (ncols - 1) as f32 * COL_GAP;
    for (row_idx, row) in s.rows.iter().enumerate() {
        let is_focused = s.focused_row == Some(row_idx);
        let row_y0 = ui.cursor_pos_y();
        let line_h = ui.text_line_height() + LINE_GAP;

        if is_focused {
            let win_pos = ui.window_pos();
            let x0 = win_pos[0] + row_left_x - ui.scroll_x() - 4.0;
            let y0 = win_pos[1] + row_y0 - ui.scroll_y() - 1.0;
            let x1 = x0 + total_row_w + 8.0;
            let y1 = y0 + line_h;
            let dl = ui.get_window_draw_list();
            dl.add_rect(
                [x0, y0],
                [x1, y1],
                crate::utils::color::pack_color_f32(FOCUS_BG),
            )
            .filled(true)
            .build();
            let border = [FOCUS_BG[0], FOCUS_BG[1], FOCUS_BG[2], 0.75];
            dl.add_rect(
                [x0, y0],
                [x1, y1],
                crate::utils::color::pack_color_f32(border),
            )
            .build();
        }

        for (i, cell) in row.cells.iter().enumerate().take(ncols) {
            if i > 0 {
                ui.same_line();
            }
            ui.set_cursor_pos_x(row_left_x + col_x[i]);
            let color = if row.muted { Some(DIM) } else { cell.color };
            let text = cell.text.as_str();
            match s.columns[i].align {
                RichHoverAlign::Left => draw_cell(ui, text, color),
                RichHoverAlign::Right => {
                    let cw = crate::utils::text::calc_text_size(text)[0];
                    ui.set_cursor_pos_x(row_left_x + col_x[i] + col_w[i] - cw);
                    draw_cell(ui, text, color);
                }
            }
        }
    }
}

fn draw_cell(ui: &Ui, text: &str, color: Option<[f32; 4]>) {
    match color {
        Some(c) => ui.text_colored(c, text),
        None => ui.text(text),
    }
}
