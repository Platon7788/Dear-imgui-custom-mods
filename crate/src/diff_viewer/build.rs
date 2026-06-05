//! Display-line construction: turns the raw [`DiffOp`] edit script into
//! the paired `left_lines` / `right_lines` the renderer walks, applying
//! the fold-unchanged collapsing of long equal runs. Pure data
//! transformation — no ImGui. Split out of `mod.rs` to keep state and
//! rendering separate.

use super::*;

impl DiffViewer {
    pub(super) fn build_display_lines(
        &mut self,
        ops: &[DiffOp],
        old_lines: &[&str],
        new_lines: &[&str],
    ) {
        self.left_lines.clear();
        self.right_lines.clear();

        // Track equal runs for folding
        let mut equal_run = Vec::new();

        let flush_equal = |left: &mut Vec<DisplayLine>,
                           right: &mut Vec<DisplayLine>,
                           run: &mut Vec<(usize, usize)>,
                           fold: bool,
                           ctx: usize,
                           old_l: &[&str],
                           new_l: &[&str]| {
            let push_equal = |left: &mut Vec<DisplayLine>,
                              right: &mut Vec<DisplayLine>,
                              oi: usize,
                              ni: usize| {
                left.push(DisplayLine {
                    old_num: Some(oi + 1),
                    new_num: None,
                    text: old_l.get(oi).unwrap_or(&"").to_string(),
                    kind: LineKind::Equal,
                });
                right.push(DisplayLine {
                    old_num: None,
                    new_num: Some(ni + 1),
                    text: new_l.get(ni).unwrap_or(&"").to_string(),
                    kind: LineKind::Equal,
                });
            };

            if !fold || run.len() <= ctx * 2 + 1 {
                // Short enough run (or folding off): show every line.
                for &(oi, ni) in run.iter() {
                    push_equal(left, right, oi, ni);
                }
            } else {
                // Leading context.
                for &(oi, ni) in &run[..ctx] {
                    push_equal(left, right, oi, ni);
                }

                let hidden = run.len() - ctx * 2;
                let fold_text = format!("... {hidden} unchanged lines ...");
                left.push(DisplayLine {
                    old_num: None,
                    new_num: None,
                    text: fold_text.clone(),
                    kind: LineKind::FoldMarker,
                });
                right.push(DisplayLine {
                    old_num: None,
                    new_num: None,
                    text: fold_text,
                    kind: LineKind::FoldMarker,
                });

                // Trailing context.
                let start = run.len() - ctx;
                for &(oi, ni) in &run[start..] {
                    push_equal(left, right, oi, ni);
                }
            }
            run.clear();
        };

        for op in ops {
            match op {
                DiffOp::Equal { old_idx, new_idx } => {
                    equal_run.push((*old_idx, *new_idx));
                }
                DiffOp::Delete { old_idx } => {
                    if !equal_run.is_empty() {
                        flush_equal(
                            &mut self.left_lines,
                            &mut self.right_lines,
                            &mut equal_run,
                            self.config.fold_unchanged,
                            self.config.context_lines,
                            old_lines,
                            new_lines,
                        );
                    }
                    self.left_lines.push(DisplayLine {
                        old_num: Some(old_idx + 1),
                        new_num: None,
                        text: old_lines.get(*old_idx).unwrap_or(&"").to_string(),
                        kind: LineKind::Removed,
                    });
                    self.right_lines.push(DisplayLine {
                        old_num: None,
                        new_num: None,
                        text: String::new(),
                        kind: LineKind::Removed,
                    });
                }
                DiffOp::Insert { new_idx } => {
                    if !equal_run.is_empty() {
                        flush_equal(
                            &mut self.left_lines,
                            &mut self.right_lines,
                            &mut equal_run,
                            self.config.fold_unchanged,
                            self.config.context_lines,
                            old_lines,
                            new_lines,
                        );
                    }
                    self.left_lines.push(DisplayLine {
                        old_num: None,
                        new_num: None,
                        text: String::new(),
                        kind: LineKind::Added,
                    });
                    self.right_lines.push(DisplayLine {
                        old_num: None,
                        new_num: Some(new_idx + 1),
                        text: new_lines.get(*new_idx).unwrap_or(&"").to_string(),
                        kind: LineKind::Added,
                    });
                }
            }
        }

        if !equal_run.is_empty() {
            flush_equal(
                &mut self.left_lines,
                &mut self.right_lines,
                &mut equal_run,
                self.config.fold_unchanged,
                self.config.context_lines,
                old_lines,
                new_lines,
            );
        }
    }
}
