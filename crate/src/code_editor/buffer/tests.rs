//! Buffer test suite.
//!
//! Extracted from buffer/mod.rs so the module file stays under the
//! 500-line ceiling. Declared via #[cfg(test)] mod tests; in mod.rs.

use super::*;

fn buf(text: &str) -> TextBuffer {
    let mut b = TextBuffer::default();
    b.set_text(text);
    b
}

#[test]
fn test_set_text() {
    let b = buf("hello\nworld");
    assert_eq!(b.line_count(), 2);
    assert_eq!(b.line(0), "hello");
    assert_eq!(b.line(1), "world");
}

#[test]
fn test_insert_char() {
    let mut b = buf("ab");
    b.set_cursor(CursorPos::new(0, 1));
    b.insert_char('X');
    assert_eq!(b.line(0), "aXb");
    assert_eq!(b.cursor().col, 2);
    assert!(b.is_modified());
}

#[test]
fn test_newline() {
    let mut b = buf("hello world");
    b.set_cursor(CursorPos::new(0, 5));
    b.insert_newline(false, 4);
    assert_eq!(b.line_count(), 2);
    assert_eq!(b.line(0), "hello");
    assert_eq!(b.line(1), " world");
}

#[test]
fn test_auto_indent_after_brace() {
    let mut b = buf("fn main() {");
    b.set_cursor(CursorPos::new(0, 11));
    b.insert_newline(true, 4);
    assert_eq!(b.line(1), "    ");
    assert_eq!(b.cursor().col, 4);
}

#[test]
fn test_backspace() {
    let mut b = buf("abc");
    b.set_cursor(CursorPos::new(0, 2));
    b.backspace();
    assert_eq!(b.line(0), "ac");
}

#[test]
fn test_backspace_merge_lines() {
    let mut b = buf("ab\ncd");
    b.set_cursor(CursorPos::new(1, 0));
    b.backspace();
    assert_eq!(b.line_count(), 1);
    assert_eq!(b.line(0), "abcd");
    assert_eq!(b.cursor(), CursorPos::new(0, 2));
}

#[test]
fn test_delete_forward() {
    let mut b = buf("abc");
    b.set_cursor(CursorPos::new(0, 1));
    b.delete();
    assert_eq!(b.line(0), "ac");
}

#[test]
fn test_selection_and_delete() {
    let mut b = buf("hello world");
    b.set_selection(CursorPos::new(0, 0), CursorPos::new(0, 5));
    b.backspace();
    assert_eq!(b.line(0), " world");
}

#[test]
fn test_selected_text() {
    let b = {
        let mut b = buf("hello\nworld\nfoo");
        b.set_selection(CursorPos::new(0, 3), CursorPos::new(1, 3));
        b
    };
    assert_eq!(b.selected_text(), "lo\nwor");
}

#[test]
fn test_bracket_matching() {
    let b = {
        let mut b = buf("fn foo(bar(baz))");
        b.set_cursor(CursorPos::new(0, 6));
        b
    };
    let m = b.find_matching_bracket();
    assert_eq!(m, Some(CursorPos::new(0, 15)));
}

#[test]
fn test_move_word() {
    let mut b = buf("hello world_foo bar");
    b.set_cursor(CursorPos::new(0, 0));
    b.move_word_right();
    assert_eq!(b.cursor().col, 6); // after "hello "
    b.move_word_right();
    assert_eq!(b.cursor().col, 16); // after "world_foo "
}

#[test]
fn test_smart_home() {
    let mut b = buf("    hello");
    b.set_cursor(CursorPos::new(0, 7));
    b.move_home();
    assert_eq!(b.cursor().col, 4); // first non-whitespace
    b.move_home();
    assert_eq!(b.cursor().col, 0); // absolute start
}

#[test]
fn test_select_all() {
    let mut b = buf("hello\nworld");
    b.select_all();
    assert_eq!(b.selected_text(), "hello\nworld");
}

#[test]
fn test_insert_multiline_text() {
    let mut b = buf("ab");
    b.set_cursor(CursorPos::new(0, 1));
    b.insert_text("X\nY\nZ");
    assert_eq!(b.line_count(), 3);
    assert_eq!(b.line(0), "aX");
    assert_eq!(b.line(1), "Y");
    assert_eq!(b.line(2), "Zb");
}

#[test]
fn test_char_to_byte() {
    assert_eq!(char_to_byte("hello", 2), 2);
    assert_eq!(char_to_byte("hello", 5), 5);
    assert_eq!(char_to_byte("hello", 10), 5); // clamped
}

// ── Regression tests from audit ────────────────────────────────────

/// CRLF files must round-trip through `set_text` → `text()` cleanly —
/// load a Windows file, serialize it back out, line endings preserved.
#[test]
fn test_crlf_round_trip() {
    let src = "line1\r\nline2\r\nline3";
    let mut b = TextBuffer::default();
    b.set_text(src);
    assert_eq!(b.line_ending(), LineEnding::Crlf);
    assert_eq!(b.line_count(), 3);
    assert_eq!(b.text(), src);
}

