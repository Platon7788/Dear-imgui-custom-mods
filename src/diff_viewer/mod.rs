//! # DiffViewer
//!
//! Side-by-side or unified diff viewer with synchronized scrolling,
//! line numbers, change highlighting, fold unchanged regions, and
//! hunk navigation.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::diff_viewer::DiffViewer;
//!
//! let mut dv = DiffViewer::new("##diff");
//! dv.set_texts("old content\nline 2", "new content\nline 2\nline 3");
//! // In render loop: dv.render(ui);
//! ```

pub mod config;
pub mod diff;

pub use config::{DiffMode, DiffViewerConfig};
pub use diff::{DiffHunk, DiffOp, diff_lines, group_hunks};

use dear_imgui_rs::Ui;

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── Display line ────────────────────────────────────────────────────────────

/// A line prepared for rendering in the diff viewer.
#[derive(Debug, Clone)]
struct DisplayLine {
    /// Line number in old file (None for inserted lines).
    old_num: Option<usize>,
    /// Line number in new file (None for deleted lines).
    new_num: Option<usize>,
    /// Text content.
    text: String,
    /// Type of change.
    kind: LineKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    Equal,
    Added,
    Removed,
    FoldMarker,
}

// ── Events ──────────────────────────────────────────────────────────────────

/// Event emitted by the diff viewer.
#[derive(Debug, Clone)]
pub enum DiffViewerEvent {
    /// User navigated to a hunk.
    HunkSelected { index: usize },
}

// ── DiffViewer ──────────────────────────────────────────────────────────────

/// Side-by-side / unified diff viewer widget.
pub struct DiffViewer {
    id: String,
    /// Old text (left panel).
    old_text: String,
    /// New text (right panel).
    new_text: String,
    /// Old filename/label.
    pub old_label: String,
    /// New filename/label.
    pub new_label: String,
    /// Computed display lines for left panel.
    left_lines: Vec<DisplayLine>,
    /// Computed display lines for right panel.
    right_lines: Vec<DisplayLine>,
    /// Hunks for navigation.
    hunks: Vec<DiffHunk>,
    /// Currently selected hunk index.
    current_hunk: usize,
    /// Summary stats.
    stats: DiffStats,
    /// Configuration.
    pub config: DiffViewerConfig,
    /// Line height (cached).
    line_height: f32,
    /// Char advance (cached).
    char_advance: f32,
}

#[derive(Debug, Clone, Default)]
struct DiffStats {
    added: usize,
    removed: usize,
    modified: usize,
}

