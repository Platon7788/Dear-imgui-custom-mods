//! Filter state for the knowledge-graph sidebar.
//!
//! [`FilterState`] is embedded inside [`crate::knowledge_graph::GraphViewer`]
//! and exposed read-only via its public field. Callers may mutate the state
//! directly or through the built-in sidebar UI — both paths set the
//! [`crate::knowledge_graph::event::GraphEvent::FilterChanged`] event.

/// Sidebar filter state — controls which nodes and edges are visible.
///
/// All fields are `pub` so the host application can pre-populate them before
/// handing the viewer to the render loop, or read them after a
/// [`crate::knowledge_graph::event::GraphEvent::FilterChanged`] event.
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
    /// Edges whose [`crate::knowledge_graph::style::Edge::weight`] is strictly
    /// less than this value are hidden. `0.0` (the default) shows all edges.
    pub min_edge_weight: f32,
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
}
