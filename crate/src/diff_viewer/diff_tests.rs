//! Unit tests for the Myers diff core (`super::diff_lines`,
//! `super::group_hunks`). Kept in a sibling file so `diff.rs`
//! stays under the 500-line module budget.

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
    let deletes = ops
        .iter()
        .filter(|o| matches!(o, DiffOp::Delete { .. }))
        .count();
    let inserts = ops
        .iter()
        .filter(|o| matches!(o, DiffOp::Insert { .. }))
        .count();
    assert_eq!(deletes, 1);
    assert_eq!(inserts, 1);
}

#[test]
fn add_lines() {
    let old = vec!["a", "c"];
    let new = vec!["a", "b", "c"];
    let ops = diff_lines(&old, &new);
    let inserts = ops
        .iter()
        .filter(|o| matches!(o, DiffOp::Insert { .. }))
        .count();
    assert_eq!(inserts, 1);
}

#[test]
fn remove_lines() {
    let old = vec!["a", "b", "c"];
    let new = vec!["a", "c"];
    let ops = diff_lines(&old, &new);
    let deletes = ops
        .iter()
        .filter(|o| matches!(o, DiffOp::Delete { .. }))
        .count();
    assert_eq!(deletes, 1);
}

#[test]
fn group_hunks_basic() {
    let old: Vec<&str> = (0..10)
        .map(|i| match i {
            3 => "OLD",
            _ => "same",
        })
        .collect();
    let new: Vec<&str> = (0..10)
        .map(|i| match i {
            3 => "NEW",
            _ => "same",
        })
        .collect();
    let ops = diff_lines(&old, &new);
    let hunks = group_hunks(&ops, 2);
    assert!(!hunks.is_empty());
}

// ── Correctness invariants ──────────────────────────────────────────────
//
// Reference-quality `O(NM)` DP that returns the LCS length, used
// purely to assert that the Myers script is *minimal*. Independent
// of the algorithm under test.
fn lcs_len(a: &[&str], b: &[&str]) -> usize {
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            dp[i + 1][j + 1] = if a[i] == b[j] {
                dp[i][j] + 1
            } else {
                dp[i][j + 1].max(dp[i + 1][j])
            };
        }
    }
    dp[n][m]
}

/// Assert the three invariants every valid diff must satisfy:
/// 1. every `Equal` pairs lines that are actually identical
///    (the historic backtrack bug fabricated `Equal` ops over
///    *different* lines);
/// 2. `Equal`+`Delete` reconstructs `old` exactly in order, and
///    `Equal`+`Insert` reconstructs `new` exactly in order
///    (no dropped / duplicated / reordered indices);
/// 3. the edit count equals `n + m - 2·LCS` — i.e. the script is
///    minimal.
fn assert_valid_diff(old: &[&str], new: &[&str]) {
    let ops = diff_lines(old, new);
    let mut o = Vec::new();
    let mut nw = Vec::new();
    let mut edits = 0usize;
    for op in &ops {
        match op {
            DiffOp::Equal { old_idx, new_idx } => {
                assert_eq!(
                    old[*old_idx], new[*new_idx],
                    "bogus Equal: old[{old_idx}] != new[{new_idx}]"
                );
                o.push(*old_idx);
                nw.push(*new_idx);
            }
            DiffOp::Delete { old_idx } => {
                o.push(*old_idx);
                edits += 1;
            }
            DiffOp::Insert { new_idx } => {
                nw.push(*new_idx);
                edits += 1;
            }
        }
    }
    let identity_old: Vec<usize> = (0..old.len()).collect();
    let identity_new: Vec<usize> = (0..new.len()).collect();
    assert_eq!(o, identity_old, "old not reconstructed in order");
    assert_eq!(nw, identity_new, "new not reconstructed in order");
    let minimal = old.len() + new.len() - 2 * lcs_len(old, new);
    assert_eq!(edits, minimal, "edit script is not minimal");
}

