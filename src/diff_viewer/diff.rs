//! Myers diff algorithm — computes the shortest edit script between two
//! sequences of lines.

/// A single edit operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    /// Line is the same in both versions.
    Equal { old_idx: usize, new_idx: usize },
    /// Line was added (only in new).
    Insert { new_idx: usize },
    /// Line was removed (only in old).
    Delete { old_idx: usize },
}

/// Compute the diff between `old` and `new` line slices using Myers' algorithm.
pub fn diff_lines(old: &[&str], new: &[&str]) -> Vec<DiffOp> {
    let n = old.len();
    let m = new.len();

    if n == 0 && m == 0 {
        return Vec::new();
    }
    if n == 0 {
        return (0..m).map(|j| DiffOp::Insert { new_idx: j }).collect();
    }
    if m == 0 {
        return (0..n).map(|i| DiffOp::Delete { old_idx: i }).collect();
    }

    // Myers' algorithm with O((N+M)D) time, O((N+M)^2) space for trace.
    // Cap max_d to avoid excessive memory usage on very large inputs.
    let max_d = (n + m).min(50_000);
    let offset = max_d; // shift to allow negative indices
    let size = 2 * max_d + 1;

    // v[k + offset] = furthest x on diagonal k
    let mut v = vec![0usize; size];
    // Store trace for backtracking
    let mut trace: Vec<Vec<usize>> = Vec::new();

    'outer: for d in 0..=max_d {
        trace.push(v.clone());

        let d_i = d as isize;
        let mut k = -d_i;
        while k <= d_i {
            let ki = (k + offset as isize) as usize;
            let mut x = if k == -d_i
                || (k != d_i && v[ki - 1] < v[ki + 1])
            {
                v[ki + 1] // move down (insert)
            } else {
                v[ki - 1] + 1 // move right (delete)
            };

            let mut y = (x as isize - k) as usize;

            // Follow diagonal (equal lines)
            while x < n && y < m && old[x] == new[y] {
                x += 1;
                y += 1;
            }

            v[ki] = x;

            if x >= n && y >= m {
                break 'outer;
            }

            k += 2;
        }
    }

    // Backtrack to build the edit script
    backtrack(&trace, old, new, n, m, offset)
}

fn backtrack(
    trace: &[Vec<usize>],
    old: &[&str],
    new: &[&str],
    n: usize,
    m: usize,
    offset: usize,
) -> Vec<DiffOp> {
    let mut ops = Vec::new();
    let mut x = n;
    let mut y = m;

    for d in (0..trace.len()).rev() {
        let v = &trace[d];
        let k = x as isize - y as isize;
        let ki = (k + offset as isize) as usize;

        if d == 0 {
            // Base case: follow diagonal
            while x > 0 && y > 0 && old[x - 1] == new[y - 1] {
                x -= 1;
                y -= 1;
                ops.push(DiffOp::Equal { old_idx: x, new_idx: y });
            }
            break;
        }

        let prev_k;
        {
            let d_i = d as isize;
            prev_k = if k == -d_i
                || (k != d_i && v[ki - 1] < v[ki + 1])
            {
                k + 1 // came from down (insert)
            } else {
                k - 1 // came from right (delete)
            };
        }

        let prev_ki = (prev_k + offset as isize) as usize;
        let prev_x = trace[d - 1][prev_ki];
        let prev_y = (prev_x as isize - prev_k) as usize;

        // Diagonal (equal lines)
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            ops.push(DiffOp::Equal { old_idx: x, new_idx: y });
        }

        // The edit
        if x > prev_x {
            x -= 1;
            ops.push(DiffOp::Delete { old_idx: x });
        } else if y > prev_y {
            y -= 1;
            ops.push(DiffOp::Insert { new_idx: y });
        }
    }

    ops.reverse();
    ops
}

/// A diff hunk — contiguous group of changes with context.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// Start line in old file.
    pub old_start: usize,
    /// Number of lines from old file.
    pub old_count: usize,
    /// Start line in new file.
    pub new_start: usize,
    /// Number of lines from new file.
    pub new_count: usize,
    /// Operations in this hunk.
    pub ops: Vec<DiffOp>,
}

