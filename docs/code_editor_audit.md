# `code_editor` — deep audit report

Состояние на 2026-04-17 после серии фиксов. Фиксирует findings и то,
что сделано vs отложено. Используется как baseline для будущих
refactor passes.

## Обзор

Объём: 5 697 строк в 6 файлах.

| File | Lines | Role |
|------|-------|------|
| `mod.rs` | 3 600 | Editor state, render, input, find/replace, fold, ctx menu |
| `buffer.rs` | 1 189 | `TextBuffer`, `CursorPos`, `Selection`, text ops |
| `config.rs` | 742 | `EditorConfig` (40+ fields), theme, font registry |
| `undo.rs` | 169 | `UndoStack` с grouping |
| `lang/` | ~150 | Per-language syntax definitions |
| `token.rs`/`tokenizer.rs` | 55 | Token types + tokenizer trait |

## Fixes в этом заходе

### P0 (production-impact, исправлено)

**`snapshot_undo` allocate per keystroke** — `mod.rs:2856`,
`undo.rs:push`. Полный `buffer.text()` клонился на КАЖДОЕ нажатие,
даже когда grouping немедленно дропал результат. На 1 MB буфере =
1 MB alloc per char. Фикс: `UndoStack::should_push(version, force)`
— вызывающий проверяет первым, аллокация только когда реально нужно.
Эффект: ~100 keystrokes в большой hex-дамп = 1 alloc вместо 100.

**`compute_wrap_points` stall защита** — `mod.rs:3256`. Теоретический
риск stall при экстремально узких layout'ах (tab шире viewport).
Добавлен hard-cap iterations + guard `wrap_col <= row_start`
с fallback-advance. Если что-то сломается — break вместо OOM.

**Line-comment toggle ломал `//` внутри строк** — `buffer.rs:685`.
`line.find("//")` матчил любое появление, включая `let s = "a//b";`.
Фикс: матчим только когда `//` это первое non-whitespace содержимое
строки (`line[indent..].starts_with("//")`).

### P1 (reliability, исправлено)

**`set_text` не сбрасывал scroll** — `mod.rs:573`. После загрузки
нового документа viewport мог указывать за end-of-document до первого
движения курсора (которое теперь не форсирует pull, см. прошлый коммит).
Фикс: reset `scroll_x/y` + `target_scroll_y` + `last_set_scroll_y`.

**`whole_word` пропускал non-ASCII boundaries** — `mod.rs:353`.
`is_ascii_alphanumeric` считал é/ж/你 за non-word, так что "ana"
внутри "mañana" или "рад" внутри "радуга" проходили фильтр
whole-word. Фикс: `buffer::is_word_char` (Unicode `is_alphanumeric`
+ underscore) на chars-итерации вокруг match.

## Findings отложены (не critical для текущего use case)

### P1 (reliability)
- **UTF-8 риски в `draw_hex_color_swatches`** проверены — OK (ASCII
  enforced через `is_ascii_hexdigit`). `tok_start/tok_end` safe.
- **`find_next_occurrence` может дублировать primary cursor при
  Ctrl+D**, если match единственный — silent no-op вместо UI nudge.
- **`is_escaped` не считает consecutive backslashes**: `\\"` (literal
  `\` + `"`) неправильно трактуется как escape. Редкий case.
- **Clipboard NUL byte** → пустой clipboard из-за `CString::new`.
- **Right-click popup + left-click overlap** — курсор может съехать
  под popup'ом при race.

### P2 (performance)
- **`Vec<char>` per line** в 8+ местах (`move_word_left/right`,
  `find_matching_bracket`, `select_word_at_cursor`, bracket matching).
  Для длинных строк (hex mode: 1000+ chars) — O(n) alloc per call.
  Фикс: перейти на `char_indices` forward-scan c byte-offset
  адресацией.
- **`get_text` / `selected_text`** всегда allocate `lines.join("\n")`
  — на 10K строк = 1 MB alloc per call. Добавить `get_text_into(&mut String)`.
- **`FindReplaceState::update_matches` lowercases line по
  line** — на 10K строк case-insensitive find = 10K allocs. Кешировать
  lowercase view keyed по `edit_version`.
- **`detect_fold_regions`** re-scan всех строк на каждый `edit_version`
  — считай на каждое нажатие клавиши для больших файлов. Debounce
  или lazy compute.
- **`col_to_x` O(col)** вызывается per-token per-frame. Prefix-sum
  per-line (invalidated с tokens) решит.
- **Bracket matching** `find_matching_bracket` allocates `Vec<char>`
  per line. Deeply nested code → quadratic.

### P3 (architecture + UX)
- **`mod.rs` 3 600 строк** — split candidate: `render.rs`, `input.rs`,
  `find_replace.rs`, `wrap.rs`, `fold.rs`, `context_menu.rs`.
- **Ctrl+L** (select line) — есть Ctrl+Shift+K/D, Ctrl+L отсутствует.
- **Alt+Shift+Down** (duplicate line) — только Ctrl+Shift+D.
- **"Find in selection"** не реализован.
- **Auto-detect tabs vs spaces** при paste.
- **Line endings LF vs CRLF** не сохраняются (set_text нормализует).
- **IME preedit overlay** отсутствует — визуально нет индикации dead
  key composition.
- **`config_mut` + 40 полей** — `theme` меняется без обновления
  `colors` (только `set_theme` делает атомарно). Сделать `theme`
  приватным и требовать `set_theme`.
- **`SyntaxDefinition`** не композируется с built-ins — добавить
  builder.

### Положительные findings
- **`buffer.rs` well-encapsulated** — `mod.rs` не лезет напрямую
  в private поля (всё через публичные методы).
- **Tokenizer tests** есть по языку. Good.
- **UTF-8 handling** в общем аккуратный — основные hot-paths используют
  `char_indices` / byte_to_char.
- **Thread-local / static** — только `CODE_EDITOR_FONT_PTR: AtomicUsize`,
  документирован как Send+Sync cast (acceptable для ImGui global).

## Метрики

| Метрика | До фиксов | После |
|---------|-----------|-------|
| Tests passed | 370 (lib) + 16 (int) | 380 (lib) + 16 (int) |
| Clippy warnings | 0 | 0 |
| Build time (clean) | ~90 s | ~90 s |
| `snapshot_undo` alloc on 100 keystrokes | 100× buffer.text() | 1× buffer.text() |

## Рекомендации

**Priority 1 в будущем PR** — P2 perf кластер:
1. `Vec<char>` elimination — самая крупная оптимизация. Все 8 сайтов
   за один проход. Sub-1 day. Ожидаемый win: 30-50% reduction CPU
   в hex-mode на длинных строках.
2. `FindReplaceState::update_matches` lowercase кеш — 10 минут
   работы, большой win для Ctrl+F в больших файлах.
3. `detect_fold_regions` debounce — 15 минут.

**Priority 2** — architectural split mod.rs → 6 модулей. 2-3 часа.
Не уменьшает LOC, но делает диффы / review much easier.

**Priority 3** — UX parity с VSCode: Ctrl+L, Alt+Shift+Down, find-in-selection.
