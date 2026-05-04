//! Locale + per-module string catalogues.
//!
//! Single source for English / Russian text in widgets that render
//! user-facing labels, tooltips, popups, and context menus. Each
//! widget owns a `Locale` field (default [`Locale::En`]) which a
//! host can override at construction:
//!
//! ```rust,no_run
//! # use dear_imgui_custom_mod::i18n::Locale;
//! # use dear_imgui_custom_mod::hex_viewer::HexViewer;
//! let viewer = HexViewer::new("dump").with_locale(Locale::Ru);
//! ```
//!
//! Catalogues are static `&'static Strings` values resolved by a single
//! `match` — zero allocation, zero indirection, zero runtime overhead.
//!
//! ## Adding a string
//!
//! 1. Add a `pub const_field: &'static str` to the relevant
//!    `Strings` struct.
//! 2. Add the same key to **both** `EN` and `RU` constants.
//! 3. Use `widget.strings().const_field` at the call site instead of
//!    a literal.
//! 4. Add an entry to the table in `tests::all_keys_present_in_both_locales`.
//!
//! Format-template strings (`"Result {n}/{m}"`) live as helper
//! functions — see [`hex_viewer::result_n_of_m`].

use serde::{Deserialize, Serialize};

/// User-visible language. Default English.
///
/// `serde` for ron round-trip in saved widget configs (so a saved
/// `HexViewerConfig` ron file remembers the locale).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Locale {
    /// English. The default — no extra glyph ranges required, every
    /// label fits in Basic Latin.
    #[default]
    En,
    /// Russian. Requires the host to bake `GlyphRanges::Cyrillic`
    /// (or a superset) into the active font atlas — otherwise
    /// non-ASCII characters render as `?` placeholders.
    Ru,
}

impl Locale {
    /// Short tag (`"en"` / `"ru"`) — useful for logs / debug overlays.
    pub fn tag(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ru => "ru",
        }
    }
}

// ── Hex Viewer catalogue ────────────────────────────────────────────────────

pub mod hex_viewer {
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
}

// ── Disasm View catalogue ───────────────────────────────────────────────────

pub mod disasm_view {
    //! `crate::disasm_view` localisation strings.

    #![allow(missing_docs)] // every field is self-describing by name

    use super::Locale;

    /// Every user-visible label rendered by [`crate::disasm_view::DisasmView`].
    #[derive(Debug)]
    pub struct Strings {
        // Goto popup
        pub goto_title: &'static str,
        pub action_go: &'static str,
        pub action_cancel: &'static str,

        // Search popup
        pub search_hint: &'static str,
        pub action_find: &'static str,
        pub no_matches: &'static str,
        pub result_step_hint: &'static str, // suffix " (F3 / Shift+F3 to step)"

        // Context menu
        pub menu_goto_address: &'static str,
        pub menu_search_bytes: &'static str,
        pub menu_copy_address: &'static str,
        pub menu_copy_instruction: &'static str,
        pub menu_follow: &'static str,
        pub menu_jump_func_start: &'static str,
        pub menu_jump_func_end: &'static str,
        pub menu_select_function: &'static str,
        pub menu_toggle_breakpoint: &'static str,
        pub menu_toggle_watchpoint: &'static str,
        pub menu_add_bookmark: &'static str,
        pub menu_remove_bookmark: &'static str,
        pub menu_settings: &'static str,

        // Settings popup
        pub settings_title: &'static str,
        pub settings_display: &'static str,
        pub settings_show_bytes: &'static str,
        pub settings_show_comments: &'static str,
        pub settings_show_branch_arrows: &'static str,
        pub settings_show_breakpoints: &'static str,
        pub settings_show_bookmarks: &'static str,
        pub settings_show_block_tints: &'static str,
        pub settings_show_header: &'static str,
        pub settings_show_column_dividers: &'static str,
        pub settings_format: &'static str,
        pub settings_uppercase: &'static str,
        pub settings_address_width_64: &'static str,
        pub settings_byte_category_colors: &'static str,
        pub settings_behavior: &'static str,
        pub settings_editable: &'static str,
        pub settings_follow_execution: &'static str,
        pub settings_show_explanation: &'static str,
        pub settings_show_idiom: &'static str,
        pub settings_show_gotcha: &'static str,
        pub settings_show_operand_hint: &'static str,
        pub settings_show_compiler_pattern: &'static str,
        pub settings_show_antidisasm: &'static str,
        pub settings_show_boundary: &'static str,
        pub settings_show_branch_direction: &'static str,
        pub action_close: &'static str,

