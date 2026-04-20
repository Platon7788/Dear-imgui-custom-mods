# NodeGraph

Visual node graph editor for Dear ImGui, inspired by Blender and Unreal Blueprint.

## Overview

`NodeGraph<T>` is a fully interactive visual programming canvas. Nodes contain user-defined data of type `T`, connected by typed wires between input/output pins. All rendering uses the native `ImDrawList` API for zero-overhead bezier curves, shapes, and text.

## Features

- **Pan and zoom** (middle/right mouse + scroll wheel, zoom to cursor)
- **Smooth zoom** animation with exponential ease-out interpolation
- **3 wire styles**: Bezier, Straight, Orthogonal — all with **obstacle-aware routing** that detects overlapping node AABBs in the wire corridor and routes around them
- **Wire flow animation**: optional animated dots along wires showing data direction
- **Per-pin color and style overrides** via `PinInfo`
- **4 pin shapes**: Circle, Triangle, Square, Diamond
- **Multi-select** (Ctrl+Click) and **rectangle selection**
- **Node collapse/expand** (chevron button in header)
- **Node drop shadow** for depth perception (configurable offset/alpha)
- **Snap-to-grid** with configurable grid size
- **Interactive mini-map** (click/drag to navigate; no viewport clutter)
- **Canvas stats overlay** (node count, wire count, zoom, selection — configurable corner)
- **Wire yanking** (Ctrl+Click on wire to detach and redirect)
- **Dropped wire menu** (drop wire on canvas to create + auto-connect)
- **Context menus**: right-click on canvas or nodes
- **Keyboard shortcuts**: Delete (remove selected), Ctrl+A (select all), Escape (cancel)
- **LOD (level of detail)**: labels, pins, and bodies hidden at low zoom
- **Wire layer** control: render wires behind or above nodes
- **Custom node bodies**: sliders, color pickers, combos via `render_body(&mut T)`; body clipped to node bounds
- **Per-node body height** override via `body_height()` for multi-row widget nodes
- **Custom header colors** per node
- **Tooltips** on nodes, input pins, and output pins (with configurable delay)
- **Frustum culling**: only visible nodes rendered — scales to 100,000+ nodes
- **O(1) selection** via `HashSet<NodeId>`
- **Zero per-frame allocations** (scratch buffers for visible nodes, draw order, stats overlay)

## Quick Start

```rust
use dear_imgui_custom_mod::node_graph::*;

// 1. Define your node type
#[derive(Clone)]
enum MyNode {
    Value(f32),
    Add,
    Output,
}

// 2. Implement the viewer trait
struct MyViewer;

impl NodeGraphViewer<MyNode> for MyViewer {
    fn title<'a>(&'a self, node: &'a MyNode) -> &'a str {
        match node {
            MyNode::Value(_) => "Value",
            MyNode::Add => "Add",
            MyNode::Output => "Output",
        }
    }

    fn inputs(&self, node: &MyNode) -> u8 {
        match node {
            MyNode::Value(_) => 0,
            MyNode::Add => 2,
            MyNode::Output => 1,
        }
    }

    fn outputs(&self, node: &MyNode) -> u8 {
        match node {
            MyNode::Value(_) | MyNode::Add => 1,
            MyNode::Output => 0,
        }
    }

    fn has_body(&self, node: &MyNode) -> bool {
        matches!(node, MyNode::Value(_))
    }

    fn render_body(&self, ui: &dear_imgui_rs::Ui, node: &mut MyNode, _id: NodeId) {
        if let MyNode::Value(v) = node {
            ui.set_next_item_width(80.0);
            ui.slider("##v", -10.0, 10.0, v);
        }
    }
}

// 3. Create the graph
let mut ng: NodeGraph<MyNode> = NodeGraph::new("my_graph");
let val = ng.add_node(MyNode::Value(5.0), [100.0, 100.0]);
let add = ng.add_node(MyNode::Add, [300.0, 100.0]);
let out = ng.add_node(MyNode::Output, [500.0, 100.0]);

// Wire them up
ng.connect(OutPinId { node: val, output: 0 }, InPinId { node: add, input: 0 });
ng.connect(OutPinId { node: add, output: 0 }, InPinId { node: out, input: 0 });

// 4. Render each frame
let viewer = MyViewer;
for action in ng.render(&ui, &viewer) {
    match action {
        GraphAction::Connected(wire) => {
            ng.graph.connect(wire.out_pin, wire.in_pin);
        }
        GraphAction::Disconnected(wire) => {
            ng.graph.disconnect(wire.out_pin, wire.in_pin);
        }
        GraphAction::DeleteSelected => {
            for id in ng.selected() { ng.remove_node(id); }
        }
        GraphAction::CanvasMenu(pos) => {
            // Open context menu to add nodes at `pos`
        }
        _ => {}
    }
}
```

