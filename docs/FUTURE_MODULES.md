# Future modules — analysis & specs

Куда библиотека имеет смысл расти дальше. Список составлен из опыта
построения dev-тулинга на ImGui + анализа того, что уже есть
(17 компонентов) и где пробелы видны на практике.

Отсортировано по убыванию impact/effort — сверху то, что добавит больше
всего функциональности при разумной стоимости реализации.

---

## 🌟 Priority 1 — знания и связи

### 1. `knowledge_graph` — force-directed графовый viewer

**Аналог:** Obsidian Graph View, but enhanced.
**Существующий `node_graph`** — это редактор узлов (прямоугольники с
pin'ами для data-flow). Для знаний нужен совсем другой виджет —
безлейблов-лучей, физика пружин, кластеризация.

**Фичи (то что есть у Obsidian):**
- Force-directed simulation (Barnes-Hut O(N log N) для больших графов)
- Узлы разного размера — degree-based или вес-based
- Связи разной толщины — вес/strength
- Pan + zoom с inertia, бокс-выделение
- Hover tooltip, click → navigate
- Фильтры: по тегам / типу узла / расстоянию от выделенного
- Группировка цветом: по метаданным, по кластеру, по тегу
- Настройки сил (repulsion / attraction / center / collision radius)
- Auto-layout при изменении структуры

**Чего Obsidian-у не хватает и что надо добавить:**
- **Level-of-detail (LOD).** На графе > 10K узлов Obsidian тормозит —
  у нас будет GPU-quadtree / KDTree culling и адаптивные радиусы.
- **Time-travel.** Слайдер "состояние графа на дату X" — показать как
  граф рос со временем. Требует timestamped-edges, но крайне ценно
  для визуализации истории проекта / commit graph / log analysis.
- **Semantic colouring.** Не просто по тегу — по вычисленным метрикам:
  PageRank centrality, betweenness, community (Louvain modulariy).
  Obsidian рисует это плоско; у нас — с live slider.
- **Sub-graph focus / isolate.** `Ctrl+Click` на узле → показать только
  его окрестность радиуса N с плавным анимированным коллапсом
  остального. Возврат — тоже анимация. Obsidian делает filter,
  но без плавности.
- **Search-as-highlight.** Ввод в поиск → узлы подсвечиваются без
  фильтрации (все остаются видны, но не-совпавшие тускнеют). Пример
  лучшего паттерна — DevTools flame graph.
- **Pin / anchor.** Зафиксировать узел позиционно (drag → drop).
  Физика уважает pin-и. Obsidian не даёт анкорить.
- **Edge bundling.** Для плотных графов (> 1K рёбер в vicinity) —
  гиббс / force-directed edge bundling чтобы разгрузить визуал.
- **Export.** SVG / PNG / GraphViz DOT / Mermaid — в Obsidian этого нет.

**API scetch:**

```rust
use dear_imgui_custom_mod::knowledge_graph::{
    GraphData, GraphViewer, GraphEvent, LayoutConfig, ForceConfig,
    NodeStyle, EdgeStyle, SelectionMode,
};

let mut graph = GraphData::new();
let a = graph.add_node(NodeStyle::new("Concept A").with_tag("core").with_size(8.0));
let b = graph.add_node(NodeStyle::new("Concept B"));
graph.add_edge(a, b, EdgeStyle::new().with_weight(0.8));

let mut viewer = GraphViewer::new("##kg")
    .with_force_config(ForceConfig {
        repulsion: 120.0,
        attraction: 0.04,
        center_pull: 0.002,
        collision_radius: 20.0,
        velocity_decay: 0.6,
    })
    .with_layout(LayoutConfig::barnes_hut(theta = 0.9))
    .with_selection_mode(SelectionMode::Box)
    .with_lod_threshold(5_000); // above this, use simplified rendering

loop {
    let events = viewer.render(ui, &mut graph);
    for e in events {
        match e {
            GraphEvent::NodeClicked(id)   => { /* navigate */ }
            GraphEvent::NodeDoubleClicked(id) => { /* focus */ }
            GraphEvent::SelectionChanged(set) => { /* multi-select */ }
            GraphEvent::FilterChanged(f)  => { /* update */ }
        }
    }
}
```

**Sidebar panel** (как на скриншоте: «Фильтры / Группировка /
Отображение / Силы»):

- **Фильтры** — чекбоксы по тагам, слайдер "глубина от выделенного",
  regex по имени, диапазон веса рёбер.
- **Группировка** — цвет узлов по: теге / типе / community /
  создателе / дате.
- **Отображение** — размеры узлов, толщина линий, стрелки (directed?),
  метки (всегда / hover / никогда), фоновая сетка.
- **Силы** — repulsion, attraction, center-pull, collision —
  слайдерами. Отдельный кнопк "reset layout" и "freeze simulation".

**Effort:** 3-4 дня. Barnes-Hut quadtree — самая содержательная часть,
~400 строк. Симуляция ~150 строк. Рендер через draw-list ~200 строк.
Sidebar panel ~250 строк.

---

### 2. `markdown_viewer` — рендер Markdown с CommonMark

Недостаёт. Ни один виджет в библиотеке не умеет показать Markdown.
Для developer tools это обязательный элемент — README превью, changelog
превью, docstring превью, in-app help.

**Фичи:**
- CommonMark 0.31 + GFM extensions (tables, task lists, strikethrough)
- Заголовки с anchor links
- Inline code + fenced code blocks с syntect highlighting (integrate
  с существующим `code_editor::SyntaxDefinition`)
- Изображения через ImGui texture handles (caller provides loader)
- Таблицы через `virtual_table` (re-use)
- Scroll position / search / find-in-document
- Selectable text (через ImGui InputTextMultiline в read-only mode)

**Effort:** 2 дня. Основное — интеграция `pulldown-cmark` и маппинг
событий парсера в draw-list вызовы.

---

## 🚀 Priority 2 — виджеты разработчика

### 3. `chart_view` — графики (line / bar / scatter / area / stacked)

Для профайлеров / дашбордов. У нас уже есть `timeline` (flame graph),
но обычные X-Y графики нет.

**Фичи:**
- X-Y axes с автоскейлом
- 4 типа: line, bar, scatter, area stacked
- Multiple series, легенда, пикер цветов
- Pan + zoom (как у timeline)
- Tooltip at hover-X (показывает Y для каждой серии)
- Crosshair mode (точное значение под курсором)
- Download PNG / CSV export
- Отметки (vertical markers с лейблами)

**Existing:** `implot-rs` существует, но у него свой стиль и большой
трансигтив. Написать простой нативный виджет ~500 строк стоит того.

**Effort:** 2-3 дня.

### 4. `log_viewer` — лог-viewer с фильтрами

Недостаёт. Приложения типа NxT логгируют тысячи строк в минуту —
`virtual_table` работает, но специфики лог-UX не даёт.

**Фичи:**
- Multi-level filter (TRACE / DEBUG / INFO / WARN / ERROR)
- Per-module filter (regex / contains / starts-with)
- Time range slider
- Live-tail режим с auto-scroll, останавливающийся на hover
- Цветное подсвечивание по уровню
- JSON-structured parsing (если строка валидный JSON — рендер по полям)
- Search с highlights
- "Pin" отдельных строк — остаются видимы при фильтрах
- Bookmarks / marks
- Export selected / export filtered

**Effort:** 2 дня.

### 5. `command_palette` — Ctrl+K picker

Аналог VS Code Command Palette. Обязателен для любого серьёзного tool.

**Фичи:**
- Fuzzy-match по label + category
- Keyboard-first: arrow keys, Enter, Esc
- Rank по recency + frequency + match score
- Icon per category
- Hint column с keyboard shortcut
- Sections/groups
- Callback-based API — caller регистрирует commands

**Existing:** fuzzy-matcher crate есть, интегрировать.

**Effort:** 1 день.

### 6. `split_pane` — resizable panes

Не хватает для layout. Пользователю не должен писать свой splitter
code каждый раз.

**Фичи:**
- Horizontal / vertical split
- Nested (пара внутри пары)
- Drag-to-resize с live preview
- Min/max bounds per pane
- Collapse-on-double-click-handle
- Persist sizes (serde)
- Smooth animation on collapse

**Effort:** 1 день.

### 7. `context_menu` — композируемый правый клик-menu

Есть `ImGui::Popup`, но сложно строить иерархические меню из config.

**Фичи:**
- Builder API с sub-menu поддержкой
- Icons / keyboard hints / disabled items / separators / checkboxes
- Auto-position чтобы не уходить за экран
- Keyboard nav

**Effort:** 1 день.

---

## 🎨 Priority 3 — визуализация и полировка

### 8. `toast_notifications` — стек уведомлений

Уже упомянут в планах. NxT имеет свою реализацию; стоит поднять
в mod.

**Фичи:**
- Queue с priority
- Auto-dismiss по таймеру + hover-to-pause
- Slide-in/out анимации
- 4 типа (info / success / warning / error)
- Actions (buttons inside toast)
- Стек с max-visible, overflow → bundled "3 more"
- Positions: top-right / top-left / bottom-right / bottom-left / center

**Effort:** 1 день.

### 9. `color_picker_ex` — расширенный color picker

Встроенный в ImGui `color_picker` минимальный. Нужно:

- Palette tabs (recent / favorites / material / tailwind / custom)
- HSL + HSV + OKLCH modes
- Alpha slider
- Hex input с live preview
- Eyedropper (integrate с Win32 GetCursorPos + GetPixel)
- Contrast checker (WCAG AA/AAA, shows ratio)
- Gradient builder (2-stop / 3-stop / N-stop)

**Effort:** 2 дня.

### 10. `shortcut_editor` — редактор keybindings

Приложения типа NxT / IDE имеют множество хоткеев. Сейчас редактируются
через config-файл руками.

**Фичи:**
- Grid: action × keybinding
- "Click to record" — слушает ImGui keyboard, предлагает.
- Conflict detection (два action-а на одной комбинации → warn)
- Per-context scopes ("в редакторе", "в hex viewer", "глобально")
- Preset switcher (VSCode / IntelliJ / vim)
- Import / export JSON

**Effort:** 1.5 дня.

---

## 🧠 Priority 4 — данные и инспекция

### 11. `inspector_tree` — расширяемое tree view для структурированных данных

Дополняет `property_inspector` (который плоский key=value с категориями).
`inspector_tree` это tree view для произвольных nested struct'ов с
auto-expand, type colouring, edit-in-place.

**Use case:** отладчик JSON / TOML / RON документов, inspector для
custom struct'ов в runtime.

**Effort:** 1.5 дня.

### 12. `query_builder` — визуальный SQL/filter builder

Для `virtual_table` — каждую колонку можно отфильтровать
через text-match, но составные query (A AND B OR C) вручную не
строятся.

**Фичи:**
- Drag-drop condition tokens
- AND / OR / NOT группировка
- Per-column operator picker (=, !=, >, <, contains, regex, between)
- Live preview (сколько строк пройдёт)
- Save / load named queries

**Effort:** 2 дня.

### 13. `network_waterfall` — network request timeline

Аналог DevTools Network tab. Для отладки протоколов.

**Фичи:**
- Timeline с многоуровневыми треками
- Per-request detail pane (headers / body / timings)
- Filter by type / status / domain
- Export HAR

**Effort:** 2 дня.

---

## 🎛️ Priority 5 — полный low-priority

### 14. `regex_tester` — интерактивный regex playground

Показать матчи в live-input. Backrefs, groups, alternatives.

**Effort:** 1 день.

### 15. `date_picker` — календарь / таймпикер

Простой полноценный date/time picker.

**Effort:** 1 день.

### 16. `breadcrumbs` — navigation breadcrumbs

Отдельный reusable widget.

**Effort:** 0.5 дня.

### 17. `virtual_scroller_gallery` — image gallery с thumbnails

Для asset browser / review-tool.

**Effort:** 2 дня.

---

## Что явно НЕ добавлять

- **3D viewport** — wgpu renderer уже есть, но это отдельная крупная
  работа. Не стоит размазывать фокус библиотеки.
- **Animation curve editor** — очень узкая ниша (game dev), переусложнит API.
- **State machine editor** — проще интегрировать существующий crate
  или сам node_graph с меткой "state".

---

## Приоритеты для немедленной реализации

Если бы выбирал три, взял бы:

1. **`knowledge_graph`** — уникальная фича, визуально эффектная,
   downstream-пользователям NxT она понадобится для визуализации
   packet-graph и script dependency graph. Core algorithm (Barnes-Hut)
   переиспользуется в любом другом force-layout виджете.
2. **`markdown_viewer`** — закрывает пробел, часто нужен для
   in-app help / release notes / runbooks.
3. **`command_palette`** — низкая стоимость, огромный UX-lift.
   Должен быть в любом IDE-подобном tool.

Остальное может ждать, пока не появится конкретный driver.
