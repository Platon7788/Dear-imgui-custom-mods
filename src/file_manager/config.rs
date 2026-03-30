//! Configuration types for the file manager dialog.
//!
//! Contains [`DialogMode`], [`FileFilter`], [`FmStrings`], and [`FileManagerConfig`].
//!
//! All types are configurable at construction time. [`FileFilter`] instances are
//! passed per-call to [`open_file()`](super::FileManager::open_file) /
//! [`save_file()`](super::FileManager::save_file), while [`FileManagerConfig`]
//! is set once via [`new_with_config()`](super::FileManager::new_with_config).

/// Callback type for custom file icon/color mapping by extension.
pub type IconOverrideFn = fn(&str) -> Option<(&'static str, [f32; 4])>;

// ─── Dialog mode ─────────────────────────────────────────────────────────────

/// Determines the behavior and appearance of the file manager dialog.
///
/// Each mode controls which entries are visible, what the confirm button says,
/// and whether a filename input is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogMode {
    /// Pick a directory. Shows only folders, confirm button = "Select Folder".
    /// The confirmed path is the current directory.
    SelectFolder,
    /// Pick an existing file. Shows folders + files, confirm button = "Open".
    /// Supports multi-select with Ctrl+Click (if enabled in config).
    OpenFile,
    /// Choose a save location + filename. Shows folders + files, has a filename
    /// text input, confirm button = "Save". Triggers overwrite confirmation if
    /// the target file already exists.
    SaveFile,
}

// ─── File type filter ────────────────────────────────────────────────────────

/// A filter entry for the file type dropdown.
///
/// Extensions are stored without the leading dot and in lowercase.
/// An empty `extensions` vec matches all files.
#[derive(Debug, Clone)]
pub struct FileFilter {
    /// Display name, e.g. "Image Files (*.png, *.jpg)"
    pub label: String,
    /// Lowercase extensions without dot, e.g. `["png", "jpg"]`.
    pub extensions: Vec<String>,
}

impl FileFilter {
    /// Create a new filter. Pass extensions without dots.
    pub fn new(label: impl Into<String>, extensions: &[&str]) -> Self {
        let extensions: Vec<String> = extensions.iter().map(|e| e.to_lowercase()).collect();
        Self {
            label: label.into(),
            extensions,
        }
    }

    /// "All Files" filter — matches everything.
    pub fn all() -> Self {
        Self {
            label: "All Files (*.*)".into(),
            extensions: vec![],
        }
    }

    /// Test whether a lowercase file extension matches this filter.
    /// Pass the pre-computed lowercase extension from `FsEntry`.
    pub(crate) fn matches_ext(&self, ext_lower: &str) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        self.extensions.iter().any(|e| e == ext_lower)
    }
}

// ─── Strings (localizable) ──────────────────────────────────────────────────

/// All user-facing strings for the file manager dialog.
///
/// Override for localization by creating a `static` instance with translated
/// strings and passing it to [`FileManagerConfig::strings`].
///
/// Default English strings are available as [`STRINGS_EN`].
pub struct FmStrings {
    // ── Dialog titles ──
    /// Window title for SelectFolder mode.
    pub select_folder: &'static str,
    /// Window title for OpenFile mode.
    pub open_file: &'static str,
    /// Window title for SaveFile mode.
    pub save_file: &'static str,

    // ── Toolbar buttons ──
    /// Tooltip for the "go to parent" button.
    pub up: &'static str,
    /// Tooltip for the "go back" button.
    pub back: &'static str,
    /// Tooltip for the "go forward" button.
    pub forward: &'static str,
    /// Label for "New Folder" toolbar button.
    pub new_folder: &'static str,
    /// Label for "New File" toolbar button.
    pub new_file: &'static str,
    /// Label for the "Create" button in new folder/file inputs.
    pub create: &'static str,
    /// Label for the "Cancel" button.
    pub cancel: &'static str,
    /// Label for the confirm button in SaveFile mode.
    pub save: &'static str,
    /// Label for the confirm button in OpenFile mode.
    pub open: &'static str,

