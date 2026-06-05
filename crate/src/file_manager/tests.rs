//! Pure-logic regression tests for [`FileManager`](super::FileManager) state
//! transitions that do not require an ImGui context.
//!
//! These exercise the index-invalidation contract of `refresh_directory`
//! (BUG-FM1) and the drive-letter heuristic (BUG-FM2, also covered in
//! `util::tests`). The directory actually read is the process working
//! directory, which always exists, so the tests are not FS-flaky — they assert
//! on the *reset* of transient index state, not on the listing contents.

use super::*;

/// Build a `FileManager` and plant stale transient index state, as if a
/// previous directory listing had been interacted with.
fn fm_with_stale_indices() -> FileManager {
    let mut fm = FileManager::new();
    fm.selected_indices = vec![3, 7, 11];
    fm.last_click_index = Some(7);
    fm.scroll_to_index = Some(11);
    fm.rename_index = Some(3);
    fm.rename_buf.push_str("stale");
    fm.context_menu_target = Some(7);
    fm.delete_target = Some(11);
    fm.show_delete_confirm = true;
    fm
}

#[test]
fn refresh_clears_all_stale_indices() {
    // BUG-FM1 regression: a re-listing invalidates every index into `entries`.
    // After `refresh_directory`, no transient index from the old listing may
    // survive — otherwise an inline rename / queued delete / range-select
    // anchor would resolve against the wrong (or out-of-bounds) entry.
    let mut fm = fm_with_stale_indices();
    fm.refresh_directory();

    assert!(
        fm.selected_indices.is_empty(),
        "selection must reset on refresh"
    );
    assert_eq!(fm.last_click_index, None, "shift-anchor must reset");
    assert_eq!(fm.scroll_to_index, None, "scroll target must reset");
    assert_eq!(fm.rename_index, None, "rename target must reset");
    assert!(fm.rename_buf.is_empty(), "rename buffer must clear");
    assert_eq!(fm.context_menu_target, None, "context target must reset");
    assert_eq!(fm.delete_target, None, "delete target must reset");
    assert!(!fm.show_delete_confirm, "delete-confirm flag must reset");
}

#[test]
fn open_common_resets_history_and_indices() {
    // Opening the dialog afresh must not inherit a previous session's
    // navigation history or selection.
    let mut fm = fm_with_stale_indices();
    fm.history.push(std::path::Path::new("C:\\"));
    fm.open_folder(None);

    assert!(!fm.history.can_go_back(), "history cleared on open");
    assert!(fm.selected_indices.is_empty());
    assert_eq!(fm.delete_target, None);
    assert!(fm.is_open, "dialog is open after open_folder");
}

#[test]
fn current_drive_letter_matches_enumerated_drives() {
    // The highlighted-drive comparison in the drive bar relies on
    // `current_drive_letter` returning the same case as `enumerate_drives`
    // (uppercase `X`). Drive a known absolute path through it.
    let mut fm = FileManager::new();
    fm.current_path = std::path::PathBuf::from("c:\\windows");
    // On non-Windows the path is treated as relative -> None, which is correct
    // there too (no drive bar). Only assert the Windows normalisation.
    #[cfg(windows)]
    assert_eq!(fm.current_drive_letter(), Some('C'));
    #[cfg(not(windows))]
    let _ = &fm;
}
