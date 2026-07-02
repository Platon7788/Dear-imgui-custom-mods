//! Word-wrap point computation.
//!
//! Extracted from `mod.rs`. Takes a raw line, target pixel width, and
//! glyph metrics; returns the char-column positions at which the line
//! should break into new visual rows. Prefers breaking at the last
//! whitespace (word boundary); falls back to a hard break at the column
//! that overflows the width.
//!
//! Kept as a pure function — `CodeEditor::update_wrap_cache` owns the
//! per-line cache invalidated by edit-version + wrap-width change, and
//! calls this function per line when the cache is stale.

/// Compute column indices where a line should wrap.
///
/// Returns an empty vec if the line fits within `max_width`.
/// Each entry is the char-column where a new visual row begins.
/// Prefers breaking at the last space (word boundary); falls back to
/// a hard break at the column that exceeds the width.
///
/// Allocating convenience wrapper — the per-frame hot path uses
/// [`compute_wrap_points_into`] with reused scratch instead.
#[cfg(test)]
pub(super) fn compute_wrap_points(
    line: &str,
    max_width: f32,
    char_advance: f32,
    tab_size: u8,
) -> Vec<usize> {
    let mut widths = Vec::new();
    let mut is_ws = Vec::new();
    let mut out = Vec::new();
    compute_wrap_points_into(
        line,
        max_width,
        char_advance,
        tab_size,
        &mut widths,
        &mut is_ws,
        &mut out,
    );
    out
}

/// Allocation-free variant: writes the wrap columns into `out` (cleared
/// first) and reuses the caller-owned `widths` / `is_ws` scratch buffers, so
/// the per-frame wrap rebuild allocates nothing after warm-up. `compute_wrap_points`
/// is the convenience wrapper used by tests and cold callers.
pub(super) fn compute_wrap_points_into(
    line: &str,
    max_width: f32,
    char_advance: f32,
    tab_size: u8,
    widths: &mut Vec<f32>,
    is_ws: &mut Vec<bool>,
    out: &mut Vec<usize>,
) {
    out.clear();
    if max_width <= char_advance || !max_width.is_finite() {
        return;
    }

    // Precompute per-char widths + wrap-candidate flags in one pass. Both use
    // reused scratch (cleared, capacity retained) so the hot path is alloc-free.
    widths.clear();
    is_ws.clear();
    for ch in line.chars() {
        widths.push(if ch == '\t' {
            char_advance * tab_size as f32
        } else {
            char_advance
        });
        is_ws.push(ch == ' ' || ch == '\t');
    }
    let len = widths.len();

    let wraps = out;
    let mut x = 0.0f32;
    let mut last_space: Option<usize> = None;
    let mut row_start = 0usize;

    // Belt-and-braces: the loop body always either advances `col` or pushes
    // a wrap entry (and changes `row_start`). A malformed edge case should
    // never sustain a position-stall, but an infinite `wraps.push` would be
    // catastrophic (memory blow-up → OOM). Hard-cap iterations at
    // `len * 2 + 4` which is comfortably above the worst legitimate case
    // (single-char rows = len wraps, we allow a small slack for ties).
    let max_iters = len.saturating_mul(2).saturating_add(4);
    let mut iters = 0usize;

    let mut col = 0usize;
    while col < len {
        iters += 1;
        if iters > max_iters {
            debug_assert!(false, "compute_wrap_points stalled");
            break;
        }
        let w = widths[col];

        // Check BEFORE adding: will this character overflow the row?
        // Exception: first character of a row always goes on that row
        // (prevents infinite loop on very narrow widths).
        if x + w > max_width && col > row_start {
            // Prefer breaking at a word boundary (last space).
            let wrap_col = match last_space {
                Some(sp) if sp > row_start && sp <= col => sp,
                _ => col,
            };
            if wrap_col <= row_start {
                x += w;
                col += 1;
                continue;
            }
            wraps.push(wrap_col);

            // Reset x: re-measure from wrap_col up to (but not including)
            // the current col — single slice sum instead of closure calls.
            x = widths[wrap_col..col].iter().sum();
            row_start = wrap_col;
            last_space = None;
            continue;
        }

        x += w;

        if is_ws[col] {
            last_space = Some(col + 1); // wrap AFTER whitespace
        }

        col += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_fits() {
        // Line fits — no wraps.
        let wraps = compute_wrap_points("hello", 100.0, 10.0, 4);
        assert!(wraps.is_empty());
    }

    #[test]
    fn test_wrap_word_boundary() {
        // "hello world" at 55px width / 10px advance fits 5 chars per row.
        // Algorithm takes two passes: first hard-break at col 5 (no earlier
        // whitespace), then another break at col 6 when the dangling space
        // gets promoted to last_space on the new row. Exact output documented
        // to catch accidental logic drift.
        let wraps = compute_wrap_points("hello world", 55.0, 10.0, 4);
        assert!(!wraps.is_empty());
        assert_eq!(wraps[0], 5);
    }

    #[test]
    fn test_wrap_prefers_space() {
        // "aaa bbb ccc" wider viewport: first wrap should land on a space
        // boundary rather than a hard break mid-word.
        let wraps = compute_wrap_points("aaa bbb ccc", 85.0, 10.0, 4);
        assert!(!wraps.is_empty());
        // First wrap should be >= 4 (after "aaa " completes — col 4).
        assert!(wraps[0] >= 4);
    }

    #[test]
    fn test_wrap_narrow_width() {
        // max_width <= char_advance → no wrap (returns empty to avoid stall).
        let wraps = compute_wrap_points("abcdef", 8.0, 10.0, 4);
        assert!(wraps.is_empty());
    }

    #[test]
    fn test_wrap_nan_width() {
        // NaN width must not panic or loop.
        let wraps = compute_wrap_points("abcdef", f32::NAN, 10.0, 4);
        assert!(wraps.is_empty());
    }
}
