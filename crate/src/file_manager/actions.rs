//! Deferred UI actions and their execution.
//!
//! Render functions return [`Action`] values instead of mutating
//! [`FileManager`](super::FileManager) directly. This avoids borrow conflicts
//! between `&self` reads (display) and `&mut self` writes (state changes) — the
//! whole render frame produces at most one `Action`, which is applied after the
//! frame in [`FileManager::apply_action`].
//!
//! The filesystem-mutating operations (create / rename / delete) and the
//! header-click re-sort are implemented as `ui`-free methods
//! ([`create_folder`](FileManager::create_folder) etc.) so they can be unit
//! tested against a temp directory without an ImGui context; `apply_action`
//! is a thin dispatcher over them.

use std::collections::HashSet;
use std::path::PathBuf;

use dear_imgui_rs::Ui;

use super::FileManager;
use super::FmError;
use super::entry::sort_entries;
use super::util::{is_valid_filename, ui_set_clipboard};

// ─── Action enum ────────────────────────────────────────────────────────────

/// Deferred UI action collected during rendering, applied after the frame.
pub(super) enum Action {
    /// Navigate into a specific directory.
    NavigateTo(PathBuf),
    /// Navigate to the parent directory.
    GoParent,
    /// Navigate back in history.
    GoBack,
    /// Navigate forward in history.
    GoForward,
    /// Create a new folder with the given name in the current directory.
    CreateFolder(String),
    /// Create a new empty file with the given name in the current directory.
    CreateFile(String),
    /// Switch to a different file type filter (by index).
    SelectFilter(usize),
    /// Navigate to a path entered in the breadcrumb text input.
    NavigateToInput(String),
    /// Re-read the current directory.
    Refresh,
    /// Re-sort entries (column/order already updated in-place by the table
    /// header click, this just triggers the identity-preserving re-sort).
    Resort,
    /// Confirm the current selection (confirm button, double-click, or Enter).
    ConfirmSelection,
    /// Rename entry at `index` to `new_name`.
    RenameEntry { index: usize, new_name: String },
    /// Delete entry at `index` (after confirmation).
    DeleteEntry(usize),
    /// Copy full path of entry at `index` to clipboard.
    CopyPath(usize),
    /// Toggle visibility of hidden files.
    ToggleHidden,
}

// ─── Action execution ───────────────────────────────────────────────────────

impl FileManager {
    /// Navigate to a path: set current_path, refresh.
    /// If read_dir fails, shows error and reverts current_path locally.
    ///
    /// History stack mutation is the caller's responsibility. Earlier versions
    /// called `history.go_back` on failure as a "revert", but that corrupted
    /// both back/forward stacks since the navigation never actually happened
    /// from the user's perspective (P0-5).
    pub(super) fn try_navigate(&mut self, path: PathBuf) {
        self.error = None;
        let prev = std::mem::replace(&mut self.current_path, path);
        self.refresh_directory();
        if self.error.is_some() {
            // Revert in-place — keep history stacks intact.
            self.current_path = prev;
            // Don't refresh again — entries from the prior directory stay
            // visible alongside the error message.
        }
    }

    /// Navigate to `path`, pushing history only if the listing succeeds.
    ///
    /// Guards against re-navigating to the already-current directory (which
    /// would otherwise push a redundant back-entry and wipe the forward stack).
    /// The history push happens *after* `try_navigate` confirms success, so a
    /// failed navigation (deleted/unreadable target) never corrupts history.
    fn navigate_pushing_history(&mut self, path: PathBuf) {
        if path == self.current_path {
            return;
        }
        let prev = self.current_path.clone();
        self.try_navigate(path);
        if self.error.is_none() && self.config.enable_history {
            self.history.push(&prev);
        }
    }

