//! Code-folding region detection.
//!
//! Extracted from `mod.rs`. Matches two kinds of foldable blocks:
//! - Brace blocks: `{ … }` spanning ≥ 2 lines.
//! - Comment region markers: `// region: Name` … `// endregion`.
//!
//! The scanner is deliberately simple (no per-token awareness of strings
//! or block comments); good enough for coarse folding in source code.

/// A foldable region in the code.
#[derive(Debug, Clone)]
pub(super) struct FoldRegion {
    /// Start line (the line with `fn`, `struct`, `impl`, `{`, etc.).
    pub(super) start_line: usize,
    /// End line (the line with the closing `}`).
    pub(super) end_line: usize,
    /// Whether this region is currently folded.
    pub(super) folded: bool,
}

/// Detects fold regions by matching `{` / `}` and `// region:` / `// endregion`.
pub(super) fn detect_fold_regions(lines: &[String]) -> Vec<FoldRegion> {
    let mut regions = Vec::new();
    let mut brace_stack: Vec<usize> = Vec::new();
    let mut region_stack: Vec<usize> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Comment-based region markers: `// region: Name` / `// endregion`
        if let Some(rest) = trimmed.strip_prefix("//") {
            let comment = rest.trim_start();
            if comment.starts_with("region:") || comment.starts_with("region ") {
                region_stack.push(i);
                continue;
            }
            if comment.starts_with("endregion") {
                if let Some(start) = region_stack.pop()
                    && i > start
                {
                    regions.push(FoldRegion {
                        start_line: start,
                        end_line: i,
                        folded: false,
                    });
                }
                continue;
            }
        }

        // Brace matching (simplified: doesn't handle strings/comments perfectly,
        // but good enough for Rust code structure)
        for ch in trimmed.chars() {
            match ch {
                '{' => brace_stack.push(i),
                '}' => {
                    if let Some(start) = brace_stack.pop() {
                        // Only create fold region if it spans multiple lines
                        if i > start + 1 {
                            regions.push(FoldRegion {
                                start_line: start,
                                end_line: i,
                                folded: false,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    regions.sort_by_key(|r| r.start_line);
    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_regions() {
        let lines: Vec<String> = [
            "fn main() {",
            "    let x = 1;",
            "    let y = 2;",
            "}",
            "",
            "// region: Utils",
            "fn helper() {}",
            "// endregion",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let regions = detect_fold_regions(&lines);
        assert_eq!(regions.len(), 2);
        // Brace region
        assert_eq!(regions[0].start_line, 0);
        assert_eq!(regions[0].end_line, 3);
        // Comment region
        assert_eq!(regions[1].start_line, 5);
        assert_eq!(regions[1].end_line, 7);
    }
}