        // Tooltip on hovered instruction
        pub tooltip_address_prefix: &'static str,
        pub tooltip_address32_prefix: &'static str, // "     32: " (alignment-padded)
        pub tooltip_size_label: &'static str,       // "Size: "
        pub tooltip_bytes_label: &'static str,      // "Bytes: "
        pub tooltip_instr_label: &'static str,      // "Instr: "
        pub tooltip_flow_label: &'static str,       // "Flow: "
        pub tooltip_target_label: &'static str,     // "Target: "
        pub tooltip_block_label: &'static str,      // "Block: "
        pub tooltip_breakpoint_label: &'static str, // "Breakpoint: "
        pub tooltip_breakpoint_yes: &'static str,   // "Breakpoint: YES"
        pub tooltip_current_ip: &'static str,       // ">> CURRENT INSTRUCTION POINTER <<"
        pub tooltip_comment_label: &'static str,
        pub tooltip_double_click_copy: &'static str,
        pub tooltip_unit_bytes: &'static str, // "bytes" suffix in "Size: 4 bytes"
        pub tooltip_explanation_label: &'static str, // "What it does: " / "Что делает: "
        pub tooltip_idiom_label: &'static str,        // "Pattern: " / "Шаблон: "
        pub tooltip_gotcha_label: &'static str,       // "Watch out: " / "Внимание: "
        pub tooltip_operand_label: &'static str,      // "Operand: " / "Операнд: "
        pub tooltip_compiler_label: &'static str,     // "Compiler: " / "Компилятор: "
        pub tooltip_antidisasm_label: &'static str,   // "Anti-RE: " / "Анти-RE: "
        pub tooltip_boundary_label: &'static str,     // "Boundary: " / "Граница: "
        pub tooltip_branch_label: &'static str,       // "Branch: " / "Переход: "

        // Flow kinds
        pub flow_normal: &'static str,
        pub flow_jump: &'static str,
        pub flow_call: &'static str,
        pub flow_return: &'static str,
        pub flow_nop: &'static str,
        pub flow_stack: &'static str,
        pub flow_system: &'static str,
        pub flow_invalid: &'static str,

        // Search hint variants
        pub search_pattern_too_short_template: &'static str, // see helper below
    }

