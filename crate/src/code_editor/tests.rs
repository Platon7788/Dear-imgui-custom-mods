//! `CodeEditor` integration tests.
//!
//! Extracted from mod.rs (500-line rule). Declared as a `#[cfg(test)]`
//! submodule so it reaches the editor's private fields and methods.

use super::*;
use dear_imgui_rs::{BackendFlags, Context};

#[test]
fn test_new_editor() {
    let editor = CodeEditor::new("test");
    assert_eq!(editor.line_count(), 1);
    assert!(!editor.is_modified());
    assert!(!editor.is_read_only());
}

#[test]
fn test_set_get_text() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("fn main() {\n    println!(\"hi\");\n}");
    assert_eq!(editor.line_count(), 3);
    let text = editor.get_text();
    assert!(text.contains("fn main()"));
    assert!(text.contains("println!"));
}

#[test]
fn diagnostic_real_fold_detection_collapse() {
    let mut ed = CodeEditor::new("t");
    ed.config_mut().show_fold_indicators = true;
    ed.set_language(Language::Rust);
    // Brace block lines 0..=4, then two trailing lines (5, 6).
    ed.set_text("fn a() {\n    let x = 1;\n    let y = 2;\n    let z = 3;\n}\nlet after1 = 1;\nlet after2 = 2;");
    let n = ed.line_count();
    assert_eq!(n, 7);
    ed.update_fold_regions();
    ed.rebuild_fold_display();
    assert!(!ed.folds_active, "nothing folded on load");
    assert_eq!(ed.total_visual_rows(), n, "no collapse when unfolded");

    ed.toggle_fold(0);
    ed.rebuild_fold_display();
    assert!(ed.folds_active, "fold should be active after toggle");
    // Region (0,4): hide lines 1..=4 (4 lines) → 7 − 4 = 3 display rows.
    assert_eq!(ed.total_visual_rows(), 3, "collapse total");
    assert_eq!(
        ed.visual_row_of(5, 0),
        1,
        "after1 sits right after the header"
    );
    assert_eq!(ed.visual_row_to_line(1), (5, 0), "row 1 maps to after1");
    assert_eq!(ed.visual_row_to_line(2), (6, 0), "row 2 maps to after2");
}

#[test]
fn folded_region_collapses_visual_rows() {
    // 8 lines; fold header on line 1 hides lines 2..=4 (3 lines). The display
    // must collapse those 3 rows: total shrinks, rows below shift up, and
    // display-row↔line mapping (scroll + click) stays consistent.
    let mut ed = CodeEditor::new("t");
    ed.config_mut().show_fold_indicators = true;
    ed.set_text("0\n1\n2\n3\n4\n5\n6\n7");
    ed.fold_regions = vec![FoldRegion {
        start_line: 1,
        end_line: 4,
        folded: true,
    }];
    ed.rebuild_fold_display();
    assert!(ed.folds_active);
    assert_eq!(
        ed.total_visual_rows(),
        5,
        "8 lines − 3 hidden = 5 display rows"
    );
    // Rows: line0→0, header line1→1, [2..4 hidden], line5→2, line6→3, line7→4.
    assert_eq!(ed.visual_row_of(1, 0), 1);
    assert_eq!(ed.visual_row_of(5, 0), 2);
    assert_eq!(ed.visual_row_of(7, 0), 4);
    // Inverse (used by scroll + click mapping) must skip hidden lines and land
    // on the visible line.
    assert_eq!(ed.visual_row_to_line(1), (1, 0));
    assert_eq!(ed.visual_row_to_line(2), (5, 0));
    assert_eq!(ed.visual_row_to_line(4), (7, 0));
}

#[test]
fn folded_region_composes_with_word_wrap() {
    // Fold + wrap together: hidden lines contribute 0 rows, visible lines
    // contribute their wrapped row count, and the row of the line after the
    // fold accounts for the header line's wrapping.
    let mut ed = CodeEditor::new("t");
    ed.config_mut().show_fold_indicators = true;
    ed.config_mut().word_wrap = true;
    ed.char_advance = 10.0;
    ed.set_text("aaaa bbbb cccc dddd\nhdr\nhidden1\nhidden2\ntail");
    ed.update_wrap_cache(45.0);
    ed.fold_regions = vec![FoldRegion {
        start_line: 1,
        end_line: 3,
        folded: true,
    }];
    ed.rebuild_fold_display();
    let l0 = ed.wrap_cols[0].len() + 1; // line 0 wrapped rows
    let l4 = ed.wrap_cols[4].len() + 1; // "tail" rows
    assert!(l0 >= 2, "long line should wrap");
    assert_eq!(ed.total_visual_rows(), l0 + 1 + l4);
    // "tail" (line 4) starts right after line0's rows + the 1-row header.
    assert_eq!(ed.visual_row_of(4, 0), l0 + 1);
}

