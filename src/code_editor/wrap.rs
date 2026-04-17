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
pub(super) fn compute_wrap_points(
    line: &str,
    max_width: f32,
    char_advance: f32,
    tab_size: u8,
) -> Vec<usize> {
    if max_width <= char_advance || !max_width.is_finite() {
        return Vec::new();
    }

    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut wraps = Vec::new();
    let mut x = 0.0f32;
    let mut last_space: Option<usize> = None;
    let mut row_start = 0usize;

    let char_w = |ch: char| -> f32 {
        if ch == '\t' { char_advance * tab_size as f32 } else { char_advance }
    };

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
        let ch = chars[col];
        let w = char_w(ch);

        // Check BEFORE adding: will this character overflow the row?
        // Exception: first character of a row always goes on that row
        // (prevents infinite loop on very narrow widths).
        if x + w > max_width && col > row_start {
            // Prefer breaking at a word boundary (last space).
            // Guard: never push a wrap equal to the previous one or
            // `row_start` itself — that would leave `row_start` unchanged
            // after the continue, which is the classic stall shape.
            let wrap_col = match last_space {
                Some(sp) if sp > row_start && sp <= col => sp,
                _ => col,
            };
            if wrap_col <= row_start {
                // Defensive: should be unreachable given the guard above,
                // but if it ever fires we advance by one char rather than
                // loop forever.
                x += w;
                col += 1;
                continue;
            }
            wraps.push(wrap_col);

            // Reset x: re-measure from wrap_col up to (but not including)
            // the current col — those characters landed on the new row.
            x = 0.0;
            for &c in &chars[wrap_col..col] {
                x += char_w(c);
            }
            row_start = wrap_col;
            last_space = None;
            // Do NOT advance col — re-evaluate the current character
            // against the fresh row (handles lines wider than 2× max_width).
            continue;
        }

        x += w;

        if ch == ' ' || ch == '\t' {
            last_space = Some(col + 1); // wrap AFTER whitespace
        }

        col += 1;
    }
    wraps
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
