# ForceGraph

Obsidian-style force-directed knowledge graph widget for Dear ImGui.

## Overview

`GraphViewer` renders an interactive graph from a `GraphData` snapshot using
Barnes-Hut O(N log N) physics, an ImDrawList pipeline, and a built-in sidebar
panel. It scales from small (< 100 nodes) to medium (≈ 5 000 nodes at 60 FPS).

## Features

### Physics
- **Barnes-Hut simulation** — O(N log N) quad-tree repulsion, Hooke springs, center gravity
- **Collision resolution** — early-reject via squared-distance, no overlap at rest
- **Velocity decay** + **sleep-on-idle** — simulation pauses when kinetic energy drops
- **Manual drag + auto-pin** — dragged nodes pin in place on release

### Rendering
- **6 node shapes** — Circle, Square, Diamond, Attachment (small circle), Tag (square), Cluster (octagon)
- **LOD** — at high node counts switches to 4-segment circles and hides labels
- **Color modes** — Static, ByTag (Okabe-Ito palette), ByCommunity (Louvain), ByPageRank, ByBetweenness, Custom
- **Color groups** — priority-based regex/label/kind/tag matchers with per-group color
- **Hover fade** — dims non-adjacent nodes and edges to highlight neighborhood
- **Glow on hover/select** — soft multi-ring halo behind focused nodes
- **Search-as-highlight** — active search query dims non-matching nodes (α × 0.15) instead of hiding them
- **Minimap** — 160 × 100 overlay in the bottom-right corner; click or drag to pan the main camera

### Camera
- **Pan** — left-drag on empty canvas
- **Zoom** — scroll wheel, zoom-to-cursor
- **Inertia** — smooth deceleration after fast pan
- **Fit-to-screen** — `F` key or `GraphEvent::FitToScreen`
- **Animated focus** — `GraphViewer::focus(id)` smooth-pans to a node

### Interaction
- **Box-select** — Shift+drag rectangle
- **Keyboard shortcuts** — Delete (remove selected), Ctrl+A (select all), Space (pause/resume), F (fit)
- **Context menu** — right-click node: pin, focus, remove, set as depth root
- **Double-click** — emits `NodeDoubleClicked` for host to handle

### Sidebar
- **Search** — case-insensitive label/tag substring filter
- **Tag whitelist** — show only nodes matching enabled tags
- **Depth filter** — BFS hop limit from a focused node (0–6)
- **Time-travel slider** — hide nodes/edges created after a threshold timestamp; "All" resets to INFINITY
- **Orphan / kind visibility** — toggles for unresolved links, tag nodes, attachment nodes
- **Min degree** — hide nodes with fewer than N edges
- **Color groups editor** — add/remove groups, toggle, pick color
- **Display sliders** — node size, edge width, text fade, hover fade, edge curve
- **Physics sliders** — link distance, repulsion, attraction, center pull, decay, gravity
- **Export** — Copy SVG / Copy DOT / Copy Mermaid buttons (write to system clipboard)

### Export
- **SVG** — standalone `<svg>` with baked world-space positions, directed arrow markers
- **DOT (Graphviz)** — `digraph` / `graph` with node labels and edge weights
- **Mermaid** — `flowchart LR` compatible with GitHub Markdown and the Mermaid live editor

### Metrics (on demand)
- **PageRank** — power-iteration, configurable damping + iterations
- **Betweenness centrality** — Brandes O(V · E) algorithm
- **Louvain community detection** — modularity-based clustering for ByCommunity color mode

## Quick Start

