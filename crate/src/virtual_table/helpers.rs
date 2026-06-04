//! Free helper functions: clipboard text, row-stride and height snapping.
//!
//! Split out of `mod.rs`. `row_height_to_stride` is re-exported by the
//! parent so `crate::virtual_table::row_height_to_stride` stays stable.

use super::*;

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Build tab-separated text from the given selection.
/// `fill(row, col, buf)` writes display text for each cell into `buf`.
pub(super) fn build_copy_text<F>(selected: &IndexSet, col_count: usize, fill: F) -> String
where
    F: Fn(usize, usize, &mut String),
{
    let mut sorted: Vec<usize> = selected.iter().copied().collect();
    sorted.sort_unstable();
    let mut out = String::new();
    let mut cell_buf = String::new();
    for row_idx in sorted {
        for col_idx in 0..col_count {
            if col_idx > 0 {
                out.push('\t');
            }
            cell_buf.clear();
            fill(row_idx, col_idx, &mut cell_buf);
            out.push_str(&cell_buf);
        }
        out.push('\n');
    }
    out
}

/// Physical pixel height of a row inside a Dear ImGui table.
///
/// The value passed to `Selectable::size([_, row_height])` (and to
/// `TableNextRow`'s `min_row_height`) is NOT what ImGui actually lays out.
/// Every row is augmented by `2 * CellPadding.y`:
///
/// * `TableBeginCell` offsets the cursor by `+CellPadding.y` from `RowPosY1`
///   (imgui_tables.cpp:2188 — `window->DC.CursorPos.y = table->RowPosY1 +
///   table->RowCellPaddingY;`).
/// * `TableEndCell` extends `RowPosY2` to `CursorMaxPos.y + CellPadding.y`
///   (imgui_tables.cpp:2247).
///
/// Therefore `RowPosY2 - RowPosY1 == row_height + 2*CellPadding.y` for any
/// row whose content (here: the SPAN_ALL_COLUMNS Selectable) equals
/// `row_height`.
///
/// `ListClipper::items_height` must be set to this stride: ImGui's
/// `ImGuiListClipper::End` seeks the cursor to
/// `StartPosY + ItemsCount * items_height` (imgui.cpp:3401, 3406), which
/// fixes the inner scroll-window's content size. Using the bare `row_height`
/// there understates the content size by `row_count * 2*CellPadding.y` and
/// ImGui clamps `scroll_y <= scroll_max_y`, making the final rows
/// unreachable via manual scroll. This matches the upstream hint at
/// imgui.cpp:3319 ("If your clipper item height is != from actual table
/// row height, consider using ImGuiListClipperFlags_NoSetTableRowCounters").
#[inline]
pub(crate) fn row_height_to_stride(row_height: f32, cell_padding_y: f32) -> f32 {
    row_height + 2.0 * cell_padding_y.max(0.0)
}

/// Quantize a Dear ImGui table's outer height so it holds a whole number of
/// rows plus the header — used by `TableConfig::snap_last_row`.
///
/// Ensures at least one row fits even when the available area is very small.
/// `row_stride` must already include the `2*CellPadding.y` surcharge (see
/// `row_height_to_stride`).
#[inline]
pub(crate) fn snap_outer_height(avail_h: f32, header_h: f32, row_stride: f32) -> f32 {
    let content_h = (avail_h - header_h).max(0.0);
    let row_count_fit = if row_stride > 0.0 {
        (content_h / row_stride).floor().max(0.0)
    } else {
        0.0
    };
    let quantized = row_count_fit * row_stride + header_h;
    quantized.max(row_stride + header_h)
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn row_stride_adds_two_cell_paddings() {
        // Normal density with default ImGui CellPadding (2 px).
        assert_eq!(row_height_to_stride(25.0, 2.0), 29.0);
        // Generous padding used in some themes.
        assert_eq!(row_height_to_stride(25.0, 4.0), 33.0);
        // Dense density, zero padding.
        assert_eq!(row_height_to_stride(17.0, 0.0), 17.0);
    }

    #[test]
    fn row_stride_clamps_negative_padding() {
        // Bogus negative padding from corrupted style must not shrink the stride.
        assert_eq!(row_height_to_stride(20.0, -5.0), 20.0);
    }

    #[test]
    fn snap_fits_nine_rows() {
        // avail=300, header=20, stride=29 → floor(280/29)=9 → 9*29+20=281.
        assert!((snap_outer_height(300.0, 20.0, 29.0) - 281.0).abs() < f32::EPSILON);
    }

    #[test]
    fn snap_guarantees_at_least_one_row() {
        // avail too small to fit even header+row → still returns header+stride.
        let out = snap_outer_height(10.0, 20.0, 29.0);
        assert!((out - (29.0 + 20.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn snap_with_exact_fit() {
        // avail exactly fits 10 rows + header → quantized unchanged.
        let stride = 30.0;
        let header = 25.0;
        let avail = 10.0 * stride + header;
        let out = snap_outer_height(avail, header, stride);
        assert!((out - avail).abs() < f32::EPSILON);
    }

    #[test]
    fn snap_zero_stride_does_not_panic() {
        // Pathological input must not divide by zero.
        let out = snap_outer_height(200.0, 20.0, 0.0);
        assert!((out - 20.0).abs() < f32::EPSILON);
    }

    /// Regression test for the scroll-unreachability bug: with 500 rows of
    /// `row_h=25` and `cell_padding_y=2`, the total content height the clipper
    /// reports must match the rendered height (500 * 29 = 14500), not the
    /// bogus 500 * 25 = 12500 that the pre-fix code produced.
    #[test]
    fn clipper_content_matches_rendered_height_large_row_count() {
        let row_count = 500usize;
        let row_h = 25.0;
        let cp_y = 2.0;

        // Pre-fix (bogus): items_height == row_h
        let bogus_total = row_count as f32 * row_h;
        // Post-fix: items_height == row_h + 2*CellPadding.y
        let stride = row_height_to_stride(row_h, cp_y);
        let correct_total = row_count as f32 * stride;

        assert_eq!(correct_total, 14500.0);
        assert_eq!(bogus_total, 12500.0);
        // The gap equals exactly `row_count * 2 * CellPadding.y`.
        assert_eq!(correct_total - bogus_total, row_count as f32 * 2.0 * cp_y);
    }
}
