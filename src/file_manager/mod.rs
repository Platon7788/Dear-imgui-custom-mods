//! # FileManager v2
//!
//! Production-ready file/folder picker dialog for Dear ImGui.
//!
//! Provides a native-feeling file browser with modern UX: table view with
//! sortable columns, breadcrumb path navigation, back/forward history,
//! favorites sidebar, keyboard navigation, and type-to-search — all with
//! zero per-frame allocations.
//!
//! ## Architecture
//!
//! The module is split into focused sub-modules:
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | [`config`] | [`DialogMode`], [`FileFilter`], [`FmStrings`], [`FileManagerConfig`] |
//! | [`entry`](entry) | [`FsEntry`](entry::FsEntry) with pre-computed display strings, sorting |
//! | [`render`](render) | All ImGui rendering (drive bar, breadcrumb, toolbar, table, footer) |
//! | [`favorites`](favorites) | Favorites sidebar with well-known folders + custom bookmarks |
//! | [`history`](history) | Back/forward navigation stack |
//!
//! ## Features
//!
//! - **Three dialog modes**: [`SelectFolder`](DialogMode::SelectFolder),
//!   [`OpenFile`](DialogMode::OpenFile), [`SaveFile`](DialogMode::SaveFile)
//! - **Table view**: Name, Size, Date Modified, Type columns with click-to-sort
//! - **Breadcrumb navigation**: clickable path segments, double-click to edit
//! - **Back/forward history**: browser-style navigation with capped stacks
//! - **Favorites sidebar**: Desktop, Documents, Downloads + custom bookmarks
//! - **Keyboard navigation**: Arrow keys, Enter (open/confirm), Backspace (parent), Escape (cancel)
//! - **Multi-select**: Ctrl+Click in OpenFile mode (opt-in via config)
//! - **Type-to-search**: incremental filename matching with auto-reset timeout
//! - **Drive selector**: quick-access drive buttons (Windows), root "/" (Unix)
//! - **New folder / New file**: inline creation with Enter/Create/Cancel
//! - **File filters**: dropdown with extension matching, configurable per-call
//! - **Overwrite confirmation**: nested modal for SaveFile when target exists
//! - **Modal dialog**: `begin_modal_popup` blocks background interaction
//! - **Resizable**: configurable initial and minimum size via [`FileManagerConfig`]
//! - **Zero per-frame allocations**: display strings pre-computed on directory refresh,
//!   scratch buffer (`fmt_buf`) reused for all formatting
//! - **Theme integration**: colors from shared `theme::*` palette
//! - **Localizable**: all user-facing strings via [`FmStrings`] (default: English)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::file_manager::{FileManager, FileFilter};
//!
//! let mut fm = FileManager::new();
//!
//! // Open a file picker with filters
//! fm.open_file(None, vec![
//!     FileFilter::new("Rust Files (*.rs)", &["rs"]),
//!     FileFilter::all(),
//! ]);
//!
//! // Each frame in your render loop:
//! if fm.render(&ui) {
//!     // User confirmed selection
//!     if let Some(path) = &fm.selected_path {
//!         println!("Selected: {}", path.display());
//!     }
//!     // For multi-select:
//!     for path in fm.selected_paths() {
//!         println!("  {}", path.display());
//!     }
//! }
//! ```
//!
//! ## Configuration
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::file_manager::{FileManager, FileManagerConfig};
//!
//! let config = FileManagerConfig {
//!     enable_multi_select: true,
//!     show_favorites: true,
//!     initial_size: [800.0, 600.0],
//!     ..Default::default()
//! };
//! let mut fm = FileManager::new_with_config(config);
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
mod entry;
mod favorites;
mod history;
mod render;

pub use config::{DialogMode, FileFilter, FileManagerConfig, FmStrings, STRINGS_EN};

use std::path::PathBuf;

use dear_imgui_rs::{Key, Ui, WindowFlags};

use config::FmStrings as Strings;
use entry::{sort_entries, FsEntry, SortColumn, SortOrder};
use favorites::FavoritesPanel;
use history::NavigationHistory;

// ─── Error ──────────────────────────────────────────────────────────────────