```rust
use dear_imgui_custom_mod::force_graph::{
    data::GraphData,
    style::{NodeStyle, EdgeStyle},
    config::{ViewerConfig, ForceConfig, SidebarKind},
    GraphViewer,
};

// Build graph once at startup.
let mut graph = GraphData::new();
let a = graph.add_node(NodeStyle::new("Alpha").with_tag("core"));
let b = graph.add_node(NodeStyle::new("Beta").with_tag("core"));
let c = graph.add_node(NodeStyle::new("Gamma"));
graph.add_edge(a, b, EdgeStyle::new(), 1.0, false);
graph.add_edge(b, c, EdgeStyle::new(), 0.5, true); // directed

// Create the viewer once.
let mut viewer = GraphViewer::new("my_graph")
    .with_sidebar(SidebarKind::Right);

// In the ImGui render loop (inside a window):
for event in viewer.render(&ui, &mut graph) {
    match event {
        GraphEvent::NodeClicked(id) => { /* ... */ }
        GraphEvent::FilterChanged  => { /* ... */ }
        _ => {}
    }
}
```

## Key Types

| Type | Role |
|------|------|
| `GraphData` | Node + edge storage (SlotMap backend, stable handles) |
| `GraphViewer` | Per-view state: camera, physics, selection, filter |
| `NodeStyle` | Label, tags, color, icon, radius, kind, tooltip, created_at |
| `EdgeStyle` | Color, label, dash, created_at |
| `ViewerConfig` | Visual settings — label mode, LOD threshold, color mode, minimap, search_highlight_mode |
| `ForceConfig` | Physics parameters — link distance, repulsion, attraction, gravity |
| `FilterState` | Runtime filter — search_query, enabled_tags, depth, time_threshold, min_degree |
| `GraphEvent` | Typed events per frame — NodeClicked, SelectionChanged, FilterChanged, CameraChanged, … |
| `NodeKind` | Regular / Tag / Attachment / Unresolved / Cluster / Custom |
| `ColorMode` | Static / ByTag / ByCommunity / ByPageRank / ByBetweenness / Custom(fn) |

## Feature Flag

```toml
[dependencies]
dear-imgui-custom-mod = { version = "0.9", features = ["force_graph"] }
```

`force_graph` depends on `slotmap = "1.0"` (pure Rust, no unsafe beyond SlotMap internals).

## Configuration

### ViewerConfig Fields

| Field | Default | Description |
|-------|---------|-------------|
| `theme` | `Dark` | Application theme for built-in colour palette |
| `colors_override` | `None` | Custom `GraphColors` palette (bypasses theme) |
| `show_labels` | `HoverOnly` | `Always` / `HoverOnly` / `BySize` / `Never` |
| `min_label_zoom` | `0.6` | Minimum zoom for labels in `BySize` mode |
| `show_edge_labels` | `false` | Draw edge labels at midpoint |
| `edge_arrow` | `true` | Draw arrowhead on directed edges |
| `edge_bundling` | `false` | Bundle edges to reduce visual clutter |
| `background_grid` | `true` | Dot-grid background on canvas |
| `minimap` | `false` | Show minimap overlay |
| `selection_mode` | `Additive` | `Single` / `Box` / `Additive` |
| `lod_threshold` | `5000` | Node count above which LOD activates |
| `time_travel` | `None` | `Option<TimeTravelSlider>` for sidebar time slider |
| `color_mode` | `Static` | `Static` / `ByTag` / `ByCommunity` / `ByPageRank` / `ByBetweenness` / `Custom(fn)` |
| `drag_enabled` | `true` | Nodes can be dragged |
| `context_menu_enabled` | `true` | Right-click context menu on nodes |
| `pin_on_drag` | `true` | Auto-pin dragged nodes on release |
| `hover_fade_opacity` | `0.15` | Opacity of non-hovered nodes/edges (0=hidden, 1=no fade) |
| `glow_on_hover` | `true` | Soft glow halo on hovered/selected nodes |
| `text_fade_threshold` | `0.0` | Label visibility bias by zoom (-5..5) |
| `node_size_multiplier` | `1.0` | Global node radius multiplier |
| `edge_thickness_multiplier` | `1.0` | Global edge thickness multiplier |
| `edge_curve` | `0.0` | Bézier curve amount (0=straight, 1=fully curved) |
| `color_groups` | `[]` | Ordered `Vec<ColorGroup>` — first match wins |
| `show_orphans` | `true` | Show degree-0 (isolated) nodes |
| `show_unresolved` | `true` | Show `NodeKind::Unresolved` ghost nodes |
| `show_tags` | `true` | Show `NodeKind::Tag` nodes |
| `gravity_direction` | `[0,0]` | Directional gravity vector `[x, y]` |
| `fit_padding` | `40.0` | Padding (canvas units) when fitting to screen |
| `depth_fade` | `false` | Fade nodes beyond depth hops from focused node |
| `cluster_hulls` | `false` | Draw convex hull around Louvain communities |
| `search_highlight_mode` | `true` | Dim non-matching nodes instead of hiding |

