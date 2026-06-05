//! Visual node graph editor for Dear ImGui.
//!
//! Port of [egui-snarl](https://github.com/zakarumych/egui-snarl) concepts
//! to Dear ImGui with full optimization: zero per-frame allocations,
//! native `ImDrawList` bezier curves, index-based hit testing.
//!
//! # Architecture
//!
//! - [`Graph<T>`] — framework-agnostic data: nodes (slab) + wires (hash set)
//! - [`NodeGraphViewer<T>`] — user trait: title, pins, body, connection rules
//! - [`NodeGraph<T>`] — the widget: owns graph + state, renders via `ImDrawList`
//! - [`NodeGraphConfig`] — colors, sizes, behavior toggles
//!
//! # Quick Start
//!
//! ```ignore
//! use dear_imgui_custom_mod::node_graph::*;
//!
//! // 1. Define your node type
//! enum MyNode { Add, Multiply, Output(f32) }
//!
//! // 2. Implement the viewer trait
//! struct MyViewer;
//! impl NodeGraphViewer<MyNode> for MyViewer {
//!     fn title(&self, node: &MyNode) -> &str { /* ... */ }
//!     fn inputs(&self, node: &MyNode) -> u8 { /* ... */ }
//!     fn outputs(&self, node: &MyNode) -> u8 { /* ... */ }
//! }
//!
//! // 3. Create and render
//! let mut ng: NodeGraph<MyNode> = NodeGraph::new("my_graph");
//! ng.graph.insert_node(MyNode::Add, [100.0, 100.0]);
//!
//! // In render loop:
//! for action in ng.render(&ui, &MyViewer) {
//!     match action {
//!         GraphAction::Connected(wire) => { ng.graph.connect(wire.out_pin, wire.in_pin); }
//!         GraphAction::Disconnected(wire) => { ng.graph.disconnect(wire.out_pin, wire.in_pin); }
//!         _ => {}
//!     }
//! }
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
pub mod graph;
mod render;
pub mod state;
pub mod types;
pub mod viewer;

pub use config::{NgColors, NodeGraphConfig};
pub use graph::{Graph, Node};
pub use state::InteractionState;
pub use types::*;
pub use viewer::NodeGraphViewer;

use dear_imgui_rs::Ui;

// ─── NodeGraph widget ────────────────────────────────────────────────────────

/// Visual node graph editor widget.
///
/// Owns the graph data, interaction state, and configuration.
/// Call [`render`](Self::render) each frame inside an ImGui window.
pub struct NodeGraph<T> {
    /// ImGui ID for this widget instance.
    pub imgui_id: String,
    /// The underlying graph data (nodes + wires).
    pub graph: Graph<T>,
    /// All interaction state (viewport, selection, drag, etc.).
    pub state: InteractionState,
    /// Visual and behavioral configuration.
    pub config: NodeGraphConfig,
}