#[test]
fn no_active_fold_keeps_identity_rows() {
    // A region that exists but isn't folded must leave the (well-tested)
    // non-fold row math completely untouched.
    let mut ed = CodeEditor::new("t");
    ed.config_mut().show_fold_indicators = true;
    ed.set_text("a\nb\nc\nd");
    ed.fold_regions = vec![FoldRegion {
        start_line: 0,
        end_line: 2,
        folded: false,
    }];
    ed.rebuild_fold_display();
    assert!(!ed.folds_active);
    assert_eq!(ed.total_visual_rows(), 4);
    assert_eq!(ed.visual_row_of(2, 0), 2);
    assert_eq!(ed.visual_row_to_line(3), (3, 0));
}

#[test]
fn doc_max_line_width_is_document_wide_and_tab_aware() {
    let mut ed = CodeEditor::new("t");
    ed.char_advance = 10.0;
    ed.config_mut().tab_size = 4;
    // "\tx" = tab(4*10=40) + x(10) = 50 — the widest, and only correct if
    // tabs are measured by width (char-count would give 20 < "abc"'s 30).
    ed.set_text("abc\n\tx\nab");
    let w = ed.doc_max_line_width();
    assert!(
        (w - 50.0).abs() < 0.01,
        "expected 50 (tab-aware, doc-wide), got {w}"
    );
}

#[test]
fn block_comment_state_correct_after_line_delete_above() {
    // Deleting a line above a /* … */ block used to leave downstream lines
    // wrongly flagged in_block_comment (the convergence early-exit trusted the
    // positionally-shifted stored states).
    let mut ed = CodeEditor::new("t");
    ed.set_language(Language::Rust);
    ed.set_text("x\n/*\nstill\n*/\ncode");
    ed.update_block_comment_states();
    // Delete line 0 ("x") → "/*" line 0, "still" line 1, "*/" line 2, "code" line 3.
    ed.buffer.set_cursor(buffer::CursorPos::new(0, 0));
    ed.buffer.delete_line();
    ed.bc_dirty_from = Some(0);
    ed.update_block_comment_states();
    assert!(
        matches!(ed.block_comment_states[1], LineState::BlockComment(_)),
        "'still' must remain in the comment"
    );
    assert_eq!(
        ed.block_comment_states[3],
        LineState::Code,
        "'code' must NOT be flagged as in a block comment"
    );
}

#[test]
fn wrap_cache_incremental_matches_full_rebuild() {
    // Editing one line and rebuilding incrementally must yield the exact same
    // wrap layout as building the final text from scratch.
    let width = 45.0;
    let mut ed = CodeEditor::new("t");
    ed.config_mut().word_wrap = true;
    ed.char_advance = 10.0;
    ed.set_text("aaaa bbbb cccc dddd\nshort\neeee ffff gggg hhhh");
    ed.update_wrap_cache(width);
    ed.buffer.set_cursor(buffer::CursorPos::new(1, 5));
    ed.buffer.insert_text(" plus a good deal more text here");
    ed.update_wrap_cache(width); // incremental

    let mut fresh = CodeEditor::new("t");
    fresh.config_mut().word_wrap = true;
    fresh.char_advance = 10.0;
    fresh.set_text(&ed.get_text());
    fresh.update_wrap_cache(width); // full rebuild

    assert_eq!(ed.wrap_row_offsets, fresh.wrap_row_offsets);
    assert_eq!(ed.wrap_cols, fresh.wrap_cols);
}

#[test]
fn blink_toggles_by_parity_across_multiple_periods() {
    let mut ed = CodeEditor::new("t");
    ed.config_mut().cursor_blink_rate = 0.5;
    // cursor starts visible.
    ed.update_blink(1.0); // 2 whole periods → even → stays visible
    assert!(ed.cursor_visible);
    ed.update_blink(0.5); // 1 period → toggles → hidden
    assert!(!ed.cursor_visible);
    ed.update_blink(1.5); // 3 periods → odd → toggles → visible
    assert!(ed.cursor_visible);
}

