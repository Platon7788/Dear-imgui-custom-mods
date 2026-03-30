//! Core types for the node graph editor.
//!
//! Framework-agnostic data types: identifiers, pins, wires, pin visuals.

// ─── Node identifier ─────────────────────────────────────────────────────────

/// Unique identifier for a node in the graph.
///
/// Internally an index into the node slab. Cheap to copy and compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    /// Raw index (for serialization / debug).
    #[inline]
    pub fn index(self) -> u32 {
        self.0
    }
}

// ─── Pin identifiers ─────────────────────────────────────────────────────────

/// Identifies an input pin on a specific node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InPinId {
    pub node: NodeId,
    pub input: u8,
}

/// Identifies an output pin on a specific node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutPinId {
    pub node: NodeId,
    pub output: u8,
}

// ─── Wire ────────────────────────────────────────────────────────────────────

/// A directed connection from an output pin to an input pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Wire {
    pub out_pin: OutPinId,
    pub in_pin: InPinId,
}

// ─── Pin shape ───────────────────────────────────────────────────────────────

/// Shape used to draw a pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinShape {
    #[default]
    Circle,
    Triangle,
    Square,
    Diamond,
}

// ─── Wire style ──────────────────────────────────────────────────────────────

/// How wires are drawn between pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireStyle {
    /// Cubic bezier curve (smooth, default).
    #[default]
    Bezier,
    /// Straight line.
    Line,
    /// Orthogonal routing: horizontal → vertical → horizontal (90° turns).
    Orthogonal,
}

// ─── Wire layer ──────────────────────────────────────────────────────────────

/// Rendering layer for wires relative to nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireLayer {
    /// Wires render behind nodes (default).
    #[default]
    BehindNodes,
    /// Wires render above nodes.
    AboveNodes,
}

// ─── Pin info ────────────────────────────────────────────────────────────────

/// Visual description of a pin, returned by viewer callbacks.
///
/// Controls shape, color, and wire appearance for a single pin.
#[derive(Debug, Clone, Copy)]
pub struct PinInfo {
    /// Pin shape.
    pub shape: PinShape,
    /// Fill color `[R, G, B]` (0–255).
    pub fill: [u8; 3],
    /// Border color `[R, G, B]`.
    pub stroke: [u8; 3],
    /// Wire color override. If `None`, uses `fill`.
    pub wire_color: Option<[u8; 3]>,
    /// Wire style override. If `None`, uses graph default.
    pub wire_style: Option<WireStyle>,
}

impl Default for PinInfo {
    fn default() -> Self {
        Self {
            shape: PinShape::Circle,
            fill: [0x5b, 0x9b, 0xd5],
            stroke: [0x80, 0x85, 0x90],
            wire_color: None,
            wire_style: None,
        }
    }
}

impl PinInfo {
    /// Create a pin with the given shape and fill color.
    #[inline]
    pub fn new(shape: PinShape, fill: [u8; 3]) -> Self {
        Self {
            shape,
            fill,
            stroke: [0x80, 0x85, 0x90],
            wire_color: None,
            wire_style: None,
        }
    }

    /// Circle pin with fill color.
    #[inline]
    pub fn circle(fill: [u8; 3]) -> Self {
        Self::new(PinShape::Circle, fill)
    }

    /// Triangle pin with fill color.
    #[inline]
    pub fn triangle(fill: [u8; 3]) -> Self {
        Self::new(PinShape::Triangle, fill)
    }

    /// Square pin with fill color.
    #[inline]
    pub fn square(fill: [u8; 3]) -> Self {
        Self::new(PinShape::Square, fill)
    }

    /// Diamond pin with fill color.
    #[inline]
    pub fn diamond(fill: [u8; 3]) -> Self {
        Self::new(PinShape::Diamond, fill)
    }

    /// Override the wire color.
    #[inline]
    pub fn with_wire_color(mut self, color: [u8; 3]) -> Self {
        self.wire_color = Some(color);
        self
    }

    /// Override the wire style.
    #[inline]
    pub fn with_wire_style(mut self, style: WireStyle) -> Self {
        self.wire_style = Some(style);
        self
    }

    /// Override the border/stroke color.
    #[inline]
    pub fn with_stroke(mut self, stroke: [u8; 3]) -> Self {
        self.stroke = stroke;
        self
    }

    /// Effective wire color (wire_color or fill fallback).
    #[inline]
    pub fn effective_wire_color(&self) -> [u8; 3] {
        self.wire_color.unwrap_or(self.fill)
    }
}

// ─── Graph action ────────────────────────────────────────────────────────────

/// Actions returned by [`NodeGraph::render`](super::NodeGraph::render).
///
/// Multiple actions can occur per frame (e.g. NodeSelected + NodeMoved).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphAction {
    /// A node was selected (clicked).
    NodeSelected(NodeId),
    /// A node was deselected.
    NodeDeselected(NodeId),
    /// A wire was created.
    Connected(Wire),
    /// A wire was removed.
    Disconnected(Wire),
    /// A node was moved to a new position.
    NodeMoved(NodeId),
    /// Right-click on empty canvas at position `[x, y]` (graph space).
    CanvasMenu([f32; 2]),
    /// Right-click on a node.
    NodeMenu(NodeId),
    /// A node was double-clicked.
    NodeDoubleClicked(NodeId),
    /// A wire was dropped on empty canvas (from output pin).
    /// Show a context menu to create a new node + auto-connect.
    DroppedWireOut(OutPinId, [f32; 2]),
    /// A wire was dropped on empty canvas (from input pin).
    DroppedWireIn(InPinId, [f32; 2]),
    /// Delete key pressed with selected nodes.
    DeleteSelected,
    /// Select all (Ctrl+A).
    SelectAll,
    /// Node collapse/expand toggled.
    NodeToggled(NodeId),
}
