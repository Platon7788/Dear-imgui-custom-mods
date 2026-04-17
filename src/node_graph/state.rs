//! Interaction state for the node graph editor.
//!
//! Viewport transform (pan + zoom), node drag state, wire drag state,
//! selection state, rectangle selection.

use std::collections::HashMap;

use super::types::{InPinId, NodeId, OutPinId};

// ─── Viewport ────────────────────────────────────────────────────────────────

/// 2D viewport transform: pan offset + uniform zoom + canvas origin.
///
/// `offset` is the pan translation in *canvas-local* pixels.
/// `canvas_origin` is the top-left corner of the canvas in screen space
/// (set each frame before rendering).
///
/// **Screen position** = `graph_pos * zoom + offset + canvas_origin`
pub struct Viewport {
    /// Pan translation in canvas-local pixels.
    pub offset: [f32; 2],
    /// Zoom level (1.0 = default).
    pub zoom: f32,
    /// Top-left corner of the canvas in screen space. Set each frame.
    pub(crate) canvas_origin: [f32; 2],
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            zoom: 1.0,
            canvas_origin: [0.0, 0.0],
        }
    }
}

impl Viewport {
    /// Convert graph-space position to screen-space position.
    #[inline]
    pub fn graph_to_screen(&self, pos: [f32; 2]) -> [f32; 2] {
        [
            pos[0] * self.zoom + self.offset[0] + self.canvas_origin[0],
            pos[1] * self.zoom + self.offset[1] + self.canvas_origin[1],
        ]
    }

    /// Convert screen-space position to graph-space position.
    ///
    /// Returns `[0, 0]` if zoom is zero (should never happen with valid config).
    #[inline]
    pub fn screen_to_graph(&self, pos: [f32; 2]) -> [f32; 2] {
        if self.zoom <= 0.0 {
            return [0.0, 0.0];
        }
        [
            (pos[0] - self.offset[0] - self.canvas_origin[0]) / self.zoom,
            (pos[1] - self.offset[1] - self.canvas_origin[1]) / self.zoom,
        ]
    }

    /// Scale a distance from graph space to screen space.
    #[inline]
    pub fn scale(&self, v: f32) -> f32 {
        v * self.zoom
    }
}

// ─── Wire drag ───────────────────────────────────────────────────────────────

/// In-progress wire being dragged from a pin.
#[derive(Debug, Clone)]
pub enum NewWire {
    /// Dragging from an output pin (looking for an input).
    FromOutput(OutPinId),
    /// Dragging from an input pin (looking for an output).
    FromInput(InPinId),
}

// ─── Node drag ───────────────────────────────────────────────────────────────

/// State for dragging nodes.
pub(crate) struct NodeDrag {
    /// Node being dragged (the "primary" — selected nodes follow).
    pub node: NodeId,
    /// Offset from mouse to node origin at drag start.
    pub offset: [f32; 2],
    /// Whether the drag has actually moved (vs. just a click).
    pub moved: bool,
}

// ─── Rectangle selection ─────────────────────────────────────────────────────

/// State for rectangle / marquee selection.
pub(crate) struct RectSelect {
    /// Starting point in screen space.
    pub start: [f32; 2],
    /// Current end point in screen space.
    pub end: [f32; 2],
}

impl RectSelect {
    /// Normalized rectangle: `[min_x, min_y, max_x, max_y]` in screen space.
    pub(crate) fn rect(&self) -> [f32; 4] {
        [
            self.start[0].min(self.end[0]),
            self.start[1].min(self.end[1]),
            self.start[0].max(self.end[0]),
            self.start[1].max(self.end[1]),
        ]
    }
}

// ─── Hovered element ─────────────────────────────────────────────────────────

/// What element the mouse is currently hovering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HoveredElement {
    None,
    Node(NodeId),
    InputPin(InPinId),
    OutputPin(OutPinId),
    Wire(OutPinId, InPinId),
}

// ─── Composite interaction state ─────────────────────────────────────────────