#[test]
fn config_ron_colors_match_dark_default_theme() {
    // config.ron inlines the full dark palette; guard against it drifting from
    // EditorTheme::DarkDefault so a fresh editor's colours match set_theme().
    assert_eq!(
        EditorConfig::default().colors,
        EditorTheme::DarkDefault.colors()
    );
}

#[test]
fn default_locale_is_english() {
    assert_eq!(EditorConfig::default().locale, crate::i18n::Locale::En);
    let ed = CodeEditor::new("t");
    assert_eq!(ed.locale(), crate::i18n::Locale::En);
}

#[test]
fn locale_round_trips_through_ron() {
    // Lock in that `locale` is really Serialize + Deserialize (not skipped).
    let cfg = EditorConfig {
        locale: crate::i18n::Locale::Ru,
        ..EditorConfig::default()
    };
    let text = ron::ser::to_string(&cfg).unwrap();
    let back: EditorConfig = ron::from_str(&text).unwrap();
    assert_eq!(back.locale, crate::i18n::Locale::Ru);
}

#[test]
fn locale_field_optional_in_ron() {
    // Older config.ron files predate the locale field; `#[serde(default)]`
    // must let them parse (defaulting to En). Strip the locale line and
    // confirm the canonical ron still deserialises.
    let ron_src = include_str!("config.ron");
    let without_locale = ron_src
        .lines()
        .filter(|l| !l.trim_start().starts_with("locale:"))
        .collect::<Vec<_>>()
        .join("\n");
    let cfg: EditorConfig =
        ron::from_str(&without_locale).expect("config.ron parses without a locale field");
    assert_eq!(cfg.locale, crate::i18n::Locale::En);
}

#[test]
fn word_at_cursor_at_end_of_word() {
    // A caret sitting just past the last char of a word (a valid position)
    // should still report that word, like every mainstream editor.
    let mut editor = CodeEditor::new("t");
    editor.set_text("bar foo");
    // Caret one past the last char — also the end of the line (col == len),
    // which the old `col >= len` guard rejected outright.
    editor.buffer.set_cursor(buffer::CursorPos::new(0, 7)); // "bar foo|"
    assert_eq!(editor.word_at_cursor().as_deref(), Some("foo"));
}

#[test]
fn replace_current_does_not_re_match_its_own_replacement() {
    // Find "cat" / Replace "cats": the replacement contains the query, so a
    // naive rebuild would keep re-selecting the replacement and grow the same
    // spot on every click. Replace must advance past the inserted text.
    let mut editor = CodeEditor::new("t");
    editor.set_text("cat cat cat");
    editor.find_replace.query = "cat".to_string();
    editor.find_replace.replacement = "cats".to_string();
    editor.update_find_matches();
    editor.find_replace.current_match = 0;
    editor.replace_current();
    editor.replace_current();
    editor.replace_current();
    assert_eq!(editor.get_text(), "cats cats cats");
}

#[test]
fn test_language() {
    let mut editor = CodeEditor::new("test");
    editor.set_language(Language::Toml);
    assert_eq!(editor.config().language, Language::Toml);
}

#[test]
fn test_read_only() {
    let mut editor = CodeEditor::new("test");
    editor.set_read_only(true);
    assert!(editor.is_read_only());
}

#[test]
fn test_goto_line() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("line1\nline2\nline3\nline4\nline5");
    editor.goto_line(3);
    assert_eq!(editor.cursor().line, 3);
}

#[test]
fn test_modified_flag() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("hello");
    assert!(!editor.is_modified());
    editor.buffer.insert_char('x');
    assert!(editor.is_modified());
    editor.clear_modified();
    assert!(!editor.is_modified());
}

#[test]
fn test_error_markers() {
    let mut editor = CodeEditor::new("test");
    editor.set_error_markers(vec![LineMarker {
        line: 5,
        message: "error here".into(),
        is_error: true,
    }]);
    assert_eq!(editor.error_markers.len(), 1);
}

// digit_count moved to helpers.rs with its own tests.
// hash_line likewise moved to helpers.rs.
// find_replace tests (test_find_matches, test_find_case_insensitive)
// moved to find_replace.rs.
// test_fold_regions moved to fold.rs.

#[test]
fn test_block_comment_states() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("/* start\nmiddle\nend */ code");
    editor.update_block_comment_states();
    assert_eq!(
        editor.block_comment_states,
        vec![
            LineState::Code,
            LineState::BlockComment(1),
            LineState::BlockComment(1)
        ]
    );
}

