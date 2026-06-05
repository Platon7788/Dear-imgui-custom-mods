//! `crate::hex_viewer` localisation strings.

#![allow(missing_docs)] // every field is self-describing by name

use super::Locale;

/// Every user-visible label rendered by [`crate::hex_viewer::HexViewer`].
/// Format-template strings live as helper functions below.
#[derive(Debug)]
pub struct Strings {
    // Column headers (above the byte grid)
    pub header_address: &'static str,
    pub header_ascii: &'static str,

    // Goto popup
    pub goto_title: &'static str,
    pub action_go: &'static str,
    pub action_cancel: &'static str,

    // Search popup
    pub mode_hex: &'static str,
    pub mode_string: &'static str,
    pub hint_hex: &'static str,
    pub hint_ascii: &'static str,
    pub hint_utf8: &'static str,
    pub hint_utf16le: &'static str,
    pub action_find: &'static str,

    // Context menu
    pub menu_goto: &'static str,
    pub menu_search: &'static str,
    pub menu_step_back: &'static str,
    pub menu_step_forward: &'static str,
    pub menu_settings: &'static str,

    // Settings popup
    pub settings_title: &'static str,
    pub settings_bytes_per_row: &'static str,
    pub settings_display: &'static str,
    pub settings_show_ascii: &'static str,
    pub settings_show_inspector: &'static str,
    pub settings_show_offsets: &'static str,
    pub settings_show_column_headers: &'static str,
    pub settings_show_column_dividers: &'static str,
    pub settings_show_group_dividers: &'static str,
    pub settings_show_splitter: &'static str,
    pub settings_format: &'static str,
    pub settings_uppercase: &'static str,
    pub settings_category_colors: &'static str,
    pub settings_dim_zeros: &'static str,
    pub action_close: &'static str,

    // Per-byte hover tooltip
    pub tooltip_address_prefix: &'static str, // "Address: "
    pub tooltip_label_hex: &'static str,      // "Hex"
    pub tooltip_label_dec: &'static str,      // "Dec"
    pub tooltip_label_oct: &'static str,      // "Oct"
    pub tooltip_label_bin: &'static str,      // "Bin"
    pub tooltip_label_category: &'static str, // "Category"
    pub tooltip_label_char: &'static str,     // "Char"
    pub tooltip_double_click_copy: &'static str, // "Double-click to copy"

    // Inspector / status footer
    pub inspector_endian_le: &'static str, // "little-endian"
    pub inspector_endian_be: &'static str, // "big-endian"
    pub inspector_bytes: &'static str,     // "bytes"
    pub editing_hex: &'static str,         // "[EDITING HEX]"
    pub editing_ascii: &'static str,       // "[EDITING ASCII]"
}

/// English catalogue — the historic strings, unchanged.
pub const EN: Strings = Strings {
    header_address: "Address",
    header_ascii: "ASCII",

    goto_title: "Goto address (hex or decimal):",
    action_go: "Go",
    action_cancel: "Cancel",

    mode_hex: "Hex",
    mode_string: "String",
    hint_hex: "Hex pattern (e.g. 4D 5A ?? 00):",
    hint_ascii: "ASCII string:",
    hint_utf8: "UTF-8 string:",
    hint_utf16le: "UTF-16LE string (e.g. Windows wchar_t):",
    action_find: "Find",

    menu_goto: "\u{00BB}  Go to Address\tCtrl+G",
    menu_search: "\u{00BB}  Search\tCtrl+F",
    menu_step_back: "\u{2190}  Step back\tAlt+Left",
    menu_step_forward: "\u{2192}  Step forward\tAlt+Right",
    menu_settings: "Settings...",

    settings_title: "Hex Viewer Settings",
    settings_bytes_per_row: "Bytes per row:",
    settings_display: "Display:",
    settings_show_ascii: "Show ASCII",
    settings_show_inspector: "Show inspector",
    settings_show_offsets: "Show offsets",
    settings_show_column_headers: "Show column headers",
    settings_show_column_dividers: "Show column dividers",
    settings_show_group_dividers: "Show group dividers (dashed)",
    settings_show_splitter: "Show splitter",
    settings_format: "Format:",
    settings_uppercase: "Uppercase hex",
    settings_category_colors: "Category colors",
    settings_dim_zeros: "Dim zero bytes",
    action_close: "Close",

    tooltip_address_prefix: "Address: ",
    tooltip_label_hex: "Hex",
    tooltip_label_dec: "Dec",
    tooltip_label_oct: "Oct",
    tooltip_label_bin: "Bin",
    tooltip_label_category: "Category",
    tooltip_label_char: "Char",
    tooltip_double_click_copy: "Double-click to copy",

    inspector_endian_le: "little-endian",
    inspector_endian_be: "big-endian",
    inspector_bytes: "bytes",
    editing_hex: "[EDITING HEX]",
    editing_ascii: "[EDITING ASCII]",
};