    pub const EN: Strings = Strings {
        goto_title: "Goto address (hex):",
        action_go: "Go",
        action_cancel: "Cancel",

        search_hint: "Search bytes (min 5 bytes; ?? wildcard):",
        action_find: "Find",
        no_matches: "No matches.",
        result_step_hint: " (F3 / Shift+F3 to step)",

        menu_goto_address: "\u{00BB}  Goto Address...\tG",
        menu_search_bytes: "\u{00BB}  Search bytes...\tCtrl+F",
        menu_copy_address: "\u{00BB}  Copy Address",
        menu_copy_instruction: "\u{00BB}  Copy Instruction\tCtrl+C",
        menu_follow: "\u{2192}  Follow\tEnter / Space",
        menu_jump_func_start: "\u{2191}  Jump to function start\tCtrl+Up",
        menu_jump_func_end: "\u{2193}  Jump to function end\tCtrl+Down",
        menu_select_function: "\u{00BB}  Select function\tCtrl+L",
        menu_toggle_breakpoint: "\u{25CF}  Toggle Breakpoint\tF9",
        menu_toggle_watchpoint: "\u{25CF}  Toggle Watchpoint",
        menu_add_bookmark: "Add to bookmarks\tCtrl+B",
        menu_remove_bookmark: "Remove from bookmarks\tCtrl+B",
        menu_settings: "Settings...",

        settings_title: "Disassembly Settings",
        settings_display: "Display:",
        settings_show_bytes: "Show bytes",
        settings_show_comments: "Show comments",
        settings_show_branch_arrows: "Show branch arrows",
        settings_show_breakpoints: "Show breakpoints",
        settings_show_bookmarks: "Show bookmarks",
        settings_show_block_tints: "Show block tints",
        settings_show_header: "Show header",
        settings_show_column_dividers: "Show column dividers",
        settings_format: "Format:",
        settings_uppercase: "Uppercase hex",
        settings_address_width_64: "64-bit address width",
        settings_byte_category_colors: "Byte category colors",
        settings_behavior: "Behavior:",
        settings_editable: "Editable (double-click to patch)",
        settings_follow_execution: "Follow execution",
        settings_show_explanation: "Mnemonic explainer (educational)",
        settings_show_idiom: "Idiom detector (prologue / cmp+Jcc / NULL-check / ...)",
        settings_show_gotcha: "Anti-RE / anti-debug warnings",
        settings_show_operand_hint: "Operand decoder (memory / register roles)",
        settings_show_compiler_pattern: "Compiler-pattern recognizer (Win64 leaf / __chkstk / vtable / SEH / TIB / PEB)",
        settings_show_antidisasm: "Anti-disasm / anti-debug trick recognizer",
        settings_show_boundary: "Boundary recognizer (function prologue / epilogue / block terminator)",
        settings_show_branch_direction: "Branch-direction hint (forward = if-then, backward = loop)",
        action_close: "Close",

        tooltip_address_prefix: "Address: ",
        tooltip_address32_prefix: "     32: ",
        tooltip_size_label: "Size: ",
        tooltip_bytes_label: "Bytes: ",
        tooltip_instr_label: "Instr: ",
        tooltip_flow_label: "Flow: ",
        tooltip_target_label: "Target: ",
        tooltip_block_label: "Block: ",
        tooltip_breakpoint_label: "Breakpoint: ",
        tooltip_breakpoint_yes: "Breakpoint: YES",
        tooltip_current_ip: ">> CURRENT INSTRUCTION POINTER <<",
        tooltip_comment_label: "Comment: ",
        tooltip_double_click_copy: "Double-click to copy",
        tooltip_unit_bytes: "bytes",
        tooltip_explanation_label: "What it does: ",
        tooltip_idiom_label: "Pattern: ",
        tooltip_gotcha_label: "Watch out: ",
        tooltip_operand_label: "Operand: ",
        tooltip_compiler_label: "Compiler: ",
        tooltip_antidisasm_label: "Anti-RE: ",
        tooltip_boundary_label: "Boundary: ",
        tooltip_branch_label: "Branch: ",

        flow_normal: "Normal (sequential)",
        flow_jump: "Jump (conditional/unconditional)",
        flow_call: "Call (function call)",
        flow_return: "Return (function epilogue)",
        flow_nop: "NOP / padding",
        flow_stack: "Stack operation (push/pop/sub rsp)",
        flow_system: "System (syscall/int/sysenter)",
        flow_invalid: "INVALID (undecodable)",

        search_pattern_too_short_template: "Pattern too short: {n} / {min} bytes",
    };