## NodeGraphViewer Trait

Required methods:

```rust
fn title<'a>(&'a self, node: &'a T) -> &'a str;
fn inputs(&self, node: &T) -> u8;
fn outputs(&self, node: &T) -> u8;
```

Optional overrides:

| Method | Default | Description |
|--------|---------|-------------|
| `input_label(node, pin)` | `""` | Label shown next to input pin |
| `output_label(node, pin)` | `""` | Label shown next to output pin |
| `input_pin(node, pin)` | Blue circle | Pin visual: shape, fill, stroke, wire color |
| `output_pin(node, pin)` | Blue circle | Pin visual for output side |
| `has_body(node)` | `false` | Whether node has an expandable body section |
| `render_body(ui, node, id)` | no-op | Render ImGui widgets in the body (`&mut T`) |
| `header_color(node)` | `None` | RGB header tint override |
| `can_connect(from, to, graph)` | `true` | Connection validation (type checking, cycle prevention) |
| `on_connect(from, to, graph)` | no-op | Post-connection callback |
| `on_disconnect(from, to, graph)` | no-op | Post-disconnection callback |
| `node_tooltip(node)` | `None` | Hover tooltip |
| `input_tooltip(node, pin)` | `None` | Input pin tooltip |
| `output_tooltip(node, pin)` | `None` | Output pin tooltip |
| `node_width(node)` | `None` | Custom node width (falls back to `config.node_min_width`) |
| `body_height(node)` | `None` | Override body height for nodes with multiple widget rows (e.g. `Some(54.0)` for a Vec2 node with two sliders) |

### Lifetime Note

Methods returning `&str` use a unified lifetime `'a` for `&self` and `&T`, so the returned string can come from either the viewer struct or the node data.

## Pin Customization

```rust
fn output_pin(&self, node: &MyNode, _output: u8) -> PinInfo {
    match node {
        MyNode::FloatValue => PinInfo::circle([0x5b, 0x9b, 0xd5]),  // blue circle
        MyNode::Vec2Value  => PinInfo::square([0x7b, 0xbb, 0x55]),  // green square
        MyNode::Color      => PinInfo::triangle([0xd5, 0x5b, 0x9b]) // pink triangle
                                  .with_wire_color([0xff, 0x80, 0xc0])
                                  .with_wire_style(WireStyle::Line),
        _ => PinInfo::default(),
    }
}
```

Available shapes: `Circle`, `Triangle`, `Square`, `Diamond`.

## GraphAction

Actions returned by `render()` — process in a loop:

| Action | Description |
|--------|-------------|
| `Connected(Wire)` | User completed a wire connection — call `graph.connect()` |
| `Disconnected(Wire)` | Wire removed — call `graph.disconnect()` |
| `NodeSelected(NodeId)` | Node was clicked |
| `NodeDeselected(NodeId)` | Node was deselected |
| `NodeMoved(NodeId)` | Node was dragged to a new position |
| `NodeDoubleClicked(NodeId)` | Double-click on node |
| `NodeToggled(NodeId)` | Collapse/expand toggled (handled internally) |
| `CanvasMenu([f32; 2])` | Right-click on empty canvas at graph-space position |
| `NodeMenu(NodeId)` | Right-click on a node |
| `DroppedWireOut(OutPinId, [f32; 2])` | Wire dropped on canvas from output pin |
| `DroppedWireIn(InPinId, [f32; 2])` | Wire dropped on canvas from input pin |
| `DeleteSelected` | Delete key pressed with selection |
| `SelectAll` | Ctrl+A pressed (handled internally) |

