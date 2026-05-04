//! Deferred UI actions and their execution.
//!
//! Render functions return [`Action`] values instead of mutating
//! [`FileManager`](super::FileManager) directly. This avoids borrow conflicts
//! between `&self` reads (display) and `&mut self` writes (state changes) — the
//! whole render frame produces at most one `Action`, which is applied after the
//! frame in [`FileManager::apply_action`].

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
    /// header click, this just triggers `sort_entries`).
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

    /// Execute a deferred [`Action`] collected during rendering.
    pub(super) fn apply_action(&mut self, action: Action, ui: &Ui) {
        // Clear stale errors on any user action
        self.error = None;

        match action {
            Action::NavigateTo(path) => {
                if self.config.enable_history {
                    self.history.push(&self.current_path);
                }
                self.try_navigate(path);
            }
            Action::GoParent => {
                if let Some(parent) = self.current_path.parent() {
                    let mut p = parent.to_path_buf();
                    if p.as_os_str().len() == 2 && p.to_string_lossy().ends_with(':') {
                        p.push("\\");
                    }
                    if p != self.current_path {
                        if self.config.enable_history {
                            self.history.push(&self.current_path);
                        }
                        self.try_navigate(p);
                    }
                }
            }
            Action::GoBack => {
                if let Some(prev) = self.history.go_back(&self.current_path) {
                    self.current_path = prev;
                    self.refresh_directory();
                }
            }
            Action::GoForward => {
                if let Some(next) = self.history.go_forward(&self.current_path) {
                    self.current_path = next;
                    self.refresh_directory();
                }
            }
            Action::CreateFolder(name) => {
                if !is_valid_filename(&name) {
                    self.error = Some(FmError::CreateFolderFailed(format!(
                        "Invalid name: \"{name}\""
                    )));
                    self.show_new_folder = false;
                } else {
                    let new_path = self.current_path.join(&name);
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
            }
            Action::CreateFile(name) => {
                if !is_valid_filename(&name) {
                    self.error = Some(FmError::CreateFileFailed(format!(
                        "Invalid name: \"{name}\""
                    )));
                    self.show_new_file = false;
                } else {
                    let new_path = self.current_path.join(&name);
                    match std::fs::File::create(&new_path) {
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
            }
            Action::SelectFilter(idx) => {
                if idx < self.filters.len() {
                    self.active_filter = idx;
                    self.refresh_directory();
                }
            }
            Action::NavigateToInput(input) => {
                let path = PathBuf::from(input.trim());
                if path.is_dir() {
                    if self.config.enable_history {
                        self.history.push(&self.current_path);
                    }
                    self.try_navigate(path);
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
            Action::Resort => {
                // sort_column/sort_order already updated by render_file_table
                sort_entries(
                    &mut self.entries,
                    self.sort_column,
                    self.sort_order,
                    self.config.dirs_first,
                );
            }
            Action::ConfirmSelection => {}
            Action::RenameEntry { index, new_name } => {
                if !is_valid_filename(&new_name) {
                    self.error = Some(FmError::RenameFailed(format!(
                        "Invalid name: \"{new_name}\""
                    )));
                    self.rename_index = None;
                } else if let Some(entry) = self.entries.get(index) {
                    let old_path = entry.path.clone();
                    let new_path = old_path
                        .parent()
                        .unwrap_or(&self.current_path)
                        .join(&new_name);
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
            }
            Action::DeleteEntry(index) => {
                if let Some(entry) = self.entries.get(index) {
                    let path = entry.path.clone();
                    // P0-3: never `remove_dir_all` on a symlink/junction —
                    // Windows would walk into the target and delete *those*
                    // files. Use `symlink_metadata` (which does NOT follow
                    // links) to detect links, then remove the link itself.
                    let result = match std::fs::symlink_metadata(&path) {
                        Ok(meta) if meta.file_type().is_symlink() => {
                            // Windows: directory junctions need `remove_dir`
                            // (which does NOT recurse); file symlinks need
                            // `remove_file`. Try both.
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
            }
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
