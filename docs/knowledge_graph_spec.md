# `knowledge_graph` — спецификация

Force-directed graph viewer для визуализации знаний, связей, зависимостей.
Вдохновлён Obsidian Graph View, но с расчётом на большие графы
(> 10K узлов), scripting-friendly API и несколько функций, которых
у Obsidian нет.

Статус: **design doc**, не реализовано. Оценка ~3-4 дня работы.

---

## 1. Цели

- Визуализировать граф 100-100000 узлов с интерактивным pan / zoom /
  select. Плавный 60 FPS на графах ≤ 10K узлов без GPU, ≤ 50K с
  адаптивным LOD.
- Force-directed симуляция (spring + repulsion + center) сходится за
  ≤ 200 тиков на graph 1K узлов.
- API в стиле остальных виджетов библиотеки: `GraphData` отделено от
  `GraphViewer`, конфиг через builder, результат через
  `GraphEvent` enum, `#[must_use]` на результате.
- Sidebar panel «Фильтры / Группировка / Отображение / Силы» встроенная
  + опционально отключаемая (пользователь может нарисовать свою).

## 2. Не-цели

- **Редактор узлов** — для этого уже есть `node_graph`. `knowledge_graph`
  только показывает; drag-to-create / edge-dragging не планируется.
- **3D.** 2D-only — зачем усложнять.
- **Auto-layout других типов** (hierarchical / orthogonal / radial) —
  фокус строго на force-directed.

---

## 3. Публичный API

### 3.1. `GraphData` — модель

```rust
/// Opaque handle to a node in [`GraphData`]. Stable under insertion and
/// removal (slab-backed).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct NodeId(u32);

/// Opaque handle to an edge.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct EdgeId(u32);

/// The graph model. Users build it once (or stream updates) and hand
/// it to [`GraphViewer`] for rendering.
pub struct GraphData {
    nodes: SlotMap<NodeId, NodeStyle>,
    edges: SlotMap<EdgeId, Edge>,
    // Spatial index rebuilt per frame for hit-testing.
    // Adjacency for force sim. Both private.
}

impl GraphData {
    pub fn new() -> Self;
    pub fn with_capacity(nodes: usize, edges: usize) -> Self;

    // ── Mutation ───────────────────────────────────────────────
    pub fn add_node(&mut self, style: NodeStyle) -> NodeId;
    pub fn add_edge(&mut self, a: NodeId, b: NodeId, style: EdgeStyle) -> EdgeId;
    pub fn remove_node(&mut self, id: NodeId);
    pub fn remove_edge(&mut self, id: EdgeId);
    pub fn clear(&mut self);

    // ── Query ──────────────────────────────────────────────────
    pub fn node(&self, id: NodeId) -> Option<&NodeStyle>;
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut NodeStyle>;
    pub fn edge(&self, id: EdgeId) -> Option<&Edge>;
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &NodeStyle)>;
    pub fn edges(&self) -> impl Iterator<Item = (EdgeId, &Edge)>;
    pub fn neighbors(&self, id: NodeId) -> impl Iterator<Item = NodeId>;
    pub fn degree(&self, id: NodeId) -> usize;
    pub fn node_count(&self) -> usize;
    pub fn edge_count(&self) -> usize;

    // ── Derived metrics (lazy, cached, invalidated on mutation) ─
    pub fn pagerank(&self) -> &[f32];
    pub fn betweenness_centrality(&self) -> &[f32];
    pub fn community_assignment(&self) -> &[u32]; // Louvain
}
```

### 3.2. `NodeStyle` / `EdgeStyle`

```rust
pub struct NodeStyle {
    pub label: String,
    pub tags: SmallVec<[&'static str; 4]>,
    /// Absolute radius in logical pixels. If None, radius is derived
    /// from `degree()` via `ForceConfig::radius_by_degree`.
    pub radius: Option<f32>,
    /// Base fill colour. If None, derived from tag / community / PageRank
    /// per [`ColorMode`].
    pub color: Option<[f32; 4]>,
    /// Anchor position — if Some, physics skips this node and keeps it here.
    pub anchor: Option<[f32; 2]>,
    /// Timestamp for time-travel filter. NaN means "always visible".
    pub created_at: f32,
    /// Arbitrary user payload. Widget doesn't touch it.
    pub user_data: u64,
}

impl NodeStyle {
    pub fn new(label: impl Into<String>) -> Self;
    pub fn with_tag(mut self, t: &'static str) -> Self;
    pub fn with_radius(mut self, r: f32) -> Self;
    pub fn with_color(mut self, c: [f32; 4]) -> Self;
    pub fn with_anchor(mut self, pos: [f32; 2]) -> Self;
    pub fn with_timestamp(mut self, t: f32) -> Self;
    pub fn with_user_data(mut self, v: u64) -> Self;
}

pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub directed: bool,
    pub weight: f32,        // 0..=1, scales attraction force + line thickness
    pub style: EdgeStyle,
}

pub struct EdgeStyle {
    pub color: Option<[f32; 4]>,
    pub dashed: bool,
    pub label: Option<String>,
    pub created_at: f32,
}
```