/// Russian catalogue. Field-by-field translation keeping the same
/// semantic register as English (terse imperative for menus,
/// neutral descriptive for tooltips). Hex / Dec / Oct / Bin /
/// ASCII / UTF-8 / UTF-16LE / Cyrillic-untranslated technical
/// abbreviations stay as-is — they're conventional in Russian
/// reverse-engineering / debugging context.
pub const RU: Strings = Strings {
    header_address: "Адрес",
    header_ascii: "ASCII",

    goto_title: "Перейти к адресу (hex или dec):",
    action_go: "Перейти",
    action_cancel: "Отмена",

    mode_hex: "Hex",
    mode_string: "Строка",
    hint_hex: "Hex-шаблон (например 4D 5A ?? 00):",
    hint_ascii: "ASCII-строка:",
    hint_utf8: "UTF-8 строка:",
    hint_utf16le: "UTF-16LE строка (Windows wchar_t):",
    action_find: "Найти",

    menu_goto: "\u{00BB}  Перейти к адресу\tCtrl+G",
    menu_search: "\u{00BB}  Поиск\tCtrl+F",
    menu_step_back: "\u{2190}  Шаг назад\tAlt+Left",
    menu_step_forward: "\u{2192}  Шаг вперёд\tAlt+Right",
    menu_settings: "Настройки...",

    settings_title: "Настройки Hex-просмотра",
    settings_bytes_per_row: "Байт в строке:",
    settings_display: "Отображение:",
    settings_show_ascii: "Колонка ASCII",
    settings_show_inspector: "Инспектор данных",
    settings_show_offsets: "Колонка адресов",
    settings_show_column_headers: "Заголовки колонок",
    settings_show_column_dividers: "Разделители колонок",
    settings_show_group_dividers: "Пунктир между группами",
    settings_show_splitter: "Разделитель размера",
    settings_format: "Формат:",
    settings_uppercase: "Hex заглавными",
    settings_category_colors: "Цвета по категориям",
    settings_dim_zeros: "Тусклые нули",
    action_close: "Закрыть",

    tooltip_address_prefix: "Адрес: ",
    tooltip_label_hex: "Hex",
    tooltip_label_dec: "Dec",
    tooltip_label_oct: "Oct",
    tooltip_label_bin: "Bin",
    tooltip_label_category: "Категория",
    tooltip_label_char: "Символ",
    tooltip_double_click_copy: "Двойной клик копирует",

    inspector_endian_le: "little-endian",
    inspector_endian_be: "big-endian",
    inspector_bytes: "байт",
    editing_hex: "[РЕДАКТИРОВАНИЕ HEX]",
    editing_ascii: "[РЕДАКТИРОВАНИЕ ASCII]",
};

/// Resolve the static catalogue for `locale`.
pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}

/// `"Result {idx}/{total}"` — formatted for the search-popup status row.
pub fn result_n_of_m(locale: Locale, idx: usize, total: usize) -> String {
    match locale {
        Locale::En => format!("Result {idx}/{total}"),
        Locale::Ru => format!("Результат {idx}/{total}"),
    }
}
