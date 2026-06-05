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

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
pub mod diff;

mod build; // DiffViewer::build_display_lines
mod render; // DiffViewer::render + panel/unified drawing

pub use config::{DiffMode, DiffViewerConfig};
pub use diff::{DiffHunk, DiffOp, diff_lines, group_hunks};

use dear_imgui_rs::Ui;

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// Half-open `[first, last)` index range of rows that intersect the
/// visible scroll viewport, with one row of slack on each side so a
/// partially-scrolled row is never clipped. Returns `0..0` when there
/// is nothing to draw. Keeps the per-frame draw cost proportional to
/// the visible row count instead of the whole (potentially huge) diff.
fn visible_range(scroll_y: f32, view_h: f32, line_height: f32, total: usize) -> (usize, usize) {
    if total == 0 || line_height <= 0.0 {
        return (0, 0);
    }
    let first = ((scroll_y / line_height).floor() as isize - 1).max(0) as usize;
    let visible_rows = (view_h / line_height).ceil() as usize + 2;
    let last = first.saturating_add(visible_rows).min(total);
    (first.min(total), last)
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
    /// Last known scroll Y for sync (SideBySide mode).
    sync_scroll_y: f32,
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
            sync_scroll_y: 0.0,
        }
    }

    /// Override the user-visible language. Default English; pass
    /// [`crate::i18n::Locale::Ru`] for Russian. The host must bake
    /// `GlyphRanges::Cyrillic` into the active font atlas.
    #[must_use]
    pub fn with_locale(mut self, locale: crate::i18n::Locale) -> Self {
        self.config.locale = locale;
        self
    }

    /// Mid-flight language switch.
    pub fn set_locale(&mut self, locale: crate::i18n::Locale) {
        self.config.locale = locale;
    }

    /// Currently-active locale.
    pub fn locale(&self) -> crate::i18n::Locale {
        self.config.locale
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
        let fold_count = dv
            .left_lines
            .iter()
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

    #[test]
    fn left_right_lines_stay_equal_length() {
        // Side-by-side row pairing must keep both columns the same
        // height for any mix of edits, else the renderer would mis-align
        // gutters. Exercise inserts, deletes and changes together.
        for (old, new) in [
            ("a\nb\nc", "a\nx\nc"),
            ("a\nb\nc\nd\ne", "x\nb\ny\nd\nz"),
            ("a", "a\nb\nc\nd"),
            ("a\nb\nc\nd", "a"),
            ("", "a\nb"),
            ("a\nb", ""),
        ] {
            let mut dv = DiffViewer::new("##test");
            dv.config.fold_unchanged = false;
            dv.set_texts(old, new);
            assert_eq!(
                dv.left_lines.len(),
                dv.right_lines.len(),
                "left/right mismatch for {old:?} -> {new:?}"
            );
        }
    }

    #[test]
    fn redisplay_is_stable_across_recompute() {
        // Re-running the same diff must yield identical display lines
        // (same texts, kinds and line numbers) — the renderer caches
        // these between frames and relies on stability.
        let mut dv = DiffViewer::new("##test");
        dv.set_texts("a\nb\nc\nd", "a\nB\nc\nD");
        let snapshot: Vec<_> = dv
            .left_lines
            .iter()
            .map(|l| (l.old_num, l.new_num, l.kind, l.text.clone()))
            .collect();
        dv.set_texts("a\nb\nc\nd", "a\nB\nc\nD");
        let again: Vec<_> = dv
            .left_lines
            .iter()
            .map(|l| (l.old_num, l.new_num, l.kind, l.text.clone()))
            .collect();
        assert_eq!(snapshot, again);
    }

    #[test]
    fn changed_lines_have_correct_line_numbers() {
        // Regression guard against off-by-one gutter math: the changed
        // middle line must keep 1-based numbering, and equal lines must
        // carry the right old/new indices on their respective sides.
        let mut dv = DiffViewer::new("##test");
        dv.config.fold_unchanged = false;
        dv.set_texts("a\nb\nc", "a\nx\nc");
        // Left side: line 1 = a(eq), 2 = b(removed), then added blank, 3 = c(eq)
        let removed = dv
            .left_lines
            .iter()
            .find(|l| l.kind == LineKind::Removed)
            .expect("a removed line");
        assert_eq!(removed.old_num, Some(2));
        let added = dv
            .right_lines
            .iter()
            .find(|l| l.kind == LineKind::Added)
            .expect("an added line");
        assert_eq!(added.new_num, Some(2));
    }

    // ── Viewport culling ────────────────────────────────────────────────────

    #[test]
    fn visible_range_empty_and_degenerate() {
        assert_eq!(visible_range(0.0, 100.0, 14.0, 0), (0, 0));
        assert_eq!(visible_range(0.0, 100.0, 0.0, 50), (0, 0));
    }

    #[test]
    fn visible_range_top_of_scroll() {
        // At scroll 0 with a 100px viewport and 10px rows, ~12 rows
        // (10 visible + 2 slack) starting at 0.
        let (first, last) = visible_range(0.0, 100.0, 10.0, 1000);
        assert_eq!(first, 0);
        assert_eq!(last, 12);
    }

    #[test]
    fn visible_range_scrolled_middle() {
        // Scrolled 500px -> row 50; window shows ~rows 49..61.
        let (first, last) = visible_range(500.0, 100.0, 10.0, 1000);
        assert_eq!(first, 49);
        assert!(last > first && last <= 1000);
        assert!(last - first <= 13);
    }

    #[test]
    fn visible_range_clamps_to_total() {
        let (first, last) = visible_range(99_999.0, 100.0, 10.0, 30);
        assert!(first <= 30 && last <= 30 && first <= last);
    }

    // ── i18n guard tests (project requirement) ──────────────────────────────

    #[test]
    fn diff_viewer_strings_resolve() {
        let en = crate::i18n::diff_viewer::strings(crate::i18n::Locale::En);
        let ru = crate::i18n::diff_viewer::strings(crate::i18n::Locale::Ru);
        assert_eq!(en.prev_button, "Prev (Shift+F7)");
        assert_eq!(en.next_button, "Next (F7)");
        assert_eq!(ru.prev_button, "Назад (Shift+F7)");
        assert_eq!(ru.next_button, "Вперёд (F7)");
    }

    #[test]
    fn default_locale_is_english() {
        assert_eq!(DiffViewerConfig::default().locale, crate::i18n::Locale::En);
        assert_eq!(DiffViewer::new("##test").locale(), crate::i18n::Locale::En);
    }

    #[test]
    fn locale_round_trips_through_ron() {
        let cfg = DiffViewerConfig {
            locale: crate::i18n::Locale::Ru,
            ..DiffViewerConfig::default()
        };
        let text = ron::ser::to_string(&cfg).unwrap();
        let back: DiffViewerConfig = ron::from_str(&text).unwrap();
        assert_eq!(back.locale, crate::i18n::Locale::Ru);
    }

    #[test]
    fn locale_field_optional_in_ron() {
        // A `config.ron` predating the `locale` field must still parse,
        // defaulting to English via `#[serde(default)]`.
        let cfg: DiffViewerConfig = ron::from_str(
            r#"(
                mode: SideBySide,
                show_line_numbers: true,
                fold_unchanged: true,
                context_lines: 3,
                show_minimap: false,
                sync_scroll: true,
                color_bg: (0.11, 0.11, 0.13, 1.0),
                color_gutter_bg: (0.13, 0.14, 0.16, 1.0),
                color_line_number: (0.40, 0.42, 0.48, 1.0),
                color_text: (0.85, 0.87, 0.90, 1.0),
                color_added_bg: (0.15, 0.30, 0.18, 0.5),
                color_added_text: (0.55, 0.90, 0.55, 1.0),
                color_removed_bg: (0.35, 0.15, 0.15, 0.5),
                color_removed_text: (0.90, 0.55, 0.55, 1.0),
                color_modified_bg: (0.30, 0.28, 0.15, 0.4),
                color_inline_change: (0.90, 0.75, 0.20, 0.35),
                color_fold: (0.35, 0.38, 0.45, 0.7),
                color_header: (0.50, 0.55, 0.65, 1.0),
                color_separator: (0.25, 0.27, 0.32, 0.8),
                color_current_hunk: (0.30, 0.45, 0.65, 0.3),
            )"#,
        )
        .expect("legacy ron without locale must parse");
        assert_eq!(cfg.locale, crate::i18n::Locale::En);
    }

    #[test]
    fn with_locale_and_set_locale_round_trip() {
        let mut dv = DiffViewer::new("##test").with_locale(crate::i18n::Locale::Ru);
        assert_eq!(dv.locale(), crate::i18n::Locale::Ru);
        dv.set_locale(crate::i18n::Locale::En);
        assert_eq!(dv.locale(), crate::i18n::Locale::En);
    }

    #[test]
    fn config_ron_round_trips_fully() {
        // Defaults come from config.ron; a full serialize/deserialize
        // cycle must be lossless (DDD config pattern guard).
        let cfg = DiffViewerConfig::default();
        let text = ron::ser::to_string(&cfg).unwrap();
        let back: DiffViewerConfig = ron::from_str(&text).unwrap();
        assert_eq!(back.mode, cfg.mode);
        assert_eq!(back.context_lines, cfg.context_lines);
        assert_eq!(back.color_added_bg, cfg.color_added_bg);
        assert_eq!(back.locale, cfg.locale);
    }
}