    /// Create a new folder named `name` in the current directory.
    ///
    /// Rejects invalid names up front; surfaces a collision (`create_dir`
    /// errors if the directory already exists) as an [`FmError`]. Refreshes on
    /// success. `ui`-free so it is unit-testable.
    pub(super) fn create_folder(&mut self, name: &str) {
        if !is_valid_filename(name) {
            let inv = self.config.strings.invalid_name;
            self.error = Some(FmError::CreateFolderFailed(format!("{inv}: \"{name}\"")));
            self.show_new_folder = false;
            return;
        }
        let new_path = self.current_path.join(name);
        match std::fs::create_dir(&new_path) {
            Ok(()) => {
                self.show_new_folder = false;
                self.new_folder_buf.clear();
                self.refresh_directory();
            }
            Err(e) => {
                self.error = Some(FmError::CreateFolderFailed(e.to_string()));
            }
        }
    }

    /// Create a new empty file named `name` in the current directory.
    ///
    /// Uses `create_new` so an existing file is **never** truncated (the old
    /// `File::create` silently clobbered a same-named file — data loss); a
    /// collision surfaces as `AlreadyExists`. `ui`-free so it is unit-testable.
    pub(super) fn create_file(&mut self, name: &str) {
        if !is_valid_filename(name) {
            let inv = self.config.strings.invalid_name;
            self.error = Some(FmError::CreateFileFailed(format!("{inv}: \"{name}\"")));
            self.show_new_file = false;
            return;
        }
        let new_path = self.current_path.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&new_path)
        {
            Ok(_) => {
                self.show_new_file = false;
                self.new_file_buf.clear();
                self.refresh_directory();
            }
            Err(e) => {
                self.error = Some(FmError::CreateFileFailed(e.to_string()));
            }
        }
    }

    /// Rename the entry at `index` to `new_name`.
    ///
    /// Rejects invalid names and refuses to overwrite an existing target
    /// (`fs::rename` would otherwise silently replace it — data loss). `is_valid_filename`
    /// blocks path separators so the rename stays in the same parent.
    pub(super) fn rename_entry(&mut self, index: usize, new_name: &str) {
        if !is_valid_filename(new_name) {
            let inv = self.config.strings.invalid_name;
            self.error = Some(FmError::RenameFailed(format!("{inv}: \"{new_name}\"")));
            self.rename_index = None;
            return;
        }
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let old_path = entry.path.clone();
        let new_path = old_path
            .parent()
            .unwrap_or(&self.current_path)
            .join(new_name);
        // Refuse to clobber a different existing entry. `symlink_metadata`
        // does not follow links, so a dangling symlink target still counts as
        // "occupied". A no-op rename to the same path is allowed to proceed.
        if new_path != old_path && new_path.symlink_metadata().is_ok() {
            let exists = self.config.strings.target_exists;
            self.error = Some(FmError::RenameFailed(format!("{exists}: \"{new_name}\"")));
            return;
        }
        match std::fs::rename(&old_path, &new_path) {
            Ok(()) => {
                self.rename_index = None;
                self.rename_buf.clear();
                self.refresh_directory();
            }
            Err(e) => {
                self.error = Some(FmError::RenameFailed(e.to_string()));
            }
        }
    }

    /// Delete the entry at `index` (symlink/junction-safe).
    ///
    /// A stale `index` (out of range after a refresh) safely no-ops.
    pub(super) fn delete_entry(&mut self, index: usize) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let path = entry.path.clone();
        // P0-3: never `remove_dir_all` on a symlink/junction — Windows would
        // walk into the target and delete *those* files. Use `symlink_metadata`
        // (which does NOT follow links) to detect links, then remove the link.
        let result = match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                // Windows: directory junctions need `remove_dir` (no recurse);
                // file symlinks need `remove_file`. Try both.
                // Unix: `remove_file` works for any symlink.
                #[cfg(windows)]
                {
                    std::fs::remove_dir(&path).or_else(|_| std::fs::remove_file(&path))
                }
                #[cfg(not(windows))]
                {
                    std::fs::remove_file(&path)
                }
            }
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&path),
            Ok(_) => std::fs::remove_file(&path),
            Err(e) => Err(e),
        };
        match result {
            Ok(()) => {
                self.delete_target = None;
                self.show_delete_confirm = false;
                self.refresh_directory();
            }
            Err(e) => {
                self.error = Some(FmError::DeleteFailed(e.to_string()));
            }
        }
    }

    /// Re-sort `entries` after a column-header click, preserving the selection
    /// and shift-anchor by **path identity**.
    ///
    /// A header sort permutes `entries` without going through
    /// `refresh_directory`, so the transient indices must be remapped here or
    /// they would resolve to the wrong (reordered) row — the exact BUG-FM1
    /// hazard that `refresh_directory`'s comment lists ("sort"). Selection and
    /// the shift-anchor are remapped through their paths; interaction state
    /// that must not survive a reorder (scroll target, inline rename, context
    /// menu, pending delete) is cleared.
    pub(super) fn resort(&mut self) {
        let selected: HashSet<PathBuf> = self
            .selected_indices
            .iter()
            .filter_map(|&i| self.entries.get(i).map(|e| e.path.clone()))
            .collect();
        let anchor = self
            .last_click_index
            .and_then(|i| self.entries.get(i).map(|e| e.path.clone()));

        sort_entries(
            &mut self.entries,
            self.sort_column,
            self.sort_order,
            self.config.dirs_first,
        );

        self.selected_indices = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| selected.contains(&e.path))
            .map(|(i, _)| i)
            .collect();
        self.last_click_index = anchor.and_then(|p| self.entries.iter().position(|e| e.path == p));
        self.scroll_to_index = None;
        self.rename_index = None;
        self.rename_buf.clear();
        self.context_menu_target = None;
        self.delete_target = None;
        self.show_delete_confirm = false;
    }

    /// Execute a deferred [`Action`] collected during rendering.
    pub(super) fn apply_action(&mut self, action: Action, ui: &Ui) {
        // Clear stale errors on any user action
        self.error = None;

        match action {
            Action::NavigateTo(path) => self.navigate_pushing_history(path),
            Action::GoParent => {
                if let Some(parent) = self.current_path.parent() {
                    let mut p = parent.to_path_buf();
                    if p.as_os_str().len() == 2 && p.to_string_lossy().ends_with(':') {
                        p.push("\\");
                    }
                    self.navigate_pushing_history(p);
                }
            }
            Action::GoBack => {
                // Peek first, navigate, and only commit the stack move if the
                // target lists successfully — a failed Back no longer consumes
                // a history entry or leaves an unlistable path committed.
                if let Some(target) = self.history.peek_back() {
                    let target = target.to_path_buf();
                    let prev = self.current_path.clone();
                    self.try_navigate(target);
                    if self.error.is_none() {
                        self.history.go_back(&prev);
                    }
                }
            }
            Action::GoForward => {
                if let Some(target) = self.history.peek_forward() {
                    let target = target.to_path_buf();
                    let prev = self.current_path.clone();
                    self.try_navigate(target);
                    if self.error.is_none() {
                        self.history.go_forward(&prev);
                    }
                }
            }
            Action::CreateFolder(name) => self.create_folder(&name),
            Action::CreateFile(name) => self.create_file(&name),
            Action::SelectFilter(idx) => {
                if idx < self.filters.len() {
                    self.active_filter = idx;
                    self.refresh_directory();
                }
            }
            Action::NavigateToInput(input) => {
                let path = PathBuf::from(input.trim());
                if path.is_dir() {
                    self.navigate_pushing_history(path);
                } else {
                    self.error = Some(FmError::PathNotFound(path.display().to_string()));
                    self.path_input_buf.clear();
                    self.path_input_buf
                        .push_str(&self.current_path.to_string_lossy());
                }
            }
            Action::Refresh => {
                self.refresh_directory();
            }
            Action::Resort => self.resort(),
            // Intentional no-op: every `ConfirmSelection` producer is intercepted
            // in `view.rs` and turned into `do_confirm_selection`, so it never
            // reaches `apply_action`. Kept for enum completeness.
            Action::ConfirmSelection => {}
            Action::RenameEntry { index, new_name } => self.rename_entry(index, &new_name),
            Action::DeleteEntry(index) => self.delete_entry(index),
            Action::CopyPath(index) => {
                if let Some(entry) = self.entries.get(index) {
                    ui_set_clipboard(ui, &entry.path.to_string_lossy());
                }
            }
            Action::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.refresh_directory();
            }
        }
    }
}