/// LF files stay LF on round-trip — no accidental CRLF injection.
#[test]
fn test_lf_round_trip() {
    let src = "line1\nline2\nline3";
    let mut b = TextBuffer::default();
    b.set_text(src);
    assert_eq!(b.line_ending(), LineEnding::Lf);
    assert_eq!(b.text(), src);
}

/// `insert_text` must normalise CRLF → LF on entry. The Windows
/// clipboard delivers paste payloads as `\r\n`, but the buffer
/// stores lines without their terminator — without normalisation
/// a stray `\r` ends up appended to every pasted line.
#[test]
fn test_insert_text_normalises_crlf() {
    let mut b = TextBuffer::default();
    b.set_text("");
    b.insert_text("a\r\nb\r\nc");
    assert_eq!(b.line_count(), 3);
    assert_eq!(b.line(0), "a");
    assert_eq!(b.line(1), "b");
    assert_eq!(b.line(2), "c");
    // No stray \r should have leaked into any stored line.
    for i in 0..b.line_count() {
        assert!(
            !b.line(i).contains('\r'),
            "line {i} = {:?} contains stray \\r",
            b.line(i)
        );
    }
}

/// Single-line CRLF paste (no embedded newline) is a no-op for the
/// normaliser — but if a future refactor breaks the `\r\n` check,
/// this catches the regression.
#[test]
fn test_insert_text_preserves_pure_lf() {
    let mut b = TextBuffer::default();
    b.set_text("");
    b.insert_text("hello\nworld");
    assert_eq!(b.line_count(), 2);
    assert_eq!(b.line(0), "hello");
    assert_eq!(b.line(1), "world");
}

/// `restore_from_undo` must preserve `modified = true` and bump
/// `edit_version` — otherwise the save-prompt dirty indicator
/// silently goes false after Ctrl+Z.
#[test]
fn test_undo_restore_preserves_dirty_state() {
    let mut b = TextBuffer::default();
    b.set_text("clean state");
    assert!(!b.is_modified());
    let v0 = b.edit_version();

    b.insert_char('X');
    assert!(b.is_modified());

    // Simulate undo: restore previous snapshot.
    b.restore_from_undo("clean state", CursorPos::default());
    assert!(
        b.is_modified(),
        "buffer differs from last-saved (which was the dirty edit \
         before the undo) — must stay modified"
    );
    assert!(
        b.edit_version() > v0,
        "edit_version must bump so version-keyed caches invalidate"
    );
}

/// `char_to_byte` / `byte_to_char` must be inverse on all valid
/// input — critical for search/replace correctness on non-ASCII text.
#[test]
fn test_char_byte_roundtrip_non_ascii() {
    let samples = ["mañana", "радуга", "你好", "café", "αβγ"];
    for s in &samples {
        let n_chars = s.chars().count();
        for i in 0..=n_chars {
            let b = char_to_byte(s, i);
            let back = byte_to_char(s, b);
            assert_eq!(
                back, i,
                "roundtrip failed for {s:?} at char {i} → byte {b} → char {back}"
            );
        }
    }
}

// ── New audit regression tests ──────────────────────────────────────

/// Multi-byte UTF-8 editing: inserting / deleting around Cyrillic and
/// CJK text must never split a char boundary (would panic on slice).
#[test]
fn test_insert_delete_around_multibyte() {
    let mut b = TextBuffer::default();
    b.set_text("аб");
    // Cursor between the two Cyrillic chars.
    b.set_cursor(CursorPos::new(0, 1));
    b.insert_char('X');
    assert_eq!(b.line(0), "аXб");
    // Backspace removes the X (char-indexed, not byte-indexed).
    b.backspace();
    assert_eq!(b.line(0), "аб");
    // Delete forward removes the second Cyrillic char.
    b.delete();
    assert_eq!(b.line(0), "а");
}

/// Deleting a multi-line selection that starts/ends mid multi-byte
/// char must slice on char boundaries.
#[test]
fn test_delete_selection_multibyte_multiline() {
    let mut b = TextBuffer::default();
    b.set_text("café\nналив\nfoo");
    // Select from inside line 0 ("ca|fé") to inside line 1 ("на|лив").
    b.set_selection(CursorPos::new(0, 2), CursorPos::new(1, 2));
    b.backspace();
    assert_eq!(b.line_count(), 2);
    assert_eq!(b.line(0), "caлив");
    assert_eq!(b.line(1), "foo");
}

/// `clamp_pos` must keep cursor within bounds even when callers pass
/// wildly out-of-range positions (defends every `self.lines[idx]`).
#[test]
fn test_set_cursor_clamps_out_of_range() {
    let mut b = TextBuffer::default();
    b.set_text("a\nbb\nccc");
    b.set_cursor(CursorPos::new(999, 999));
    let c = b.cursor();
    assert_eq!(c.line, 2);
    assert_eq!(c.col, 3); // "ccc".chars().count()
}
