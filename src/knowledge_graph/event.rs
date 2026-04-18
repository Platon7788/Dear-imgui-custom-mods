//! Graph events emitted by [`crate::knowledge_graph::GraphViewer::render`].

use std::collections::HashSet;

use super::data::NodeId;

/// A public-facing event emitted by the knowledge-graph viewer each frame.
///
/// Events are returned in a `Vec<GraphEvent>` from
/// [`crate::knowledge_graph::GraphViewer::render`]. Callers should drain this
/// vec after every frame and react to relevant variants.
#[derive(Debug, Clone)]
#[must_use]
pub enum GraphEvent {
    /// A node was left-clicked.
    NodeClicked(NodeId),
    /// A node was double-clicked (two left-clicks within the platform
    /// double-click interval on the same node).
    NodeDoubleClicked(NodeId),
    /// The cursor entered a node's hit area (radius + small padding).
    NodeHovered(NodeId),
    /// A node was right-clicked.
    ///
    /// The inner `[f32; 2]` is the cursor position in screen space at the
    /// moment of the click — suitable for positioning a context-menu popup.
    NodeContextMenu(NodeId, [f32; 2]),
    /// The current selection set changed.
    ///
    /// Contains the *full* new set of selected [`NodeId`]s. An empty set
    /// means all nodes were deselected.
    SelectionChanged(HashSet<NodeId>),
    /// The sidebar filter state changed (search query, tag toggles, distance
    /// filter, edge-weight threshold, or time-travel slider).
    FilterChanged,
    /// The camera pan or zoom changed.
    ///
    /// Fired at most once per frame, only when the viewport actually moved or
    /// scaled. Callers that store a separate "visible region" should
    /// recalculate it on this event.
    CameraChanged,
}
