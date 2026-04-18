//! Visual style types for the knowledge-graph widget.
//!
//! These types are the *data layer* style — they describe what a node or edge
//! should look like and carry user metadata. Actual rendering (draw-list
//! commands) lives in the `render` sub-module.

use super::data::NodeId;

/// Per-node visual and metadata style.
///
/// Build with the provided builder methods for a fluent API:
///
/// ```rust,ignore
/// let style = NodeStyle::new("My Node")
///     .with_tag("core")
///     .with_color([0.4, 0.8, 0.4, 1.0])
///     .with_radius(12.0);
/// ```
#[derive(Debug, Clone)]
pub struct NodeStyle {
    /// Human-readable label shown below (or inside) the node circle.
    pub label: String,

    /// Zero or more semantic tags used for filtering and colour-by-tag mode.
    ///
    /// Using a `Vec` with a small inline capacity for the common case of
    /// 0–4 tags per node.
    pub tags: Vec<&'static str>,

    /// Override node radius in canvas units. `None` → use the physics-derived
    /// radius (based on degree when `radius_by_degree` is on, otherwise
    /// [`crate::knowledge_graph::config::ForceConfig::radius_base`]).
    pub radius: Option<f32>,

    /// RGBA override colour for the node fill. `None` → theme default or
    /// colour-mode-computed colour.
    pub color: Option<[f32; 4]>,

    /// Fixed canvas-space position. When `Some`, the physics simulation treats
    /// this node as pinned and does not update its position. `None` → free.
    pub anchor: Option<[f32; 2]>,

    /// Creation timestamp used by the time-travel slider.
    ///
    /// Nodes with `created_at > filter.time_threshold` are hidden. Defaults
    /// to `f32::INFINITY` so nodes without a timestamp are always visible.
    pub created_at: f32,

    /// Arbitrary 64-bit user data — not interpreted by the viewer.
    ///
    /// Useful for mapping back to domain objects without a secondary
    /// `HashMap`.
    pub user_data: u64,
}

impl NodeStyle {
    /// Create a new [`NodeStyle`] with the given label and all other fields at
    /// their defaults (no override, always visible).
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tags: Vec::new(),
            radius: None,
            color: None,
            anchor: None,
            created_at: f32::INFINITY,
            user_data: 0,
        }
    }

    /// Add a semantic tag to this node.
    #[must_use]
    pub fn with_tag(mut self, tag: &'static str) -> Self {
        self.tags.push(tag);
        self
    }

    /// Override the node's rendered radius (canvas units).
    #[must_use]
    pub fn with_radius(mut self, r: f32) -> Self {
        self.radius = Some(r);
        self
    }

    /// Override the node's fill colour (linear RGBA, each component 0.0–1.0).
    #[must_use]
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = Some(color);
        self
    }

    /// Pin the node at a fixed canvas-space position.
    ///
    /// The physics simulation will not move a pinned node.
    #[must_use]
    pub fn with_anchor(mut self, pos: [f32; 2]) -> Self {
        self.anchor = Some(pos);
        self
    }

    /// Set the creation timestamp for time-travel filtering.
    #[must_use]
    pub fn with_timestamp(mut self, t: f32) -> Self {
        self.created_at = t;
        self
    }

    /// Attach arbitrary user data to the node.
    #[must_use]
    pub fn with_user_data(mut self, data: u64) -> Self {
        self.user_data = data;
        self
    }
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self::new("")
    }
}

// ─── Edge style ──────────────────────────────────────────────────────────────

/// Per-edge visual style (colours, dash pattern, optional label).
///
/// Edge *geometry* (endpoints, direction, weight) lives in [`Edge`].
#[derive(Debug, Clone)]
pub struct EdgeStyle {
    /// RGBA override colour for the edge line. `None` → theme default
    /// (`GraphColors::edge_default` or `edge_highlight` when incident on a
    /// hovered/selected node).
    pub color: Option<[f32; 4]>,

    /// When `true` the edge is rendered as a dashed line.
    pub dashed: bool,

    /// Optional text label rendered at the midpoint of the edge.
    pub label: Option<String>,

    /// Creation timestamp — same semantics as [`NodeStyle::created_at`].
    ///
    /// Edges with `created_at > filter.time_threshold` are hidden.
    pub created_at: f32,
}