### LabelVisibility

| Variant | Behavior |
|---------|---------|
| `Always` | Labels always drawn |
| `HoverOnly` | Labels appear only on cursor hover |
| `BySize` | Labels shown when rendered radius ≥ `min_label_zoom` |
| `Never` | No labels drawn |

### SelectionMode

| Variant | Behavior |
|---------|---------|
| `Single` | Click selects one node; empty-space click clears |
| `Box` | Drag rectangle selects enclosed nodes |
| `Additive` | Box-select + Shift adds to selection |

### SidebarKind

| Variant | Behavior |
|---------|---------|
| `None` | No sidebar — viewer uses full width |
| `Built` | Built-in sidebar, fully expanded (default) |
| `BuiltCollapsed` | Built-in sidebar, starts collapsed |

### ColorGroup

Color groups allow priority-based node coloring by label/tag/kind/regex. First match wins.

```rust
use dear_imgui_custom_mod::force_graph::config::{ColorGroup, ColorGroupQuery};

ColorGroup::new("Core nodes", ColorGroupQuery::Tag("core".into()), [0.3, 0.7, 1.0, 1.0])
ColorGroup::new("Warnings",   ColorGroupQuery::Label("WARN".into()), [1.0, 0.7, 0.0, 1.0])
ColorGroup::new("All",        ColorGroupQuery::All, [0.6, 0.6, 0.6, 1.0])
```

| `ColorGroupQuery` | Matches |
|-------------------|---------|
| `Label(s)` | Case-insensitive substring of node label |
| `Tag(s)` | Exact tag match (without `#`) |
| `Kind(s)` | NodeKind name: `"regular"`, `"tag"`, `"unresolved"`, etc. |
| `Regex(s)` | Regex pattern on node label |
| `All` | Every node (catch-all) |

### ForceConfig Fields

| Field | Default | Description |
|-------|---------|-------------|
| `barnes_hut_theta` | `0.9` | Approximation threshold θ (0=exact, 2=coarse) |
| `repulsion` | `120.0` | Coulomb repulsion strength |
| `attraction` | `0.04` | Spring attraction along edges |
| `center_pull` | `0.002` | Pull toward canvas origin (prevents drift) |
| `collision_radius` | `20.0` | Min distance before collision correction |
| `link_distance` | `80.0` | Spring rest length (canvas units) |
| `velocity_decay` | `0.6` | Damping per tick (0=instant stop, 1=no damping) |
| `gravity_strength` | `0.0` | Downward gravity (`0.0` = disabled) |
| `radius_by_degree` | `true` | Node radius grows with degree |
| `radius_base` | `4.0` | Base radius when `radius_by_degree` enabled |
| `radius_per_degree` | `1.5` | Extra radius per incident edge |

### TimeTravelSlider

When set on `ViewerConfig::time_travel`, adds a slider to the sidebar:

| Field | Description |
|-------|-------------|
| `min` | Earliest timestamp value |
| `max` | Latest timestamp value |
| `step` | Slider step granularity |

## Performance

| Graph size | FPS (release) | Notes |
|-----------|:---:|-------|
| 100 nodes | 60+ | Full quality — labels, glow, LOD off |
| 1 000 nodes | 60 | LOD kicks in, labels hidden |
| 5 000 nodes | ~60 | Barnes-Hut tree helps repulsion pass |
| 10 000 nodes | ~30 | Recommend disabling minimap + glow |
