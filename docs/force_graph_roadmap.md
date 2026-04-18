# `knowledge_graph` — implementation roadmap

Конкретный пошаговый план реализации виджета согласно
[`knowledge_graph_spec.md`](./knowledge_graph_spec.md).

Четыре фазы, каждая — самодостаточный merge-able milestone.
Оценки по часам для senior engineer; умножать ×2 на первую реализацию.

---

## Pre-flight — 1 час

Создать скелет без функциональности:

```
src/knowledge_graph/
├── mod.rs               # pub use re-exports + lib.rs gate
├── data.rs              # empty: GraphData, NodeId, EdgeId
├── config.rs            # empty: ViewerConfig, ForceConfig, ColorMode
├── style.rs             # empty: NodeStyle, EdgeStyle, GraphColors
├── event.rs             # empty: GraphEvent
├── sim/
│   ├── mod.rs           # empty: Simulation trait
│   └── spring.rs        # empty
├── layout/
│   └── mod.rs           # empty
├── metrics/
│   └── mod.rs           # empty
└── render/
    ├── mod.rs           # empty
    └── camera.rs        # empty
```

+ `[features]` block: `knowledge_graph = ["nav_panel"]` (sidebar reuses
  some of the nav panel's palette).
+ `[[example]] demo_knowledge_graph` registered.
+ `lib.rs`: `#[cfg(feature = "knowledge_graph")] pub mod knowledge_graph;`
+ CI: passes with `--no-default-features --features=knowledge_graph`.

Outcome: empty module compiles, no functionality.

---

## Phase A — MVP (naïve O(N²), functional) · 10–12 часов

**Goal**: render a graph of 100 nodes, panning/zooming works, clicks
fire events. No performance optimizations yet.

### A.1. `data.rs` — `GraphData` / `NodeId` / `EdgeId` (2h)

- Use `slotmap` dep (new dep: `slotmap = "1.0"`).
- `GraphData { nodes: SlotMap<NodeId, Node>, edges: SlotMap<EdgeId, Edge> }`
- `Node { style: NodeStyle, pos: [f32; 2], vel: [f32; 2] }` — pos/vel
  part of the model (not a parallel Vec) so they survive removal
  without re-index pass.
- Public: `add_node`, `remove_node`, `add_edge`, `remove_edge`,
  `nodes()`, `edges()`, `neighbors(id)`, `degree(id)`, `node_count`,
  `edge_count`, `clear`.
- Tests: create 1000 nodes, remove half, verify counts and that
  remaining IDs still resolve.

### A.2. `style.rs` — `NodeStyle` / `EdgeStyle` (1h)

- Builder methods (`.with_tag`, `.with_color`, `.with_radius`,
  `.with_anchor`, `.with_timestamp`, `.with_user_data`).
- `Edge { from, to, directed, weight, style }`.
- `GraphColors` — palette struct (matches the library's theme pattern).

### A.3. `config.rs` — builders (1h)

- `ViewerConfig::default()` — sane defaults.
- `ForceConfig::default()` — initial physics tuning:
  `repulsion 120.0, attraction 0.04, center_pull 0.002,
   collision_radius 20.0, velocity_decay 0.6`.
- `ColorMode::Static` only (other modes → Phase C).
- Tests: builder chains compile and pass values through.

### A.4. `sim/` — naïve O(N²) simulation (2h)

- `Simulation::tick(&mut GraphData, &ForceConfig, dt: f32)`:
  - For each pair (i, j): Coulomb-style repulsion.
  - For each edge: Hooke spring attraction.
  - Center pull toward (0, 0).
  - Anchor: skip position update for anchored nodes.
  - Velocity decay, position += vel * dt.
- Sleep-on-idle: if sum(vel²) < ε for K consecutive ticks, freeze.
  Any mutation wakes sim.
- Tests:
  - 2 connected nodes converge to spring-rest length
  - cycle of 4 → positions ≈ regular polygon
  - anchored node stays put
  - positions stay finite after 500 ticks

### A.5. `render/camera.rs` — pan/zoom (1.5h)

- `Camera { offset: [f32; 2], zoom: f32 }`.
- `screen_to_world` / `world_to_screen` transforms.
- Mouse drag on empty area → pan. Wheel → zoom around cursor.
- Inertia on release (velocity_decay on camera).

### A.6. `render/mod.rs` — draw pipeline (3h)

- Main render fn: `GraphViewer::render(&mut self, ui: &Ui,
  graph: &mut GraphData) -> Vec<GraphEvent>`.
- Consume `ui.io().delta_time()` → `Simulation::tick`.
- Draw edges via `draw_list.add_line` (straight, AA on).
- Draw nodes via `draw_list.add_circle_filled` + outline.
- Draw labels if zoom > `min_label_zoom`.
- Hover detection: linear scan over nodes (O(N)), closest-to-cursor.
- Click → `GraphEvent::NodeClicked`.
- Double-click → `GraphEvent::NodeDoubleClicked`.
- Right-click → `GraphEvent::NodeContextMenu`.

### A.7. `demo_knowledge_graph.rs` (0.5h)

- Random graph 50 nodes, pan/zoom/click works.
- Used to eyeball during development.

### Milestone A exit criteria
- `cargo test -p dear-imgui-custom-mod -- knowledge_graph`: green
- `cargo run --example demo_knowledge_graph --release`: 60 FPS stable
  at 100 nodes; smooth pan/zoom/click.

---

## Phase B — performance + interaction (Barnes-Hut, select) · 10 часов

**Goal**: 60 FPS on 10K nodes. Box-select + multi-select.

### B.1. `sim/barnes_hut.rs` — Quadtree (4h)

- Recursive quad subdivision until leaf has ≤ 1 node.
- Center-of-mass per internal node.
- Force compute: `if (size / dist) < theta → approximate as CoM;
  else recurse`.
- Single arena-allocated tree, reused across frames.
- Tests:
  - tree contains every node exactly once
  - force on uniform grid ≈ O(log N) nodes visited per query
  - theta=0.0 matches O(N²) naïve sim within 1% RMSE

### B.2. Simulation integration (1h)

- Swap O(N²) repulsion loop for Barnes-Hut traversal.
- Attraction + spring stay per-edge (already O(E)).
- Bench vs naïve on 500 / 1000 / 5000 nodes.

### B.3. Spatial hit-test index (2h)

- `render/hit_index.rs`: same quadtree, rebuilt each frame.
- `nearest_node_at(screen_pos, radius) -> Option<NodeId>` — O(log N).
- Replaces linear hover scan.

### B.4. Selection (2h)

- Single click → clear + add.
- Ctrl+click → toggle.
- Shift+click → add (no toggle).
- Drag on empty area (no pan, handled by mode):
  `SelectionMode::Box` → rubber-band rectangle, all nodes inside added.
- Visual: selected nodes get a highlight ring + label in bold.
- `GraphEvent::SelectionChanged(HashSet<NodeId>)`.

### B.5. Basic sidebar (1h)

- `sidebar.rs`: right-side collapsible panel with placeholder sections
  ("Фильтры", "Отображение").
- Just layout for now; controls added in Phase C.

### Milestone B exit criteria
- Barnes-Hut bench: 10K nodes ≤ 2 ms/frame
- Selection box hits all visible nodes inside rectangle
- Hover query O(log N) (verified with bench)
- `cargo run --example demo_knowledge_graph --release` smooth on 10K

---

## Phase C — metrics + sidebar controls · 6 часов

**Goal**: ColorMode::ByCommunity / ByPageRank work. Sidebar controls
every force + filter.

### C.1. `metrics/pagerank.rs` (1.5h)

- Power iteration with damping 0.85.
- Cache in `GraphData`, invalidate on mutation.
- Normalize to [0, 1] for color gradient.
- Test: 3-node star → center has highest rank.

### C.2. `metrics/community.rs` — Louvain modularity (2h)

- Standard Louvain (modularity optimization).
- Returns `Vec<u32>` = community id per node.
- Cache + invalidate on mutation.
- Test: two disconnected triangles → two communities.

### C.3. `metrics/centrality.rs` — Brandes' betweenness (1h)

- Brandes' O(V·E) algorithm.
- Unweighted version first (weighted later if needed).
- Test: bridge node in dumbbell graph has highest betweenness.

### C.4. Color mode dispatch (0.5h)

- `ColorMode::ByTag` — first tag → stable hash → Okabe-Ito palette.
- `ColorMode::ByCommunity` — community id → palette.
- `ColorMode::ByPageRank` — gradient (low → high).
- `ColorMode::ByBetweenness` — gradient.
- `ColorMode::Custom(Box<dyn Fn>)` — callback.

### C.5. Full sidebar (1h)

- **Фильтры**: tag checkboxes (harvested from all nodes), search input
  (highlight-only, not filter), weight range.
- **Группировка**: `ColorMode` radio buttons.
- **Отображение**: `LabelVisibility` radio, show_arrows, minimap,
  background_grid, edge_bundling (placeholder — Phase D).
- **Силы**: sliders for every `ForceConfig` field, "Reset" button,
  "Freeze simulation" toggle.

### Milestone C exit criteria
- `ColorMode::ByCommunity` highlights natural clusters on a toy graph
- Sliders re-apply on every change (simulation wakes up)
- Benches: metrics computed lazily, cached until mutation

---

## Phase D — advanced features · 4 часов

**Goal**: time-travel, search-as-highlight, minimap, export.

### D.1. Time-travel slider (1.5h)

- Slider widget at bottom of graph if
  `ViewerConfig::time_travel: Some(TimeTravelSlider)`.
- Filter nodes/edges by `created_at <= slider_value`.
- Fade in/out animated (linear over 300 ms).

### D.2. Search-as-highlight (0.5h)

- Text input in sidebar → match against `label` + `tags`.
- Non-matching nodes get `alpha *= 0.15`.
- Matching nodes get extra highlight ring.

### D.3. Minimap (1h)

- Small overlay in corner (configurable).
- Shows whole graph at low zoom.
- Viewport rect drawn on it.
- Click-drag minimap → pan main view.

### D.4. Export (1h)

- SVG: iterate nodes/edges → emit SVG text to `&mut String`.
- DOT: GraphViz format.
- Mermaid: simple flowchart-style.
- Hook into sidebar "Export" submenu.

### Milestone D exit criteria
- Time-travel slider smoothly animates graph over a sequence of
  timestamps
- Search highlights matching nodes without hiding others
- SVG export opens cleanly in any SVG viewer

---

## Summary

| Phase | Effort | Deliverable |
|-------|--------|-------------|
| Pre-flight | 1h    | Skeleton + feature flag |
| A (MVP)    | 10-12h | 100-node graph, pan/zoom/click |
| B (perf)   | 10h    | 10K nodes at 60 FPS + selection |
| C (metrics)| 6h     | PageRank/Louvain/betweenness + full sidebar |
| D (extras) | 4h     | Time-travel + search + minimap + export |
| **Total**  | **31-33h** | Production-ready knowledge_graph |

Parallel work possible within a phase — e.g. someone can write
tests for A.4 while another implements A.6.

Each phase ends on a clean merge — `cargo test` green, no
`#[allow]`, CI passes. Then next phase builds on top.

---

## Dependencies to add

```toml
# Cargo.toml dev-dependencies or main, at the right section

[dependencies]
slotmap = "1.0"        # stable NodeId/EdgeId under mutation
# smallvec, rustc-hash already in tree

[dev-dependencies]
# criterion + proptest already in tree
```

Three pure-Rust deps, no C code.

---

## Open questions to resolve during implementation

1. **Labels on dense graphs** — rotate along edge or just straight?
   Straight is simpler; rotate is cooler. Pick straight for Phase A,
   revisit in Phase D.
2. **Persist layout** — add `GraphData::freeze_layout()` which converts
   current positions into anchors? Low priority; add if user asks.
3. **Streaming API** — should `GraphData::add_node` during sim wake
   sim or queue? Queue is safer; add during Phase A.
4. **Undirected vs directed** — `Edge::directed: bool`. Arrowhead
   rendering in Phase B. Curved arrows (to distinguish A→B vs B→A)
   in Phase D if needed.

---

## Risks and mitigations

- **Risk**: Barnes-Hut bugs cause instability (NaN positions).
  **Mitigation**: proptest `prop_positions_finite` over random graphs,
  run before every commit to phase B.
- **Risk**: Sidebar rendering conflicts with graph hit-testing.
  **Mitigation**: render sidebar FIRST, then disable graph
  interaction if `ui.is_item_hovered()` inside sidebar bounds.
- **Risk**: Large graphs (50K+) hit draw-list triangle limit.
  **Mitigation**: LOD kicks in above `ViewerConfig::lod_threshold` —
  simplified circles (4 segments vs 12), straight-line edges without
  AA, no labels.
- **Risk**: sidebar panel "Фильтры" enumerating all unique tags on
  large graphs is slow.
  **Mitigation**: cache tag set in `GraphData`, invalidate on
  `add_node`. Rebuild only when flag set.