/// Internal error type for file manager operations.
///
/// Displayed as colored text in the dialog using localized strings from [`FmStrings`].
enum FmError {
    /// Failed to read directory contents (permissions, path gone, etc.).
    CannotReadDir(String),
    /// `std::fs::create_dir` failed.
    CreateFolderFailed(String),
    /// `std::fs::File::create` failed.
    CreateFileFailed(String),
    /// User-entered path in the breadcrumb text input does not exist.
    PathNotFound(String),
    /// `std::fs::rename` failed.
    RenameFailed(String),
    /// `std::fs::remove_file` / `std::fs::remove_dir` failed.
    DeleteFailed(String),
}

impl FmError {
    /// Format the error for display, using localized prefixes from [`FmStrings`].
    fn format(&self, s: &Strings) -> String {
        match self {
            Self::CannotReadDir(d) => format!("{}: {d}", s.cannot_read_dir),
            Self::CreateFolderFailed(d) => format!("{}: {d}", s.create_folder_failed),
            Self::CreateFileFailed(d) => format!("{}: {d}", s.create_file_failed),
            Self::PathNotFound(p) => format!("{}: {p}", s.path_not_found),
            Self::RenameFailed(d) => format!("{}: {d}", s.rename_failed),
            Self::DeleteFailed(d) => format!("{}: {d}", s.delete_failed),
        }
    }
}

// ─── Deferred actions ───────────────────────────────────────────────────────

/// Deferred UI action collected during rendering, applied after the frame.
///
/// Render functions return `Option<Action>` instead of mutating `FileManager`
/// directly — this avoids borrow conflicts between `&self` reads (for display)
/// and `&mut self` writes (for state changes).
enum Action {
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
    /// Re-sort entries (column/order already updated by table header click).
    SetSort(SortColumn),
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

// ─── FileManager ────────────────────────────────────────────────────────────

/// Universal file manager dialog for Dear ImGui.
///
/// # Lifecycle
///
/// 1. Create: [`new()`](Self::new) or [`new_with_config()`](Self::new_with_config)
/// 2. Open: [`open_folder()`](Self::open_folder), [`open_file()`](Self::open_file),
///    or [`save_file()`](Self::save_file)
/// 3. Render: call [`render()`](Self::render) every frame
/// 4. Result: when `render()` returns `true`, read [`selected_path`](Self::selected_path)
///    or [`selected_paths()`](Self::selected_paths)
///
/// The dialog is a modal popup — it blocks interaction with background windows
/// while open. The instance is reusable: call any `open_*` method to show it again.
pub struct FileManager {
    // ── Configuration ──
    config: FileManagerConfig,
    mode: DialogMode,
    filters: Vec<FileFilter>,
    active_filter: usize,

    // ── Navigation state ──
    current_path: PathBuf,
    drives: Vec<String>,
    history: NavigationHistory,

    // ── Directory contents ──
    entries: Vec<FsEntry>,
    sort_column: SortColumn,
    sort_order: SortOrder,

    // ── UI state ──
    /// Indices into `entries` for currently selected rows.
    selected_indices: Vec<usize>,
    /// Last clicked index for Shift+Click range selection.
    last_click_index: Option<usize>,
    /// When set, scroll the table to bring this row index into view.
    scroll_to_index: Option<usize>,
    /// Text buffer for the filename input (SaveFile mode).
    filename_buf: String,
    /// Text buffer for the "New Folder" inline input.
    new_folder_buf: String,
    /// Text buffer for the breadcrumb text-input mode.
    path_input_buf: String,
    /// Accumulated characters for type-to-search.
    search_buf: String,
    /// Timer for type-to-search reset (resets after 0.5s of no input).
    search_timer: f32,

    /// Text buffer for the "New File" inline input.
    new_file_buf: String,

    /// Whether the "New Folder" inline input is visible.
    show_new_folder: bool,
    /// Whether the "New File" inline input is visible.
    show_new_file: bool,
    /// Whether the overwrite confirmation modal should open.
    show_overwrite_confirm: bool,
    /// Whether the breadcrumb bar is in text-editing mode.
    breadcrumb_editing: bool,

    // ── Context menu / Rename / Delete state ──
    /// Index of the entry targeted by the context menu (right-click).
    context_menu_target: Option<usize>,
    /// Index of the entry currently being renamed (inline input).
    rename_index: Option<usize>,
    /// Text buffer for the rename input.
    rename_buf: String,
    /// Whether the delete confirmation modal should open.
    show_delete_confirm: bool,
    /// Index of the entry pending deletion.
    delete_target: Option<usize>,
    /// Whether hidden files are shown.
    show_hidden: bool,

