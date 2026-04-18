//! Force-directed graph viewer for Dear ImGui.
//!
//! An Obsidian-style graph widget with Barnes-Hut physics, pan/zoom/select,
//! a built-in sidebar (search, tag filter, time-travel), graph metrics
//! (PageRank / betweenness centrality / Louvain community detection), and
//! SVG/DOT/Mermaid export (Phase D).
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::force_graph::{
//!     data::{GraphData, NodeId},
//!     style::{NodeStyle, EdgeStyle},
//!     config::{ViewerConfig, ForceConfig},
//!     GraphViewer,
//! };
//!
//! // Build the graph.
//! let mut graph = GraphData::new();
//! let a = graph.add_node(NodeStyle::new("Alpha").with_tag("core"));
//! let b = graph.add_node(NodeStyle::new("Beta").with_tag("core"));
//! graph.add_edge(a, b, EdgeStyle::new(), 1.0, false);
//!
//! // Create the viewer once at startup.
//! let mut viewer = GraphViewer::new("my_kg");
//!
//! // Inside the ImGui render loop:
//! // for event in viewer.render(&ui, &mut graph) { ... }
//! ```
//!
//! # Architecture
//!
//! | Module | Role |
//! |--------|------|
//! | [`data`] | Graph model — nodes, edges, adjacency (SlotMap backend) |
//! | [`style`] | Visual styles: [`style::NodeStyle`], [`style::EdgeStyle`], [`style::Edge`], [`style::GraphColors`] |
//! | [`filter`] | Sidebar filter state |
//! | [`config`] | Viewer + physics configuration, [`config::ColorMode`] |
//! | [`event`] | Events emitted per frame by the viewer |
//! | `sim` | Physics simulation (Barnes-Hut in Phase B) |
//! | `render` | ImDrawList rendering pipeline |
//! | `layout` | Initial node placement strategies |
//! | `metrics` | PageRank / Louvain / betweenness (Phase C) |

pub mod config;
pub mod data;
pub mod event;
pub mod filter;
pub mod style;

pub(crate) mod layout;
pub(crate) mod metrics;
pub(crate) mod render;
pub(crate) mod sidebar;
pub(crate) mod sim;

use std::collections::HashSet;

use dear_imgui_rs::Ui;

use config::{ForceConfig, SidebarKind, ViewerConfig};
use data::{GraphData, NodeId};
use event::GraphEvent;
use filter::FilterState;
use render::{camera::Camera, RenderCtx};
use sim::Simulation;
// ─── GraphViewer ─────────────────────────────────────────────────────────────

/// Force-directed knowledge-graph viewer widget.
///
/// Owns all per-viewer runtime state: camera, physics simulation, selection,
/// hover state, and sidebar configuration. The data lives in [`GraphData`],
/// which is passed mutably to [`render`](Self::render) each frame.
///
/// # Lifetime
///
/// Create one `GraphViewer` per distinct graph view at startup and keep it
/// alive as long as the view is shown. The simulation state persists across
/// frames so nodes settle naturally.
pub struct GraphViewer {
    /// ImGui ID used for the invisible-button hit area. Must be unique per window.
    id: String,
    /// Visual and behavioral configuration.
    pub config: ViewerConfig,
    /// Physics parameters (separate from visual config for easy tuning).
    pub force_config: ForceConfig,
    /// Whether and how the built-in sidebar is shown.
    pub sidebar: SidebarKind,

    // ── Private runtime state ──────────────────────────────────────────────
    sim: Simulation,
    camera: Camera,
    filter: FilterState,
    selection: HashSet<NodeId>,
    hovered: Option<NodeId>,

    // Interaction state.
    dragging_node: Option<NodeId>,
    drag_world_offset: [f32; 2],
    box_select_start: Option<[f32; 2]>,
    ctx_menu_node: Option<NodeId>,
    /// Node to smoothly pan to on the next render frame.
    pending_focus: Option<NodeId>,

    // ── Hover-neighbor cache ──────────────────────────────────────────────────
    // Rebuilt only when `hovered` changes — avoids per-frame HashSet allocation.
    hover_neighbors: HashSet<NodeId>,
    last_hovered: Option<NodeId>,
}

impl GraphViewer {
    /// Create a new viewer with the given ImGui widget ID and default settings.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            config: ViewerConfig::default(),
            force_config: ForceConfig::default(),
            sidebar: SidebarKind::None,
            sim: Simulation::new(),
            camera: Camera::new(),
            filter: FilterState::new(),
            selection: HashSet::new(),
            hovered: None,
            dragging_node: None,
            drag_world_offset: [0.0; 2],
            box_select_start: None,
            ctx_menu_node: None,
            pending_focus: None,
            hover_neighbors: HashSet::new(),
            last_hovered: None,
        }
    }

    /// Override the viewer configuration (builder pattern).
    #[must_use]
    pub fn with_config(mut self, c: ViewerConfig) -> Self {
        self.config = c;
        self
    }

    /// Override the force / physics configuration (builder pattern).
    #[must_use]
    pub fn with_force_config(mut self, f: ForceConfig) -> Self {
        self.force_config = f;
        self
    }

    /// Set the sidebar display mode (builder pattern).
    #[must_use]
    pub fn with_sidebar(mut self, kind: SidebarKind) -> Self {
        self.sidebar = kind;
        self
    }

    /// Render the graph for one ImGui frame.
    ///
    /// Must be called inside an ImGui window or child window. Uses all
    /// available content region space.
    ///
    /// Returns a `Vec` of [`GraphEvent`]s describing user interactions this
    /// frame. Callers should process events rather than polling state.
    #[must_use = "graph events (click / select / filter change) are emitted here"]
    pub fn render(&mut self, ui: &Ui, graph: &mut GraphData) -> Vec<GraphEvent> {
        // Resolve pending focus now that we know canvas size.
        if let Some(id) = self.pending_focus.take()
            && let Some(node) = graph.nodes.get(id) {
                let sidebar_w = match &self.sidebar {
                    SidebarKind::None => 0.0_f32,
                    _ => 220.0_f32,
                };
                let avail = ui.content_region_avail();
                let canvas_size = [avail[0] - sidebar_w, avail[1].max(100.0)];
                self.camera.animate_to_node(node.pos, canvas_size, self.camera.zoom);
        }

        let mut events = {
            let mut ctx = RenderCtx {
                camera: &mut self.camera,
                sim: &mut self.sim,
                selection: &mut self.selection,
                hovered: &mut self.hovered,
                filter: &mut self.filter,
                dragging_node: &mut self.dragging_node,
                drag_world_offset: &mut self.drag_world_offset,
                box_select_start: &mut self.box_select_start,
                ctx_menu_node: &mut self.ctx_menu_node,
                hover_neighbors: &mut self.hover_neighbors,
                last_hovered: &mut self.last_hovered,
            };
            render::render(
                ui,
                graph,
                &self.config,
                &self.force_config,
                &mut ctx,
                &self.id,
                &self.sidebar,
            )
            // ctx + borrowed fields released here
            // ctx dropped here — releases mutable borrows of self fields.
        };

        sidebar::render_sidebar(
            ui,
            graph,
            &mut self.config,
            &mut self.force_config,
            &mut self.filter,
            &mut events,
            &self.sidebar,
        );

        // Post-process events that need GraphViewer state.  Sidebar and keyboard
        // handlers emit these without access to sim/camera, so we fix them here.
        let mut sim_toggle_done = false;
        for ev in events.iter_mut() {
            match ev {
                GraphEvent::SimulationToggled(_) => {
                    if !sim_toggle_done {
                        sim_toggle_done = true;
                        self.sim.asleep = !self.sim.asleep;
                        if !self.sim.asleep {
                            self.sim.wake();
                        }
                        *ev = GraphEvent::SimulationToggled(self.sim.asleep);
                    }
                }
                GraphEvent::ResetLayout => {
                    self.reset_layout(graph);
                }
                _ => {}
            }
        }

        events
    }

    // ── Imperative controls ───────────────────────────────────────────────────

    /// Pan the camera smoothly so `id` is centred on screen.
    ///
    /// Has no effect if `id` is not a valid node handle. The pan is resolved
    /// on the next [`render`](Self::render) call once canvas size is known.
    pub fn focus(&mut self, id: NodeId) {
        self.pending_focus = Some(id);
    }

    /// Add `id` to the selection set (replaces existing selection).
    pub fn select(&mut self, id: NodeId) {
        self.selection.clear();
        self.selection.insert(id);
    }

    /// Add multiple nodes to the selection set.
    pub fn select_multi(&mut self, ids: &[NodeId]) {
        self.selection.extend(ids.iter().copied());
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Scatter all non-anchored nodes back to initial spiral positions and
    /// re-wake the simulation.
    pub fn reset_layout(&mut self, graph: &mut GraphData) {
        layout::scatter_positions(graph);
        self.sim.wake();
    }

    /// Pause or resume the physics simulation.
    pub fn freeze(&mut self, frozen: bool) {
        if frozen {
            self.sim.asleep = true;
        } else {
            self.sim.wake();
        }
    }

    /// Returns a copy of the current camera state (pan offset + zoom).
    pub fn camera(&self) -> Camera {
        self.camera
    }

    /// Replace the camera state (e.g. to restore a saved view).
    pub fn set_camera(&mut self, c: Camera) {
        self.camera = c;
    }

    /// Returns the current set of selected node IDs.
    pub fn selection(&self) -> &HashSet<NodeId> {
        &self.selection
    }

    /// Returns the node currently under the cursor, if any.
    pub fn hovered(&self) -> Option<NodeId> {
        self.hovered
    }

    /// Returns the current filter state.
    pub fn filter(&self) -> &FilterState {
        &self.filter
    }

    /// Mutable access to the filter state (for programmatic filter updates).
    pub fn filter_mut(&mut self) -> &mut FilterState {
        &mut self.filter
    }
}
