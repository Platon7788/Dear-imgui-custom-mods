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

## Performance

| Graph size | FPS (release) | Notes |
|-----------|:---:|-------|
| 100 nodes | 60+ | Full quality — labels, glow, LOD off |
| 1 000 nodes | 60 | LOD kicks in, labels hidden |
| 5 000 nodes | ~60 | Barnes-Hut tree helps repulsion pass |
| 10 000 nodes | ~30 | Recommend disabling minimap + glow |