### 3.3. `GraphViewer` — виджет

```rust
pub struct GraphViewer {
    id: String,
    pub config: ViewerConfig,
    // All runtime state (camera, simulation, hover/select, sidebar expansion).
    // Private.
}

impl GraphViewer {
    pub fn new(id: impl Into<String>) -> Self;

    pub fn with_config(mut self, c: ViewerConfig) -> Self;
    pub fn with_force_config(mut self, f: ForceConfig) -> Self;
    pub fn with_color_mode(mut self, m: ColorMode) -> Self;
    pub fn with_sidebar(mut self, kind: SidebarKind) -> Self;

    /// Render for one frame. Returns events.
    #[must_use = "graph events (click / select / filter change) are emitted here"]
    pub fn render(&mut self, ui: &Ui, graph: &mut GraphData) -> Vec<GraphEvent>;

    // ── Imperative controls (for hotkeys etc.) ────────────────
    pub fn focus(&mut self, id: NodeId);             // smooth pan + zoom
    pub fn select(&mut self, id: NodeId);
    pub fn select_multi(&mut self, ids: &[NodeId]);
    pub fn clear_selection(&mut self);
    pub fn reset_layout(&mut self);                  // re-randomize + re-sim
    pub fn freeze(&mut self, frozen: bool);          // pause sim
    pub fn camera(&self) -> Camera;
    pub fn set_camera(&mut self, c: Camera);
}
```

### 3.4. `ViewerConfig` / `ForceConfig` / `ColorMode`

```rust
pub struct ViewerConfig {
    pub theme: Theme,                    // uses unified Theme enum
    pub colors_override: Option<Box<GraphColors>>,

    pub show_labels: LabelVisibility,    // Always / HoverOnly / Never / BySize
    pub min_label_zoom: f32,             // labels fade in above this zoom
    pub show_edge_labels: bool,
    pub edge_arrow: bool,                // directed edges draw arrowheads
    pub edge_bundling: bool,             // force-directed bundling for dense graphs

    pub background_grid: bool,
    pub minimap: bool,                   // small navigator overlay
    pub selection_mode: SelectionMode,   // Single / Box / Additive

    /// Above this node count, switch to low-detail rendering (no labels,
    /// simplified circles, edges as straight lines without AA).
    pub lod_threshold: usize,

    pub time_travel: Option<TimeTravelSlider>,
}

pub struct ForceConfig {
    /// Barnes-Hut parameter — higher = faster but less accurate.
    pub barnes_hut_theta: f32,           // 0.5..=2.0, default 0.9

    pub repulsion: f32,                  // Coulomb-like, default 120.0
    pub attraction: f32,                 // Hooke spring, default 0.04
    pub center_pull: f32,                // pulls nodes toward (0,0)
    pub collision_radius: f32,           // prevents overlap
    pub velocity_decay: f32,             // 0..=1, default 0.6
    pub gravity_strength: f32,

    /// If true, node radius = radius_base + radius_per_degree * degree
    pub radius_by_degree: bool,
    pub radius_base: f32,
    pub radius_per_degree: f32,
}

pub enum ColorMode {
    /// Use `NodeStyle::color` or a neutral fallback.
    Static,
    /// Color by first tag (stable hash → palette).
    ByTag,
    /// Color by Louvain community id.
    ByCommunity,
    /// Gradient by PageRank centrality.
    ByPageRank,
    /// Gradient by betweenness.
    ByBetweenness,
    /// User-supplied callback.
    Custom(Box<dyn Fn(&NodeStyle, &GraphData) -> [f32; 4]>),
}

pub enum LabelVisibility { Always, HoverOnly, BySize, Never }
pub enum SelectionMode { Single, Box, Additive }
pub enum SidebarKind { None, Built, BuiltCollapsed }
```

### 3.5. `GraphEvent`