impl<T> NodeGraph<T> {
    /// Create a new empty node graph with the given ImGui ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            imgui_id: id.into(),
            graph: Graph::new(),
            state: InteractionState::default(),
            config: NodeGraphConfig::default(),
        }
    }

    /// Create with a custom configuration.
    pub fn with_config(id: impl Into<String>, config: NodeGraphConfig) -> Self {
        Self {
            imgui_id: id.into(),
            graph: Graph::new(),
            state: InteractionState::default(),
            config,
        }
    }

    /// Render the node graph, filling the available content region.
    ///
    /// Returns a list of [`GraphAction`]s describing user interactions this frame.
    /// The caller is responsible for applying mutations (connect/disconnect)
    /// to the graph. Internal actions (`NodeToggled`, `SelectAll`) are handled
    /// automatically before returning.
    pub fn render(&mut self, ui: &Ui, viewer: &dyn NodeGraphViewer<T>) -> Vec<GraphAction> {
        // Apply pending node drags (must mutate graph positions)
        self.apply_node_drags(ui);

        // Apply pending comment move/resize (must mutate graph comments).
        // Returns the changed comment index if it actually moved this frame.
        let comment_changed = self.apply_comment_drag(ui);

        let avail = ui.content_region_avail();

        // Use cursor_screen_pos() — NOT window_pos() + cursor_pos() — because
        // cursor_pos() includes window Scroll offset, which would shift canvas_pos
        // by the scroll amount and break all graph-to-screen coordinate transforms.
        let canvas_pos: [f32; 2] = ui.cursor_screen_pos();
        let canvas_size = avail;

        // Invisible button captures all mouse input on the canvas area
        ui.invisible_button(&self.imgui_id, canvas_size);
        let canvas_hovered = ui.is_item_hovered();

        // Palette drop: a host drag-drop payload landed on the canvas
        // invisible-button. This MUST run immediately after the invisible
        // button is submitted (so it is the "last item"); render_graph below
        // submits node bodies, which would otherwise become the drop target and
        // swallow drops onto empty canvas area when nodes are present.
        //
        // `screen_to_graph` reads `viewport.canvas_origin`. render_graph sets it
        // (to `canvas_pos`) but only after this point, so set it here too —
        // render_graph re-assigns the same value, making this idempotent.
        self.state.viewport.canvas_origin = canvas_pos;
        let mut actions: Vec<GraphAction> = Vec::new();
        if let Some(target) = ui.drag_drop_target() {
            if let Some(Ok(pod)) = target
                .accept_payload::<u32, _>("NODE_GRAPH_DND", dear_imgui_rs::DragDropFlags::empty())
                && pod.delivery
            {
                let gp = self.state.viewport.screen_to_graph(ui.mouse_pos());
                actions.push(GraphAction::PaletteDropped(pod.data, gp));
            }
            target.pop();
        }

        actions.extend(render::render_graph(
            &mut self.graph,
            &mut self.state,
            &self.config,
            viewer,
            ui,
            canvas_pos,
            canvas_size,
            canvas_hovered,
        ));

        // Surface comment move/resize applied above as a frame action.
        if let Some(index) = comment_changed {
            actions.push(GraphAction::CommentChanged(index));
        }

        // Handle internal actions before returning to the caller
        for action in &actions {
            match *action {
                GraphAction::NodeToggled(id) => {
                    if let Some(node) = self.graph.get_node_mut(id) {
                        node.open = !node.open;
                    }
                }
                GraphAction::SelectAll => {
                    self.state.selected = self.graph.node_ids().into_iter().collect();
                }
                _ => {}
            }
        }

        actions
    }

    /// Apply pending node drag movements to graph positions.
    ///
    /// Computes delta from the *snapped* position to avoid drift on
    /// secondary selected nodes when snap-to-grid is enabled.
    fn apply_node_drags(&mut self, ui: &Ui) {
        if let Some(ref drag) = self.state.node_drag {
            let mouse = ui.io().mouse_pos();
            let nid = drag.node;
            let offset = drag.offset;

            // Compute new graph-space position for the primary node
            let new_screen = [mouse[0] - offset[0], mouse[1] - offset[1]];
            let new_graph = self.state.viewport.screen_to_graph(new_screen);

            if let Some(node) = self.graph.get_node_mut(nid) {
                let old_pos = node.pos;

                // Snap the primary node
                let snapped = if self.config.snap_to_grid {
                    let s = self.config.snap_size;
                    [
                        (new_graph[0] / s).round() * s,
                        (new_graph[1] / s).round() * s,
                    ]
                } else {
                    new_graph
                };
                node.pos = snapped;

                // Delta from *snapped* positions — prevents multi-select drift
                let delta = [snapped[0] - old_pos[0], snapped[1] - old_pos[1]];

                // Move other selected nodes by the same snapped delta
                let selected: Vec<NodeId> = self
                    .state
                    .selected
                    .iter()
                    .copied()
                    .filter(|&id| id != nid)
                    .collect();
                for sel_id in selected {
                    if let Some(sel_node) = self.graph.get_node_mut(sel_id) {
                        sel_node.pos[0] += delta[0];
                        sel_node.pos[1] += delta[1];
                    }
                }
            }
        }
    }

    /// Apply a pending comment move/resize to the graph.
    ///
    /// Returns `Some(index)` if the comment actually changed this frame, so the
    /// caller can surface a [`GraphAction::CommentChanged`]. Mirrors the node
    /// drag mechanics: the title bar moves the comment, the bottom-right handle
    /// resizes it. Moves snap to the grid when `config.snap_to_grid` is set.
    fn apply_comment_drag(&mut self, ui: &Ui) -> Option<usize> {
        use render::comments::{MIN_H, MIN_W};
        use state::CommentDragKind;

        let drag = self.state.comment_drag.as_ref()?;
        let index = drag.index;
        let kind = drag.kind;
        let offset = drag.offset;

        // Defensive: the host may have removed/reordered comments (via
        // `comments_mut()` / `remove_comment`) between frames while a drag was
        // live. The stored index would then point at a *different* comment — or
        // out of range. Cancel the gesture instead of silently moving the wrong
        // box. `get_mut(index)?` below also guards the bound; this makes the
        // intent explicit and clears the dangling drag state.
        if index >= self.graph.comments().len() {
            self.state.comment_drag = None;
            return None;
        }

        let mouse = ui.io().mouse_pos();
        let vp = &self.state.viewport;
        // Target graph-space point: mouse minus the grab offset, both in screen.
        let target_screen = [mouse[0] - offset[0], mouse[1] - offset[1]];
        let target_graph = vp.screen_to_graph(target_screen);

        let snap = |v: f32| -> f32 {
            if self.config.snap_to_grid {
                let s = self.config.snap_size;
                (v / s).round() * s
            } else {
                v
            }
        };

        let comment = self.graph.comments_mut().get_mut(index)?;
        let mut changed = false;
        match kind {
            CommentDragKind::Move => {
                let new_pos = [snap(target_graph[0]), snap(target_graph[1])];
                if new_pos != comment.pos {
                    comment.pos = new_pos;
                    changed = true;
                }
            }
            CommentDragKind::Resize => {
                // target_graph is the new bottom-right corner.
                let new_w = snap((target_graph[0] - comment.pos[0]).max(MIN_W)).max(MIN_W);
                let new_h = snap((target_graph[1] - comment.pos[1]).max(MIN_H)).max(MIN_H);
                let new_size = [new_w, new_h];
                if new_size != comment.size {
                    comment.size = new_size;
                    changed = true;
                }
            }
        }

        if changed && let Some(d) = self.state.comment_drag.as_mut() {
            d.moved = true;
        }

        changed.then_some(index)
    }

    // ── Convenience methods ──────────────────────────────────────────────

    /// Add a node at the given graph-space position.
    pub fn add_node(&mut self, value: T, pos: [f32; 2]) -> NodeId {
        let id = self.graph.insert_node(value, pos);
        self.state.ensure_in_draw_order(id);
        id
    }

    /// Remove a node by ID. Returns the user payload.
    pub fn remove_node(&mut self, id: NodeId) -> Option<T> {
        self.state.remove_from_draw_order(id);
        self.state.selected.remove(&id);
        self.graph.remove_node(id)
    }

    /// Connect two pins. Returns `true` if the wire is new.
    pub fn connect(&mut self, from: OutPinId, to: InPinId) -> bool {
        self.graph.connect(from, to)
    }

    /// Disconnect two pins. Returns `true` if the wire existed.
    pub fn disconnect(&mut self, from: OutPinId, to: InPinId) -> bool {
        self.graph.disconnect(from, to)
    }

    /// Currently selected node IDs.
    #[must_use]
    pub fn selected(&self) -> Vec<NodeId> {
        self.state.selected.iter().copied().collect()
    }

    /// Center the viewport on all nodes, using actual node sizes from the viewer.
    pub fn fit_to_content(&mut self, canvas_size: [f32; 2], viewer: &dyn NodeGraphViewer<T>) {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for (_, node) in self.graph.nodes() {
            let w = viewer
                .node_width(&node.value)
                .unwrap_or(self.config.node_min_width);
            let h = self.config.node_height(
                viewer.inputs(&node.value),
                viewer.outputs(&node.value),
                viewer.has_body(&node.value),
                node.open,
                viewer.body_height(&node.value),
            );
            min_x = min_x.min(node.pos[0]);
            min_y = min_y.min(node.pos[1]);
            max_x = max_x.max(node.pos[0] + w);
            max_y = max_y.max(node.pos[1] + h);
        }

        if min_x >= max_x || min_y >= max_y {
            return;
        }

        let pad = 50.0;
        let graph_w = max_x - min_x + pad * 2.0;
        let graph_h = max_y - min_y + pad * 2.0;
        let zoom = (canvas_size[0] / graph_w)
            .min(canvas_size[1] / graph_h)
            .min(self.config.zoom_max);

        self.state.viewport.zoom = zoom;
        self.state.zoom_target = zoom;
        self.state.viewport.offset[0] = canvas_size[0] * 0.5 - f32::midpoint(min_x, max_x) * zoom;
        self.state.viewport.offset[1] = canvas_size[1] * 0.5 - f32::midpoint(min_y, max_y) * zoom;
    }

    /// Reset viewport to default (zoom 1.0, centered at origin).
    ///
    /// Also cancels any in-progress interactive state — node drag,
    /// new-wire drag, rectangle select. Without this guard a viewport
    /// reset mid-drag would leave dangling interaction state with
    /// stale screen-space coordinates, producing visual glitches
    /// (rect-select rectangle pinned at the wrong location, wire
    /// preview from the previous mouse position) until the user
    /// clicks again.
    pub fn reset_viewport(&mut self) {
        self.state.node_drag = None;
        self.state.comment_drag = None;
        self.state.new_wire = None;
        self.state.rect_select = None;
        self.state.viewport.zoom = 1.0;
        self.state.zoom_target = 1.0;
        self.state.viewport.offset = [0.0, 0.0];
    }
}