/// All interaction state for one `NodeGraph` instance.
pub struct InteractionState {
    /// Viewport pan + zoom.
    pub viewport: Viewport,
    /// Currently selected node IDs.
    pub(crate) selected: std::collections::HashSet<NodeId>,
    /// Draw order (last = on top). Authoritative ordering for rendering.
    pub(crate) draw_order: Vec<NodeId>,
    /// Fast membership check for draw_order (avoids O(n) `contains`).
    pub(crate) draw_order_set: std::collections::HashSet<NodeId>,
    /// Node currently being dragged.
    pub(crate) node_drag: Option<NodeDrag>,
    /// Wire currently being dragged from a pin.
    pub(crate) new_wire: Option<NewWire>,
    /// Active rectangle selection.
    pub(crate) rect_select: Option<RectSelect>,
    /// What the mouse is hovering this frame.
    pub(crate) hovered: HoveredElement,
    /// Screen-space pin positions — O(1) lookup via HashMap.
    /// Rebuilt each frame during node rendering.
    pub(crate) input_pin_pos: HashMap<InPinId, [f32; 2]>,
    pub(crate) output_pin_pos: HashMap<OutPinId, [f32; 2]>,
    /// Minimap drag state.
    pub(crate) minimap_dragging: bool,

    // ── Smooth zoom ──
    /// Target zoom level (for smooth interpolation).
    pub(crate) zoom_target: f32,

    // ── Tooltip delay ──
    /// Time the current element has been hovered (seconds).
    pub(crate) hover_time: f32,
    /// Previous frame's hovered element (to detect hover changes).
    pub(crate) prev_hovered: HoveredElement,

    // ── Scratch buffers (reused each frame, zero alloc) ──
    /// Visible node IDs after frustum culling.
    pub(crate) scratch_visible: Vec<NodeId>,
    /// Snapshot of draw_order for immutable iteration.
    pub(crate) scratch_draw_order: Vec<NodeId>,
    /// Format buffer for stats overlay.
    pub(crate) fmt_buf: String,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            selected: std::collections::HashSet::with_capacity(16),
            draw_order: Vec::with_capacity(32),
            draw_order_set: std::collections::HashSet::with_capacity(32),
            node_drag: None,
            new_wire: None,
            rect_select: None,
            hovered: HoveredElement::None,
            input_pin_pos: HashMap::with_capacity(64),
            output_pin_pos: HashMap::with_capacity(64),
            minimap_dragging: false,
            zoom_target: 1.0,
            hover_time: 0.0,
            prev_hovered: HoveredElement::None,
            scratch_visible: Vec::with_capacity(64),
            scratch_draw_order: Vec::with_capacity(64),
            fmt_buf: String::with_capacity(128),
        }
    }
}

impl InteractionState {
    /// Bring a node to front of the draw order. O(n) find + O(1) swap_remove + push.
    pub(crate) fn node_to_top(&mut self, id: NodeId) {
        if let Some(pos) = self.draw_order.iter().position(|&n| n == id) {
            self.draw_order.swap_remove(pos);
        }
        self.draw_order.push(id);
    }

    /// Ensure a node is in the draw order (appended if missing). O(1) check.
    pub(crate) fn ensure_in_draw_order(&mut self, id: NodeId) {
        if self.draw_order_set.insert(id) {
            self.draw_order.push(id);
        }
    }

    /// Remove a node from the draw order.
    pub(crate) fn remove_from_draw_order(&mut self, id: NodeId) {
        if self.draw_order_set.remove(&id) {
            self.draw_order.retain(|&n| n != id);
        }
    }