```rust
#[must_use]
pub enum GraphEvent {
    NodeClicked(NodeId),
    NodeDoubleClicked(NodeId),
    NodeHovered(NodeId),
    NodeContextMenu(NodeId, [f32; 2]),      // right-click, screen pos
    EdgeClicked(EdgeId),
    SelectionChanged(HashSet<NodeId>),
    FilterChanged(FilterState),             // sidebar updates
    CameraChanged(Camera),                  // for persisting user zoom/pan
}
```

---

## 4. Архитектура реализации

### 4.1. Модули

```
src/knowledge_graph/
├── mod.rs              # public API + render()
├── data.rs             # GraphData + SlotMap handles
├── config.rs           # ViewerConfig / ForceConfig / builders
├── style.rs            # NodeStyle / EdgeStyle / GraphColors
├── event.rs            # GraphEvent enum
├── sim/
│   ├── mod.rs          # Simulation orchestration
│   ├── barnes_hut.rs   # Quadtree for O(N log N) repulsion
│   ├── spring.rs       # Hooke-spring edge attraction
│   └── collision.rs    # Node-node collision resolution
├── layout/
│   ├── mod.rs          # Initial node placement (random / circular)
│   └── community.rs    # Louvain community detection
├── metrics/
│   ├── pagerank.rs     # Power-iteration PageRank
│   └── centrality.rs   # Betweenness (Brandes' algorithm)
├── render/
│   ├── mod.rs          # Main render path
│   ├── camera.rs       # Pan/zoom state
│   ├── minimap.rs      # Small overlay navigator
│   ├── labels.rs       # Label visibility + collision
│   └── edge_bundle.rs  # Optional force-directed edge bundling
├── sidebar.rs          # "Фильтры / Группировка / Отображение / Силы"
└── filter.rs           # FilterState + hit-test pipeline
```

### 4.2. Физика

**Barnes-Hut quadtree** — O(N log N) вместо O(N²) для repulsion:

```
For each frame:
  1. Build quadtree из текущих позиций (O(N))
  2. For each node n:
       compute force_n = Σ repulsion(n, tree_node) для всех tree nodes
                       где расстояние/size > theta
       (O(log N) средний случай)
  3. For each edge (a, b):
       force_a += attraction(a, b)
       force_b -= attraction(a, b)
  4. For each node: position += velocity * dt; velocity *= decay
  5. For each collision: push apart
```

Типовые параметры (60 FPS, graph ≤ 10K):
- theta = 0.9
- tick_dt = 1/60 (inside `ui.io().delta_time()`)
- decay = 0.6 (сходится за ~150 тиков после mutation)

**Стабильность:** после 2 секунд без изменений симуляция
автоматически "засыпает" (позиции замораживаются) — экономит CPU в
static-режиме. Любое изменение графа / drag узла просыпает её.

### 4.3. Рендер

**Per-frame steps:**
1. `Simulation::tick` — update positions (skipped if asleep)
2. Apply pan/zoom from `Camera`
3. Spatial index rebuild (quadtree, для hit-testing)
4. Apply filter → bitset видимых узлов
5. LOD decision: если visible_count > lod_threshold → simplified path
6. Render edges: iterate visible edges, `draw.add_line` с толщиной
   из веса
7. Render nodes: iterate visible nodes, `draw.add_circle_filled`
   + stroke + опциональный label
8. Render selection: highlight ring на выделенных
9. Render hover: tooltip с node info
10. Render sidebar (если включён)
11. Render minimap (если включён)

**Simplified path (LOD):** только узлы без labels, edges без AA, без
hover highlight. Переход автоматический.

### 4.4. Sidebar panel

Разворачивающиеся секции (как на скриншоте Obsidian):

**Фильтры:**
- Tag checkboxes (авто-собранные из всех NodeStyle::tags)
- Slider "Distance from selection": 0 = только выделенные, ∞ = все
- Regex input для имени
- Weight range for edges

**Группировка:**
- Radio: Static / ByTag / ByCommunity / ByPageRank / ByBetweenness
- Color palette picker (Okabe-Ito / Tableau 10 / custom)

**Отображение:**
- Show labels: Always / Hover / By size / Never
- Show arrows checkbox
- Show minimap checkbox
- Background grid checkbox
- Edge bundling toggle

**Силы:**
- Sliders для всех `ForceConfig` полей
- Button: Reset to defaults
- Checkbox: Freeze simulation

### 4.5. Time-travel

Если `ViewerConfig::time_travel: Some(...)`, рендерится slider
в нижней части графа. Передвижение слайдера фильтрует узлы + рёбра
по `created_at <= slider_value`. Плавная анимация появления/исчезания.

