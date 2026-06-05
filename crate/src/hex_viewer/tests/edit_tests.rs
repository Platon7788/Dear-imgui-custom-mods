//! Undo/redo, navigation history, pending-nibble commit, cursor
//! movement, and select-all behaviours.

use crate::hex_viewer::input::EditColumn;
use crate::hex_viewer::*;

#[test]
fn test_undo_stack() {
    let mut stack = UndoStack::new(10);
    assert!(!stack.can_undo());
    stack.push(UndoEntry {
        offset: 0,
        old_bytes: vec![0xAA],
        new_bytes: vec![0xBB],
    });
    assert!(stack.can_undo());
    let entry = stack.undo().unwrap();
    assert_eq!(entry.old_bytes, vec![0xAA]);
    assert!(stack.can_redo());
}

#[test]
fn test_undo_stack_depth_limit_trims_oldest() {
    // Depth 2: pushing a third entry must drop the oldest so the stack
    // never grows past `max_depth`, and `undo_count` reflects the trim.
    let mut stack = UndoStack::new(2);
    for v in 0u8..3 {
        stack.push(UndoEntry {
            offset: v as u64,
            old_bytes: vec![v],
            new_bytes: vec![v + 1],
        });
    }
    assert_eq!(stack.undo_count(), 2, "capacity capped at max_depth");
    // The remaining two entries are the two most-recent (offsets 1, 2).
    assert_eq!(stack.undo().unwrap().offset, 2);
    assert_eq!(stack.undo().unwrap().offset, 1);
    assert!(!stack.can_undo(), "oldest (offset 0) was trimmed");
}

#[test]
fn test_nav_history() {
    let mut nav = NavHistory::new(10);
    nav.push(0x1000);
    let back = nav.go_back(0x2000);
    assert_eq!(back, Some(0x1000));
    let fwd = nav.go_forward(0x1000);
    assert_eq!(fwd, Some(0x2000));
}

#[test]
fn test_nav_history_bounds_empty_back_forward() {
    // Fresh history: both back/forward are unavailable and return None
    // without panicking.
    let mut nav = NavHistory::new(4);
    assert!(!nav.can_go_back());
    assert!(!nav.can_go_forward());
    assert_eq!(nav.go_back(0x10), None);
    assert_eq!(nav.go_forward(0x10), None);
}

#[test]
fn test_nav_history_push_clears_forward() {
    // A new `push` after going back must invalidate the forward stack —
    // classic browser-history semantics. Otherwise a forward jump could
    // land on a stale, now-unreachable address.
    let mut nav = NavHistory::new(8);
    nav.push(0x1000);
    assert_eq!(nav.go_back(0x2000), Some(0x1000)); // forward now holds 0x2000
    assert!(nav.can_go_forward());
    nav.push(0x3000); // diverge — forward must clear
    assert!(!nav.can_go_forward(), "push clears the forward stack");
}

#[test]
fn test_commit_pending_edit_replaces_upper_nibble() {
    // Half-typed nibble must commit as upper-nibble replacement
    // (lower nibble of the original byte preserved). Mirrors HxD
    // behavior — gives the user a way to write a single nibble.
    let mut v = HexViewer::new("test");
    v.set_data(&[0xAB, 0xCD]);
    v.cursor = 0;
    v.edit_column = Some(EditColumn::Hex);
    v.edit_nibble = Some(0xF);
    v.commit_pending_edit();
    assert_eq!(v.data()[0], 0xFB, "upper nibble replaced, lower kept");
    assert_eq!(v.edit_nibble, None, "nibble consumed");
    assert!(v.undo_stack().can_undo(), "undo entry pushed");
}

#[test]
fn test_commit_pending_edit_no_op_when_unchanged() {
    // Typing the same nibble that's already there must not
    // pollute undo history with a no-op entry.
    let mut v = HexViewer::new("test");
    v.set_data(&[0xAB]);
    v.cursor = 0;
    v.edit_nibble = Some(0xA); // upper already 0xA
    v.commit_pending_edit();
    assert_eq!(v.data()[0], 0xAB);
    assert!(!v.undo_stack().can_undo(), "no undo for no-op");
}

#[test]
fn test_move_commits_pending_nibble() {
    // Arrow keys / page nav route through move_cursor_with_selection,
    // which must flush any half-typed nibble before moving.
    let mut v = HexViewer::new("test");
    v.set_data(&[0xAB, 0xCD]);
    v.cursor = 0;
    v.edit_column = Some(EditColumn::Hex);
    v.edit_nibble = Some(0x9);
    v.move_cursor_with_selection(1, false);
    assert_eq!(v.cursor, 1);
    assert_eq!(v.data()[0], 0x9B, "nibble flushed before move");
    assert_eq!(v.edit_nibble, None);
}

#[test]
fn test_undo_then_redo_round_trips_single_byte_edit() {
    // Commit an edit, undo it (byte reverts + cursor parks on it),
    // then redo (byte re-applied). Pins the provider-less public
    // undo/redo path that hosts call outside a render frame.
    let mut v = HexViewer::new("test");
    v.set_data(&[0xAB, 0xCD]);
    v.cursor = 0;
    v.edit_nibble = Some(0xF);
    v.commit_pending_edit(); // 0xAB -> 0xFB
    assert_eq!(v.data()[0], 0xFB);
    v.undo();
    assert_eq!(v.data()[0], 0xAB, "undo reverts the byte");
    assert_eq!(v.cursor, 0, "undo parks cursor on the reverted byte");
    v.redo();
    assert_eq!(v.data()[0], 0xFB, "redo re-applies the byte");
}