    pub const RU: Strings = Strings {
        goto_title: "Перейти к адресу (hex):",
        action_go: "Перейти",
        action_cancel: "Отмена",

        search_hint: "Поиск байтов (минимум 5; ?? — джокер):",
        action_find: "Найти",
        no_matches: "Совпадений нет.",
        result_step_hint: " (F3 / Shift+F3 — шаг)",

        menu_goto_address: "\u{00BB}  Перейти к адресу...\tG",
        menu_search_bytes: "\u{00BB}  Поиск байтов...\tCtrl+F",
        menu_copy_address: "\u{00BB}  Копировать адрес",
        menu_copy_instruction: "\u{00BB}  Копировать инструкцию\tCtrl+C",
        menu_follow: "\u{2192}  Перейти по ссылке\tEnter / Space",
        menu_jump_func_start: "\u{2191}  В начало функции\tCtrl+Up",
        menu_jump_func_end: "\u{2193}  В конец функции\tCtrl+Down",
        menu_select_function: "\u{00BB}  Выделить функцию\tCtrl+L",
        menu_toggle_breakpoint: "\u{25CF}  Точка останова\tF9",
        menu_toggle_watchpoint: "\u{25CF}  Точка наблюдения",
        menu_add_bookmark: "Добавить закладку\tCtrl+B",
        menu_remove_bookmark: "Удалить закладку\tCtrl+B",
        menu_settings: "Настройки...",

        settings_title: "Настройки дизассемблера",
        settings_display: "Отображение:",
        settings_show_bytes: "Показывать байты",
        settings_show_comments: "Показывать комментарии",
        settings_show_branch_arrows: "Стрелки переходов",
        settings_show_breakpoints: "Точки останова",
        settings_show_bookmarks: "Закладки",
        settings_show_block_tints: "Подсветка блоков",
        settings_show_header: "Заголовок",
        settings_show_column_dividers: "Разделители колонок",
        settings_format: "Формат:",
        settings_uppercase: "Hex заглавными",
        settings_address_width_64: "64-битный адрес",
        settings_byte_category_colors: "Цвета байтов по категориям",
        settings_behavior: "Поведение:",
        settings_editable: "Редактируемо (двойной клик)",
        settings_follow_execution: "Следовать за выполнением",
        settings_show_explanation: "Подсказки по мнемоникам (для обучения)",
        settings_show_idiom: "Поиск шаблонов (пролог / cmp+Jcc / NULL-check / …)",
        settings_show_gotcha: "Предупреждения об анти-RE / анти-debug",
        settings_show_operand_hint: "Расшифровка операндов (память / роли регистров)",
        settings_show_compiler_pattern: "Распознаватель шаблонов компилятора (Win64 leaf / __chkstk / vtable / SEH / TIB / PEB)",
        settings_show_antidisasm: "Распознаватель анти-disasm / анти-debug приёмов",
        settings_show_boundary: "Распознаватель границ (пролог / эпилог / конец блока)",
        settings_show_branch_direction: "Подсказка направления перехода (вперёд = if-then, назад = цикл)",
        action_close: "Закрыть",

        tooltip_address_prefix: "Адрес: ",
        tooltip_address32_prefix: "     32: ",
        tooltip_size_label: "Размер: ",
        tooltip_bytes_label: "Байты: ",
        tooltip_instr_label: "Инстр: ",
        tooltip_flow_label: "Поток: ",
        tooltip_target_label: "Цель: ",
        tooltip_block_label: "Блок: ",
        tooltip_breakpoint_label: "Точка останова: ",
        tooltip_breakpoint_yes: "Точка останова: ДА",
        tooltip_current_ip: ">> ТЕКУЩИЙ УКАЗАТЕЛЬ ИНСТРУКЦИЙ <<",
        tooltip_comment_label: "Комментарий: ",
        tooltip_double_click_copy: "Двойной клик копирует",
        tooltip_unit_bytes: "байт",
        tooltip_explanation_label: "Что делает: ",
        tooltip_idiom_label: "Шаблон: ",
        tooltip_gotcha_label: "Внимание: ",
        tooltip_operand_label: "Операнд: ",
        tooltip_compiler_label: "Компилятор: ",
        tooltip_antidisasm_label: "Анти-RE: ",
        tooltip_boundary_label: "Граница: ",
        tooltip_branch_label: "Переход: ",

        flow_normal: "Обычный (последовательный)",
        flow_jump: "Переход (условный/безусловный)",
        flow_call: "Вызов функции",
        flow_return: "Возврат (эпилог функции)",
        flow_nop: "NOP / выравнивание",
        flow_stack: "Стековая операция (push/pop/sub rsp)",
        flow_system: "Системная (syscall/int/sysenter)",
        flow_invalid: "НЕДЕЙСТВИТЕЛЬНАЯ (не декодируется)",

        search_pattern_too_short_template: "Шаблон слишком короткий: {n} / {min} байт",
    };