#[test]
fn test_closing_bracket() {
    assert_eq!(closing_bracket('('), Some(')'));
    assert_eq!(closing_bracket('{'), Some('}'));
    assert_eq!(closing_bracket('['), Some(']'));
    assert_eq!(closing_bracket('a'), None);
}

#[test]
fn test_duplicate_line() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("line1\nline2\nline3");
    editor.buffer.set_cursor(CursorPos::new(1, 0));
    editor.buffer.duplicate_line();
    assert_eq!(editor.line_count(), 4);
    assert_eq!(editor.buffer.line(1), "line2");
    assert_eq!(editor.buffer.line(2), "line2");
}

#[test]
fn test_move_line_up() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("aaa\nbbb\nccc");
    editor.buffer.set_cursor(CursorPos::new(1, 0));
    editor.buffer.move_line_up();
    assert_eq!(editor.buffer.line(0), "bbb");
    assert_eq!(editor.buffer.line(1), "aaa");
}

#[test]
fn test_move_line_down() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("aaa\nbbb\nccc");
    editor.buffer.set_cursor(CursorPos::new(1, 0));
    editor.buffer.move_line_down();
    assert_eq!(editor.buffer.line(1), "ccc");
    assert_eq!(editor.buffer.line(2), "bbb");
}

#[test]
fn test_toggle_comment() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("fn main() {\n    let x = 1;\n}");
    editor.buffer.toggle_line_comment(1..2);
    assert_eq!(editor.buffer.line(1), "    // let x = 1;");
    // Toggle again to uncomment
    editor.buffer.toggle_line_comment(1..2);
    assert_eq!(editor.buffer.line(1), "    let x = 1;");
}

#[test]
fn test_sub_row_col_range_stale_wrap_cols() {
    // Reproduces the packet-editor panic: when the buffer shrinks but
    // wrap_cols is still sized for the old content (the in-frame window
    // between handle_keyboard editing the buffer and update_wrap_cache
    // refreshing the cache), sub_row_col_range can return start > end.
    // handle_mouse then computes `end - start` as a usize subtraction and
    // panics with attempt-to-subtract-with-overflow.
    let mut editor = CodeEditor::new("test");
    editor.config_mut().word_wrap = true;

    // Pretend the cache reflects a long wrapped line that has since been
    // cleared — wrap_cols still has 5 wrap points, but buffer.line(0) is
    // now empty.
    editor.wrap_cols = vec![vec![10, 20, 30, 40, 50]];
    editor.wrap_row_offsets = vec![0, 6];
    // buffer is empty (default state — one empty line).

    // sub_row = wraps.len() falls off the end: wraps.get(sub_row) is None
    // so `end = buffer.line(0).chars().count() = 0`, while `start =
    // wraps.get(sub_row - 1) = 50`. Before the fix this returned (50, 0).
    let (start, end) = editor.sub_row_col_range(0, 5);
    assert!(
        start <= end,
        "sub_row_col_range returned start={start} > end={end}"
    );
}

#[test]
fn test_nxt_hex_editor_select_all_delete() {
    // Reproduces NxT packet-editor crash: select-all + delete on a
    // hex editor configured like packet_monitor's send buffer.
    let mut editor = CodeEditor::new("##hex_editor");
    {
        let cfg = editor.config_mut();
        cfg.language = Language::Hex;
        cfg.hex_auto_uppercase = true;
        cfg.hex_auto_space = true;
        cfg.word_wrap = true;
        cfg.force_english_on_focus = true;
        cfg.smooth_scrolling = false;
        cfg.show_fold_indicators = false;
        cfg.max_lines = 999;
        cfg.max_line_length = 65000;
    }
    // Simulate 50 captured packets.
    let mut text = String::new();
    for _ in 0..50 {
        text.push_str("AA BB CC DD EE FF 01 02 03 04\n");
    }
    editor.set_text(&text);
    assert!(editor.line_count() > 10);

    // Ctrl+A
    editor.buffer.select_all();
    // Delete — the same path the editor takes for Delete/Backspace.
    editor.snapshot_undo(true);
    editor.buffer.delete();
    editor.invalidate_token_cache_from(editor.buffer.cursor().line);

    assert_eq!(editor.line_count(), 1);
    assert_eq!(editor.get_text(), "");
}

