//! Drag-and-drop state for node reparenting.
//!
//! Uses Dear ImGui's drag-and-drop API with a custom payload type
//! identifier `"VTREE_NODE"`.

/// ImGui drag-drop payload type identifier for tree nodes.
pub(crate) const DRAG_DROP_TYPE: &str = "VTREE_NODE";