    pub fn strings(locale: Locale) -> &'static Strings {
        match locale {
            Locale::En => &EN,
            Locale::Ru => &RU,
        }
    }

    pub fn result_n_of_m(locale: Locale, idx: usize, total: usize) -> String {
        match locale {
            Locale::En => format!("Result {idx}/{total}"),
            Locale::Ru => format!("Результат {idx}/{total}"),
        }
    }

    pub fn pattern_too_short(locale: Locale, parsed: usize, min: usize) -> String {
        match locale {
            Locale::En => format!("Pattern too short: {parsed} / {min} bytes"),
            Locale::Ru => format!("Шаблон слишком короткий: {parsed} / {min} байт"),
        }
    }

    /// `"Copy {n} Instructions\tCtrl+C"` for multi-selection copy.
    pub fn copy_n_instructions(locale: Locale, n: usize) -> String {
        match locale {
            Locale::En => format!("\u{00BB}  Copy {n} Instructions\tCtrl+C"),
            Locale::Ru => format!("\u{00BB}  Копировать {n} инструкций\tCtrl+C"),
        }
    }
}

// ── Timeline catalogue ──────────────────────────────────────────────────────

pub mod timeline {
    //! `crate::timeline` localisation strings.

    #![allow(missing_docs)]

    use super::Locale;

    /// Tooltip labels rendered by [`crate::timeline::Timeline`] when
    /// `cfg.show_tooltip` is on.
    #[derive(Debug)]
    pub struct Strings {
        pub category_label: &'static str, // "Category: "
        pub source_label: &'static str,   // "Source: "
        pub start_end_template: &'static str, // "Start: {:.4}ms  End: {:.4}ms"
        pub depth_label: &'static str,    // "Depth: "
    }

    pub const EN: Strings = Strings {
        category_label: "Category: ",
        source_label: "Source: ",
        start_end_template: "Start: {start:.4}ms  End: {end:.4}ms",
        depth_label: "Depth: ",
    };

    pub const RU: Strings = Strings {
        category_label: "Категория: ",
        source_label: "Источник: ",
        start_end_template: "Начало: {start:.4}мс  Конец: {end:.4}мс",
        depth_label: "Глубина: ",
    };

    pub fn strings(locale: Locale) -> &'static Strings {
        match locale {
            Locale::En => &EN,
            Locale::Ru => &RU,
        }
    }

    /// `"Start: 1.2345ms  End: 6.7890ms"` for the tooltip's time line.
    /// Accepts `f64` because `Span::start` / `Span::end` are stored as
    /// `f64` for sub-millisecond precision over long captures.
    pub fn start_end(locale: Locale, start_ms: f64, end_ms: f64) -> String {
        match locale {
            Locale::En => format!("Start: {start_ms:.4}ms  End: {end_ms:.4}ms"),
            Locale::Ru => format!("Начало: {start_ms:.4}мс  Конец: {end_ms:.4}мс"),
        }
    }
}

// ── Diff Viewer catalogue ───────────────────────────────────────────────────

pub mod diff_viewer {
    //! `crate::diff_viewer` localisation strings.

    #![allow(missing_docs)]

    use super::Locale;

    /// Toolbar button labels.
    #[derive(Debug)]
    pub struct Strings {
        pub prev_button: &'static str,    // "Prev (Shift+F7)"
        pub next_button: &'static str,    // "Next (F7)"
    }

    pub const EN: Strings = Strings {
        prev_button: "Prev (Shift+F7)",
        next_button: "Next (F7)",
    };

    pub const RU: Strings = Strings {
        prev_button: "Назад (Shift+F7)",
        next_button: "Вперёд (F7)",
    };

    pub fn strings(locale: Locale) -> &'static Strings {
        match locale {
            Locale::En => &EN,
            Locale::Ru => &RU,
        }
    }
}

// ── Nav Panel catalogue ─────────────────────────────────────────────────────

pub mod nav_panel {
    //! `crate::nav_panel` localisation strings.

    #![allow(missing_docs)]

    use super::Locale;

