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
//! | [`lifecycle`](lifecycle) | `open_*` entry points, directory (re)listing, confirm logic |
//! | [`view`](view) | Per-frame `render()` driver |
//! | [`search`](search) | Type-to-search incremental matching |
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

mod actions;
pub mod config;
mod entry;
mod favorites;
mod history;
mod lifecycle;
mod render;
mod search;
mod util;
mod view;

pub use config::{
    DialogMode, FileFilter, FileManagerConfig, FmStrings, STRINGS_EN, STRINGS_RU,
    strings_for_locale,
};

use std::path::PathBuf;

use actions::Action;
use config::FmStrings as Strings;
use entry::{FsEntry, SortColumn, SortOrder, sort_entries};
use favorites::FavoritesPanel;
use history::NavigationHistory;
use util::{BreadcrumbSegment, drive_letter_of, enumerate_drives, rebuild_breadcrumb_segments};

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

    /// Cached breadcrumb segments — rebuilt only when `current_path` changes
    /// (P1-3: was being re-collected per frame as `Vec<&str>`).
    breadcrumb_segments: Vec<BreadcrumbSegment>,

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

impl FileManager {
    /// Create with default configuration.
    pub fn new() -> Self {
        Self::new_with_config(FileManagerConfig::default())
    }

    /// Create with custom configuration.
    pub fn new_with_config(mut config: FileManagerConfig) -> Self {
        let show_hidden_default = config.show_hidden_files;
        let max_history = config.max_history;
        // Sync `strings` to the resolved locale catalogue so a config
        // loaded from ron with `locale: Ru` already comes up Russian
        // without the host having to call `set_locale` first. Hosts
        // that pre-set `config.strings` to a custom catalogue lose
        // that override here only if `locale` was also explicitly
        // changed — which is the right precedence: explicit locale
        // wins over the implicit STRINGS_EN serde-default.
        config.strings = crate::file_manager::config::strings_for_locale(config.locale);
        Self {
            config,
            mode: DialogMode::SelectFolder,
            filters: vec![FileFilter::all()],
            active_filter: 0,
            current_path: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" })),
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
            breadcrumb_segments: Vec::with_capacity(8),
            is_open: false,
            popup_needs_open: false,
            selected_path: None,
            selected_paths: Vec::new(),
            error: None,
            loaded: false,
            fmt_buf: String::with_capacity(256),
        }
    }

    // ─── Localisation ────────────────────────────────────────────────

    /// Override the user-visible language on construction. Default
    /// is English; pass [`crate::i18n::Locale::Ru`] for Russian. The
    /// host must bake `GlyphRanges::Cyrillic` (or a superset) into
    /// the active font atlas — without that, Cyrillic characters
    /// render as `?`.
    ///
    /// The locale is stored on [`FileManagerConfig::locale`] so it
    /// round-trips through `ron::to_string` / `ron::from_str` along
    /// with every other setting.
    #[must_use]
    pub fn with_locale(mut self, locale: crate::i18n::Locale) -> Self {
        self.set_locale(locale);
        self
    }

    /// Mid-flight language switch — refreshes both `config.locale`
    /// and `config.strings`. Same caveat as [`Self::with_locale`]
    /// regarding font atlas glyph ranges.
    pub fn set_locale(&mut self, locale: crate::i18n::Locale) {
        self.config.locale = locale;
        self.config.strings = crate::file_manager::config::strings_for_locale(locale);
    }

    /// Currently-active locale (mirror of `self.config.locale`).
    pub fn locale(&self) -> crate::i18n::Locale {
        self.config.locale
    }

    // `try_navigate` and `apply_action` are implemented in [`actions.rs`](super::actions).

    /// Extract the drive letter from the current path (Windows), or `None`.
    ///
    /// BUG-FM2: only reports a letter for a true `X:` drive prefix. The old
    /// "first alphabetic char" heuristic mis-reported the drive for a relative
    /// path like `foo\bar` (→ `'F'`), wrongly highlighting a drive button.
    pub(super) fn current_drive_letter(&self) -> Option<char> {
        drive_letter_of(&self.current_path.to_string_lossy())
    }

    /// Whether the current path has a navigable parent directory.
    pub(super) fn has_parent(&self) -> bool {
        self.current_path
            .parent()
            .is_some_and(|p| p != self.current_path && !p.as_os_str().is_empty())
    }
}

// `ui_set_clipboard` and `enumerate_drives` live in [`util.rs`](util).
// `open_*` / `refresh_directory` / `try_confirm` live in [`lifecycle.rs`](lifecycle).
// `render` / `handle_type_to_search` live in [`view.rs`](view) / [`search.rs`](search).

#[cfg(test)]
mod tests;
