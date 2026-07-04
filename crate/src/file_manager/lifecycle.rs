//! Dialog lifecycle: open/save entry points, directory (re)listing, and
//! selection-confirmation logic for [`FileManager`](super::FileManager).
//!
//! Split out of `mod.rs` (was > 500 lines, per CLAUDE.md). These methods
//! form the non-rendering half of the public API surface plus the private
//! `refresh_directory` / `open_common` glue and the mode-specific confirm
//! flow. `refresh_directory`, `try_confirm` and `finalize_selection` are
//! `pub(super)` because the per-frame driver in [`view`](super::view) and
//! the deferred-action executor in [`actions`](super::actions) call them.

use std::path::PathBuf;

use dear_imgui_rs::Ui;

use super::util::is_valid_filename;
use super::{
    DialogMode, FavoritesPanel, FileFilter, FileManager, FmError, FsEntry,
    rebuild_breadcrumb_segments, sort_entries,
};

impl FileManager {
    // ─── Public open methods ─────────────────────────────────────────

    /// Open a folder picker dialog.
    ///
    /// Only directories are shown. The confirm button reads "Select Folder".
    /// Pass `initial_path` to start in a specific directory.
    pub fn open_folder(&mut self, initial_path: Option<PathBuf>) {
        self.mode = DialogMode::SelectFolder;
        self.filters = vec![FileFilter::all()];
        self.active_filter = 0;
        self.open_common(initial_path);
    }

    /// Open a file picker dialog for opening an existing file.
    ///
    /// Shows both directories and files. The confirm button reads "Open".
    /// Pass `filters` to limit visible file types (empty = show all).
    /// If [`enable_multi_select`](FileManagerConfig::enable_multi_select) is `true`,
    /// Ctrl+Click selects multiple files.
    pub fn open_file(&mut self, initial_path: Option<PathBuf>, filters: Vec<FileFilter>) {
        self.mode = DialogMode::OpenFile;
        self.filters = if filters.is_empty() {
            vec![FileFilter::all()]
        } else {
            filters
        };
        self.active_filter = 0;
        self.open_common(initial_path);
    }

    /// Open a save dialog (choose location + filename).
    ///
    /// Shows a filename text input at the bottom. The confirm button reads "Save".
    /// If the target file already exists, an overwrite confirmation modal appears.
    /// `default_filename` pre-fills the filename input.
    pub fn save_file(
        &mut self,
        initial_path: Option<PathBuf>,
        default_filename: &str,
        filters: Vec<FileFilter>,
    ) {
        self.mode = DialogMode::SaveFile;
        self.filename_buf.clear();
        self.filename_buf.push_str(default_filename);
        self.filters = if filters.is_empty() {
            vec![FileFilter::all()]
        } else {
            filters
        };
        self.active_filter = 0;
        self.open_common(initial_path);
    }

    /// Selected paths for multi-select results.
    pub fn selected_paths(&self) -> &[PathBuf] {
        &self.selected_paths
    }

    /// Access favorites panel for adding/removing bookmarks.
    pub fn favorites_mut(&mut self) -> &mut FavoritesPanel {
        &mut self.favorites
    }

    // ─── Internal: open ─────────────────────────────────────────────

    /// Shared setup for all `open_*` methods: reset state, resolve initial path, refresh.
    fn open_common(&mut self, initial_path: Option<PathBuf>) {
        self.is_open = true;
        self.popup_needs_open = true;
        self.selected_path = None;
        self.selected_paths.clear();
        self.error = None;
        self.show_new_folder = false;
        self.show_new_file = false;
        self.show_overwrite_confirm = false;
        self.show_delete_confirm = false;
        self.breadcrumb_editing = false;
        self.new_folder_buf.clear();
        self.new_file_buf.clear();
        self.context_menu_target = None;
        self.rename_index = None;
        self.rename_buf.clear();
        self.delete_target = None;
        self.selected_indices.clear();
        self.last_click_index = None;
        self.scroll_to_index = None;
        self.search_buf.clear();
        self.search_timer = 0.0;
        self.history.clear();

        if let Some(path) = initial_path {
            if path.is_dir() {
                self.current_path = path;
            } else if let Some(parent) = path.parent()
                && parent.is_dir()
            {
                self.current_path = parent.to_path_buf();
                if self.mode == DialogMode::SaveFile
                    && let Some(name) = path.file_name()
                {
                    self.filename_buf.clear();
                    self.filename_buf.push_str(&name.to_string_lossy());
                }
            }
        }

        self.refresh_directory();
    }

    // ─── Internal: directory operations ─────────────────────────────

