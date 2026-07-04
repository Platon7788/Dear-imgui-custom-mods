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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[must_use]
pub struct FileFilter {
    /// Display name, e.g. "Image Files (*.png, *.jpg)"
    pub label: String,
    /// Lowercase extensions without dot, e.g. `["png", "jpg"]`.
    pub extensions: Vec<String>,
}

impl FileFilter {
    /// Create a new filter. Pass extensions without dots.
    ///
    /// Accepts any iterable yielding string-like values, so all of these work:
    /// ```ignore
    /// FileFilter::new("Rust", &["rs", "toml"]);
    /// FileFilter::new("Rust", ["rs", "toml"]);
    /// FileFilter::new("Rust", vec!["rs".to_string(), "toml".to_string()]);
    /// ```
    pub fn new<I, S>(label: impl Into<String>, extensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let extensions: Vec<String> = extensions
            .into_iter()
            .map(|e| e.as_ref().to_lowercase())
            .collect();
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

    /// Add an extension to this filter, returning self for chaining.
    pub fn with_extension(mut self, ext: impl AsRef<str>) -> Self {
        self.extensions.push(ext.as_ref().to_lowercase());
        self
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
//
// The catalogue now lives in `crate::i18n::file_manager` (project-wide i18n
// convention). These shims preserve the historic public paths
// (`file_manager::FmStrings`, `STRINGS_EN`, `STRINGS_RU`,
// `strings_for_locale`) so existing callers keep compiling unchanged.

/// All user-facing strings for the file manager dialog.
///
/// Alias of [`crate::i18n::file_manager::Strings`], kept for backward
/// compatibility with the historic `file_manager::FmStrings` path.
pub type FmStrings = crate::i18n::file_manager::Strings;

/// Default English catalogue — re-export of [`crate::i18n::file_manager::EN`].
pub use crate::i18n::file_manager::EN as STRINGS_EN;

/// Russian catalogue — re-export of [`crate::i18n::file_manager::RU`].
pub use crate::i18n::file_manager::RU as STRINGS_RU;

/// Resolve the static catalogue for `locale`.
///
/// ```rust,no_run
/// # use dear_imgui_custom_mod::i18n::Locale;
/// # use dear_imgui_custom_mod::file_manager::strings_for_locale;
/// let s = strings_for_locale(Locale::Ru);
/// assert_eq!(s.cancel, "Отмена");
/// ```
#[must_use]
pub fn strings_for_locale(locale: crate::i18n::Locale) -> &'static FmStrings {
    crate::i18n::file_manager::strings(locale)
}

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
#[must_use]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct FileManagerConfig {
    /// Localized UI strings. Default: [`STRINGS_EN`].
    /// Skipped by serde — restored to [`STRINGS_EN`] on deserialization.
    #[serde(skip, default = "default_strings")]
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
    /// Skipped by serde — always `None` after deserialization.
    #[serde(skip, default)]
    pub custom_title: Option<&'static str>,
    /// Maximum navigation history entries per stack. Default: `100`.
    pub max_history: usize,
    /// Type-to-search timeout in seconds before resetting the search buffer. Default: `0.5`.
    pub search_timeout: f32,
    /// Whether directories are always sorted before files. Default: `true`.
    /// When `false`, directories and files are sorted together alphabetically.
    pub dirs_first: bool,
    /// Button width in the footer (Confirm / Cancel). Default: `100.0`.
    pub button_width: f32,
    /// Button height in the footer. Default: `24.0`.
    pub button_height: f32,
    /// Width of the filter dropdown in the footer. Default: `180.0`.
    pub filter_width: f32,
    /// Width of the inline input for New Folder / New File / Rename. Default: `200.0`.
    /// Set to `0.0` to auto-size to available width.
    pub inline_input_width: f32,
    /// Custom icon/color mapping callback. If `None`, uses built-in `file_icon_for_ext`.
    /// The callback takes a lowercase file extension and returns `(icon: &'static str, color: [f32; 4])`.
    /// Skipped by serde — function pointers are not serializable; always `None` after deserialization.
    #[serde(skip, default)]
    pub icon_override: Option<IconOverrideFn>,

    /// User-visible language for the dialog's labels, buttons, and
    /// tooltips. Default [`crate::i18n::Locale::En`]. Switching to
    /// [`crate::i18n::Locale::Ru`] requires the host to bake
    /// `GlyphRanges::Cyrillic` into the active font atlas — without
    /// that, non-ASCII characters render as `?` placeholders.
    ///
    /// `strings` is automatically refreshed whenever `locale` changes
    /// through [`FileManager::set_locale`], so the field on this struct
    /// stays in sync. Callers can still set `strings` directly to a
    /// custom catalogue (e.g. for a third language) — that bypasses
    /// the locale match entirely.
    #[serde(default)]
    pub locale: crate::i18n::Locale,
}

fn default_strings() -> &'static FmStrings {
    &STRINGS_EN
}

impl Default for FileManagerConfig {
    fn default() -> Self {
        ron::from_str(include_str!("config.ron"))
            .expect("built-in file_manager/config.ron is valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Locale;

    #[test]
    fn default_locale_is_english() {
        let cfg = FileManagerConfig::default();
        assert_eq!(cfg.locale, Locale::En);
    }

    #[test]
    fn locale_round_trips_through_ron() {
        let cfg = FileManagerConfig {
            locale: Locale::Ru,
            ..FileManagerConfig::default()
        };
        let text = ron::ser::to_string(&cfg).unwrap();
        let back: FileManagerConfig = ron::from_str(&text).unwrap();
        assert_eq!(back.locale, Locale::Ru);
    }

    #[test]
    fn locale_field_optional_in_ron() {
        // Older configs without `locale:` must fall back to English.
        let cfg: FileManagerConfig = ron::from_str(
            r#"(
                initial_size: (750.0, 520.0),
                min_size: (500.0, 350.0),
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
                max_history: 100,
                search_timeout: 0.5,
                dirs_first: true,
                button_width: 100.0,
                button_height: 24.0,
                filter_width: 180.0,
                inline_input_width: 200.0,
            )"#,
        )
        .expect("file_manager config without `locale` field must still parse");
        assert_eq!(cfg.locale, Locale::En);
    }
}
