//! Viewer configuration for the knowledge-graph widget.
//!
//! [`ViewerConfig`] is the top-level configuration bundle. It is cheap to
//! clone (the only heap allocation is the optional boxed [`GraphColors`]
//! override and the label-visibility variant). Build one at startup and hand
//! it to [`crate::force_graph::GraphViewer`].
//!
//! [`ForceConfig`] controls Barnes-Hut force-directed physics separately so
//! callers can tweak the simulation without touching visual settings.

use crate::theme::Theme;
use super::style::GraphColors;

/// Heap-allocated custom node-colour function for [`ColorMode::Custom`].
pub type NodeColorFn =
    Box<dyn Fn(&super::style::NodeStyle, &super::data::GraphData) -> [f32; 4] + Send + Sync>;

// ─── Color group query ───────────────────────────────────────────────────────

/// A pattern used to match nodes for a [`ColorGroup`].
#[derive(Debug, Clone)]
pub enum ColorGroupQuery {
    /// Case-insensitive substring match on the node label.
    Label(String),
    /// Exact match on one of the node's tags (without the `#` prefix).
    Tag(String),
    /// Match by [`super::style::NodeKind`] variant name: `"regular"`, `"tag"`,
    /// `"attachment"`, `"unresolved"`, `"cluster"`, or `"custom"`.
    Kind(String),
    /// Match nodes whose label matches this regex pattern (string stored; compiled lazily by caller).
    Regex(String),
    /// Matches every node (useful as a catch-all / default group).
    All,
}

// ─── Color group ─────────────────────────────────────────────────────────────

/// A named color override applied to all nodes matching a query.
///
/// Groups are evaluated in order; the first matching group wins. Non-matching
/// nodes fall back to the viewer's [`ViewerConfig::color_mode`].
#[derive(Debug, Clone)]
pub struct ColorGroup {
    /// Display name shown in the sidebar group list.
    pub name: String,
    /// Pattern that determines which nodes this group colors.
    pub query: ColorGroupQuery,
    /// RGBA fill color applied to matching nodes.
    pub color: [f32; 4],
    /// When `false`, this group is skipped during color resolution.
    pub enabled: bool,
}

impl ColorGroup {
    /// Create a new enabled color group.
    pub fn new(name: impl Into<String>, query: ColorGroupQuery, color: [f32; 4]) -> Self {
        Self { name: name.into(), query, color, enabled: true }
    }
}

// ─── Color mode ─────────────────────────────────────────────────────────────

/// Determines how node fill colours are computed each frame.
///
/// The five static variants are cheaply comparable; [`ColorMode::Custom`]
/// carries a heap-allocated function and is intentionally not `Clone` — wrap
/// it in an `Arc` if shared ownership is needed.
pub enum ColorMode {
    /// All nodes use the theme's `node_default` colour (or their
    /// [`super::style::NodeStyle::color`] override, if set).
    Static,
    /// Node colour is derived from its first tag using a stable palette.
    ByTag,
    /// Node colour reflects its detected community (Louvain algorithm).
    ///
    /// Requires the metrics pass to have run; falls back to [`Self::Static`]
    /// before the first metrics computation.
    ByCommunity,
    /// Node colour encodes its normalised PageRank score on a gradient from
    /// `node_default` (low) to `node_selected` (high).
    ByPageRank,
    /// Node colour encodes its normalised betweenness centrality on the same
    /// gradient as [`Self::ByPageRank`].
    ByBetweenness,
    /// Fully custom colour function called once per visible node per frame.
    ///
    /// The closure receives the node's style and the full [`super::data::GraphData`]
    /// reference and must return a linear RGBA `[f32; 4]`.
    ///
    /// `Send + Sync` is required so `ViewerConfig` can safely be sent across
    /// threads when rendering on a worker thread.
    Custom(NodeColorFn),
}

impl std::fmt::Debug for ColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static => write!(f, "ColorMode::Static"),
            Self::ByTag => write!(f, "ColorMode::ByTag"),
            Self::ByCommunity => write!(f, "ColorMode::ByCommunity"),
            Self::ByPageRank => write!(f, "ColorMode::ByPageRank"),
            Self::ByBetweenness => write!(f, "ColorMode::ByBetweenness"),
            Self::Custom(_) => write!(f, "ColorMode::Custom(<fn>)"),
        }
    }
}

// ─── Label visibility ────────────────────────────────────────────────────────

/// Controls when node labels are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelVisibility {
    /// Labels are always drawn, regardless of zoom level or hover state.
    Always,
    /// Labels appear only when the cursor is inside the node's hit area.
    HoverOnly,
    /// Labels are drawn only for nodes whose rendered radius (after zoom) is
    /// at least `ViewerConfig::min_label_zoom` pixels — prevents text clutter
    /// at low zoom levels.
    BySize,
    /// No labels are ever drawn.
    Never,
}