    /// Panel-toggle button tooltips. The nav buttons themselves carry
    /// host-supplied labels via `NavItem::label` — those stay
    /// host-driven (the host owns the user's mental model of "what
    /// each button means").
    #[derive(Debug)]
    pub struct Strings {
        pub show_panel: &'static str,    // "Show panel"
        pub toggle_panel: &'static str,  // "Toggle panel"
    }

    pub const EN: Strings = Strings {
        show_panel: "Show panel",
        toggle_panel: "Toggle panel",
    };

    pub const RU: Strings = Strings {
        show_panel: "Показать панель",
        toggle_panel: "Скрыть/показать панель",
    };

    pub fn strings(locale: Locale) -> &'static Strings {
        match locale {
            Locale::En => &EN,
            Locale::Ru => &RU,
        }
    }
}

// ── Force Graph catalogue ───────────────────────────────────────────────────

pub mod force_graph {
    //! `crate::force_graph` localisation strings.

    #![allow(missing_docs)]

    use super::Locale;

    /// All user-visible labels rendered by the force-graph sidebar
    /// and the right-click context menu.
    #[derive(Debug)]
    pub struct Strings {
        // Section headers
        pub section_filters: &'static str,
        pub section_color_groups: &'static str,
        pub section_display: &'static str,
        pub section_export: &'static str,
        pub section_physics: &'static str,

        // Filters
        pub show_orphan_nodes: &'static str,
        pub hide_unresolved_links: &'static str,
        pub hide_tag_nodes: &'static str,
        pub search_label: &'static str,
        pub depth_label: &'static str,
        pub time_travel_label: &'static str,
        pub btn_all: &'static str,

        // Color Groups
        pub no_groups_defined: &'static str,
        pub add_group: &'static str,
        pub new_group_default_name: &'static str,
        pub query_label: &'static str,
        pub query_tag: &'static str,
        pub query_kind: &'static str,
        pub query_regex: &'static str,
        pub query_all: &'static str,

        // Display
        pub node_size: &'static str,
        pub edge_width: &'static str,
        pub text_fade: &'static str,
        pub hover_fade: &'static str,
        pub edge_curve: &'static str,
        pub toggle_arrows: &'static str,
        pub toggle_edge_labels: &'static str,
        pub toggle_background_grid: &'static str,
        pub toggle_glow_on_hover: &'static str,

        // Export
        pub copy_svg: &'static str,
        pub copy_dot: &'static str,
        pub copy_mermaid: &'static str,

        // Physics
        pub link_dist: &'static str,
        pub repulsion: &'static str,
        pub attraction: &'static str,
        pub center_pull: &'static str,
        pub decay: &'static str,
        pub gravity: &'static str,
        pub btn_pause_resume: &'static str,
        pub btn_reset_layout: &'static str,

        // Context menu
        pub menu_pin: &'static str,
        pub menu_unpin: &'static str,
        pub menu_select_neighbours: &'static str,
        pub menu_focus_here: &'static str,
        pub menu_clear_focus: &'static str,
        pub menu_activate: &'static str,
    }

    pub const EN: Strings = Strings {
        section_filters: "Filters",
        section_color_groups: "Color Groups",
        section_display: "Display",
        section_export: "Export",
        section_physics: "Physics",

        show_orphan_nodes: "Show orphan nodes",
        hide_unresolved_links: "Hide unresolved links",
        hide_tag_nodes: "Hide tag nodes",
        search_label: "Search:",
        depth_label: "Depth (0=all):",
        time_travel_label: "Time travel:",
        btn_all: "All",

        no_groups_defined: "No groups defined",
        add_group: "+ Add Group",
        new_group_default_name: "New group",
        query_label: "Query: label",
        query_tag: "Query: tag",
        query_kind: "Query: kind",
        query_regex: "Query: regex",
        query_all: "Query: all",

        node_size: "Node size:",
        edge_width: "Edge width:",
        text_fade: "Text fade:",
        hover_fade: "Hover fade:",
        edge_curve: "Edge curve:",
        toggle_arrows: "Arrows",
        toggle_edge_labels: "Edge labels",
        toggle_background_grid: "Background grid",
        toggle_glow_on_hover: "Glow on hover",

        copy_svg: "Copy SVG",
        copy_dot: "Copy DOT",
        copy_mermaid: "Copy Mermaid",

        link_dist: "Link dist:",
        repulsion: "Repulsion:",
        attraction: "Attraction:",
        center_pull: "Center pull:",
        decay: "Decay:",
        gravity: "Gravity:",
        btn_pause_resume: "Pause/Resume",
        btn_reset_layout: "Reset Layout",

        menu_pin: "Pin",
        menu_unpin: "Unpin",
        menu_select_neighbours: "Select neighbours",
        menu_focus_here: "Focus here",
        menu_clear_focus: "Clear focus",
        menu_activate: "Activate",
    };