## Configuration

All `NodeGraphConfig` fields with their defaults:

| Field | Default | Description |
|-------|---------|-------------|
| `show_grid` | `true` | Draw canvas grid |
| `grid_size` | `32.0` | Grid cell size (px at zoom 1.0) |
| `grid_thick_every` | `8` | Draw thick grid lines every N cells |
| `grid_rotation` | `0.0` | Grid rotation angle in degrees |
| `snap_to_grid` | `false` | Snap node positions to grid |
| `snap_size` | `16.0` | Snap granularity (px) |
| `node_rounding` | `6.0` | Node corner rounding |
| `node_border_thickness` | `1.0` | Node border line thickness |
| `node_header_height` | `26.0` | Node header bar height |
| `node_padding_h` | `12.0` | Horizontal padding inside node |
| `node_padding_v` | `6.0` | Vertical padding inside node |
| `node_min_width` | `120.0` | Minimum node width |
| `node_body_height` | `24.0` | Default body height for body nodes |
| `node_collapsible` | `true` | Show collapse/expand button in header |
| `node_shadow` | `true` | Drop shadow behind nodes |
| `node_shadow_offset` | `4.0` | Shadow offset (px, down-right) |
| `node_shadow_alpha` | `80` | Shadow alpha (0–255) |
| `pin_radius` | `5.0` | Pin circle radius |
| `pin_spacing` | `22.0` | Vertical spacing between pins |
| `pin_offset` | `0.0` | Horizontal offset of pin from node edge |
| `pin_hit_radius` | `8.0` | Hit-test radius (easier clicking) |
| `show_wires` | `true` | Draw wires between nodes |
| `wire_style` | `Bezier` | `Bezier` / `Line` / `Orthogonal` |
| `wire_thickness` | `2.0` | Wire line thickness |
| `wire_hover_distance` | `6.0` | Wire hover hit distance (px, scales with zoom) |
| `wire_curvature` | `0.5` | Bezier tangent length (fraction of Δx) |
| `wire_layer` | `BehindNodes` | `BehindNodes` / `AboveNodes` |
| `wire_yanking` | `true` | Ctrl+Click on wire to detach and redirect |
| `wire_flow` | `false` | Animate directional dots along wires |
| `wire_flow_speed` | `60.0` | Flow dot speed (px/s) |
| `wire_flow_spacing` | `20.0` | Flow dot spacing (px) |
| `drop_wire_menu` | `true` | Drop wire on canvas fires `DroppedWire` action |
| `zoom_min` | `0.25` | Minimum zoom level |
| `zoom_max` | `1.5` | Maximum zoom level |
| `zoom_speed` | `0.1` | Zoom step per scroll tick |
| `zoom_with_wheel` | `true` | Scroll wheel zooms |
| `smooth_zoom` | `true` | Animate zoom transitions |
| `smooth_zoom_speed` | `8.0` | Zoom interpolation speed |
| `pan_button_middle` | `true` | Middle mouse button pans |
| `pan_button_right` | `true` | Right mouse drag on empty canvas pans |
| `pan_shift_lmb` | `false` | Shift+LMB pans |
| `multi_select` | `true` | Ctrl+Click adds to selection |
| `rect_select` | `true` | Drag rectangle to select |
| `canvas_context_menu` | `true` | Right-click on empty canvas opens context menu |
| `node_context_menu` | `true` | Right-click on node opens context menu |
| `node_double_click` | `true` | Double-click fires `NodeDoubleClicked` |
| `keyboard_delete` | `true` | Delete key fires `DeleteSelected` |
| `keyboard_select_all` | `true` | Ctrl+A selects all nodes |
| `keyboard_escape_cancel` | `true` | Escape cancels wire drag |
| `tooltip_delay` | `0.5` | Seconds before hover tooltip appears |
| `lod_hide_labels_zoom` | `0.4` | Hide pin/node labels below this zoom |
| `lod_simplify_pins_zoom` | `0.3` | Simplify pin shapes below this zoom |
| `lod_hide_body_zoom` | `0.35` | Hide node bodies below this zoom |
| `show_stats_overlay` | `false` | Canvas stats overlay (nodes, wires, zoom) |
| `stats_overlay_corner` | `1` | 0=TL, 1=TR, 2=BL, 3=BR |
| `stats_overlay_margin` | `8.0` | Stats overlay edge margin (px) |
| `show_minimap` | `false` | Mini-map overlay |
| `minimap_size` | `[160, 100]` | Mini-map dimensions (px) |
| `minimap_corner` | `3` | 0=TL, 1=TR, 2=BL, 3=BR |
| `minimap_margin` | `8.0` | Mini-map edge margin (px) |
| `minimap_interactive` | `true` | Click/drag mini-map to pan camera |
| `colors` | default | `NgColors` palette |