// ─── Selection mode ───────────────────────────────────────────────────────────

/// Controls how mouse clicks and drag gestures select nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Clicking a node selects only that node; clicking empty space clears.
    Single,
    /// Dragging an empty area draws a selection rectangle; nodes inside it
    /// replace the current selection.
    Box,
    /// Like [`Self::Box`], but holding `Shift` adds to the selection rather
    /// than replacing it.
    Additive,
}

// ─── Sidebar kind ────────────────────────────────────────────────────────────

/// Determines whether and how the built-in sidebar is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarKind {
    /// No sidebar is rendered; the viewer uses the full panel width.
    None,
    /// Built-in sidebar is rendered and fully expanded.
    Built,
    /// Built-in sidebar is rendered but starts collapsed; user can expand it.
    BuiltCollapsed,
}

// ─── Time-travel slider ───────────────────────────────────────────────────────

/// Configuration for the time-travel slider in the sidebar.
///
/// When present on [`ViewerConfig::time_travel`], the sidebar shows a slider
/// that controls [`crate::force_graph::filter::FilterState::time_threshold`].
#[derive(Debug, Clone)]
pub struct TimeTravelSlider {
    /// Minimum slider value (earliest timestamp to show).
    pub min: f32,
    /// Maximum slider value (latest timestamp in the graph).
    pub max: f32,
    /// Slider step granularity.
    pub step: f32,
}

// ─── Force config ─────────────────────────────────────────────────────────────

/// Physics simulation parameters for the Barnes-Hut force-directed layout.
///
/// The defaults are tuned for graphs in the 100–5 000 node range. For very
/// large or very sparse graphs, adjusting [`Self::repulsion`] and
/// [`Self::attraction`] gives the most visible results.
#[derive(Debug, Clone)]
pub struct ForceConfig {
    /// Barnes-Hut approximation threshold (θ). Higher = faster but less
    /// accurate repulsion. Range: 0.0 (exact) – 2.0 (very coarse).
    pub barnes_hut_theta: f32,
    /// Coulomb-style repulsion strength between all node pairs.
    pub repulsion: f32,
    /// Spring-like attraction along edges (multiplied by edge weight).
    pub attraction: f32,
    /// Weak pull of every node toward the canvas origin — keeps disconnected
    /// components from drifting off-screen.
    pub center_pull: f32,
    /// Minimum inter-node distance below which the collision correction kicks
    /// in, preventing node overlap.
    pub collision_radius: f32,
    /// Target spring rest length in canvas units. Edges pull nodes toward this
    /// distance. Previously hardcoded to `80.0`.
    pub link_distance: f32,
    /// Velocity decay factor applied each tick (0 = instant stop, 1 = no
    /// damping). Typical values: 0.4–0.8.
    pub velocity_decay: f32,
    /// Additional downward gravity. `0.0` disables gravity (the default).
    pub gravity_strength: f32,
    /// When `true`, node radius grows with its degree:
    /// `radius = radius_base + degree * radius_per_degree`.
    pub radius_by_degree: bool,
    /// Base node radius when `radius_by_degree` is enabled (canvas units).
    pub radius_base: f32,
    /// Extra radius added per incident edge when `radius_by_degree` is
    /// enabled.
    pub radius_per_degree: f32,
}

impl Default for ForceConfig {
    fn default() -> Self {
        Self {
            barnes_hut_theta:  0.9,
            repulsion:         120.0,
            attraction:        0.04,
            center_pull:       0.002,
            collision_radius:  20.0,
            link_distance:     80.0,
            velocity_decay:    0.6,
            gravity_strength:  0.0,
            radius_by_degree:  true,
            radius_base:       4.0,
            radius_per_degree: 1.5,
        }
    }
}

// ─── Viewer config ────────────────────────────────────────────────────────────

/// Top-level configuration for [`crate::force_graph::GraphViewer`].
///
/// Construct with `ViewerConfig::default()` and override individual fields:
///
/// ```rust,ignore
/// let config = ViewerConfig {
///     show_labels: LabelVisibility::Always,
///     minimap: true,
///     ..ViewerConfig::default()
/// };
/// ```
pub struct ViewerConfig {
    /// Active application theme — used to derive the built-in colour palette
    /// when [`Self::colors_override`] is `None`.
    pub theme: Theme,

    /// Full colour palette override. When `Some`, completely replaces the
    /// theme-derived palette; when `None`, the viewer calls
    /// `Theme::graph_colors()` (or falls back to `GraphColors::default()`).
    pub colors_override: Option<Box<GraphColors>>,

    /// When and how node labels are drawn.
    pub show_labels: LabelVisibility,

    /// Minimum canvas-space zoom level below which labels are hidden in
    /// [`LabelVisibility::BySize`] mode.
    pub min_label_zoom: f32,