#[test]
fn disjoint_inputs_have_no_bogus_equal() {
    // Regression: the old backtrack emitted `Equal { 2, 2 }` here
    // (claiming "c" == "z") and silently dropped Delete(2)/Insert(2).
    assert_valid_diff(&["a", "b", "c"], &["x", "y", "z"]);
    let ops = diff_lines(&["a", "b", "c"], &["x", "y", "z"]);
    assert_eq!(ops.len(), 6, "fully-different => 3 del + 3 ins");
    assert!(ops.iter().all(|o| !matches!(o, DiffOp::Equal { .. })));
}

#[test]
fn simple_change_pairs_correctly() {
    // Regression: old code produced Insert/Equal/Equal/Delete with
    // bogus equalities; correct is Equal/Delete/Insert/Equal.
    let ops = diff_lines(&["a", "b", "c"], &["a", "x", "c"]);
    assert_eq!(
        ops,
        vec![
            DiffOp::Equal {
                old_idx: 0,
                new_idx: 0
            },
            DiffOp::Delete { old_idx: 1 },
            DiffOp::Insert { new_idx: 1 },
            DiffOp::Equal {
                old_idx: 2,
                new_idx: 2
            },
        ]
    );
}

#[test]
fn classic_lcs_abcabba_cbabac_is_minimal() {
    let old = ["a", "b", "c", "a", "b", "b", "a"];
    let new = ["c", "b", "a", "b", "a", "c"];
    assert_valid_diff(&old, &new);
    // LCS of these is length 4 => 7 + 6 - 8 = 5 edits.
    let edits = diff_lines(&old, &new)
        .iter()
        .filter(|o| !matches!(o, DiffOp::Equal { .. }))
        .count();
    assert_eq!(edits, 5);
}

#[test]
fn common_prefix_and_suffix_minimal() {
    assert_valid_diff(
        &["pre1", "pre2", "mid_old", "suf1", "suf2"],
        &["pre1", "pre2", "mid_new", "suf1", "suf2"],
    );
    let ops = diff_lines(
        &["pre1", "pre2", "mid_old", "suf1", "suf2"],
        &["pre1", "pre2", "mid_new", "suf1", "suf2"],
    );
    // Only the single middle line changes => 1 del + 1 ins.
    let edits = ops
        .iter()
        .filter(|o| !matches!(o, DiffOp::Equal { .. }))
        .count();
    assert_eq!(edits, 2);
}

#[test]
fn identical_inputs_are_all_equal() {
    let lines = ["alpha", "beta", "gamma", "delta"];
    let ops = diff_lines(&lines, &lines);
    assert_eq!(ops.len(), lines.len());
    assert!(ops.iter().all(|o| matches!(o, DiffOp::Equal { .. })));
    assert_valid_diff(&lines, &lines);
}

#[test]
fn single_line_change() {
    assert_valid_diff(&["only_old"], &["only_new"]);
    assert_valid_diff(&["same"], &["same"]);
}

#[test]
fn crlf_vs_lf_lines_compare_equal_when_content_matches() {
    // `str::lines()` (used by `DiffViewer::recompute`) strips a
    // trailing `\r\n` AND a lone `\n`, so a CRLF document and an
    // LF document with identical content split into identical
    // line slices -> the diff sees them as equal. This documents
    // the host-friendly behaviour the viewer relies on.
    let crlf: Vec<&str> = "a\r\nb\r\nc".lines().collect();
    let lf: Vec<&str> = "a\nb\nc".lines().collect();
    assert_eq!(crlf, vec!["a", "b", "c"]);
    assert_eq!(crlf, lf);
    let ops = diff_lines(&crlf, &lf);
    assert!(
        ops.iter().all(|o| matches!(o, DiffOp::Equal { .. })),
        "matching CRLF/LF content must produce an all-equal diff"
    );
    assert_valid_diff(&crlf, &lf);

    // A genuine CR-only difference (no following `\n`) is preserved
    // and shows up as a change.
    let with_cr = ["a\rmid", "b"];
    let without = ["a", "b"];
    assert_valid_diff(&with_cr, &without);
    assert!(
        diff_lines(&with_cr, &without)
            .iter()
            .any(|o| matches!(o, DiffOp::Delete { .. } | DiffOp::Insert { .. }))
    );
}

