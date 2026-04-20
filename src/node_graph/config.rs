//! Configuration types for the node graph editor.
//!
//! Colors, sizes, behavior toggles — all with sensible defaults.

use super::types::{WireLayer, WireStyle};

// ─── Color palette ───────────────────────────────────────────────────────────

/// Color palette for node graph elements.
///
/// All colors are `[R, G, B]` in 0–255 range. Alpha is applied per-use.
#[derive(Debug, Clone, Copy)]
pub struct NgColors {
    // Canvas
    pub canvas_bg: [u8; 3],
    pub grid_line: [u8; 3],
    pub grid_line_thick: [u8; 3],

    // Nodes
    pub node_bg: [u8; 3],
    pub node_bg_hovered: [u8; 3],
    pub node_bg_selected: [u8; 3],
    pub node_header_bg: [u8; 3],
    pub node_border: [u8; 3],
    pub node_border_selected: [u8; 3],

    // Text
    pub text: [u8; 3],
    pub text_muted: [u8; 3],

    // Pins
    pub pin_default: [u8; 3],
    pub pin_hovered: [u8; 3],

    // Wires
    pub wire_default: [u8; 3],
    pub wire_hovered: [u8; 3],
    pub wire_dragging: [u8; 3],

    // Selection
    pub selection_rect: [u8; 3],
    pub selection_rect_fill: [u8; 3],

    // Mini-map
    pub minimap_bg: [u8; 3],
    pub minimap_outline: [u8; 3],
    pub minimap_node: [u8; 3],
    pub minimap_viewport: [u8; 3],

    // Collapse button
    pub collapse_btn: [u8; 3],
    pub collapse_btn_hovered: [u8; 3],
}

impl Default for NgColors {
    fn default() -> Self {
        Self {
            canvas_bg: [0x1e, 0x1e, 0x2e],
            grid_line: [0x2a, 0x2a, 0x3a],
            grid_line_thick: [0x35, 0x35, 0x48],

            node_bg: [0x30, 0x30, 0x40],
            node_bg_hovered: [0x38, 0x38, 0x4a],
            node_bg_selected: [0x38, 0x38, 0x4a],
            node_header_bg: [0x3a, 0x3a, 0x50],
            node_border: [0x50, 0x50, 0x65],
            node_border_selected: [0x5b, 0x9b, 0xd5],

            text: [0xe0, 0xe4, 0xea],
            text_muted: [0x8a, 0x92, 0xa1],

            pin_default: [0x5b, 0x9b, 0xd5],
            pin_hovered: [0x7b, 0xbb, 0xf5],

            wire_default: [0x80, 0x80, 0x98],
            wire_hovered: [0xb0, 0xb0, 0xd0],
            wire_dragging: [0x5b, 0x9b, 0xd5],

            selection_rect: [0x5b, 0x9b, 0xd5],
            selection_rect_fill: [0x5b, 0x9b, 0xd5],

            minimap_bg: [0x18, 0x18, 0x25],
            minimap_outline: [0x50, 0x50, 0x65],
            minimap_node: [0x5b, 0x9b, 0xd5],
            minimap_viewport: [0xe0, 0xe4, 0xea],

            collapse_btn: [0x8a, 0x92, 0xa1],
            collapse_btn_hovered: [0xe0, 0xe4, 0xea],
        }
    }
}

// ─── Configuration ───────────────────────────────────────────────────────────

/// Full configuration for [`NodeGraph`](super::NodeGraph).
#[derive(Debug, Clone)]
pub struct NodeGraphConfig {
    // ── Grid ──
    /// Grid cell size (pixels at zoom 1.0).
    pub grid_size: f32,
    /// Draw thick grid lines every N cells.
    pub grid_thick_every: u32,
    /// Show grid.
    pub show_grid: bool,
    /// Grid rotation angle in degrees (0 = axis-aligned).
    pub grid_rotation: f32,

    // ── Nodes ──
    /// Node corner rounding.
    pub node_rounding: f32,
    /// Node border thickness.
    pub node_border_thickness: f32,
    /// Node header height.
    pub node_header_height: f32,
    /// Horizontal padding inside node.
    pub node_padding_h: f32,
    /// Vertical padding inside node.
    pub node_padding_v: f32,
    /// Minimum node width.
    pub node_min_width: f32,
    /// Default body height (used when node has a body section).
    pub node_body_height: f32,
    /// Show collapse/expand button in node header.
    pub node_collapsible: bool,

    // ── Pins ──
    /// Pin circle radius.
    pub pin_radius: f32,
    /// Vertical spacing between pins.
    pub pin_spacing: f32,
    /// Horizontal offset of pin from node edge.
    pub pin_offset: f32,
    /// Hit-test radius (larger than visual for easier clicking).
    pub pin_hit_radius: f32,