    /// Is a node selected?
    #[inline]
    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selected.contains(&id)
    }

    /// Select a single node (deselects all others unless `add` is true).
    pub fn select_node(&mut self, id: NodeId, add: bool) {
        if !add {
            self.selected.clear();
        }
        self.selected.insert(id);
    }

    /// Toggle selection for a node.
    pub(crate) fn toggle_select(&mut self, id: NodeId) {
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
    }

    /// Deselect all nodes.
    pub fn deselect_all(&mut self) {
        self.selected.clear();
    }

    /// Look up a screen-space position for an input pin. O(1).
    #[inline]
    pub(crate) fn find_input_pos(&self, pin: InPinId) -> Option<[f32; 2]> {
        self.input_pin_pos.get(&pin).copied()
    }

    /// Look up a screen-space position for an output pin. O(1).
    #[inline]
    pub(crate) fn find_output_pos(&self, pin: OutPinId) -> Option<[f32; 2]> {
        self.output_pin_pos.get(&pin).copied()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Viewport ─────────────────────────────────────────────────────────

    #[test]
    fn viewport_identity() {
        let vp = Viewport::default();
        let s = vp.graph_to_screen([100.0, 200.0]);
        assert_eq!(s, [100.0, 200.0]);
    }

    #[test]
    fn viewport_with_offset() {
        let vp = Viewport {
            offset: [50.0, 30.0],
            ..Viewport::default()
        };
        let s = vp.graph_to_screen([10.0, 20.0]);
        assert_eq!(s, [60.0, 50.0]);
    }

    #[test]
    fn viewport_with_zoom() {
        let vp = Viewport {
            zoom: 2.0,
            ..Viewport::default()
        };
        let s = vp.graph_to_screen([10.0, 20.0]);
        assert_eq!(s, [20.0, 40.0]);
    }

    #[test]
    fn viewport_with_canvas_origin() {
        let vp = Viewport {
            canvas_origin: [100.0, 50.0],
            ..Viewport::default()
        };
        let s = vp.graph_to_screen([10.0, 20.0]);
        assert_eq!(s, [110.0, 70.0]);
    }

    #[test]
    fn viewport_roundtrip() {
        let vp = Viewport {
            offset: [30.0, 40.0],
            zoom: 1.5,
            canvas_origin: [10.0, 20.0],
        };
        let graph_pos = [100.0, 200.0];
        let screen = vp.graph_to_screen(graph_pos);
        let back = vp.screen_to_graph(screen);
        assert!((back[0] - graph_pos[0]).abs() < 1e-4);
        assert!((back[1] - graph_pos[1]).abs() < 1e-4);
    }

    #[test]
    fn viewport_screen_to_graph_zero_zoom() {
        let vp = Viewport {
            zoom: 0.0,
            ..Viewport::default()
        };
        // Should return [0, 0] instead of inf/NaN
        let g = vp.screen_to_graph([100.0, 200.0]);
        assert_eq!(g, [0.0, 0.0]);
    }

    // ── InteractionState ─────────────────────────────────────────────────

    #[test]
    fn draw_order_ensure() {
        let mut state = InteractionState::default();
        let id = NodeId(5);
        state.ensure_in_draw_order(id);
        assert_eq!(state.draw_order.len(), 1);
        // Duplicate should be no-op
        state.ensure_in_draw_order(id);
        assert_eq!(state.draw_order.len(), 1);
    }

    #[test]
    fn draw_order_remove() {
        let mut state = InteractionState::default();
        let a = NodeId(1);
        let b = NodeId(2);
        state.ensure_in_draw_order(a);
        state.ensure_in_draw_order(b);
        state.remove_from_draw_order(a);
        assert_eq!(state.draw_order.len(), 1);
        assert_eq!(state.draw_order[0], b);
    }

    #[test]
    fn node_to_top() {
        let mut state = InteractionState::default();
        let a = NodeId(1);
        let b = NodeId(2);
        let c = NodeId(3);
        state.ensure_in_draw_order(a);
        state.ensure_in_draw_order(b);
        state.ensure_in_draw_order(c);
        state.node_to_top(a);
        assert_eq!(*state.draw_order.last().unwrap(), a);
    }

    #[test]
    fn selection() {
        let mut state = InteractionState::default();
        let a = NodeId(1);
        let b = NodeId(2);
        state.select_node(a, false);
        assert!(state.is_selected(a));
        assert!(!state.is_selected(b));

        // Add to selection
        state.select_node(b, true);
        assert!(state.is_selected(a));
        assert!(state.is_selected(b));

        // Replace selection
        state.select_node(b, false);
        assert!(!state.is_selected(a));
        assert!(state.is_selected(b));
    }

    #[test]
    fn deselect_all_clears() {
        let mut state = InteractionState::default();
        state.select_node(NodeId(1), false);
        state.select_node(NodeId(2), true);
        state.deselect_all();
        assert!(state.selected.is_empty());
    }

    #[test]
    fn toggle_select() {
        let mut state = InteractionState::default();
        let a = NodeId(1);
        state.toggle_select(a);
        assert!(state.is_selected(a));
        state.toggle_select(a);
        assert!(!state.is_selected(a));
    }

    #[test]
    fn pin_pos_lookup() {
        let mut state = InteractionState::default();
        let pin = InPinId { node: NodeId(0), input: 0 };
        state.input_pin_pos.insert(pin, [50.0, 75.0]);
        assert_eq!(state.find_input_pos(pin), Some([50.0, 75.0]));

        let opin = OutPinId { node: NodeId(0), output: 0 };
        assert_eq!(state.find_output_pos(opin), None);
    }
}