### 4.6. Search-as-highlight

Text input в sidebar (или через Ctrl+F шорткат). Матч по `label` / тагам.
Не-совпавшие узлы рендерятся с alpha = 0.15; совпавшие — как обычно +
кольцо подсветки.

---

## 5. Производительность — целевые числа

| Graph size | FPS target | Memory      | Sim convergence |
|------------|-----------|-------------|-----------------|
| 100 nodes  | 60 stable | 30 KB       | < 20 ticks      |
| 1000       | 60 stable | 300 KB      | < 100 ticks     |
| 10 000     | 60 stable | 3 MB        | < 400 ticks     |
| 50 000     | 30+ (LOD) | 15 MB       | < 2000 ticks    |
| 100 000    | 20+ (LOD) | 30 MB       | < 5000 ticks    |

**Ключ к перфу:**
- Barnes-Hut (O(N log N) вместо O(N²))
- Quadtree — single allocation pool, reused across frames
- Simplified LOD path для больших графов
- Sleep-on-idle — не крутить sim если ничего не меняется
- Draw-list primitives (без per-node ImGui widgets — слишком накладно)

---

## 6. Публичный тест-план

**Integration tests (`tests/knowledge_graph.rs`):**
- Empty graph: render + tick без паники
- Single node: positioned at (0,0), no force applied
- Two connected nodes: converge to spring rest length
- 1000 random nodes: sim finishes under 500 ticks with positions in
  finite bounds
- Cycle: positions form approximately regular polygon
- Anchor: anchored node stays put while others orbit
- Filter: filtered-out nodes not in hit-test results

**Benches:**
- Barnes-Hut build: 100 / 1K / 10K / 50K nodes
- Full tick: same sizes
- Render frame: same sizes
- Filter pass: with / without filter

**Property tests:**
- `prop_positions_finite`: после N тиков никакая координата не NaN / Inf
- `prop_energy_decreases`: сумма кинетической энергии монотонно падает
  при static graph (после > 50 тиков)

---

## 7. Зависимости

| Crate          | Why                                   | Estimated size |
|----------------|----------------------------------------|----------------|
| `slotmap` 1.x  | Stable NodeId under insertion/removal | tiny           |
| `smallvec`     | tags без per-node Vec allocation      | already in tree |
| `rustc-hash`   | быстрый hash для NodeId HashSet       | tiny           |

Всё — pure Rust, no C deps.

---

## 8. Ranged rollout

**Phase A (MVP, ~1.5 дня):**
- GraphData + NodeId/EdgeId
- Naive O(N²) simulation
- Basic render (circles + lines)
- Pan/zoom
- Single-click + hover

**Phase B (~1 день):**
- Barnes-Hut quadtree
- Selection (single + box)
- Label visibility modes
- Sidebar panel с "Фильтры / Отображение"

**Phase C (~1 день):**
- PageRank + Louvain + betweenness
- ColorMode::ByCommunity / ByPageRank
- Sidebar "Группировка / Силы"
- Minimap

**Phase D (~0.5 дня):**
- Time-travel slider
- Search-as-highlight
- Export (SVG / DOT)

Total: ~4 дня.

---

## 9. Почему это круче Obsidian graph view

| Feature | Obsidian | knowledge_graph |
|---------|----------|-----------------|
| Max node count smooth | ~5K | 50K+ (LOD) |
| Force tuning | 4 sliders | 8 sliders + per-category controls |
| Color modes | By тег | By tag + Louvain + PageRank + betweenness + custom |
| Time-travel | ❌ | ✅ slider + animated |
| Search highlight | фильтрация | highlight без скрытия |
| Pin/anchor nodes | ❌ | ✅ drag-drop |
| Edge bundling | ❌ | ✅ optional |
| Export | ❌ | SVG / PNG / DOT / Mermaid |
| Programmatic API | ❌ (closed) | ✅ full Rust API |
| Custom color callback | ❌ | ✅ |
| Sub-graph focus animated | partial | ✅ plush animation |

---

## 10. Открытые вопросы

- **Edge labels** — рендерить по середине всегда? Или rotate along edge?
  (Последнее выглядит лучше но сложнее hit-test.)
- **Selection box on top of everything** — alpha overlay + stroke?
  Или marching-ants? Голосую за solid alpha, быстрее.
- **Save layout** — стоит ли сохранять финальные позиции в `GraphData`?
  Плюс: повторные визиты стартуют быстро. Минус: mutability.
  Компромисс — opt-in method `freeze_layout()` который персистит
  текущие позиции как anchors.
