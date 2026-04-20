//! Graph events emitted by [`crate::knowledge_graph::GraphViewer::render`].

use std::collections::HashSet;

use super::data::NodeId;

/// A public-facing event emitted by the knowledge-graph viewer each frame.
///
/// Events are returned in a `Vec<GraphEvent>` from
/// [`crate::knowledge_graph::GraphViewer::render`]. Callers should drain this
/// vec after every frame and react to relevant variants.
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
    /// Node was activated (double-click or Enter key) — callers should open/navigate to it.
    NodeActivated(NodeId),
    /// Node was dragged and released at a new world-space position.
    NodeMoved(NodeId, [f32; 2]),
    /// Node pin state changed (true = now pinned, false = unpinned).
    NodePinned(NodeId, bool),
    /// User pressed Delete/Backspace with nodes selected.
    SelectionDeleteRequested(HashSet<NodeId>),
    /// Fit-to-screen was triggered (button or 'F' key).
    FitToScreen,
    /// The search query in the sidebar changed.
    SearchChanged(String),
    /// A color group was added, removed, or modified.
    GroupChanged,
    /// Physics simulation paused/resumed (true = now paused).
    SimulationToggled(bool),
}

impl std::fmt::Debug for GraphEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeClicked(id) => write!(f, "GraphEvent::NodeClicked({id:?})"),
            Self::NodeDoubleClicked(id) => write!(f, "GraphEvent::NodeDoubleClicked({id:?})"),
            Self::NodeHovered(id) => write!(f, "GraphEvent::NodeHovered({id:?})"),
            Self::NodeContextMenu(id, pos) => {
                write!(f, "GraphEvent::NodeContextMenu({id:?}, {pos:?})")
            }
            Self::SelectionChanged(set) => {
                write!(f, "GraphEvent::SelectionChanged({} nodes)", set.len())
            }
            Self::FilterChanged => write!(f, "GraphEvent::FilterChanged"),
            Self::CameraChanged => write!(f, "GraphEvent::CameraChanged"),
            Self::NodeActivated(id) => write!(f, "GraphEvent::NodeActivated({id:?})"),
            Self::NodeMoved(id, pos) => write!(f, "GraphEvent::NodeMoved({id:?}, {pos:?})"),
            Self::NodePinned(id, pinned) => {
                write!(f, "GraphEvent::NodePinned({id:?}, {pinned})")
            }
            Self::SelectionDeleteRequested(set) => {
                write!(
                    f,
                    "GraphEvent::SelectionDeleteRequested({} nodes)",
                    set.len()
                )
            }
            Self::FitToScreen => write!(f, "GraphEvent::FitToScreen"),
            Self::SearchChanged(q) => write!(f, "GraphEvent::SearchChanged({q:?})"),
            Self::GroupChanged => write!(f, "GraphEvent::GroupChanged"),
            Self::SimulationToggled(paused) => {
                write!(f, "GraphEvent::SimulationToggled({paused})")
            }
        }
    }
}