    // ── Footer / inputs ──
    /// Label for the filename text input (SaveFile mode).
    pub filename: &'static str,
    /// Label for the "All Files" filter entry.
    pub all_files: &'static str,
    /// Shown when directory is empty.
    pub empty_parens: &'static str,

    // ── Error messages ──
    /// Prefix for "cannot read directory" errors.
    pub cannot_read_dir: &'static str,
    /// Prefix for "create folder failed" errors.
    pub create_folder_failed: &'static str,
    /// Prefix for "create file failed" errors.
    pub create_file_failed: &'static str,
    /// Prefix for "path not found" errors.
    pub path_not_found: &'static str,

    // ── Overwrite confirmation ──
    /// Title for the overwrite confirmation modal.
    pub overwrite_title: &'static str,
    /// Body text for the overwrite confirmation modal.
    pub overwrite_message: &'static str,
    /// "Yes" button label.
    pub yes: &'static str,
    /// "No" button label.
    pub no: &'static str,

    // ── Sidebar ──
    /// Header label for the favorites sidebar.
    pub favorites: &'static str,

    // ── Table column headers ──
    /// Column header: file name.
    pub col_name: &'static str,
    /// Column header: file size.
    pub col_size: &'static str,
    /// Column header: date modified.
    pub col_date: &'static str,
    /// Column header: file type/extension.
    pub col_type: &'static str,

    // ── Context menu / actions ──
    /// Context menu item: rename entry.
    pub rename: &'static str,
    /// Context menu item: delete entry.
    pub delete: &'static str,
    /// Title for the delete confirmation modal.
    pub confirm_delete_title: &'static str,
    /// Body text prefix for the delete confirmation modal.
    pub confirm_delete_message: &'static str,
    /// Prefix for "rename failed" errors.
    pub rename_failed: &'static str,
    /// Prefix for "delete failed" errors.
    pub delete_failed: &'static str,
    /// Context menu item: copy file path to clipboard.
    pub copy_path: &'static str,
    /// Toolbar toggle: show/hide hidden files.
    pub show_hidden: &'static str,

    // ── Status bar ──
    /// Suffix for item count, e.g. "42 items".
    pub status_items: &'static str,
    /// Suffix for selection count, e.g. "3 selected".
    pub status_selected: &'static str,
    /// Tooltip: keyboard shortcut hint for status bar.
    pub shortcut_hint: &'static str,
    /// "Select All" label (Ctrl+A context).
    pub select_all: &'static str,
}

/// Default English strings for the file manager dialog.
///
/// Pass to [`FileManagerConfig::strings`] or use as a reference when creating
/// translated string tables.
pub static STRINGS_EN: FmStrings = FmStrings {
    select_folder: "Select Folder",
    open_file: "Open File",
    save_file: "Save File",
    up: "Up",
    back: "Back",
    forward: "Forward",
    new_folder: "New Folder",
    new_file: "New File",
    create: "Create",
    cancel: "Cancel",
    save: "Save",
    open: "Open",
    filename: "Filename:",
    all_files: "All Files (*.*)",
    empty_parens: "(empty)",
    cannot_read_dir: "Cannot read directory",
    create_folder_failed: "Failed to create folder",
    create_file_failed: "Failed to create file",
    path_not_found: "Path not found",
    overwrite_title: "Confirm Overwrite",
    overwrite_message: "File already exists. Overwrite?",
    yes: "Yes",
    no: "No",
    favorites: "Favorites",
    col_name: "Name",
    col_size: "Size",
    col_date: "Date Modified",
    col_type: "Type",
    rename: "Rename",
    delete: "Delete",
    confirm_delete_title: "Confirm Delete",
    confirm_delete_message: "Are you sure you want to delete",
    rename_failed: "Failed to rename",
    delete_failed: "Failed to delete",
    copy_path: "Copy Path",
    show_hidden: "Hidden",
    status_items: "items",
    status_selected: "selected",
    shortcut_hint: "F2: Rename · Del: Delete · Backspace: Parent · Type to search",
    select_all: "Select All",
};