/// Group diff operations into hunks with context lines.
pub fn group_hunks(ops: &[DiffOp], context: usize) -> Vec<DiffHunk> {
    if ops.is_empty() {
        return Vec::new();
    }

    let mut hunks = Vec::new();
    let mut current_ops: Vec<DiffOp> = Vec::new();
    let mut hunk_start = None;
    let mut consecutive_equal = 0;

    for op in ops {
        let is_change = !matches!(op, DiffOp::Equal { .. });

        if is_change {
            if hunk_start.is_none() {
                // Start new hunk — include preceding context
                let ctx_start = current_ops.len().saturating_sub(context);
                let ctx = current_ops[ctx_start..].to_vec();
                current_ops = ctx;
                hunk_start = Some(current_ops.len());
            }
            consecutive_equal = 0;
            current_ops.push(op.clone());
        } else {
            consecutive_equal += 1;
            current_ops.push(op.clone());

            if hunk_start.is_some() && consecutive_equal > context * 2 {
                // Close this hunk — trim trailing equal ops beyond context
                let trim = consecutive_equal - context;
                // Save the trimmed equal ops as leading context for next hunk
                let trailing_start = current_ops.len() - trim;
                let trailing: Vec<DiffOp> = current_ops[trailing_start..].to_vec();
                current_ops.truncate(trailing_start);
                hunks.push(build_hunk(&current_ops));
                // Seed next hunk buffer with the trailing context
                current_ops = trailing;
                hunk_start = None;
                consecutive_equal = context; // the trailing ops are equal
            }
        }
    }

    if hunk_start.is_some() && !current_ops.is_empty() {
        hunks.push(build_hunk(&current_ops));
    }

    hunks
}

fn build_hunk(ops: &[DiffOp]) -> DiffHunk {
    let mut old_start = usize::MAX;
    let mut old_end = 0;
    let mut new_start = usize::MAX;
    let mut new_end = 0;

    for op in ops {
        match op {
            DiffOp::Equal { old_idx, new_idx } => {
                old_start = old_start.min(*old_idx);
                old_end = old_end.max(*old_idx + 1);
                new_start = new_start.min(*new_idx);
                new_end = new_end.max(*new_idx + 1);
            }
            DiffOp::Delete { old_idx } => {
                old_start = old_start.min(*old_idx);
                old_end = old_end.max(*old_idx + 1);
            }
            DiffOp::Insert { new_idx } => {
                new_start = new_start.min(*new_idx);
                new_end = new_end.max(*new_idx + 1);
            }
        }
    }

    if old_start == usize::MAX { old_start = 0; }
    if new_start == usize::MAX { new_start = 0; }

    DiffHunk {
        old_start,
        old_count: old_end.saturating_sub(old_start),
        new_start,
        new_count: new_end.saturating_sub(new_start),
        ops: ops.to_vec(),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_both() {
        let ops = diff_lines(&[], &[]);
        assert!(ops.is_empty());
    }

    #[test]
    fn empty_old() {
        let ops = diff_lines(&[], &["a", "b"]);
        assert_eq!(ops.len(), 2);
        assert!(ops.iter().all(|op| matches!(op, DiffOp::Insert { .. })));
    }

    #[test]
    fn empty_new() {
        let ops = diff_lines(&["a", "b"], &[]);
        assert_eq!(ops.len(), 2);
        assert!(ops.iter().all(|op| matches!(op, DiffOp::Delete { .. })));
    }

    #[test]
    fn identical() {
        let ops = diff_lines(&["a", "b", "c"], &["a", "b", "c"]);
        assert_eq!(ops.len(), 3);
        assert!(ops.iter().all(|op| matches!(op, DiffOp::Equal { .. })));
    }

    #[test]
    fn simple_change() {
        let old = vec!["a", "b", "c"];
        let new = vec!["a", "x", "c"];
        let ops = diff_lines(&old, &new);
        // Should be: Equal(a), Delete(b), Insert(x), Equal(c)
        let deletes = ops.iter().filter(|o| matches!(o, DiffOp::Delete { .. })).count();
        let inserts = ops.iter().filter(|o| matches!(o, DiffOp::Insert { .. })).count();
        assert_eq!(deletes, 1);
        assert_eq!(inserts, 1);
    }

    #[test]
    fn add_lines() {
        let old = vec!["a", "c"];
        let new = vec!["a", "b", "c"];
        let ops = diff_lines(&old, &new);
        let inserts = ops.iter().filter(|o| matches!(o, DiffOp::Insert { .. })).count();
        assert_eq!(inserts, 1);
    }

    #[test]
    fn remove_lines() {
        let old = vec!["a", "b", "c"];
        let new = vec!["a", "c"];
        let ops = diff_lines(&old, &new);
        let deletes = ops.iter().filter(|o| matches!(o, DiffOp::Delete { .. })).count();
        assert_eq!(deletes, 1);
    }

    #[test]
    fn group_hunks_basic() {
        let old: Vec<&str> = (0..10).map(|i| match i {
            3 => "OLD",
            _ => "same",
        }).collect();
        let new: Vec<&str> = (0..10).map(|i| match i {
            3 => "NEW",
            _ => "same",
        }).collect();
        let ops = diff_lines(&old, &new);
        let hunks = group_hunks(&ops, 2);
        assert!(!hunks.is_empty());
    }
}
