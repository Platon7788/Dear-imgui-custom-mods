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
    pub tooltip_double_click_follow: &'static str, // "Double-click / middle-click to follow"
    pub tooltip_unit_bytes: &'static str,          // "bytes" suffix in "Size: 4 bytes"
    pub tooltip_explanation_label: &'static str,   // "What it does: " / "Что делает: "
    pub tooltip_idiom_label: &'static str,         // "Pattern: " / "Шаблон: "
    pub tooltip_gotcha_label: &'static str,        // "Watch out: " / "Внимание: "
    pub tooltip_operand_label: &'static str,       // "Operand: " / "Операнд: "
    pub tooltip_compiler_label: &'static str,      // "Compiler: " / "Компилятор: "
    pub tooltip_antidisasm_label: &'static str,    // "Anti-RE: " / "Анти-RE: "
    pub tooltip_boundary_label: &'static str,      // "Boundary: " / "Граница: "
    pub tooltip_branch_label: &'static str,        // "Branch: " / "Переход: "

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
    tooltip_double_click_follow: "Double-click / middle-click to follow",
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
    tooltip_double_click_follow: "Двойной клик / средняя кнопка — переход",
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