#[test]
fn test_delete_line() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("aaa\nbbb\nccc");
    editor.buffer.set_cursor(CursorPos::new(1, 0));
    editor.buffer.delete_line();
    assert_eq!(editor.line_count(), 2);
    assert_eq!(editor.buffer.line(0), "aaa");
    assert_eq!(editor.buffer.line(1), "ccc");
}

// ── Find / Replace regression tests (audit) ───────────────────────────

/// `replace_all` must replace every match without index drift, even when
/// the replacement is a different length than the query (the bottom-to-top
/// ordering is what keeps earlier match columns valid).
#[test]
fn test_replace_all_no_index_drift() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("foo foo foo\nbar foo\nfoo");
    editor.find_replace.query = "foo".to_string();
    editor.find_replace.replacement = "longer_word".to_string();
    editor.update_find_matches();
    assert_eq!(editor.find_replace.matches.len(), 5);
    editor.replace_all();
    assert_eq!(
        editor.get_text(),
        "longer_word longer_word longer_word\nbar longer_word\nlonger_word"
    );
    // All matches consumed.
    assert_eq!(editor.find_replace.matches.len(), 0);
}

/// Replacing with a shorter string must not leave stale columns behind —
/// catches the inverse drift direction.
#[test]
fn test_replace_all_shorter_replacement() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("aXa Xa aaX");
    editor.find_replace.query = "X".to_string();
    editor.find_replace.replacement = "".to_string();
    editor.update_find_matches();
    assert_eq!(editor.find_replace.matches.len(), 3);
    editor.replace_all();
    assert_eq!(editor.get_text(), "aa a aa");
}

/// Case-insensitive find then replace preserves the document's original
/// casing everywhere except the replaced ranges.
#[test]
fn test_replace_all_case_insensitive() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("Hello HELLO hello");
    editor.find_replace.case_sensitive = false;
    editor.find_replace.query = "hello".to_string();
    editor.find_replace.replacement = "hi".to_string();
    editor.update_find_matches();
    assert_eq!(editor.find_replace.matches.len(), 3);
    editor.replace_all();
    assert_eq!(editor.get_text(), "hi hi hi");
}

/// `replace_all` on UTF-8 text must slice on char boundaries (would panic
/// otherwise) and replace the correct ranges.
#[test]
fn test_replace_all_multibyte() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("café café");
    editor.find_replace.query = "café".to_string();
    editor.find_replace.replacement = "tea".to_string();
    editor.update_find_matches();
    assert_eq!(editor.find_replace.matches.len(), 2);
    editor.replace_all();
    assert_eq!(editor.get_text(), "tea tea");
}

/// `replace_current` replaces only the active match and re-scans, so the
/// next `find_next` lands on a fresh match (no stale index into a shrunk
/// match list).
#[test]
fn test_replace_current_then_navigate() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("x x x");
    editor.find_replace.query = "x".to_string();
    editor.find_replace.replacement = "y".to_string();
    editor.update_find_matches();
    assert_eq!(editor.find_replace.matches.len(), 3);
    editor.replace_current();
    // One replaced, two remain.
    assert_eq!(editor.find_replace.matches.len(), 2);
    assert_eq!(editor.get_text(), "y x x");
    editor.replace_current();
    assert_eq!(editor.get_text(), "y y x");
}

/// Find-in-selection scope must ignore matches outside the active selection.
#[test]
fn test_find_in_selection_scope() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("foo\nfoo\nfoo");
    // Select only the middle line.
    editor
        .buffer
        .set_selection(CursorPos::new(1, 0), CursorPos::new(1, 3));
    editor.find_replace.scope = FindScope::Selection;
    editor.find_replace.query = "foo".to_string();
    editor.update_find_matches();
    assert_eq!(editor.find_replace.matches.len(), 1);
    assert_eq!(editor.find_replace.matches[0].0, 1); // line 1 only
}

// ── Undo / redo through the editor (audit) ────────────────────────────

/// Full undo/redo round-trip through the editor restores text and cursor,
/// keeps the modified flag set, and re-enables redo after undo.
#[test]
fn test_editor_undo_redo_round_trip() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("hello");
    editor.buffer.set_cursor(CursorPos::new(0, 5));

    // Force a discrete edit with its own undo snapshot.
    editor.snapshot_undo(true);
    editor.buffer.insert_text(" world");
    assert_eq!(editor.get_text(), "hello world");
    assert!(editor.can_undo());

    editor.undo();
    assert_eq!(editor.get_text(), "hello");
    assert!(
        editor.is_modified(),
        "undo leaves buffer differing from disk"
    );
    assert!(editor.can_redo());

    editor.redo();
    assert_eq!(editor.get_text(), "hello world");
}