// ─── FileManagerConfig ──────────────────────────────────────────────────────

/// Full configuration for a [`FileManager`](super::FileManager).
///
/// Pass to [`FileManager::new_with_config()`](super::FileManager::new_with_config).
/// All fields have sensible defaults via [`Default`].
///
/// # Example
///
/// ```rust,ignore
/// let config = FileManagerConfig {
///     enable_multi_select: true,
///     initial_size: [900.0, 650.0],
///     ..Default::default()
/// };
/// let fm = FileManager::new_with_config(config);
/// ```
pub struct FileManagerConfig {
    /// Localized UI strings. Default: [`STRINGS_EN`].
    pub strings: &'static FmStrings,
    /// Initial window size `[width, height]` in pixels. Default: `[750, 520]`.
    pub initial_size: [f32; 2],
    /// Minimum window size `[width, height]` in pixels. Default: `[500, 350]`.
    pub min_size: [f32; 2],
    /// Show the favorites sidebar (Desktop, Documents, Downloads). Default: `true`.
    pub show_favorites: bool,
    /// Width of the favorites sidebar in pixels. Default: `150.0`.
    pub favorites_width: f32,
    /// Allow Ctrl+Click multi-select in OpenFile mode. Default: `false`.
    pub enable_multi_select: bool,
    /// Show clickable breadcrumb path bar (vs. plain text input). Default: `true`.
    pub enable_breadcrumbs: bool,
    /// Enable Back/Forward navigation buttons. Default: `true`.
    pub enable_history: bool,
    /// Enable type-to-search (start typing to jump to matching files). Default: `true`.
    pub enable_type_to_search: bool,
    /// Show hidden files (dotfiles on Unix, hidden attribute on Windows). Default: `false`.
    pub show_hidden_files: bool,
    /// Show the Size column in the file table. Default: `true`.
    pub show_column_size: bool,
    /// Show the Date Modified column in the file table. Default: `true`.
    pub show_column_date: bool,
    /// Show the Type column in the file table. Default: `true`.
    pub show_column_type: bool,
    /// Custom window title. If `None`, uses mode-specific title from `strings`.
    /// Example: `Some("Select Output Directory")`. Default: `None`.
    pub custom_title: Option<&'static str>,
    /// Maximum navigation history entries per stack. Default: `100`.
    pub max_history: usize,
    /// Type-to-search timeout in seconds before resetting the search buffer. Default: `0.5`.
    pub search_timeout: f32,
    /// Whether directories are always sorted before files. Default: `true`.
    /// When `false`, directories and files are sorted together alphabetically.
    pub dirs_first: bool,
    /// Button width in the footer (Confirm / Cancel). Default: `120.0`.
    pub button_width: f32,
    /// Button height in the footer. Default: `28.0`.
    pub button_height: f32,
    /// Width of the filter dropdown in the footer. Default: `180.0`.
    pub filter_width: f32,
    /// Width of the inline input for New Folder / New File / Rename. Default: `200.0`.
    /// Set to `0.0` to auto-size to available width.
    pub inline_input_width: f32,
    /// Custom icon/color mapping callback. If `None`, uses built-in `file_icon_for_ext`.
    /// The callback takes a lowercase file extension and returns `(icon: &'static str, color: [f32; 4])`.
    pub icon_override: Option<IconOverrideFn>,
}

impl Default for FileManagerConfig {
    fn default() -> Self {
        Self {
            strings: &STRINGS_EN,
            initial_size: [750.0, 520.0],
            min_size: [500.0, 350.0],
            show_favorites: true,
            favorites_width: 150.0,
            enable_multi_select: false,
            enable_breadcrumbs: true,
            enable_history: true,
            enable_type_to_search: true,
            show_hidden_files: false,
            show_column_size: true,
            show_column_date: true,
            show_column_type: true,
            custom_title: None,
            max_history: 100,
            search_timeout: 0.5,
            dirs_first: true,
            button_width: 120.0,
            button_height: 28.0,
            filter_width: 180.0,
            inline_input_width: 200.0,
            icon_override: None,
        }
    }
}