    /// Read the current directory, filter entries by mode and active filter, sort.
    ///
    /// Pre-computes all display strings (`size_display`, `date_display`, etc.)
    /// via [`FsEntry::from_dir_entry()`] so the render loop does zero allocations.
    pub(super) fn refresh_directory(&mut self) {
        self.entries.clear();
        self.selected_indices.clear();
        // BUG-FM1: a re-listing invalidates *every* index into `entries`.
        // Clearing only `selected_indices` left these transient indices
        // pointing at whatever now occupies that slot — so an inline rename,
        // a queued delete-confirm, or a Shift+Click anchor could resolve to a
        // *different* file after navigate / refresh / toggle-hidden / sort.
        // (`entries.get(idx)` is bounds-safe but still returns the wrong row.)
        // Reset them all here so no stale index outlives the listing it indexed.
        self.last_click_index = None;
        self.scroll_to_index = None;
        self.rename_index = None;
        self.rename_buf.clear();
        self.context_menu_target = None;
        self.delete_target = None;
        self.show_delete_confirm = false;
        self.error = None;
        self.loaded = true;
        self.path_input_buf.clear();
        self.path_input_buf
            .push_str(&self.current_path.to_string_lossy());
        // P1-3 + P0-2: rebuild breadcrumb segment cache from canonical components.
        rebuild_breadcrumb_segments(&self.current_path, &mut self.breadcrumb_segments);

        let show_files = self.mode != DialogMode::SelectFolder;
        // `.get()` instead of indexing: `open_*`/`new()` always seed at least
        // one filter, but this avoids a panic-on-empty if that invariant ever
        // breaks. An absent filter matches all files.
        let filter = self
            .filters
            .get(self.active_filter.min(self.filters.len().saturating_sub(1)));

        match std::fs::read_dir(&self.current_path) {
            Ok(read_dir) => {
                for dir_entry in read_dir.flatten() {
                    if let Some(entry) = FsEntry::from_dir_entry(&dir_entry) {
                        // Filter hidden files
                        if entry.is_hidden && !self.show_hidden {
                            continue;
                        }
                        if entry.is_dir
                            || (show_files
                                && filter.is_none_or(|f| f.matches_ext(&entry.extension)))
                        {
                            self.entries.push(entry);
                        }
                    }
                }
                sort_entries(
                    &mut self.entries,
                    self.sort_column,
                    self.sort_order,
                    self.config.dirs_first,
                );
            }
            Err(e) => {
                self.error = Some(FmError::CannotReadDir(e.to_string()));
            }
        }
    }
    // ─── Confirmation logic ─────────────────────────────────────────

    /// Attempt to confirm the current selection based on dialog mode.
    ///
    /// - **SelectFolder**: confirms the current directory.
    /// - **OpenFile**: confirms selected file(s); does nothing if no file is selected.
    /// - **SaveFile**: checks for existing file → shows overwrite modal if needed.
    ///
    /// Returns `true` if the dialog should close with a confirmed result.
    pub(super) fn try_confirm(&mut self, ui: &Ui) -> bool {
        match self.mode {
            DialogMode::SelectFolder => {
                self.selected_path = Some(self.current_path.clone());
                self.is_open = false;
                ui.close_current_popup();
                true
            }
            DialogMode::OpenFile => {
                let paths: Vec<PathBuf> = self
                    .selected_indices
                    .iter()
                    .filter_map(|&i| self.entries.get(i))
                    .filter(|e| !e.is_dir)
                    .map(|e| e.path.clone())
                    .collect();

                if paths.is_empty() {
                    return false;
                }
                // P1-8: avoid the double-clone — `paths.first().cloned()`
                // shares the heap allocation between the lookup and the move.
                self.selected_path = paths.first().cloned();
                self.selected_paths = paths;
                self.is_open = false;
                ui.close_current_popup();
                true
            }
            DialogMode::SaveFile => {
                let fname = self.filename_buf.trim().to_string();
                if fname.is_empty() {
                    return false;
                }
                // Validate like create/rename do — otherwise a Save filename of
                // `../secret.cfg` or `C:\Windows\x` would return a target
                // *outside* the browsed directory (path-escape gap).
                if !is_valid_filename(&fname) {
                    let inv = self.config.strings.invalid_name;
                    self.error = Some(FmError::CreateFileFailed(format!("{inv}: \"{fname}\"")));
                    return false;
                }
                let target = self.current_path.join(&fname);
                if target.exists() {
                    // Show overwrite confirmation
                    self.show_overwrite_confirm = true;
                    false
                } else {
                    self.selected_path = Some(target);
                    self.is_open = false;
                    ui.close_current_popup();
                    true
                }
            }
        }
    }

    /// Finalize the selection after overwrite confirmation (SaveFile mode).
    pub(super) fn finalize_selection(&mut self) {
        if self.mode == DialogMode::SaveFile {
            let target = self.current_path.join(self.filename_buf.trim());
            self.selected_path = Some(target);
        }
        self.is_open = false;
    }
}