    /// When `true`, edge labels (from [`super::style::EdgeStyle::label`]) are
    /// rendered at the midpoint of each edge.
    pub show_edge_labels: bool,

    /// When `true`, directed edges are drawn with an arrowhead at the target
    /// node.
    pub edge_arrow: bool,

    /// Enable edge bundling to reduce visual clutter for dense graphs.
    /// Bundling is applied as a post-layout pass and does not affect physics.
    pub edge_bundling: bool,

    /// Draw a subtle dot-grid background on the canvas.
    pub background_grid: bool,

    /// Show a minimap in the corner of the canvas.
    pub minimap: bool,

    /// Selection behaviour on click and drag.
    pub selection_mode: SelectionMode,

    /// Node count threshold above which the viewer switches to a lower level
    /// of detail (hides labels, simplifies edges) to maintain framerate.
    pub lod_threshold: usize,

    /// When `Some`, a time-travel slider is shown in the sidebar.
    pub time_travel: Option<TimeTravelSlider>,

    /// How node fill colours are computed each frame.
    pub color_mode: ColorMode,

    // ── Interaction ───────────────────────────────────────────────────────────

    /// When `true`, nodes can be dragged with the mouse.
    pub drag_enabled: bool,
    /// When `true`, a right-click context menu is shown on nodes.
    pub context_menu_enabled: bool,
    /// When `true`, dragging a node automatically pins it so the physics
    /// simulation does not pull it back after release. Default `true`.
    pub pin_on_drag: bool,

    // ── Hover appearance ─────────────────────────────────────────────────────

    /// Opacity applied to nodes/edges that are NOT the hovered node or its
    /// direct neighbours. `0.0` = fully hidden, `1.0` = no fade. Default `0.15`.
    pub hover_fade_opacity: f32,
    /// When `true`, hovered and selected nodes display a soft glow halo.
    pub glow_on_hover: bool,

    // ── Label control ─────────────────────────────────────────────────────────

    /// Controls label visibility relative to zoom. `0.0` = default (labels
    /// appear as you zoom in). Negative = labels visible even when zoomed out.
    /// Positive = labels only visible when significantly zoomed in.
    /// Range: `-5.0` – `5.0`.
    pub text_fade_threshold: f32,

    // ── Node size ────────────────────────────────────────────────────────────

    /// Global multiplier applied to all node radii after per-node calculation.
    /// `1.0` = no change. Default `1.0`.
    pub node_size_multiplier: f32,

    // ── Edge style ────────────────────────────────────────────────────────────

    /// Global multiplier applied to all edge line thicknesses. Default `1.0`.
    pub edge_thickness_multiplier: f32,
    /// Bézier curve amount. `0.0` = straight lines. `1.0` = fully curved.
    /// Default `0.0`.
    pub edge_curve: f32,

    // ── Color groups ─────────────────────────────────────────────────────────

    /// Ordered list of color groups. The first matching group determines a
    /// node's fill color; non-matching nodes use [`Self::color_mode`].
    pub color_groups: Vec<ColorGroup>,

    // ── Visibility filters ────────────────────────────────────────────────────

    /// Show nodes with no edges (degree = 0). Default `true`.
    pub show_orphans: bool,
    /// Show [`super::style::NodeKind::Unresolved`] ghost nodes. Default `true`.
    pub show_unresolved: bool,
    /// Show [`super::style::NodeKind::Tag`] nodes. Default `true`.
    pub show_tags: bool,

    // ── Layout ────────────────────────────────────────────────────────────────

    /// Directional gravity vector applied in addition to center pull.
    /// `[0.0, 0.0]` = disabled. Example: `[0.0, 0.002]` gives a downward pull
    /// useful for hierarchical / call-graph layouts.
    pub gravity_direction: [f32; 2],
    /// Padding (canvas units) used when fitting the graph to screen.
    pub fit_padding: f32,

    // ── Depth filter ─────────────────────────────────────────────────────────

    /// When `true`, nodes beyond [`Self::depth_fade`] hops from the focused
    /// node are rendered more transparent (alpha multiplied by distance).
    pub depth_fade: bool,

    // ── Cluster visualisation (Phase C prep) ─────────────────────────────────

    /// Draw a convex-hull outline around nodes in the same Louvain community.
    /// Requires the metrics pass to have run. Phase C.
    pub cluster_hulls: bool,

