//! The `NodeGraphViewer` trait — user-implemented callbacks for custom nodes.
//!
//! This is the primary extension point: implement this trait to define
//! how your nodes look, what pins they have, and how connections behave.

use dear_imgui_rs::Ui;

use super::graph::Graph;
use super::types::{InPinId, NodeId, OutPinId, PinInfo};

// ─── Viewer trait ────────────────────────────────────────────────────────────

/// User-implemented trait that defines node appearance and behavior.
///
/// All methods have default implementations that produce a minimal
/// functional graph. Override the ones you need.
///
/// # Lifetime note
///
/// Methods returning `&str` use a unified lifetime `'a` for `&self` and
/// `&T`, so the returned string can come from either the viewer or the
/// node data.
///
/// # Type parameter
///
/// `T` is the user's node type (typically an enum of node variants).
pub trait NodeGraphViewer<T> {
    /// Display title for a node.
    fn title<'a>(&'a self, node: &'a T) -> &'a str;

    /// Number of input pins on this node.
    fn inputs(&self, node: &T) -> u8;

    /// Number of output pins on this node.
    fn outputs(&self, node: &T) -> u8;

    /// Label for an input pin. Shown next to the pin on the left side.
    ///
    /// Default: empty string (no label).
    fn input_label<'a>(&'a self, _node: &'a T, _input: u8) -> &'a str {
        ""
    }

    /// Label for an output pin. Shown next to the pin on the right side.
    ///
    /// Default: empty string (no label).
    fn output_label<'a>(&'a self, _node: &'a T, _output: u8) -> &'a str {
        ""
    }

    /// Visual info for an input pin (shape, color, wire style).
    ///
    /// Default: blue circle.
    fn input_pin(&self, _node: &T, _input: u8) -> PinInfo {
        PinInfo::default()
    }

    /// Visual info for an output pin (shape, color, wire style).
    ///
    /// Default: blue circle.
    fn output_pin(&self, _node: &T, _output: u8) -> PinInfo {
        PinInfo::default()
    }

    /// Whether this node has a body section (rendered below pins).
    ///
    /// Default: `false`.
    fn has_body(&self, _node: &T) -> bool {
        false
    }

    /// Render the body of a node. Called only when `has_body()` returns `true`
    /// and the node is expanded (`open = true`).
    ///
    /// Takes `&mut T` so ImGui widgets can mutate node state (sliders, etc.).
    fn render_body(&self, _ui: &Ui, _node: &mut T, _id: NodeId) {}

    /// Header color override for a node. If `None`, uses the default from config.
    ///
    /// Return `Some([R, G, B])` to tint the header.
    fn header_color(&self, _node: &T) -> Option<[u8; 3]> {
        None
    }

    /// Whether a connection from `output` to `input` is allowed.
    ///
    /// Default: `true` (all connections allowed). Override to restrict
    /// connections by type, prevent cycles, limit fan-in, etc.
    fn can_connect(
        &self,
        _from: OutPinId,
        _to: InPinId,
        _graph: &Graph<T>,
    ) -> bool {
        true
    }

    /// Called after a connection is made. Use for side-effects (e.g. propagation).
    fn on_connect(
        &mut self,
        _from: OutPinId,
        _to: InPinId,
        _graph: &Graph<T>,
    ) {
    }

    /// Called after a connection is removed.
    fn on_disconnect(
        &mut self,
        _from: OutPinId,
        _to: InPinId,
        _graph: &Graph<T>,
    ) {
    }

    /// Tooltip text when hovering a node. Return `None` for no tooltip.
    fn node_tooltip<'a>(&'a self, _node: &'a T) -> Option<&'a str> {
        None
    }

    /// Tooltip text when hovering an input pin.
    fn input_tooltip<'a>(&'a self, _node: &'a T, _input: u8) -> Option<&'a str> {
        None
    }

    /// Tooltip text when hovering an output pin.
    fn output_tooltip<'a>(&'a self, _node: &'a T, _output: u8) -> Option<&'a str> {
        None
    }

    /// Compute the width of a node. Override for dynamic sizing.
    ///
    /// Default: `None` (use `config.node_min_width`).
    fn node_width(&self, _node: &T) -> Option<f32> {
        None
    }

    /// Override the body height for a specific node.
    ///
    /// Use this when a node's body contains more than one row of widgets.
    /// For example a Vec2 node has two sliders → return `Some(60.0)`.
    ///
    /// Default: `None` (use `config.node_body_height`).
    fn body_height(&self, _node: &T) -> Option<f32> {
        None
    }
}