    pub const RU: Strings = Strings {
        section_filters: "Фильтры",
        section_color_groups: "Цветовые группы",
        section_display: "Отображение",
        section_export: "Экспорт",
        section_physics: "Физика",

        show_orphan_nodes: "Сиротские узлы",
        hide_unresolved_links: "Скрыть нерасреш. ссылки",
        hide_tag_nodes: "Скрыть теги",
        search_label: "Поиск:",
        depth_label: "Глубина (0=все):",
        time_travel_label: "По времени:",
        btn_all: "Все",

        no_groups_defined: "Группы не заданы",
        add_group: "+ Добавить группу",
        new_group_default_name: "Новая группа",
        query_label: "Запрос: метка",
        query_tag: "Запрос: тег",
        query_kind: "Запрос: тип",
        query_regex: "Запрос: regex",
        query_all: "Запрос: все",

        node_size: "Размер узла:",
        edge_width: "Толщина ребра:",
        text_fade: "Затух. текста:",
        hover_fade: "Затух. при ховере:",
        edge_curve: "Изгиб ребра:",
        toggle_arrows: "Стрелки",
        toggle_edge_labels: "Подписи рёбер",
        toggle_background_grid: "Сетка фона",
        toggle_glow_on_hover: "Свечение при ховере",

        copy_svg: "Копировать SVG",
        copy_dot: "Копировать DOT",
        copy_mermaid: "Копировать Mermaid",

        link_dist: "Дист. связи:",
        repulsion: "Отталкивание:",
        attraction: "Притяжение:",
        center_pull: "К центру:",
        decay: "Затухание:",
        gravity: "Гравитация:",
        btn_pause_resume: "Пауза/Возобн.",
        btn_reset_layout: "Сбросить раскладку",

        menu_pin: "Закрепить",
        menu_unpin: "Открепить",
        menu_select_neighbours: "Выделить соседей",
        menu_focus_here: "Фокус здесь",
        menu_clear_focus: "Сбросить фокус",
        menu_activate: "Активировать",
    };

    pub fn strings(locale: Locale) -> &'static Strings {
        match locale {
            Locale::En => &EN,
            Locale::Ru => &RU,
        }
    }
}

// ── Code Editor catalogue ───────────────────────────────────────────────────

pub mod code_editor {
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
        pub find_hint: &'static str,    // "Find…" placeholder
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
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_default_is_english() {
        assert_eq!(Locale::default(), Locale::En);
    }

    #[test]
    fn locale_tag_round_trip() {
        assert_eq!(Locale::En.tag(), "en");
        assert_eq!(Locale::Ru.tag(), "ru");
    }

    #[test]
    fn hex_viewer_strings_resolve() {
        let en = hex_viewer::strings(Locale::En);
        let ru = hex_viewer::strings(Locale::Ru);
        // English defaults must match the historic literals; a single
        // canary check guards against accidental EN-side renaming.
        assert_eq!(en.action_go, "Go");
        assert_eq!(en.header_address, "Address");
        // Russian must be different (translation actually exists).
        assert_ne!(en.action_go, ru.action_go);
        assert_ne!(en.header_address, ru.header_address);
    }

    #[test]
    fn disasm_view_strings_resolve() {
        let en = disasm_view::strings(Locale::En);
        let ru = disasm_view::strings(Locale::Ru);
        assert_eq!(en.action_close, "Close");
        assert_eq!(en.flow_normal, "Normal (sequential)");
        assert_ne!(en.action_close, ru.action_close);
        assert_ne!(en.flow_normal, ru.flow_normal);
    }