/// `set_text` must clear the undo history (loading a fresh document is not
/// undoable back to the previous document).
#[test]
fn test_set_text_clears_undo() {
    let mut editor = CodeEditor::new("test");
    editor.set_text("first");
    editor.snapshot_undo(true);
    editor.buffer.insert_char('!');
    assert!(editor.can_undo());
    editor.set_text("second");
    assert!(!editor.can_undo(), "set_text must reset the undo stack");
}

// ── Marker colour honours config (audit: dead-field fix) ──────────────

/// `set_error_markers` populates both the line set and the marker list so
/// the draw path can distinguish error vs warning (colour comes from the
/// theme, not a hardcoded value).
#[test]
fn test_error_and_warning_markers_tracked() {
    let mut editor = CodeEditor::new("test");
    editor.set_error_markers(vec![
        LineMarker {
            line: 2,
            message: "err".into(),
            is_error: true,
        },
        LineMarker {
            line: 5,
            message: "warn".into(),
            is_error: false,
        },
    ]);
    assert!(editor.error_lines.contains(&2));
    assert!(editor.error_lines.contains(&5));
    // Theme colours must be distinct so error/warning render differently.
    assert_ne!(
        editor.config().colors.error_underline,
        editor.config().colors.warning_underline
    );
}

// ── Config round-trip (DDD settings) ──────────────────────────────────

/// `EditorConfig` must survive a RON serialize → deserialize round-trip
/// with every (serializable) knob preserved. Guards the DDD config pattern.
#[test]
fn test_config_ron_round_trip() {
    let mut cfg = EditorConfig::default();
    cfg.tab_size = 2;
    cfg.word_wrap = true;
    cfg.max_lines = 1000;
    cfg.max_line_length = 120;
    cfg.smooth_scrolling = false;
    cfg.font_size_scale = 1.5;
    let s = ron::to_string(&cfg).expect("serialize EditorConfig");
    let back: EditorConfig = ron::from_str(&s).expect("deserialize EditorConfig");
    assert_eq!(back.tab_size, 2);
    assert!(back.word_wrap);
    assert_eq!(back.max_lines, 1000);
    assert_eq!(back.max_line_length, 120);
    assert!(!back.smooth_scrolling);
    assert!((back.font_size_scale - 1.5).abs() < f32::EPSILON);
}

/// The built-in `config.ron` must parse via `Default` (the DDD schema =
/// .ron values contract). Failure here means the schema and values drifted.
#[test]
fn test_default_config_loads_from_ron() {
    let cfg = EditorConfig::default();
    // Spot-check a few values that live only in config.ron.
    assert_eq!(cfg.tab_size, 4);
    assert!(cfg.insert_spaces);
    assert!(cfg.show_line_numbers);
    assert_eq!(cfg.language, Language::Rust); // set in EditorConfig::default()
}

// ── Keyboard input regression (Alt+Tab) ──────────────────────────────
//
// Dear ImGui allows at most one active `Context` per process, and `cargo
// test` runs tests on multiple threads by default. This mutex serializes
// the handful of tests below that create a live headless context — it
// mirrors the (crate-private, not exported) `lock_context()` guard
// `dear-imgui-winit`'s own test suite uses for the same reason.
static IMGUI_CTX_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Prepare a freshly-created headless `Context` so `Context::frame()` can be
/// called: set a valid display size, advertise texture-request support (so
/// `NewFrame()` doesn't require a real renderer backend), and build the font
/// atlas once up front. Also disables ini persistence — otherwise a stray
/// `imgui.ini` gets written into the working tree on every test run.
fn prepare_headless_context(ctx: &mut Context) {
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
    ctx.fonts().build();
}