#[test]
fn test_undo_with_overflowing_offset_is_a_no_op() {
    // Hardening: a crafted `UndoEntry` whose `offset as usize` is near
    // `usize::MAX` must not overflow the `off + len` bounds check
    // (debug-build panic). The entry is simply skipped.
    let mut v = HexViewer::new("test");
    v.set_data(&[0x11, 0x22, 0x33]);
    // Inject a pathological entry directly onto the stack (the `undo`
    // field is `pub(super)`-visible from the in-module test tree).
    v.undo.push(UndoEntry {
        offset: u64::MAX, // `as usize` -> usize::MAX on 64-bit
        old_bytes: vec![0x00, 0x00],
        new_bytes: vec![0xFF, 0xFF],
    });
    v.undo(); // must not panic; bounds check fails gracefully
    assert_eq!(v.data(), &[0x11, 0x22, 0x33], "buffer untouched");
}

#[test]
fn test_set_cursor_clears_selection_and_pushes_nav() {
    // Regression: pre-fix `set_cursor` left stale selection in place
    // and only pushed nav-history for jumps > bytes_per_row.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 64]);
    v.selection = Selection { start: 4, end: 10 };
    v.cursor = 4;
    v.set_cursor(8); // small jump (within one row)
    assert!(v.selection.is_empty(), "selection must clear on goto");
    assert!(v.nav_history().can_go_back(), "nav must record the jump");
}

#[test]
fn test_shift_arrow_anchors_selection() {
    // Regression: pre-fix Shift+Arrow only updated `selection.end`,
    // leaving `selection.start = 0` so growing selections always
    // started at offset 0. Post-fix anchors `start` at the previous
    // cursor position the moment the user begins selecting.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 32]);
    v.cursor = 10;
    v.move_cursor_with_selection(11, true);
    assert_eq!(v.selection.start, 10);
    assert_eq!(v.selection.end, 11);
    // Continued shift-extends keep the anchor.
    v.move_cursor_with_selection(13, true);
    assert_eq!(v.selection.start, 10);
    assert_eq!(v.selection.end, 13);
    // Releasing shift collapses selection.
    v.move_cursor_with_selection(14, false);
    assert!(v.selection.is_empty());
}

#[test]
fn ctrl_a_select_all_moves_cursor_to_end() {
    // Regression: prior implementation set `selection = 0..len`
    // but left `cursor` unchanged, so the next `shift+arrow`
    // re-anchored at the OLD cursor and silently shrank the
    // selection. Fix moves the cursor to `len-1`.
    let mut v = HexViewer::new("test");
    v.set_data(&[0u8; 32]);
    v.set_cursor(5);
    // Replicate Ctrl+A's state mutation directly (the keyboard
    // handler runs inside `render`; keep the test ImGui-context-free).
    let len = v.data().len();
    v.selection = Selection { start: 0, end: len };
    v.cursor = len.saturating_sub(1);
    assert_eq!(v.cursor, 31, "cursor must sit at end of selection");
    assert_eq!(v.selection.start, 0);
    assert_eq!(v.selection.end, 32);
}

#[test]
fn test_selection() {
    let sel = Selection { start: 5, end: 10 };
    assert!(!sel.is_empty());
    assert_eq!(sel.len(), 5);
    assert!(sel.contains(5));
    assert!(sel.contains(9));
    assert!(!sel.contains(10));
}

#[test]
fn test_selection_reverse() {
    let sel = Selection { start: 10, end: 5 };
    assert_eq!(sel.ordered(), (5, 10));
    assert_eq!(sel.len(), 5);
    assert!(sel.contains(7));
}

#[test]
fn test_selection_single_byte_and_empty() {
    // A single click yields a zero-width selection (start == end):
    // empty, no bytes, `contains` false for the click offset itself.
    let empty = Selection { start: 7, end: 7 };
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert!(!empty.contains(7));
    // A one-byte selection covers exactly its start offset.
    let one = Selection { start: 7, end: 8 };
    assert!(!one.is_empty());
    assert_eq!(one.len(), 1);
    assert!(one.contains(7));
    assert!(!one.contains(8));
}

#[test]
fn test_selected_bytes() {
    let mut v = HexViewer::new("test");
    v.set_data(&[0x10, 0x20, 0x30, 0x40, 0x50]);
    v.selection = Selection { start: 1, end: 4 };
    assert_eq!(v.selected_bytes(), &[0x20, 0x30, 0x40]);
}

#[test]
fn test_selected_bytes_clamps_out_of_range_selection() {
    // A selection whose `end` exceeds the buffer (e.g. data shrank
    // after the selection was made) must clamp, never panic on the
    // slice index.
    let mut v = HexViewer::new("test");
    v.set_data(&[0x10, 0x20, 0x30]);
    v.selection = Selection { start: 1, end: 99 };
    assert_eq!(v.selected_bytes(), &[0x20, 0x30]);
}