### NgColors

All color fields are `[u8; 3]` RGB in 0–255 range. Alpha applied per-use.

| Field | Element |
|-------|---------|
| `canvas_bg` | Canvas background |
| `grid_line` | Normal grid lines |
| `grid_line_thick` | Thick (major) grid lines |
| `node_bg` | Node body background |
| `node_bg_hovered` | Hovered node background |
| `node_bg_selected` | Selected node background |
| `node_header_bg` | Node header bar background |
| `node_border` | Node border |
| `node_border_selected` | Selected node border (accent) |
| `text` | Primary text |
| `text_muted` | Pin labels, secondary text |
| `pin_default` | Default pin fill |
| `pin_hovered` | Hovered pin fill |
| `wire_default` | Default wire color |
| `wire_hovered` | Hovered wire |
| `wire_dragging` | Wire being dragged |
| `selection_rect` | Selection rectangle outline |
| `selection_rect_fill` | Selection rectangle fill (with alpha) |
| `minimap_bg` | Mini-map background |
| `minimap_outline` | Mini-map border |
| `minimap_node` | Node dots in mini-map |
| `minimap_viewport` | Viewport rect in mini-map |
| `collapse_btn` | Collapse button icon |
| `collapse_btn_hovered` | Hovered collapse button icon |

## Architecture

```
node_graph/
  mod.rs      NodeGraph<T> struct, public API, convenience methods
  graph.rs    Graph<T> — slab-based storage (O(1) insert/remove) + HashSet<Wire>
  viewer.rs   NodeGraphViewer<T> trait — user-implemented callbacks
  config.rs   NodeGraphConfig, NgColors — all tunables
  state.rs    InteractionState — viewport, selection, drag, pin positions
  render/
    mod.rs      Main render entry point, orchestrates sub-modules
    grid.rs     Canvas grid rendering with rotation support
    nodes.rs    Node frame, pin, and body rendering
    wires.rs    Wire routing, rendering, and flow animation
    math.rs     Geometry: bezier, orthogonal routing, obstacle avoidance, hit testing
    input.rs    Mouse/keyboard input handling with wire hit testing
    overlays.rs Stats overlay and interactive mini-map
  types.rs    NodeId, InPinId, OutPinId, Wire, PinInfo, PinShape, GraphAction
```

### Data Structure

- **Nodes**: slab (Vec + free-list) for O(1) insert/remove by `NodeId`
- **Wires**: `HashSet<Wire>` for O(1) connect/disconnect/lookup
- **Pin positions**: `HashMap<PinId, [f32; 2]>` rebuilt each frame for O(1) lookup
- **Draw order**: `Vec<NodeId>` + `HashSet<NodeId>` for O(1) membership check
- **Selection**: `HashSet<NodeId>` for O(1) select/deselect/query
- **Frustum culling**: viewport bounds computed in graph space each frame; off-screen nodes skipped entirely
- **Obstacle-aware wire routing**: per-frame AABB collection (`collect_node_aabbs`) shared by rendering and hit testing — wire paths match their hit zones exactly
- **Shared wire geometry**: `ortho_wire_points()` and `obstacle_aware_bezier_cps()` are used by both `draw_wire_smart()` and `wire_hit_test()` — single source of truth