    /// When `true`, an active `search_query` *dims* non-matching nodes
    /// (alpha × 0.15) rather than hiding them. Default `true`.
    pub search_highlight_mode: bool,
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            theme:                     Theme::Dark,
            colors_override:           None,
            show_labels:               LabelVisibility::HoverOnly,
            min_label_zoom:            0.6,
            show_edge_labels:          false,
            edge_arrow:                true,
            edge_bundling:             false,
            background_grid:           true,
            minimap:                   false,
            selection_mode:            SelectionMode::Additive,
            lod_threshold:             5000,
            time_travel:               None,
            color_mode:                ColorMode::Static,
            drag_enabled:              true,
            context_menu_enabled:      true,
            pin_on_drag:               true,
            hover_fade_opacity:        0.15,
            glow_on_hover:             true,
            text_fade_threshold:       0.0,
            node_size_multiplier:      1.0,
            edge_thickness_multiplier: 1.0,
            edge_curve:                0.0,
            color_groups:              Vec::new(),
            show_orphans:              true,
            show_unresolved:           true,
            show_tags:                 true,
            gravity_direction:         [0.0, 0.0],
            fit_padding:               40.0,
            depth_fade:                false,
            cluster_hulls:             false,
            search_highlight_mode:     true,
        }
    }
}

impl std::fmt::Debug for ViewerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewerConfig")
            .field("theme", &self.theme)
            .field("colors_override", &self.colors_override.is_some())
            .field("show_labels", &self.show_labels)
            .field("min_label_zoom", &self.min_label_zoom)
            .field("show_edge_labels", &self.show_edge_labels)
            .field("edge_arrow", &self.edge_arrow)
            .field("edge_bundling", &self.edge_bundling)
            .field("background_grid", &self.background_grid)
            .field("minimap", &self.minimap)
            .field("selection_mode", &self.selection_mode)
            .field("lod_threshold", &self.lod_threshold)
            .field("time_travel", &self.time_travel)
            .field("color_mode", &self.color_mode)
            .field("drag_enabled", &self.drag_enabled)
            .field("context_menu_enabled", &self.context_menu_enabled)
            .field("pin_on_drag", &self.pin_on_drag)
            .field("hover_fade_opacity", &self.hover_fade_opacity)
            .field("glow_on_hover", &self.glow_on_hover)
            .field("text_fade_threshold", &self.text_fade_threshold)
            .field("node_size_multiplier", &self.node_size_multiplier)
            .field("edge_thickness_multiplier", &self.edge_thickness_multiplier)
            .field("edge_curve", &self.edge_curve)
            .field("color_groups_count", &self.color_groups.len())
            .field("show_orphans", &self.show_orphans)
            .field("show_unresolved", &self.show_unresolved)
            .field("show_tags", &self.show_tags)
            .field("gravity_direction", &self.gravity_direction)
            .field("fit_padding", &self.fit_padding)
            .field("depth_fade", &self.depth_fade)
            .field("cluster_hulls", &self.cluster_hulls)
            .field("search_highlight_mode", &self.search_highlight_mode)
            .finish()
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_force_config_values_from_spec() {
        let f = ForceConfig::default();
        assert!((f.repulsion - 120.0).abs() < f32::EPSILON);
        assert!((f.attraction - 0.04).abs() < f32::EPSILON);
        assert!((f.center_pull - 0.002).abs() < f32::EPSILON);
        assert!((f.velocity_decay - 0.6).abs() < f32::EPSILON);
        assert!((f.barnes_hut_theta - 0.9).abs() < f32::EPSILON);
        assert!((f.collision_radius - 20.0).abs() < f32::EPSILON);
        assert!((f.link_distance - 80.0).abs() < f32::EPSILON);
        assert!(f.radius_by_degree);
        assert!((f.radius_base - 4.0).abs() < f32::EPSILON);
        assert!((f.radius_per_degree - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn viewer_config_builder_chain() {
        let c = ViewerConfig {
            show_labels: LabelVisibility::Always,
            ..ViewerConfig::default()
        };
        assert!(matches!(c.show_labels, LabelVisibility::Always));
        assert_eq!(c.lod_threshold, 5000);
        assert!(!c.minimap);
        assert!(c.background_grid);
        assert!(c.show_orphans);
        assert!(!c.cluster_hulls);
        assert!((c.node_size_multiplier - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn viewer_config_default_theme_is_dark() {
        let c = ViewerConfig::default();
        assert_eq!(c.theme, Theme::Dark);
    }

    #[test]
    fn force_config_clone() {
        let f = ForceConfig::default();
        let g = f.clone();
        assert!((f.repulsion - g.repulsion).abs() < f32::EPSILON);
    }

    #[test]
    fn color_mode_debug_does_not_panic() {
        let _ = format!("{:?}", ColorMode::Static);
        let _ = format!("{:?}", ColorMode::ByTag);
        let _ = format!("{:?}", ColorMode::ByCommunity);
        let _ = format!("{:?}", ColorMode::ByPageRank);
        let _ = format!("{:?}", ColorMode::ByBetweenness);
        let _ = format!("{:?}", ColorMode::Custom(Box::new(|_, _| [1.0; 4])));
    }
}