#[test]
fn unicode_lines() {
    let old = ["héllo", "мир", "🦀 crab", "tail"];
    let new = ["héllo", "world", "🦀 crab", "tail"];
    assert_valid_diff(&old, &new);
    let edits = diff_lines(&old, &new)
        .iter()
        .filter(|o| !matches!(o, DiffOp::Equal { .. }))
        .count();
    assert_eq!(edits, 2, "only the second line differs");
}

#[test]
fn very_long_lines_treated_atomically() {
    let long_a = "x".repeat(50_000);
    let long_b = "y".repeat(50_000);
    let old = [long_a.as_str(), "shared"];
    let new = [long_b.as_str(), "shared"];
    assert_valid_diff(&old, &new);
}

#[test]
fn diff_is_deterministic_and_stable() {
    let old = ["x", "a", "b", "y", "c", "d", "z"];
    let new = ["x", "b", "a", "y", "c", "e", "z"];
    let first = diff_lines(&old, &new);
    let second = diff_lines(&old, &new);
    assert_eq!(first, second, "re-diff must produce identical pairings");
}

#[test]
fn huge_input_falls_back_without_blowup() {
    // n + m just over the cap: must return quickly via the
    // delete-all-then-insert-all fallback, never the Myers trace.
    let big = MAX_DIFF_INPUT_LINES / 2 + 100;
    let old: Vec<&str> = vec!["a"; big];
    let new: Vec<&str> = vec!["b"; big];
    let ops = diff_lines(&old, &new);
    assert_eq!(ops.len(), big * 2);
    let dels = ops
        .iter()
        .filter(|o| matches!(o, DiffOp::Delete { .. }))
        .count();
    let ins = ops
        .iter()
        .filter(|o| matches!(o, DiffOp::Insert { .. }))
        .count();
    assert_eq!((dels, ins), (big, big));
}

#[test]
fn at_cap_uses_real_myers() {
    // n + m == cap is NOT over threshold -> real (minimal) diff.
    let half = MAX_DIFF_INPUT_LINES / 2;
    let old: Vec<&str> = (0..half).map(|_| "same").collect();
    let mut new = old.clone();
    new[half / 2] = "changed";
    assert_eq!(old.len() + new.len(), MAX_DIFF_INPUT_LINES);
    let ops = diff_lines(&old, &new);
    let edits = ops
        .iter()
        .filter(|o| !matches!(o, DiffOp::Equal { .. }))
        .count();
    assert_eq!(edits, 2, "single changed line => 1 del + 1 ins");
}

#[test]
fn fuzz_minimality_against_dp() {
    // Deterministic xorshift fuzz: every random pair must yield a
    // valid, minimal, identical-pairing diff.
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut rng = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let alphabet = ["a", "b", "c", "d"];
    for _ in 0..3000 {
        let la = (rng() % 9) as usize;
        let lb = (rng() % 9) as usize;
        let a: Vec<&str> = (0..la).map(|_| alphabet[(rng() % 4) as usize]).collect();
        let b: Vec<&str> = (0..lb).map(|_| alphabet[(rng() % 4) as usize]).collect();
        assert_valid_diff(&a, &b);
    }
}

#[test]
fn group_hunks_empty() {
    assert!(group_hunks(&[], 3).is_empty());
}

#[test]
fn group_hunks_all_equal_yields_no_hunks() {
    let ops = diff_lines(&["a", "b", "c"], &["a", "b", "c"]);
    assert!(group_hunks(&ops, 3).is_empty());
}

#[test]
fn group_hunks_separates_distant_changes() {
    // Two changes far apart (more than 2·context equal lines
    // between them) must split into two hunks.
    let mut old: Vec<&str> = (0..20).map(|_| "same").collect();
    let mut new = old.clone();
    old[2] = "old_a";
    new[2] = "new_a";
    old[17] = "old_b";
    new[17] = "new_b";
    let ops = diff_lines(&old, &new);
    let hunks = group_hunks(&ops, 2);
    assert_eq!(hunks.len(), 2, "distant changes => two hunks");
}