    favorites: FavoritesPanel,

    // ── Public output ──
    /// `true` while the dialog is visible.
    pub is_open: bool,
    /// Internal: triggers `open_popup` on the next frame (one-shot flag).
    popup_needs_open: bool,
    /// The confirmed path. Set when `render()` returns `true`.
    pub selected_path: Option<PathBuf>,
    /// All confirmed paths (for multi-select in OpenFile mode).
    pub selected_paths: Vec<PathBuf>,
    /// Current error to display, if any.
    error: Option<FmError>,
    /// Whether the directory has been loaded at least once.
    loaded: bool,

    /// Scratch buffer reused for all `write!()` formatting in render functions.
    /// Avoids per-frame allocations for icon+label strings, error messages, etc.
    fmt_buf: String,
}

impl Default for FileManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a filename contains characters that are invalid on Windows or Unix.
fn is_valid_filename(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 {
        return false;
    }
    // Windows reserved names
    let upper = name.to_uppercase();
    let stem = upper.split('.').next().unwrap_or("");
    if matches!(stem, "CON" | "PRN" | "AUX" | "NUL"
        | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
        | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9")
    {
        return false;
    }
    // Invalid characters across platforms
    !name.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0'])
        && !name.ends_with('.')
        && !name.ends_with(' ')
}

impl FileManager {
    /// Create with default configuration.
    pub fn new() -> Self {
        Self::new_with_config(FileManagerConfig::default())
    }

    /// Create with custom configuration.
    pub fn new_with_config(config: FileManagerConfig) -> Self {
        let show_hidden_default = config.show_hidden_files;
        let max_history = config.max_history;
        Self {
            config,
            mode: DialogMode::SelectFolder,
            filters: vec![FileFilter::all()],
            active_filter: 0,
            current_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" })),
            drives: enumerate_drives(),
            history: NavigationHistory::new(max_history),
            entries: Vec::new(),
            sort_column: SortColumn::Name,
            sort_order: SortOrder::Ascending,
            selected_indices: Vec::new(),
            last_click_index: None,
            scroll_to_index: None,
            filename_buf: String::with_capacity(128),
            new_folder_buf: String::with_capacity(64),
            new_file_buf: String::with_capacity(64),
            path_input_buf: String::with_capacity(256),
            search_buf: String::with_capacity(32),
            search_timer: 0.0,
            show_new_folder: false,
            show_new_file: false,
            show_overwrite_confirm: false,
            breadcrumb_editing: false,
            context_menu_target: None,
            rename_index: None,
            rename_buf: String::with_capacity(128),
            show_delete_confirm: false,
            delete_target: None,
            show_hidden: show_hidden_default,
            favorites: FavoritesPanel::with_defaults(),
            is_open: false,
            popup_needs_open: false,
            selected_path: None,
            selected_paths: Vec::new(),
            error: None,
            loaded: false,
            fmt_buf: String::with_capacity(256),
        }
    }

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

    /// Alias for [`open_folder()`](Self::open_folder) (backward compatibility).
    pub fn open(&mut self, initial_path: Option<PathBuf>) {
        self.open_folder(initial_path);
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
    fn refresh_directory(&mut self) {
        self.entries.clear();
        self.selected_indices.clear();
        self.error = None;
        self.loaded = true;
        self.path_input_buf.clear();
        self.path_input_buf
            .push_str(&self.current_path.to_string_lossy());

        let show_files = self.mode != DialogMode::SelectFolder;
        let filter = &self.filters[self.active_filter.min(self.filters.len().saturating_sub(1))];

        match std::fs::read_dir(&self.current_path) {
            Ok(read_dir) => {
                for dir_entry in read_dir.flatten() {
                    if let Some(entry) = FsEntry::from_dir_entry(&dir_entry) {
                        // Filter hidden files
                        if entry.is_hidden && !self.show_hidden {
                            continue;
                        }
                        if entry.is_dir || (show_files && filter.matches_ext(&entry.extension)) {
                            self.entries.push(entry);
                        }
                    }
                }
                sort_entries(&mut self.entries, self.sort_column, self.sort_order, self.config.dirs_first);
            }
            Err(e) => {
                self.error = Some(FmError::CannotReadDir(e.to_string()));
            }
        }
    }

    /// Navigate to a path: set current_path, push history, refresh.
    /// If read_dir fails, shows error and stays in current directory.
    fn try_navigate(&mut self, path: PathBuf) {
        self.error = None;
        self.current_path = path;
        self.refresh_directory();
        // If refresh produced an error, revert to previous path from history
        if self.error.is_some()
            && let Some(prev) = self.history.go_back(&self.current_path)
        {
            self.current_path = prev;
            // Don't refresh again — keep the error visible, entries stay from before
        }
    }

    /// Execute a deferred [`Action`] collected during rendering.
    fn apply_action(&mut self, action: Action, ui: &Ui) {
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
                    self.error = Some(FmError::CreateFolderFailed(format!("Invalid name: \"{name}\"")));
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
                    self.error = Some(FmError::CreateFileFailed(format!("Invalid name: \"{name}\"")));
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
            Action::SetSort(_col) => {
                // sort_column/sort_order already updated by render_file_table
                sort_entries(&mut self.entries, self.sort_column, self.sort_order, self.config.dirs_first);
            }
            Action::ConfirmSelection => {}
            Action::RenameEntry { index, new_name } => {
                if !is_valid_filename(&new_name) {
                    self.error = Some(FmError::RenameFailed(format!("Invalid name: \"{new_name}\"")));
                    self.rename_index = None;
                } else if let Some(entry) = self.entries.get(index) {
                    let old_path = entry.path.clone();
                    let new_path = old_path.parent().unwrap_or(&self.current_path).join(&new_name);
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
                    let result = if entry.is_dir {
                        std::fs::remove_dir_all(&path)
                    } else {
                        std::fs::remove_file(&path)
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

    /// Extract the drive letter from the current path (Windows), or `None`.
    fn current_drive_letter(&self) -> Option<char> {
        self.current_path
            .to_string_lossy()
            .chars()
            .next()
            .filter(|c| c.is_ascii_alphabetic())
    }

    /// Whether the current path has a navigable parent directory.
    fn has_parent(&self) -> bool {
        self.current_path
            .parent()
            .is_some_and(|p| p != self.current_path && !p.as_os_str().is_empty())
    }

    // ─── Type-to-search ─────────────────────────────────────────────

    /// Handle incremental filename search: accumulate typed characters,
    /// find the first matching entry, and select it. Resets after 0.5s of no input.
    fn handle_type_to_search(&mut self, ui: &Ui) {
        if !self.config.enable_type_to_search {
            return;
        }

        let dt = ui.io().delta_time();
        let timeout = self.config.search_timeout;
        self.search_timer = (self.search_timer + dt).min(timeout + 1.0);

        // Check for alphanumeric key presses
        let mut typed_char = None;
        for c in b'A'..=b'Z' {
            let key = match c {
                b'A' => Key::A, b'B' => Key::B, b'C' => Key::C, b'D' => Key::D,
                b'E' => Key::E, b'F' => Key::F, b'G' => Key::G, b'H' => Key::H,
                b'I' => Key::I, b'J' => Key::J, b'K' => Key::K, b'L' => Key::L,
                b'M' => Key::M, b'N' => Key::N, b'O' => Key::O, b'P' => Key::P,
                b'Q' => Key::Q, b'R' => Key::R, b'S' => Key::S, b'T' => Key::T,
                b'U' => Key::U, b'V' => Key::V, b'W' => Key::W, b'X' => Key::X,
                b'Y' => Key::Y, b'Z' => Key::Z,
                _ => continue,
            };
            if ui.is_key_pressed(key) {
                typed_char = Some(c as char);
                break;
            }
        }

        if let Some(ch) = typed_char {
            if self.search_timer > self.config.search_timeout {
                self.search_buf.clear();
            }
            self.search_timer = 0.0;
            self.search_buf.push(ch.to_ascii_lowercase());

            // Find first matching entry
            let search = &self.search_buf;
            for (i, e) in self.entries.iter().enumerate() {
                if e.name_lower.contains(search.as_str()) {
                    self.selected_indices.clear();
                    self.selected_indices.push(i);
                    self.scroll_to_index = Some(i);
                    break;
                }
            }
        }
    }

    // ─── Main render ────────────────────────────────────────────────

    /// Render the file manager dialog. Returns `true` when the user confirms selection.
    pub fn render(&mut self, ui: &Ui) -> bool {
        if !self.is_open {
            return false;
        }

        let strings = self.config.strings;

        if !self.loaded {
            self.refresh_directory();
        }

        let mut confirmed = false;
        let mut do_confirm_selection = false;
        let mut deferred: Option<Action> = None;

        let title = self.config.custom_title.unwrap_or(match self.mode {
            DialogMode::SelectFolder => strings.select_folder,
            DialogMode::OpenFile => strings.open_file,
            DialogMode::SaveFile => strings.save_file,
        });

        // Set window size before opening popup
        unsafe {
            dear_imgui_rs::sys::igSetNextWindowSize(
                dear_imgui_rs::sys::ImVec2 {
                    x: self.config.initial_size[0],
                    y: self.config.initial_size[1],
                },
                dear_imgui_rs::sys::ImGuiCond_Appearing,
            );
            dear_imgui_rs::sys::igSetNextWindowSizeConstraints(
                dear_imgui_rs::sys::ImVec2 {
                    x: self.config.min_size[0],
                    y: self.config.min_size[1],
                },
                dear_imgui_rs::sys::ImVec2 {
                    x: f32::MAX,
                    y: f32::MAX,
                },
                None,
                std::ptr::null_mut(),
            );
        }

        if self.popup_needs_open {
            self.popup_needs_open = false;
            ui.open_popup(title);
        }

        if let Some(_tok) = ui
            .begin_modal_popup_config(title)
            .flags(WindowFlags::NO_COLLAPSE)
            .begin()
        {
            let _rounding = ui.push_style_var(dear_imgui_rs::StyleVar::FrameRounding(3.0));

            // ── Drive selector ──
            if let Some(a) = render::render_drive_bar(
                ui,
                &self.drives,
                self.current_drive_letter(),
                &mut self.fmt_buf,
            ) {
                deferred = Some(a);
            }
            ui.spacing();

            // ── Toolbar ──
            if deferred.is_none()
                && let Some(a) = render::render_toolbar(
                    ui,
                    strings,
                    self.has_parent(),
                    self.history.can_go_back(),
                    self.history.can_go_forward(),
                    &mut self.show_new_folder,
                    &mut self.new_folder_buf,
                    &mut self.show_new_file,
                    &mut self.new_file_buf,
                    self.show_hidden,
                    &self.config,
                    &mut self.fmt_buf,
                )
            {
                deferred = Some(a);
            }

            // ── Breadcrumb / path bar ──
            if deferred.is_none() {
                if self.config.enable_breadcrumbs {
                    if let Some(a) = render::render_breadcrumb_bar(
                        ui,
                        &self.current_path,
                        &mut self.breadcrumb_editing,
                        &mut self.path_input_buf,
                        &mut self.fmt_buf,
                    ) {
                        deferred = Some(a);
                    }
                } else {
                    // Fallback: simple text input path bar
                    let _bg = ui.push_style_color(
                        dear_imgui_rs::StyleColor::FrameBg,
                        crate::theme::BG_FRAME,
                    );
                    ui.text_colored(crate::theme::WARNING, crate::icons::FOLDER_OPEN);
                    ui.same_line_with_spacing(0.0, 6.0);
                    ui.set_next_item_width(ui.content_region_avail()[0]);
                    let enter = ui
                        .input_text("##pathbar", &mut self.path_input_buf)
                        .enter_returns_true(true)
                        .build();
                    if enter {
                        deferred = Some(Action::NavigateToInput(self.path_input_buf.clone()));
                    }
                }
            }
            ui.spacing();

            ui.separator();

            // ── Error ──
            if let Some(ref err) = self.error {
                let msg = err.format(strings);
                self.fmt_buf.clear();
                let _ = std::fmt::Write::write_fmt(
                    &mut self.fmt_buf,
                    format_args!("{} {}", crate::icons::ALERT, msg),
                );
                ui.text_colored(crate::theme::TEXT_ERROR, &self.fmt_buf);
                ui.spacing();
            }

            // ── Content area (favorites + file table) ──
            // Reserve space: status bar + spacing + footer row (filename + buttons) + padding
            let reserved = 64.0_f32;
            let content_h = (ui.content_region_avail()[1] - reserved).max(100.0);

            let show_favorites =
                self.config.show_favorites && !self.favorites.entries.is_empty();

            if show_favorites {
                // Left panel: Favorites
                ui.child_window("##fm_favorites")
                    .size([self.config.favorites_width, content_h])
                    .border(true)
                    .build(ui, || {
                        if let Some(a) = render::render_favorites_panel(
                            ui,
                            &self.favorites,
                            &self.current_path,
                            strings,
                            &mut self.fmt_buf,
                        )
                            && deferred.is_none()
                        {
                            deferred = Some(a);
                        }
                    });
                ui.same_line();
            }

            // Right panel: File table
            {
                ui.child_window("##fm_table_area")
                    .size([0.0, content_h])
                    .build(ui, || {
                        let table_result = render::render_file_table(
                            ui,
                            &self.entries,
                            &mut self.selected_indices,
                            self.mode,
                            self.config.enable_multi_select,
                            &mut self.filename_buf,
                            strings,
                            self.error.is_some(),
                            &mut self.sort_column,
                            &mut self.sort_order,
                            &mut self.rename_index,
                            &mut self.rename_buf,
                            &mut self.context_menu_target,
                            &mut self.last_click_index,
                            &mut self.scroll_to_index,
                            &self.config,
                            &mut self.fmt_buf,
                        );

                        if let Some(a) = table_result.action {
                            match a {
                                Action::ConfirmSelection => do_confirm_selection = true,
                                other => {
                                    if deferred.is_none() {
                                        deferred = Some(other);
                                    }
                                }
                            }
                        }

                        // Handle delete request from context menu (show confirmation)
                        if let Some(idx) = table_result.request_delete {
                            self.delete_target = Some(idx);
                            self.show_delete_confirm = true;
                        }
                    });
            }

            // ── Type-to-search ──
            self.handle_type_to_search(ui);

            // ── Status bar ──
            {
                self.fmt_buf.clear();
                let total = self.entries.len();
                let selected = self.selected_indices.len();
                let _ = std::fmt::Write::write_fmt(
                    &mut self.fmt_buf,
                    format_args!("{total} {}", strings.status_items),
                );
                if selected > 0 {
                    let _ = std::fmt::Write::write_fmt(
                        &mut self.fmt_buf,
                        format_args!("  ·  {selected} {}", strings.status_selected),
                    );
                }
                ui.text_colored(crate::theme::TEXT_MUTED, &self.fmt_buf);
            }

            ui.spacing();

            // ── Footer (filename input for SaveFile + buttons) ──
            let (foot_confirmed, foot_cancelled, foot_action) = render::render_footer(
                ui,
                strings,
                self.mode,
                &self.entries,
                &self.selected_indices,
                &mut self.filename_buf,
                &self.filters,
                self.active_filter,
                &self.config,
                &mut self.fmt_buf,
            );
            if foot_confirmed {
                do_confirm_selection = true;
            }
            if foot_cancelled {
                self.is_open = false;
                ui.close_current_popup();
            }
            if let Some(a) = foot_action
                && deferred.is_none()
            {
                deferred = Some(a);
            }

            // ── Escape to cancel ──
            if ui.is_key_pressed(Key::Escape) {
                if self.rename_index.is_some() {
                    self.rename_index = None;
                    self.rename_buf.clear();
                } else if !self.show_new_folder && !self.show_new_file && !self.breadcrumb_editing {
                    self.is_open = false;
                    ui.close_current_popup();
                }
            }

            // ── Ctrl+A to select all ──
            if ui.is_key_pressed(Key::A)
                && ui.io().key_ctrl()
                && !ui.is_any_item_active()
                && self.config.enable_multi_select
                && self.mode == DialogMode::OpenFile
            {
                self.selected_indices.clear();
                for i in 0..self.entries.len() {
                    if !self.entries[i].is_dir {
                        self.selected_indices.push(i);
                    }
                }
            }

            // ── Ctrl+L to edit path ──
            if ui.is_key_pressed(Key::L)
                && ui.io().key_ctrl()
                && !ui.is_any_item_active()
                && self.config.enable_breadcrumbs
            {
                self.breadcrumb_editing = true;
                self.path_input_buf.clear();
                self.path_input_buf.push_str(&self.current_path.to_string_lossy());
            }

            // ── Ctrl+H to toggle hidden files ──
            if ui.is_key_pressed(Key::H)
                && ui.io().key_ctrl()
                && !ui.is_any_item_active()
            {
                self.show_hidden = !self.show_hidden;
                self.refresh_directory();
            }

            // ── F2 to rename ──
            if ui.is_key_pressed(Key::F2)
                && !ui.is_any_item_active()
                && self.rename_index.is_none()
                && let Some(&idx) = self.selected_indices.first()
                && let Some(e) = self.entries.get(idx)
            {
                self.rename_index = Some(idx);
                self.rename_buf.clear();
                self.rename_buf.push_str(&e.name);
            }

            // ── Delete key ──
            if ui.is_key_pressed(Key::Delete)
                && !ui.is_any_item_active()
                && self.rename_index.is_none()
                && let Some(&idx) = self.selected_indices.first()
                && self.entries.get(idx).is_some()
            {
                self.delete_target = Some(idx);
                self.show_delete_confirm = true;
            }

            // ── Handle confirmation ──
            if do_confirm_selection {
                confirmed = self.try_confirm(ui);
            }

            // ── Overwrite confirmation modal ──
            if let Some(result) = render::render_overwrite_confirm(
                ui,
                strings,
                &mut self.show_overwrite_confirm,
                &mut self.fmt_buf,
            )
                && result
            {
                self.finalize_selection();
                confirmed = true;
                ui.close_current_popup();
            }

            // ── Delete confirmation modal ──
            if let Some(result) = render::render_delete_confirm(
                ui,
                strings,
                &mut self.show_delete_confirm,
                self.delete_target.and_then(|i| self.entries.get(i).map(|e| e.name.as_str())),
                &mut self.fmt_buf,
            ) {
                if result {
                    if let Some(idx) = self.delete_target {
                        deferred = Some(Action::DeleteEntry(idx));
                    }
                } else {
                    self.delete_target = None;
                }
            }
        }

        // Apply deferred action
        if let Some(action) = deferred {
            self.apply_action(action, ui);
        }

        confirmed
    }

    // ─── Confirmation logic ─────────────────────────────────────────

    /// Attempt to confirm the current selection based on dialog mode.
    ///
    /// - **SelectFolder**: confirms the current directory.
    /// - **OpenFile**: confirms selected file(s); does nothing if no file is selected.
    /// - **SaveFile**: checks for existing file → shows overwrite modal if needed.
    ///
    /// Returns `true` if the dialog should close with a confirmed result.
    fn try_confirm(&mut self, ui: &Ui) -> bool {
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
                self.selected_path = Some(paths[0].clone());
                self.selected_paths = paths;
                self.is_open = false;
                ui.close_current_popup();
                true
            }
            DialogMode::SaveFile => {
                let fname = self.filename_buf.trim();
                if fname.is_empty() {
                    return false;
                }
                let target = self.current_path.join(fname);
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
    fn finalize_selection(&mut self) {
        if self.mode == DialogMode::SaveFile {
            let target = self.current_path.join(self.filename_buf.trim());
            self.selected_path = Some(target);
        }
        self.is_open = false;
    }
}

// ─── Clipboard helper ────────────────────────────────────────────────────────

/// Set the ImGui clipboard text via raw sys API.
fn ui_set_clipboard(_ui: &Ui, text: &str) {
    let c_str = std::ffi::CString::new(text).unwrap_or_default();
    unsafe {
        dear_imgui_rs::sys::igSetClipboardText(c_str.as_ptr());
    }
}

// ─── Drive enumeration ──────────────────────────────────────────────────────

/// Enumerate available drive letters on Windows (e.g. `["C:\\", "D:\\"]`).
#[cfg(target_os = "windows")]
fn enumerate_drives() -> Vec<String> {
    use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
    let mut drives = Vec::new();
    unsafe {
        let mask = GetLogicalDrives();
        for i in 0..26u32 {
            if mask & (1 << i) != 0 {
                let letter = (b'A' + i as u8) as char;
                drives.push(format!("{letter}:\\"));
            }
        }
    }
    drives
}

/// On non-Windows platforms, returns `["/"]` as the sole root.
#[cfg(not(target_os = "windows"))]
fn enumerate_drives() -> Vec<String> {
    vec!["/".to_string()]
}