    // ── Wires ──
    /// Show wires (connections between nodes). Default: true.
    pub show_wires: bool,
    /// Default wire style.
    pub wire_style: WireStyle,
    /// Wire thickness.
    pub wire_thickness: f32,
    /// Wire hover hit distance (in screen pixels, auto-scales with zoom).
    pub wire_hover_distance: f32,
    /// Bezier tangent length factor (fraction of horizontal distance).
    pub wire_curvature: f32,
    /// Wire rendering layer (behind or above nodes).
    pub wire_layer: WireLayer,

    // ── Interaction ──
    /// Pan with middle mouse button.
    pub pan_button_middle: bool,
    /// Pan with right mouse button (drag on empty canvas).
    pub pan_button_right: bool,
    /// Pan with Shift + Left mouse button.
    pub pan_shift_lmb: bool,
    /// Zoom with scroll wheel.
    pub zoom_with_wheel: bool,
    /// Minimum zoom level.
    pub zoom_min: f32,
    /// Maximum zoom level.
    pub zoom_max: f32,
    /// Zoom speed factor.
    pub zoom_speed: f32,
    /// Allow multi-select with Ctrl+Click.
    pub multi_select: bool,
    /// Allow rectangle selection.
    pub rect_select: bool,
    /// Right-click on canvas opens context menu.
    pub canvas_context_menu: bool,
    /// Right-click on node opens context menu.
    pub node_context_menu: bool,
    /// Double-click on node triggers action.
    pub node_double_click: bool,
    /// Snap node positions to grid.
    pub snap_to_grid: bool,
    /// Snap granularity (if `snap_to_grid` is true).
    pub snap_size: f32,
    /// Ctrl+click on wire to yank (detach + redirect).
    pub wire_yanking: bool,
    /// Drop wire on empty canvas fires DroppedWire action.
    pub drop_wire_menu: bool,

    // ── Keyboard ──
    /// Delete key removes selected nodes (fires DeleteSelected action).
    pub keyboard_delete: bool,
    /// Ctrl+A selects all nodes.
    pub keyboard_select_all: bool,
    /// Escape cancels wire drag.
    pub keyboard_escape_cancel: bool,

    // ── Stats overlay ──
    /// Show a stats overlay (nodes, wires, zoom) drawn on the canvas.
    pub show_stats_overlay: bool,
    /// Stats overlay corner: 0=top-left, 1=top-right, 2=bottom-left, 3=bottom-right.
    pub stats_overlay_corner: u8,
    /// Stats overlay margin from the canvas edge.
    pub stats_overlay_margin: f32,

    // ── Mini-map ──
    /// Show a mini-map in the corner.
    pub show_minimap: bool,
    /// Mini-map size (width, height).
    pub minimap_size: [f32; 2],
    /// Mini-map corner: 0=top-left, 1=top-right, 2=bottom-left, 3=bottom-right.
    pub minimap_corner: u8,
    /// Mini-map margin from canvas edge.
    pub minimap_margin: f32,
    /// Mini-map is clickable/draggable for navigation.
    pub minimap_interactive: bool,

    // ── LOD (level of detail) ──
    /// Below this zoom level, pin labels are hidden.
    pub lod_hide_labels_zoom: f32,
    /// Below this zoom level, pin shapes simplify to dots.
    pub lod_simplify_pins_zoom: f32,
    /// Below this zoom level, node bodies are hidden.
    pub lod_hide_body_zoom: f32,

    // ── Smooth zoom ──
    /// Animate zoom transitions instead of instant jumps.
    pub smooth_zoom: bool,
    /// Smooth zoom interpolation speed (higher = faster).
    pub smooth_zoom_speed: f32,

    // ── Node shadow ──
    /// Draw a subtle drop shadow behind nodes for depth perception.
    pub node_shadow: bool,
    /// Shadow offset in pixels (down-right).
    pub node_shadow_offset: f32,
    /// Shadow alpha (0–255).
    pub node_shadow_alpha: u8,

    // ── Wire flow animation ──
    /// Animate directional dots along wires to show data flow.
    pub wire_flow: bool,
    /// Wire flow dot speed in pixels per second.
    pub wire_flow_speed: f32,
    /// Wire flow dot spacing in pixels.
    pub wire_flow_spacing: f32,

    // ── Tooltip ──
    /// Delay before showing hover tooltips (seconds).
    pub tooltip_delay: f32,

    // ── Appearance ──
    pub colors: NgColors,
}