#[test]
fn alt_tab_does_not_insert_indent() {
    // Regression test for the Alt+Tab bug: Windows delivers
    // WM_SYSKEYDOWN(VK_TAB) to the still-focused window *before* the OS
    // task-switcher moves focus away, so Dear ImGui's `io` can report
    // `key_alt() == true` and "Tab just pressed" simultaneously.
    // `handle_keyboard`'s Tab branch must treat that as the OS shortcut,
    // not an indent request — see the `!alt` guard in input.rs.
    let _guard = IMGUI_CTX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ctx = Context::create();
    prepare_headless_context(&mut ctx);

    let mut editor = CodeEditor::new("alt_tab_test");
    editor.set_text("hello");

    {
        let io = ctx.io_mut();
        // Alt held (both the synthetic ImGui modifier key and the physical
        // left-Alt key, mirroring how real backends report it) plus Tab
        // pressed this frame — the exact `io` state during an Alt+Tab.
        io.add_key_event(Key::ModAlt, true);
        io.add_key_event(Key::LeftAlt, true);
        io.add_key_event(Key::Tab, true);
    }
    let ui = ctx.frame();
    editor.handle_keyboard(ui);

    assert_eq!(
        editor.get_text(),
        "hello",
        "Tab pressed while Alt is held must not insert a tab/space indent"
    );
}

#[test]
fn tab_without_alt_still_indents() {
    // Control case for the fix above: plain Tab (no Alt) must still
    // insert an indent — guards against the `!alt` guard accidentally
    // disabling Tab-indent altogether.
    let _guard = IMGUI_CTX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ctx = Context::create();
    prepare_headless_context(&mut ctx);

    let mut editor = CodeEditor::new("tab_test");
    editor.set_text("hello");

    {
        let io = ctx.io_mut();
        io.add_key_event(Key::Tab, true);
    }
    let ui = ctx.frame();
    editor.handle_keyboard(ui);

    assert_eq!(
        editor.get_text(),
        "    hello",
        "Plain Tab (no Alt) must still insert its normal indent"
    );
}

// ── Property-based tests ─────────────────────────────────────────────

use proptest::prelude::*;

proptest! {
    /// `parse_hex_color` accepts arbitrary strings without panicking.
    /// Any Some(rgba) must be a 4-element array with values in [0,1].
    #[test]
    fn prop_parse_hex_color_never_panics(s in ".{0,16}") {
        if let Some(rgba) = parse_hex_color(&s) {
            for c in rgba {
                prop_assert!((0.0..=1.0).contains(&c));
            }
        }
    }

    /// `#RRGGBB` strings (6 hex digits) must decode successfully and
    /// the decoded RGBA must have alpha == 1.0, with each channel
    /// matching the input byte.
    #[test]
    fn prop_parse_hex_color_6_digit_decodes(
        r in any::<u8>(), g in any::<u8>(), b in any::<u8>(),
    ) {
        let s = format!("#{r:02X}{g:02X}{b:02X}");
        let rgba = parse_hex_color(&s).expect("valid 6-digit hex must parse");
        prop_assert!((rgba[3] - 1.0).abs() < f32::EPSILON);
        prop_assert_eq!((rgba[0] * 255.0).round() as u8, r);
        prop_assert_eq!((rgba[1] * 255.0).round() as u8, g);
        prop_assert_eq!((rgba[2] * 255.0).round() as u8, b);
    }
}

#[test]
fn context_menu_inline_in_config_ron_matches_canonical() {
    // `code_editor/config.ron` inlines `context_menu:(...)`; this
    // drift-test catches the case where one is updated without
    // the other.
    let canonical = ContextMenuConfig::default();
    let cfg = EditorConfig::default();
    assert_eq!(cfg.context_menu.enabled, canonical.enabled);
    assert_eq!(cfg.context_menu.show_clipboard, canonical.show_clipboard);
    assert_eq!(cfg.context_menu.show_select_all, canonical.show_select_all);
    assert_eq!(cfg.context_menu.show_undo_redo, canonical.show_undo_redo);
    assert_eq!(
        cfg.context_menu.show_code_actions,
        canonical.show_code_actions
    );
    assert_eq!(cfg.context_menu.show_transform, canonical.show_transform);
    assert_eq!(cfg.context_menu.show_find, canonical.show_find);
    assert_eq!(
        cfg.context_menu.show_view_toggles,
        canonical.show_view_toggles
    );
    assert_eq!(
        cfg.context_menu.show_language_selector,
        canonical.show_language_selector,
    );
    assert_eq!(
        cfg.context_menu.show_theme_selector,
        canonical.show_theme_selector
    );
    assert_eq!(cfg.context_menu.show_font_size, canonical.show_font_size);
    assert_eq!(
        cfg.context_menu.show_cursor_info,
        canonical.show_cursor_info
    );
}
