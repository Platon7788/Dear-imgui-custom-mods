//! Filter state for the knowledge-graph sidebar.
//!
//! [`FilterState`] is embedded inside [`crate::force_graph::GraphViewer`]
//! and exposed read-only via its public field. Callers may mutate the state
//! directly or through the built-in sidebar UI — both paths set the
//! [`crate::force_graph::event::GraphEvent::FilterChanged`] event.

use super::data::NodeId;

/// Sidebar filter state — controls which nodes and edges are visible.
///
/// All fields are `pub` so the host application can pre-populate them before
/// handing the viewer to the render loop, or read them after a
/// [`crate::force_graph::event::GraphEvent::FilterChanged`] event.
///
/// A filter is a *narrowing* operation: a node is shown when it passes
/// *every* active filter (tags, search query, time threshold, and optionally
/// the distance filter). An edge is hidden when either endpoint is hidden.
#[derive(Debug, Clone)]
pub struct FilterState {
    /// Tags that are currently enabled.
    ///
    /// An empty vec means *all* tags are enabled (no tag filtering). When
    /// non-empty, a node is visible only if at least one of its tags appears
    /// in this list.
    pub enabled_tags: Vec<&'static str>,

    /// Case-insensitive substring search applied to node labels.
    ///
    /// An empty string means no text filter is active.
    pub search_query: String,

    /// Time-travel threshold: nodes with `created_at > time_threshold` are
    /// hidden.
    ///
    /// `f32::INFINITY` (the default) disables the threshold and shows all
    /// nodes regardless of their `created_at` value.
    pub time_threshold: f32,

    /// When `true`, only nodes within [`Self::distance_hops`] graph hops from
    /// any selected node are visible.
    pub distance_filter: bool,

    /// Maximum number of graph hops from the selection when
    /// [`Self::distance_filter`] is active.
    pub distance_hops: u32,

    /// Minimum edge weight threshold.
    ///
    /// Edges whose [`crate::force_graph::style::Edge::weight`] is strictly
    /// less than this value are hidden. `0.0` (the default) shows all edges.
    pub min_edge_weight: f32,

    // ── Depth / focus filter ──────────────────────────────────────────────────
    /// Hop depth limit from `focused_node`. `None` = show all nodes.
    /// When `Some(n)`, only nodes reachable within n hops from `focused_node`
    /// are visible (BFS through undirected adjacency).
    pub depth: Option<u32>,

    /// The node from which depth is measured. When `None`, depth filter is
    /// disabled regardless of [`Self::depth`].
    pub focused_node: Option<NodeId>,

    // ── Degree filter ─────────────────────────────────────────────────────────
    /// Hide nodes whose edge count (degree) is strictly less than this value.
    /// `0` = no filtering (default).
    pub min_degree: u32,

    // ── Node type visibility ──────────────────────────────────────────────────
    /// Show nodes with no edges (degree = 0). Default `true`.
    pub show_orphans: bool,

    /// Hide [`super::style::NodeKind::Unresolved`] nodes. Default `false`.
    pub hide_unresolved: bool,

    /// Hide [`super::style::NodeKind::Tag`] nodes. Default `false`.
    pub hide_tags: bool,

    /// Hide [`super::style::NodeKind::Attachment`] nodes. Default `false`.
    pub hide_attachments: bool,

    // ── Search options ────────────────────────────────────────────────────────
    /// When `true`, the `search_query` is also matched against node tags.
    /// Default `true`.
    pub search_match_tags: bool,

    /// When `true`, `search_query` is interpreted as a regex pattern.
    /// Default `false`.
    pub search_use_regex: bool,
}

impl FilterState {
    /// Create a new [`FilterState`] with all filters at their identity values
    /// (no filtering applied).
    pub fn new() -> Self {
        Self {
            enabled_tags: Vec::new(),
            search_query: String::new(),
            time_threshold: f32::INFINITY,
            distance_filter: false,
            distance_hops: 2,
            min_edge_weight: 0.0,
            depth: None,
            focused_node: None,
            min_degree: 0,
            show_orphans: true,
            hide_unresolved: false,
            hide_tags: false,
            hide_attachments: false,
            search_match_tags: true,
            search_use_regex: false,
        }
    }

    /// Reset all fields back to their identity (no-filter) defaults.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Returns `true` when no filter is active, i.e. every node and edge
    /// would pass unchanged.
    ///
    /// Useful for skipping the per-node filter pass when it would be a no-op.
    pub fn is_identity(&self) -> bool {
        self.search_query.is_empty()
            && self.enabled_tags.is_empty()
            && self.time_threshold.is_infinite()
            && self.time_threshold.is_sign_positive()
            && !self.distance_filter
            && self.min_edge_weight == 0.0
            && self.depth.is_none()
            && self.focused_node.is_none()
            && self.min_degree == 0
            && self.show_orphans
            && !self.hide_unresolved
            && !self.hide_tags
            && !self.hide_attachments
            && self.search_match_tags
            && !self.search_use_regex
    }
}

impl Default for FilterState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_identity() {
        assert!(FilterState::new().is_identity());
    }

    #[test]
    fn reset_restores_identity() {
        let mut f = FilterState::new();
        f.search_query = "rust".to_string();
        f.min_edge_weight = 0.5;
        f.reset();
        assert!(f.is_identity());
    }

    #[test]
    fn non_empty_search_is_not_identity() {
        let mut f = FilterState::new();
        f.search_query = "hello".to_string();
        assert!(!f.is_identity());
    }

    #[test]
    fn enabled_tags_breaks_identity() {
        let mut f = FilterState::new();
        f.enabled_tags.push("core");
        assert!(!f.is_identity());
    }

    #[test]
    fn finite_time_threshold_breaks_identity() {
        let mut f = FilterState::new();
        f.time_threshold = 100.0;
        assert!(!f.is_identity());
    }

    #[test]
    fn distance_filter_breaks_identity() {
        let mut f = FilterState::new();
        f.distance_filter = true;
        assert!(!f.is_identity());
    }

    #[test]
    fn depth_filter_breaks_identity() {
        let mut f = FilterState::new();
        f.depth = Some(2);
        assert!(!f.is_identity());
    }

    #[test]
    fn min_degree_filter_breaks_identity() {
        let mut f = FilterState::new();
        f.min_degree = 1;
        assert!(!f.is_identity());
    }

    #[test]
    fn hide_tags_breaks_identity() {
        let mut f = FilterState::new();
        f.hide_tags = true;
        assert!(!f.is_identity());
    }

    #[test]
    fn reset_clears_new_fields() {
        let mut f = FilterState::new();
        f.depth = Some(3);
        f.min_degree = 2;
        f.hide_tags = true;
        f.focused_node = None; // NodeId not constructible in tests, skip
        f.reset();
        assert!(f.depth.is_none());
        assert_eq!(f.min_degree, 0);
        assert!(!f.hide_tags);
        assert!(f.is_identity());
    }
}
