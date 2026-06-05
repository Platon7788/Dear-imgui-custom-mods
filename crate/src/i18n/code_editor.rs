//! `crate::code_editor` localisation strings.
//!
//! Programming-language names (Rust / RON / JSON / TOML / YAML / …) and
//! `Theme::display_name()` are intentionally **not** localised — they are
//! proper nouns / brand identifiers, conventional in their original form
//! across all locales.
//!
//! Keyboard shortcuts (`Ctrl+X`, `F3`, `Esc`) likewise stay in their
//! cross-locale technical form.

#![allow(missing_docs)]

use super::Locale;

/// Every user-visible label rendered by [`crate::code_editor::CodeEditor`].
#[derive(Debug)]
pub struct Strings {
    // Clipboard section (right-click context menu)
    pub menu_cut: &'static str,
    pub menu_copy: &'static str,
    pub menu_paste: &'static str,
    pub menu_select_all: &'static str,

    // Undo/redo
    pub menu_undo: &'static str,
    pub menu_redo: &'static str,

    // Code actions
    pub menu_toggle_comment: &'static str,
    pub menu_duplicate_line: &'static str,
    pub menu_delete_line: &'static str,

    // Transform submenu
    pub submenu_transform: &'static str,
    pub menu_uppercase: &'static str,
    pub menu_lowercase: &'static str,
    pub menu_title_case: &'static str,
    pub menu_trim_whitespace: &'static str,

    // Find / view / language / theme submenus
    pub menu_find: &'static str,
    pub submenu_view: &'static str,
    pub view_word_wrap: &'static str,
    pub view_line_numbers: &'static str,
    pub view_highlight_current_line: &'static str,
    pub view_show_whitespace: &'static str,
    pub view_color_swatches: &'static str,
    pub view_smooth_scrolling: &'static str,
    pub view_english_on_focus: &'static str,
    pub submenu_language: &'static str,
    pub language_plain_text: &'static str,
    pub custom_language_prefix: &'static str, // "Custom: "
    pub submenu_theme: &'static str,

    // Font scale + cursor info
    pub font_scale_label: &'static str,
    pub tip_decrease_font: &'static str,
    pub tip_increase_font: &'static str,

    // Find / replace bar
    pub find_hint: &'static str, // "Find…" placeholder
    pub no_matches: &'static str,
    pub tip_prev_match: &'static str,
    pub tip_next_match: &'static str,
    pub tip_case_sensitive: &'static str,
    pub tip_whole_word: &'static str,
    pub tip_toggle_replace: &'static str,
    pub tip_close: &'static str,
    pub replace_hint: &'static str, // "Replace with…" placeholder
    pub btn_replace: &'static str,
    pub btn_replace_all: &'static str,
}

pub const EN: Strings = Strings {
    menu_cut: "Cut",
    menu_copy: "Copy",
    menu_paste: "Paste",
    menu_select_all: "Select All",

    menu_undo: "Undo",
    menu_redo: "Redo",

    menu_toggle_comment: "Toggle Comment",
    menu_duplicate_line: "Duplicate Line",
    menu_delete_line: "Delete Line",

    submenu_transform: "Transform",
    menu_uppercase: "UPPERCASE",
    menu_lowercase: "lowercase",
    menu_title_case: "Title Case",
    menu_trim_whitespace: "Trim Whitespace",

    menu_find: "Find\u{2026}",
    submenu_view: "View",
    view_word_wrap: "Word Wrap",
    view_line_numbers: "Line Numbers",
    view_highlight_current_line: "Highlight Current Line",
    view_show_whitespace: "Show Whitespace",
    view_color_swatches: "Color Swatches",
    view_smooth_scrolling: "Smooth Scrolling",
    view_english_on_focus: "English on Focus",
    submenu_language: "Language",
    language_plain_text: "Plain Text",
    custom_language_prefix: "Custom: ",
    submenu_theme: "Theme",

    font_scale_label: "Font scale:",
    tip_decrease_font: "Decrease font size",
    tip_increase_font: "Increase font size",

    find_hint: "Find\u{2026}",
    no_matches: "No matches",
    tip_prev_match: "Previous match  Shift+F3",
    tip_next_match: "Next match  F3",
    tip_case_sensitive: "Case sensitive",
    tip_whole_word: "Whole word",
    tip_toggle_replace: "Toggle replace  Ctrl+H",
    tip_close: "Close  Esc",
    replace_hint: "Replace with\u{2026}",
    btn_replace: "Replace",
    btn_replace_all: "All",
};

pub const RU: Strings = Strings {
    menu_cut: "Вырезать",
    menu_copy: "Копировать",
    menu_paste: "Вставить",
    menu_select_all: "Выделить всё",

    menu_undo: "Отменить",
    menu_redo: "Повторить",

    menu_toggle_comment: "Закомментировать",
    menu_duplicate_line: "Дублировать строку",
    menu_delete_line: "Удалить строку",

    submenu_transform: "Преобразовать",
    menu_uppercase: "ВЕРХНИЙ РЕГИСТР",
    menu_lowercase: "нижний регистр",
    menu_title_case: "С Заглавных",
    menu_trim_whitespace: "Убрать пробелы",

    menu_find: "Найти\u{2026}",
    submenu_view: "Вид",
    view_word_wrap: "Перенос строк",
    view_line_numbers: "Номера строк",
    view_highlight_current_line: "Подсветка текущей строки",
    view_show_whitespace: "Показывать пробелы",
    view_color_swatches: "Цветные плашки",
    view_smooth_scrolling: "Плавная прокрутка",
    view_english_on_focus: "Английская раскладка при фокусе",
    submenu_language: "Язык",
    language_plain_text: "Обычный текст",
    custom_language_prefix: "Свой: ",
    submenu_theme: "Тема",

    font_scale_label: "Масштаб шрифта:",
    tip_decrease_font: "Уменьшить шрифт",
    tip_increase_font: "Увеличить шрифт",

    find_hint: "Найти\u{2026}",
    no_matches: "Совпадений нет",
    tip_prev_match: "Предыдущее  Shift+F3",
    tip_next_match: "Следующее  F3",
    tip_case_sensitive: "С учётом регистра",
    tip_whole_word: "Целое слово",
    tip_toggle_replace: "Заменить  Ctrl+H",
    tip_close: "Закрыть  Esc",
    replace_hint: "Заменить на\u{2026}",
    btn_replace: "Заменить",
    btn_replace_all: "Все",
};

pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}

/// `"Ln {line}, Col {col}  /  {total} lines"` for the cursor-info row.
pub fn cursor_info(locale: Locale, line: usize, col: usize, total: usize) -> String {
    match locale {
        Locale::En => format!("Ln {line}, Col {col}  /  {total} lines"),
        Locale::Ru => format!("Стр {line}, Кол {col}  /  всего {total}"),
    }
}
