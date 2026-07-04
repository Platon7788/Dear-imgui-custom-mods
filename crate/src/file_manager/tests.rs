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

// ─── Filesystem operation tests (temp-dir backed) ────────────────────────────
//
// These exercise the `ui`-free operation methods (`create_folder`,
// `create_file`, `rename_entry`, `delete_entry`, `resort`) against a real,
// auto-cleaned scratch directory — no ImGui context required.

/// Auto-cleaning scratch directory under the OS temp dir. Removed on drop even
/// if a test panics.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let mut dir = std::env::temp_dir();
        dir.push(format!("dimcm_fm_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        Scratch(dir)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A `FileManager` listing `dir` in OpenFile mode (so files are visible).
fn fm_in(dir: &std::path::Path) -> FileManager {
    let mut fm = FileManager::new();
    fm.mode = DialogMode::OpenFile;
    fm.current_path = dir.to_path_buf();
    fm.refresh_directory();
    fm
}

fn index_of(fm: &FileManager, name: &str) -> usize {
    fm.entries
        .iter()
        .position(|e| e.name == name)
        .unwrap_or_else(|| panic!("entry {name:?} not found in listing"))
}

#[test]
fn create_folder_makes_dir_then_rejects_collision() {
    let sc = Scratch::new("create_folder");
    let mut fm = fm_in(sc.path());
    fm.create_folder("sub");
    assert!(sc.path().join("sub").is_dir());
    assert!(fm.error.is_none());
    fm.create_folder("sub");
    assert!(fm.error.is_some(), "creating an existing folder must error");
}

#[test]
fn create_folder_rejects_invalid_name() {
    let sc = Scratch::new("create_folder_bad");
    let mut fm = fm_in(sc.path());
    fm.create_folder("..");
    assert!(fm.error.is_some(), "path-traversal name must be rejected");
}

#[test]
fn create_file_never_truncates_existing() {
    // H2 regression: New File must not clobber a same-named existing file.
    let sc = Scratch::new("create_file_trunc");
    let f = sc.path().join("data.txt");
    std::fs::write(&f, b"important").unwrap();
    let mut fm = fm_in(sc.path());
    fm.create_file("data.txt");
    assert!(fm.error.is_some(), "must not overwrite an existing file");
    assert_eq!(
        std::fs::read(&f).unwrap(),
        b"important",
        "existing file must be byte-for-byte untouched"
    );
}

#[test]
fn create_file_makes_empty_file() {
    let sc = Scratch::new("create_file_ok");
    let mut fm = fm_in(sc.path());
    fm.create_file("new.txt");
    assert!(fm.error.is_none());
    assert!(sc.path().join("new.txt").is_file());
}

#[test]
fn rename_refuses_to_overwrite_existing() {
    // M1 regression: renaming onto an existing name must not clobber it.
    let sc = Scratch::new("rename_clash");
    std::fs::write(sc.path().join("a.txt"), b"AAA").unwrap();
    std::fs::write(sc.path().join("b.txt"), b"BBB").unwrap();
    let mut fm = fm_in(sc.path());
    let idx = index_of(&fm, "a.txt");
    fm.rename_entry(idx, "b.txt");
    assert!(
        fm.error.is_some(),
        "rename onto an existing name must error"
    );
    assert_eq!(std::fs::read(sc.path().join("b.txt")).unwrap(), b"BBB");
    assert!(sc.path().join("a.txt").exists());
}

#[test]
fn rename_rejects_escape_then_succeeds() {
    let sc = Scratch::new("rename_ok");
    std::fs::write(sc.path().join("old.txt"), b"X").unwrap();
    let mut fm = fm_in(sc.path());
    let idx = index_of(&fm, "old.txt");
    fm.rename_entry(idx, "../escape.txt");
    assert!(fm.error.is_some(), "path-escaping rename must be rejected");
    assert!(sc.path().join("old.txt").exists());

    let idx = index_of(&fm, "old.txt");
    fm.rename_entry(idx, "new.txt");
    assert!(fm.error.is_none());
    assert!(sc.path().join("new.txt").is_file());
    assert!(!sc.path().join("old.txt").exists());
}

#[test]
fn delete_removes_entry() {
    let sc = Scratch::new("delete");
    std::fs::write(sc.path().join("gone.txt"), b"X").unwrap();
    let mut fm = fm_in(sc.path());
    let idx = index_of(&fm, "gone.txt");
    fm.delete_entry(idx);
    assert!(fm.error.is_none());
    assert!(!sc.path().join("gone.txt").exists());
}

#[test]
fn resort_keeps_selection_on_the_same_file() {
    // H1 regression: a header-click sort must not leave the selection pointing
    // at the wrong (reordered) row.
    use super::entry::SortOrder;
    let sc = Scratch::new("resort");
    for n in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(sc.path().join(n), b"X").unwrap();
    }
    let mut fm = fm_in(sc.path());
    let c = index_of(&fm, "c.txt");
    fm.selected_indices = vec![c];
    fm.last_click_index = Some(c);
    fm.sort_order = SortOrder::Descending;
    fm.resort();
    let sel = fm
        .selected_indices
        .first()
        .copied()
        .expect("selection preserved across sort");
    assert_eq!(
        fm.entries[sel].name, "c.txt",
        "selection must follow the file, not the index, across a sort"
    );
    assert_eq!(fm.entries[fm.last_click_index.unwrap()].name, "c.txt");
}