    #[test]
    fn hex_viewer_format_helpers_change_per_locale() {
        let en = hex_viewer::result_n_of_m(Locale::En, 3, 7);
        let ru = hex_viewer::result_n_of_m(Locale::Ru, 3, 7);
        assert_eq!(en, "Result 3/7");
        assert_eq!(ru, "Результат 3/7");
    }

    #[test]
    fn disasm_view_format_helpers_change_per_locale() {
        let en = disasm_view::pattern_too_short(Locale::En, 3, 5);
        let ru = disasm_view::pattern_too_short(Locale::Ru, 3, 5);
        assert!(en.contains("3 / 5") && en.contains("Pattern"));
        assert!(ru.contains("3 / 5") && ru.contains("Шаблон"));

        let en = disasm_view::copy_n_instructions(Locale::En, 4);
        let ru = disasm_view::copy_n_instructions(Locale::Ru, 4);
        assert!(en.contains('4') && en.contains("Instructions"));
        assert!(ru.contains('4') && ru.contains("инструкций"));
    }

    #[test]
    fn timeline_strings_resolve() {
        assert_eq!(timeline::strings(Locale::En).category_label, "Category: ");
        assert_eq!(timeline::strings(Locale::Ru).category_label, "Категория: ");
    }

    #[test]
    fn timeline_start_end_helper_localises() {
        let en = timeline::start_end(Locale::En, 1.2345_f64, 6.7890_f64);
        let ru = timeline::start_end(Locale::Ru, 1.2345_f64, 6.7890_f64);
        assert!(en.contains("Start") && en.contains("End"));
        assert!(ru.contains("Начало") && ru.contains("Конец"));
        assert!(en.contains("1.2345") && ru.contains("1.2345"));
    }

    #[test]
    fn diff_viewer_strings_resolve() {
        assert_eq!(diff_viewer::strings(Locale::En).prev_button, "Prev (Shift+F7)");
        assert_eq!(diff_viewer::strings(Locale::Ru).prev_button, "Назад (Shift+F7)");
    }

    #[test]
    fn nav_panel_strings_resolve() {
        assert_eq!(nav_panel::strings(Locale::En).show_panel, "Show panel");
        assert_eq!(nav_panel::strings(Locale::Ru).show_panel, "Показать панель");
    }

    #[test]
    fn code_editor_strings_resolve() {
        assert_eq!(code_editor::strings(Locale::En).menu_cut, "Cut");
        assert_eq!(code_editor::strings(Locale::Ru).menu_cut, "Вырезать");
        assert_eq!(code_editor::strings(Locale::En).submenu_view, "View");
        assert_eq!(code_editor::strings(Locale::Ru).submenu_view, "Вид");
    }

    #[test]
    fn code_editor_cursor_info_localises() {
        let en = code_editor::cursor_info(Locale::En, 12, 5, 100);
        let ru = code_editor::cursor_info(Locale::Ru, 12, 5, 100);
        assert!(en.contains("Ln 12") && en.contains("Col 5") && en.contains("100 lines"));
        assert!(ru.contains("Стр 12") && ru.contains("Кол 5") && ru.contains("всего 100"));
    }

    #[test]
    fn force_graph_strings_resolve() {
        assert_eq!(force_graph::strings(Locale::En).section_filters, "Filters");
        assert_eq!(force_graph::strings(Locale::Ru).section_filters, "Фильтры");
        assert_eq!(force_graph::strings(Locale::En).btn_pause_resume, "Pause/Resume");
        assert_eq!(force_graph::strings(Locale::Ru).btn_pause_resume, "Пауза/Возобн.");
    }

    #[test]
    fn locale_serde_round_trip() {
        // ron round-trip: makes sure saved configs can carry a locale.
        let s = ron::ser::to_string(&Locale::Ru).unwrap();
        let back: Locale = ron::from_str(&s).unwrap();
        assert_eq!(back, Locale::Ru);
    }
}