impl Default for NodeGraphConfig {
    fn default() -> Self {
        Self {
            grid_size: 32.0,
            grid_thick_every: 4,
            show_grid: true,
            grid_rotation: 0.0,

            node_rounding: 6.0,
            node_border_thickness: 1.5,
            node_header_height: 24.0,
            node_padding_h: 8.0,
            node_padding_v: 4.0,
            node_min_width: 120.0,
            node_body_height: 30.0,
            node_collapsible: true,

            pin_radius: 5.0,
            pin_spacing: 22.0,
            pin_offset: 0.0,
            pin_hit_radius: 10.0,

            show_wires: true,
            wire_style: WireStyle::Bezier,
            wire_thickness: 2.0,
            wire_hover_distance: 8.0,
            wire_curvature: 0.5,
            wire_layer: WireLayer::BehindNodes,

            pan_button_middle: true,
            pan_button_right: false,
            pan_shift_lmb: true,
            zoom_with_wheel: true,
            zoom_min: 0.25,
            zoom_max: 1.5,
            zoom_speed: 0.1,
            multi_select: true,
            rect_select: true,
            canvas_context_menu: true,
            node_context_menu: true,
            node_double_click: true,
            snap_to_grid: false,
            snap_size: 16.0,
            wire_yanking: true,
            drop_wire_menu: true,

            keyboard_delete: true,
            keyboard_select_all: true,
            keyboard_escape_cancel: true,

            show_stats_overlay: true,
            stats_overlay_corner: 1, // top-right
            stats_overlay_margin: 8.0,

            show_minimap: true,
            minimap_size: [180.0, 120.0],
            minimap_corner: 3, // bottom-right
            minimap_margin: 10.0,
            minimap_interactive: true,

            lod_hide_labels_zoom: 0.4,
            lod_simplify_pins_zoom: 0.3,
            lod_hide_body_zoom: 0.35,

            smooth_zoom: true,
            smooth_zoom_speed: 8.0,

            node_shadow: true,
            node_shadow_offset: 3.0,
            node_shadow_alpha: 40,

            wire_flow: false,
            wire_flow_speed: 60.0,
            wire_flow_spacing: 20.0,

            tooltip_delay: 0.3,

            colors: NgColors::default(),
        }
    }
}

impl NodeGraphConfig {
    /// Compute node height in graph space.
    ///
    /// `body_height_override`: per-node body height from `NodeGraphViewer::body_height()`.
    /// When `None`, falls back to `config.node_body_height`.
    #[inline]
    pub fn node_height(
        &self,
        num_inputs: u8,
        num_outputs: u8,
        has_body: bool,
        is_open: bool,
        body_height_override: Option<f32>,
    ) -> f32 {
        let pin_count = num_inputs.max(num_outputs) as f32;
        let body_h = if has_body && is_open {
            body_height_override.unwrap_or(self.node_body_height)
        } else {
            0.0
        };
        let pins_h = if pin_count > 0.0 && is_open {
            pin_count * self.pin_spacing + self.node_padding_v
        } else {
            0.0
        };
        self.node_header_height + pins_h + body_h + self.node_padding_v
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = NodeGraphConfig::default();
        assert_eq!(cfg.grid_size, 32.0);
        assert!(cfg.show_grid);
        assert_eq!(cfg.node_rounding, 6.0);
        assert_eq!(cfg.wire_style, WireStyle::Bezier);
        assert!(cfg.multi_select);
        assert!(cfg.rect_select);
        assert!(cfg.show_minimap);
        assert!(cfg.smooth_zoom);
    }

    #[test]
    fn node_height_collapsed() {
        let cfg = NodeGraphConfig::default();
        let h_open = cfg.node_height(2, 3, true, true, None);
        let h_closed = cfg.node_height(2, 3, true, false, None);
        assert!(h_open > h_closed, "open height should exceed collapsed");
        // Collapsed = header + bottom padding only
        assert_eq!(h_closed, cfg.node_header_height + cfg.node_padding_v);
    }

    #[test]
    fn node_height_no_pins() {
        let cfg = NodeGraphConfig::default();
        let h = cfg.node_height(0, 0, true, true, None);
        assert_eq!(
            h,
            cfg.node_header_height + cfg.node_body_height + cfg.node_padding_v
        );
    }

    #[test]
    fn node_height_body_override() {
        let cfg = NodeGraphConfig::default();
        let h1 = cfg.node_height(1, 1, true, true, None);
        let h2 = cfg.node_height(1, 1, true, true, Some(100.0));
        assert!(
            h2 > h1,
            "override 100.0 should be taller than default {}",
            cfg.node_body_height
        );
    }

    #[test]
    fn node_height_symmetry() {
        let cfg = NodeGraphConfig::default();
        // Same pin count should give same height regardless of input/output distribution
        let h1 = cfg.node_height(3, 1, false, true, None);
        let h2 = cfg.node_height(1, 3, false, true, None);
        assert_eq!(h1, h2);
    }

    #[test]
    fn colors_default() {
        let c = NgColors::default();
        assert_eq!(c.canvas_bg, [0x1e, 0x1e, 0x2e]);
    }

    #[test]
    fn config_clone() {
        let cfg = NodeGraphConfig::default();
        let cfg2 = cfg.clone();
        assert_eq!(cfg2.grid_size, cfg.grid_size);
    }

    #[test]
    fn colors_copy() {
        let c = NgColors::default();
        let c2 = c; // Copy
        assert_eq!(c2.text, c.text);
    }
}
