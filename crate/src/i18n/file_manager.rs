//! `crate::file_manager` localisation strings.
//!
//! Relocated from `file_manager/config.rs` to match the project-wide i18n
//! convention (one `crate::i18n::<widget>` sub-module per localized widget).
//! The historic paths `file_manager::{FmStrings, STRINGS_EN, STRINGS_RU,
//! strings_for_locale}` are preserved as re-export shims in
//! `file_manager/config.rs`.

use super::Locale;

/// All user-facing strings for the file manager dialog.
///
/// Resolve through [`strings`] (or the `FileManager` locale API). Switching to
/// [`RU`] requires the host to bake `GlyphRanges::Cyrillic` (or a superset)
/// into the active font atlas — otherwise non-ASCII characters render as `?`.
#[derive(Debug)]
pub struct Strings {
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
    /// Tooltip for the "refresh directory" button.
    pub refresh: &'static str,
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
    /// Detail for a rejected filename (create / rename / save).
    pub invalid_name: &'static str,
    /// Detail for a rename whose destination name already exists.
    pub target_exists: &'static str,

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

/// Default English catalogue.
pub const EN: Strings = Strings {
    select_folder: "Select Folder",
    open_file: "Open File",
    save_file: "Save File",
    up: "Up",
    back: "Back",
    forward: "Forward",
    refresh: "Refresh",
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
    invalid_name: "Invalid name",
    target_exists: "Target already exists",
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

/// Russian catalogue. Requires the host to bake `GlyphRanges::Cyrillic` (or a
/// superset) into the active font atlas — without that, non-ASCII characters
/// render as `?` placeholders.
pub const RU: Strings = Strings {
    select_folder: "Выбор папки",
    open_file: "Открыть файл",
    save_file: "Сохранить файл",
    up: "Вверх",
    back: "Назад",
    forward: "Вперёд",
    refresh: "Обновить",
    new_folder: "Новая папка",
    new_file: "Новый файл",
    create: "Создать",
    cancel: "Отмена",
    save: "Сохранить",
    open: "Открыть",
    filename: "Имя файла:",
    all_files: "Все файлы (*.*)",
    empty_parens: "(пусто)",
    cannot_read_dir: "Не удаётся прочитать каталог",
    create_folder_failed: "Не удалось создать папку",
    create_file_failed: "Не удалось создать файл",
    path_not_found: "Путь не найден",
    invalid_name: "Недопустимое имя",
    target_exists: "Цель уже существует",
    overwrite_title: "Подтверждение перезаписи",
    overwrite_message: "Файл уже существует. Перезаписать?",
    yes: "Да",
    no: "Нет",
    favorites: "Избранное",
    col_name: "Имя",
    col_size: "Размер",
    col_date: "Изменён",
    col_type: "Тип",
    rename: "Переименовать",
    delete: "Удалить",
    confirm_delete_title: "Подтверждение удаления",
    confirm_delete_message: "Вы уверены, что хотите удалить",
    rename_failed: "Не удалось переименовать",
    delete_failed: "Не удалось удалить",
    copy_path: "Копировать путь",
    show_hidden: "Скрытые",
    status_items: "эл.",
    status_selected: "выделено",
    shortcut_hint: "F2: переименовать · Del: удалить · Backspace: вверх · Введите для поиска",
    select_all: "Выделить всё",
};

/// Resolve the static catalogue for `locale`.
pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}