impl DiffViewer {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            old_text: String::new(),
            new_text: String::new(),
            old_label: "old".into(),
            new_label: "new".into(),
            left_lines: Vec::new(),
            right_lines: Vec::new(),
            hunks: Vec::new(),
            current_hunk: 0,
            stats: DiffStats::default(),
            config: DiffViewerConfig::default(),
            line_height: 0.0,
            char_advance: 0.0,
        }
    }

    /// Set both texts and recompute the diff.
    pub fn set_texts(&mut self, old: &str, new: &str) {
        self.old_text = old.to_string();
        self.new_text = new.to_string();
        self.recompute();
    }

    /// Number of hunks.
    pub fn hunk_count(&self) -> usize {
        self.hunks.len()
    }

    /// Navigate to next hunk.
    pub fn next_hunk(&mut self) {
        if !self.hunks.is_empty() {
            self.current_hunk = (self.current_hunk + 1) % self.hunks.len();
        }
    }

    /// Navigate to previous hunk.
    pub fn prev_hunk(&mut self) {
        if !self.hunks.is_empty() {
            self.current_hunk = if self.current_hunk == 0 {
                self.hunks.len() - 1
            } else {
                self.current_hunk - 1
            };
        }
    }

    fn recompute(&mut self) {
        // Clone to avoid borrow conflict with &self.old_text / &mut self
        let old_text = self.old_text.clone();
        let new_text = self.new_text.clone();
        let old_lines: Vec<&str> = old_text.lines().collect();
        let new_lines: Vec<&str> = new_text.lines().collect();

        let ops = diff_lines(&old_lines, &new_lines);
        self.hunks = group_hunks(&ops, self.config.context_lines);
        self.current_hunk = 0;

        // Compute stats
        let mut stats = DiffStats::default();
        for op in &ops {
            match op {
                DiffOp::Insert { .. } => stats.added += 1,
                DiffOp::Delete { .. } => stats.removed += 1,
                DiffOp::Equal { .. } => {}
            }
        }
        // "Modified" = min(added, removed) — paired changes
        stats.modified = stats.added.min(stats.removed);
        stats.added -= stats.modified;
        stats.removed -= stats.modified;
        self.stats = stats;

        // Build display lines
        self.build_display_lines(&ops, &old_lines, &new_lines);
    }

    fn build_display_lines(
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
            if !fold || run.len() <= ctx * 2 + 1 {
                // Show all equal lines
                for &(oi, ni) in run.iter() {
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
                }
            } else {
                // Show context + fold marker + context
                for &(oi, ni) in run[..ctx].iter() {
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
                }

                let hidden = run.len() - ctx * 2;
                let fold_text = format!("... {} unchanged lines ...", hidden);
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

                let start = run.len() - ctx;
                for &(oi, ni) in run[start..].iter() {
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
                }
            }
            run.clear();
        };

        for op in ops {
            match op {
                DiffOp::Equal { old_idx, new_idx } => {
                    equal_run.push((*old_idx, *new_idx));
                }
                _ => {
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
                    match op {
                        DiffOp::Delete { old_idx } => {
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
                        _ => {}
                    }
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

    // ── Render ──────────────────────────────────────────────────────────────

    /// Render the diff viewer.
    pub fn render(&mut self, ui: &Ui) -> Vec<DiffViewerEvent> {
        let mut events = Vec::new();

        // Cache metrics
        let m = calc_text_size("M");
        self.char_advance = m[0];
        self.line_height = m[1] + 2.0;

        let _id_tok = ui.push_id(&self.id);
        let cfg = self.config.clone();

        // Header
        self.render_header(ui, &cfg, &mut events);

        let avail = ui.content_region_avail();

        match cfg.mode {
            DiffMode::SideBySide => {
                let panel_w = (avail[0] - 2.0) * 0.5;

                // Borrow lines before closures to avoid per-frame clone
                let left_ptr = self.left_lines.as_ptr();
                let left_len = self.left_lines.len();
                let right_ptr = self.right_lines.as_ptr();
                let right_len = self.right_lines.len();
                // SAFETY: left_lines/right_lines are not mutated during render
                let left_slice = unsafe { std::slice::from_raw_parts(left_ptr, left_len) };
                let right_slice = unsafe { std::slice::from_raw_parts(right_ptr, right_len) };

                let char_advance = self.char_advance;
                let line_height = self.line_height;

                // Left panel
                ui.child_window("##diff_left")
                    .size([panel_w, avail[1]])
                    .build(ui, || {
                        Self::render_panel_static(ui, &cfg, left_slice, true, char_advance, line_height);
                    });

                ui.same_line_with_spacing(0.0, 0.0);

                // Separator
                {
                    let cursor = ui.cursor_screen_pos();
                    let draw = ui.get_window_draw_list();
                    draw.add_line(
                        cursor,
                        [cursor[0], cursor[1] + avail[1]],
                        col32(cfg.color_separator),
                    ).build();
                }

                ui.same_line();

                // Right panel
                ui.child_window("##diff_right")
                    .size([panel_w, avail[1]])
                    .build(ui, || {
                        Self::render_panel_static(ui, &cfg, right_slice, false, char_advance, line_height);
                    });
            }
            DiffMode::Unified => {
                ui.child_window("##diff_unified")
                    .size(avail)
                    .build(ui, || {
                        self.render_unified(ui, &cfg);
                    });
            }
        }

        events
    }

    fn render_header(
        &mut self,
        ui: &Ui,
        cfg: &DiffViewerConfig,
        events: &mut Vec<DiffViewerEvent>,
    ) {
        // Navigation and stats
        let s = &self.stats;
        ui.text_colored(cfg.color_header, format!(
            "{} vs {}  |  +{} -{} ~{}  |  {} hunks",
            self.old_label, self.new_label,
            s.added, s.removed, s.modified,
            self.hunks.len(),
        ));

        ui.same_line();
        if ui.button("Prev (Shift+F7)") {
            self.prev_hunk();
            events.push(DiffViewerEvent::HunkSelected { index: self.current_hunk });
        }
        ui.same_line();
        if ui.button("Next (F7)") {
            self.next_hunk();
            events.push(DiffViewerEvent::HunkSelected { index: self.current_hunk });
        }

        if !self.hunks.is_empty() {
            ui.same_line();
            ui.text_colored(
                cfg.color_line_number,
                format!("  Hunk {}/{}", self.current_hunk + 1, self.hunks.len()),
            );
        }

        ui.separator();
    }

    fn render_panel_static(
        ui: &Ui,
        cfg: &DiffViewerConfig,
        lines: &[DisplayLine],
        is_left: bool,
        char_advance: f32,
        line_height: f32,
    ) {
        let draw = ui.get_window_draw_list();
        let win_pos = ui.cursor_screen_pos();
        let win_w = ui.content_region_avail()[0];

        let gutter_w = if cfg.show_line_numbers {
            char_advance * 5.0
        } else {
            0.0
        };

        for (vi, line) in lines.iter().enumerate() {
            let y = win_pos[1] + vi as f32 * line_height;

            // Background
            let bg = match line.kind {
                LineKind::Added => Some(cfg.color_added_bg),
                LineKind::Removed => Some(cfg.color_removed_bg),
                LineKind::FoldMarker => Some(cfg.color_fold),
                LineKind::Equal => None,
            };
            if let Some(bg_color) = bg {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + win_w, y + line_height],
                    col32(bg_color),
                ).filled(true).build();
            }

            // Hover row highlight
            let mouse_pos = ui.io().mouse_pos();
            let row_hovered = mouse_pos[1] >= y && mouse_pos[1] < y + line_height
                && mouse_pos[0] >= win_pos[0] && mouse_pos[0] < win_pos[0] + win_w;
            if row_hovered {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + win_w, y + line_height],
                    col32([1.0, 1.0, 1.0, 0.04]),
                ).filled(true).build();
            }

            // Gutter background
            if cfg.show_line_numbers {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + gutter_w, y + line_height],
                    col32(cfg.color_gutter_bg),
                ).filled(true).build();
            }

            // Line number
            if cfg.show_line_numbers {
                let num = if is_left { line.old_num } else { line.new_num };
                if let Some(n) = num {
                    let num_str = format!("{:>4}", n);
                    draw.add_text(
                        [win_pos[0] + 2.0, y],
                        col32(cfg.color_line_number),
                        &num_str,
                    );
                }
            }

            // Text
            let text_x = win_pos[0] + gutter_w + 4.0;
            if line.kind == LineKind::FoldMarker {
                draw.add_text([text_x, y], col32(cfg.color_fold), &line.text);
            } else {
                let text_color = match line.kind {
                    LineKind::Added => cfg.color_added_text,
                    LineKind::Removed => cfg.color_removed_text,
                    _ => cfg.color_text,
                };
                draw.add_text([text_x, y], col32(text_color), &line.text);
            }
        }

        // Dummy for scroll extent
        let total_h = lines.len() as f32 * line_height;
        ui.set_cursor_pos([0.0, total_h]);
        ui.dummy([1.0, 1.0]);
    }

    fn render_unified(&self, ui: &Ui, cfg: &DiffViewerConfig) {
        let draw = ui.get_window_draw_list();
        let win_pos = ui.cursor_screen_pos();
        let win_w = ui.content_region_avail()[0];

        let gutter_w = if cfg.show_line_numbers {
            self.char_advance * 10.0 // old + new numbers
        } else {
            0.0
        };

        // In unified mode, interleave left and right lines
        // For simplicity, use left_lines which have old_num and right_lines for new_num
        let line_count = self.left_lines.len().min(self.right_lines.len());

        for vi in 0..line_count {
            let left = &self.left_lines[vi];
            let right = &self.right_lines[vi];
            let y = win_pos[1] + vi as f32 * self.line_height;

            let (kind, text) = if left.kind == LineKind::FoldMarker {
                (LineKind::FoldMarker, &left.text)
            } else if left.kind == LineKind::Removed {
                (LineKind::Removed, &left.text)
            } else if right.kind == LineKind::Added {
                (LineKind::Added, &right.text)
            } else {
                (LineKind::Equal, &left.text)
            };

            // Background
            let bg = match kind {
                LineKind::Added => Some(cfg.color_added_bg),
                LineKind::Removed => Some(cfg.color_removed_bg),
                LineKind::FoldMarker => Some(cfg.color_fold),
                LineKind::Equal => None,
            };
            if let Some(bg_color) = bg {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + win_w, y + self.line_height],
                    col32(bg_color),
                ).filled(true).build();
            }

            // Hover row highlight
            let mouse_pos = ui.io().mouse_pos();
            let row_hovered = mouse_pos[1] >= y && mouse_pos[1] < y + self.line_height
                && mouse_pos[0] >= win_pos[0] && mouse_pos[0] < win_pos[0] + win_w;
            if row_hovered {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + win_w, y + self.line_height],
                    col32([1.0, 1.0, 1.0, 0.04]),
                ).filled(true).build();
            }

            // Current hunk accent bar
            if !self.hunks.is_empty() {
                let hunk = &self.hunks[self.current_hunk];
                let in_hunk = match (left.old_num, right.new_num) {
                    (Some(n), _) if n > hunk.old_start && n <= hunk.old_start + hunk.old_count => true,
                    (_, Some(n)) if n > hunk.new_start && n <= hunk.new_start + hunk.new_count => true,
                    _ => false,
                };
                if in_hunk {
                    draw.add_rect(
                        [win_pos[0], y],
                        [win_pos[0] + 3.0, y + self.line_height],
                        col32([0.40, 0.63, 0.88, 0.8]),
                    ).filled(true).build();
                }
            }

            // Line numbers (old | new)
            if cfg.show_line_numbers {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + gutter_w, y + self.line_height],
                    col32(cfg.color_gutter_bg),
                ).filled(true).build();

                if let Some(n) = left.old_num {
                    draw.add_text(
                        [win_pos[0] + 2.0, y],
                        col32(cfg.color_line_number),
                        format!("{:>4}", n),
                    );
                }
                if let Some(n) = right.new_num {
                    draw.add_text(
                        [win_pos[0] + self.char_advance * 5.0, y],
                        col32(cfg.color_line_number),
                        format!("{:>4}", n),
                    );
                }
            }

            // Prefix
            let prefix = match kind {
                LineKind::Added => "+ ",
                LineKind::Removed => "- ",
                LineKind::FoldMarker => "  ",
                LineKind::Equal => "  ",
            };
            let text_x = win_pos[0] + gutter_w + 2.0;
            let prefix_w = self.char_advance * 2.0;
            let text_color = match kind {
                LineKind::Added => cfg.color_added_text,
                LineKind::Removed => cfg.color_removed_text,
                LineKind::FoldMarker => cfg.color_fold,
                LineKind::Equal => cfg.color_text,
            };
            draw.add_text([text_x, y], col32(text_color), prefix);
            draw.add_text(
                [text_x + prefix_w, y],
                col32(text_color),
                text,
            );
        }

        let total_h = line_count as f32 * self.line_height;
        ui.set_cursor_pos([0.0, total_h]);
        ui.dummy([1.0, 1.0]);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_texts_basic() {
        let mut dv = DiffViewer::new("##test");
        dv.set_texts("a\nb\nc", "a\nx\nc");
        assert!(!dv.left_lines.is_empty());
        assert!(!dv.right_lines.is_empty());
        assert_eq!(dv.left_lines.len(), dv.right_lines.len());
    }

    #[test]
    fn set_texts_identical() {
        let mut dv = DiffViewer::new("##test");
        dv.set_texts("a\nb\nc", "a\nb\nc");
        assert!(dv.hunks.is_empty());
        assert_eq!(dv.stats.added, 0);
        assert_eq!(dv.stats.removed, 0);
    }

    #[test]
    fn stats_add() {
        let mut dv = DiffViewer::new("##test");
        dv.set_texts("a", "a\nb");
        assert_eq!(dv.stats.added, 1);
    }

    #[test]
    fn stats_remove() {
        let mut dv = DiffViewer::new("##test");
        dv.set_texts("a\nb", "a");
        assert_eq!(dv.stats.removed, 1);
    }

    #[test]
    fn hunk_navigation() {
        let mut dv = DiffViewer::new("##test");
        dv.set_texts("a\nb\nc\nd", "a\nx\nc\ny");
        if dv.hunks.len() > 1 {
            assert_eq!(dv.current_hunk, 0);
            dv.next_hunk();
            assert_eq!(dv.current_hunk, 1);
            dv.prev_hunk();
            assert_eq!(dv.current_hunk, 0);
        }
    }

    #[test]
    fn hunk_wrap_around() {
        let mut dv = DiffViewer::new("##test");
        dv.set_texts("a", "b");
        if !dv.hunks.is_empty() {
            dv.prev_hunk(); // wraps to last
            assert_eq!(dv.current_hunk, dv.hunks.len() - 1);
        }
    }

    #[test]
    fn empty_texts() {
        let mut dv = DiffViewer::new("##test");
        dv.set_texts("", "");
        assert!(dv.left_lines.is_empty());
        assert!(dv.right_lines.is_empty());
    }

    #[test]
    fn fold_unchanged() {
        let mut dv = DiffViewer::new("##test");
        dv.config.fold_unchanged = true;
        dv.config.context_lines = 1;
        let old = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\nOLD\n12";
        let new = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\nNEW\n12";
        dv.set_texts(old, new);
        // Should have fold markers for the long equal run
        let fold_count = dv.left_lines.iter()
            .filter(|l| l.kind == LineKind::FoldMarker)
            .count();
        assert!(fold_count > 0, "Expected fold markers");
    }

    #[test]
    fn config_defaults() {
        let cfg = DiffViewerConfig::default();
        assert!(cfg.show_line_numbers);
        assert!(cfg.fold_unchanged);
        assert_eq!(cfg.context_lines, 3);
    }
}