impl EdgeStyle {
    /// Create a plain, solid, unlabelled edge style.
    pub fn new() -> Self {
        Self {
            color: None,
            dashed: false,
            label: None,
            created_at: f32::INFINITY,
        }
    }

    /// Override the edge colour.
    #[must_use]
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = Some(color);
        self
    }

    /// Render this edge as a dashed line.
    #[must_use]
    pub fn dashed(mut self) -> Self {
        self.dashed = true;
        self
    }

    /// Attach a text label to this edge.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the creation timestamp for time-travel filtering.
    #[must_use]
    pub fn with_timestamp(mut self, t: f32) -> Self {
        self.created_at = t;
        self
    }
}

impl Default for EdgeStyle {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Edge record ─────────────────────────────────────────────────────────────

/// A graph edge, stored inside [`crate::knowledge_graph::data::GraphData`].
///
/// Edges are created through
/// [`crate::knowledge_graph::data::GraphData::add_edge`] and keyed by
/// [`EdgeId`] for stable, O(1) access.
#[derive(Debug, Clone)]
pub struct Edge {
    /// Source node.
    pub from: NodeId,
    /// Target node.
    pub to: NodeId,
    /// When `true`, the edge is rendered with an arrow at [`Self::to`].
    pub directed: bool,
    /// Normalised edge weight in `[0.0, 1.0]`.
    ///
    /// Controls both line thickness and the spring-force coefficient in the
    /// physics simulation. `1.0` is the maximum attraction; `0.0` means the
    /// edge has no physics effect (still rendered).
    pub weight: f32,
    /// Visual style overrides.
    pub style: EdgeStyle,
}

// ─── Graph colour palette ─────────────────────────────────────────────────────

/// Knowledge-graph colour sub-palette.
///
/// Follows the same per-theme palette pattern as
/// [`crate::borderless_window::TitlebarColors`] and
/// [`crate::nav_panel::NavColors`]: a plain struct of RGBA `[f32; 4]` colours
/// that the theme modules can fill in and that the viewer uses at render time.
///
/// Obtain the default values with `GraphColors::default()` (dark-mode
/// values), or supply a fully custom palette via
/// [`crate::knowledge_graph::config::ViewerConfig::colors_override`].
#[derive(Debug, Clone)]
pub struct GraphColors {
    /// Canvas background fill colour.
    pub background: [f32; 4],
    /// Subtle grid-line colour drawn over the background.
    pub grid_line: [f32; 4],
    /// Default node fill (no hover, not selected).
    pub node_default: [f32; 4],
    /// Node fill when the cursor is inside the node's hit area.
    pub node_hover: [f32; 4],
    /// Node fill when the node is part of the current selection.
    pub node_selected: [f32; 4],
    /// Node circle outline (stroke) colour.
    pub node_outline: [f32; 4],
    /// Default edge/line colour.
    pub edge_default: [f32; 4],
    /// Edge colour when incident on a hovered or selected node.
    pub edge_highlight: [f32; 4],
    /// Node label text colour.
    pub label_text: [f32; 4],
    /// Box-selection rectangle fill colour (semi-transparent).
    pub selection_fill: [f32; 4],
    /// Box-selection rectangle outline colour.
    pub selection_outline: [f32; 4],
}

impl Default for GraphColors {
    /// Dark-mode defaults (NxT palette, consistent with `theme::dark`).
    fn default() -> Self {
        Self {
            background:       [0.11, 0.12, 0.15, 1.00],
            grid_line:        [0.20, 0.22, 0.27, 0.50],
            node_default:     [0.28, 0.46, 0.68, 1.00],
            node_hover:       [0.38, 0.60, 0.85, 1.00],
            node_selected:    [0.55, 0.80, 1.00, 1.00],
            node_outline:     [0.18, 0.20, 0.26, 1.00],
            edge_default:     [0.35, 0.40, 0.48, 0.80],
            edge_highlight:   [0.55, 0.70, 0.90, 1.00],
            label_text:       [0.88, 0.90, 0.92, 1.00],
            selection_fill:   [0.35, 0.55, 0.80, 0.15],
            selection_outline:[0.35, 0.55, 0.80, 0.70],
        }
    }
}
