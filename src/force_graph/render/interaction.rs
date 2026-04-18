//! Mouse/keyboard interaction helpers for the knowledge-graph renderer.
//!
//! Extracted from `render/mod.rs` to keep that file under the 500-line limit.
//! All functions are pure in the sense that they only mutate state through
//! explicit `&mut` parameters — no global/static state.

use std::collections::HashSet;

use dear_imgui_rs::{MouseButton, Ui};

use super::super::config::{ForceConfig, ViewerConfig};
use super::super::data::{GraphData, NodeId};
use super::super::event::GraphEvent;
use super::super::filter::FilterState;
use super::super::sim::Simulation;
use super::super::style::GraphColors;
use super::camera::Camera;

// ─── Drag ─────────────────────────────────────────────────────────────────────

/// Process node dragging for one frame.
///
/// When the user holds LMB over a node, this moves the node to follow the
/// cursor. Releasing LMB ends the drag and emits [`GraphEvent::NodeMoved`].
///
/// Returns `true` if a drag is in progress this frame (suppresses canvas pan).
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_drag(
    ui: &Ui,
    graph: &mut GraphData,
    camera: &Camera,
    canvas_min: [f32; 2],
    hovered_node: Option<NodeId>,
    dragging_node: &mut Option<NodeId>,
    drag_world_offset: &mut [f32; 2],
    config: &ViewerConfig,
    events: &mut Vec<GraphEvent>,
) -> bool {
    if !config.drag_enabled {
        return false;
    }

    let io = ui.io();
    let mouse = io.mouse_pos();
    let lmb_down = ui.is_mouse_down(MouseButton::Left);
    let lmb_released = ui.is_mouse_released(MouseButton::Left);

    // Start drag when pressing LMB on a node.
    if ui.is_mouse_clicked(MouseButton::Left)
        && let Some(id) = hovered_node
        && let Some(node) = graph.nodes.get(id) {
            let world_mouse = camera.screen_to_world(mouse, canvas_min);
            *drag_world_offset = [
                node.pos[0] - world_mouse[0],
                node.pos[1] - world_mouse[1],
            ];
            *dragging_node = Some(id);
    }

    let dragging = *dragging_node;

    // Continue drag.
    if lmb_down
        && let Some(id) = dragging
    {
        let world_mouse = camera.screen_to_world(mouse, canvas_min);
        let new_pos = [
            world_mouse[0] + drag_world_offset[0],
            world_mouse[1] + drag_world_offset[1],
        ];
        if let Some(node) = graph.nodes.get_mut(id) {
            node.pos = new_pos;
            node.vel = [0.0, 0.0];
        }
        graph.dirty = true;
        return true;
    }

    // End drag.
    if lmb_released
        && let Some(id) = *dragging_node
    {
        *dragging_node = None;
        if let Some(node) = graph.nodes.get_mut(id) {
            let pos = node.pos;
            // Pin only if not already pinned (avoid spurious NodePinned events).
            if config.pin_on_drag && !node.style.pinned {
                node.style.pinned = true;
                events.push(GraphEvent::NodePinned(id, true));
            }
            events.push(GraphEvent::NodeMoved(id, pos));
        }
    }

    false
}

// ─── Box select ────────────────────────────────────────────────────────────────

/// Process box / rubber-band selection for one frame.
///
/// When the user drags on empty canvas space (no node hovered) in
/// `SelectionMode::Box` or `SelectionMode::Additive`, this accumulates nodes
/// inside the rectangle. `Shift` held = additive; otherwise replaces selection.
///
/// Returns `true` while a box drag is in progress.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_box_select(
    ui: &Ui,
    camera: &Camera,
    canvas_min: [f32; 2],
    graph: &GraphData,
    force_config: &ForceConfig,
    hovered_node: Option<NodeId>,
    selection: &mut HashSet<NodeId>,
    box_select_start: &mut Option<[f32; 2]>,
    config: &ViewerConfig,
    events: &mut Vec<GraphEvent>,
) -> bool {
    use super::super::config::SelectionMode;
    if matches!(config.selection_mode, SelectionMode::Single) {
        return false;
    }

    let io = ui.io();
    let mouse = io.mouse_pos();
    let lmb_down = ui.is_mouse_down(MouseButton::Left);
    let lmb_released = ui.is_mouse_released(MouseButton::Left);
    let shift = io.key_shift();

    // Start box select: Shift+LMB drag on empty space.
    // Without Shift, LMB drag is pan — box-select must be intentional.
    if ui.is_mouse_clicked(MouseButton::Left) && hovered_node.is_none() && shift {
        let world = camera.screen_to_world(mouse, canvas_min);
        *box_select_start = Some(world);
    }

    let active = box_select_start.is_some();

    // Complete box select on release.
    if lmb_released
        && let Some(start) = box_select_start.take()
    {
        let end = camera.screen_to_world(mouse, canvas_min);
        let rect_min = [start[0].min(end[0]), start[1].min(end[1])];
        let rect_max = [start[0].max(end[0]), start[1].max(end[1])];

        let additive = matches!(config.selection_mode, SelectionMode::Additive) && shift;
        if !additive {
            selection.clear();
        }

        for (node_id, node) in graph.nodes.iter() {
            let r = if force_config.radius_by_degree {
                let deg = graph.adjacency.get(&node_id).map_or(0, |v| v.len());
                force_config.radius_base + force_config.radius_per_degree * deg as f32
            } else {
                node.style.radius.unwrap_or(force_config.radius_base)
            };
            // Node centre inside rect (with a radius margin).
            if node.pos[0] >= rect_min[0] - r
                && node.pos[0] <= rect_max[0] + r
                && node.pos[1] >= rect_min[1] - r
                && node.pos[1] <= rect_max[1] + r
            {
                selection.insert(node_id);
            }
        }

        events.push(GraphEvent::SelectionChanged(selection.clone()));
        return false;
    }

    if !lmb_down {
        *box_select_start = None;
    }

    active
}

// ─── Context menu ──────────────────────────────────────────────────────────────

/// Open and render the node context menu popup.
///
/// Call every frame. When `ctx_menu_node` is `Some`, opens the ImGui popup
/// and renders the menu items. Returns `true` while the popup is open.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_context_menu(
    ui: &Ui,
    graph: &mut GraphData,
    ctx_menu_node: &mut Option<NodeId>,
    selection: &mut HashSet<NodeId>,
    filter: &mut FilterState,
    sim: &mut Simulation,
    events: &mut Vec<GraphEvent>,
    config: &ViewerConfig,
) -> bool {
    if !config.context_menu_enabled {
        return false;
    }

    // Open popup when a node was right-clicked (caller sets ctx_menu_node).
    if ctx_menu_node.is_some() && ui.is_mouse_clicked(MouseButton::Right) {
        ui.open_popup("##kg_ctx");
    }

    let mut open = false;
    ui.popup("##kg_ctx", || {
        open = true;
        let Some(id) = *ctx_menu_node else { return };

        let label = graph.node(id).map(|s| s.label.as_str()).unwrap_or("?");
        ui.text_disabled(label);
        ui.separator();

        // Pin / Unpin.
        let pinned = graph.node(id).is_some_and(|s| s.pinned);
        let pin_label = if pinned { "Unpin" } else { "Pin" };
        if ui.menu_item(pin_label) {
            let new_pin = !graph.node(id).is_some_and(|n| n.pinned);
            graph.node_set_pinned(id, new_pin);
            events.push(GraphEvent::NodePinned(id, new_pin));
            sim.wake();
        }

        // Select neighbours.
        if ui.menu_item("Select neighbours") {
            selection.clear();
            selection.insert(id);
            for nb in graph.neighbors(id) {
                selection.insert(nb);
            }
            events.push(GraphEvent::SelectionChanged(selection.clone()));
        }

        // Focus (depth filter from this node).
        if ui.menu_item("Focus here") {
            filter.focused_node = Some(id);
            filter.depth.get_or_insert(2);
            events.push(GraphEvent::FilterChanged);
        }

        // Clear depth focus.
        if filter.focused_node.is_some() && ui.menu_item("Clear focus") {
            filter.focused_node = None;
            filter.depth = None;
            events.push(GraphEvent::FilterChanged);
        }

        ui.separator();

        // Activate (open).
        if ui.menu_item("Activate") {
            events.push(GraphEvent::NodeActivated(id));
        }
    });

    if !open {
        *ctx_menu_node = None;
    }

    open
}

// ─── Box-select rect drawing ───────────────────────────────────────────────────

/// Draw the rubber-band selection rectangle while a box-select drag is active.
pub(crate) fn draw_box_rect(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    box_select_start: &Option<[f32; 2]>,
    camera: &Camera,
    canvas_min: [f32; 2],
    mouse: [f32; 2],
    colors: &GraphColors,
) {
    let Some(start_world) = *box_select_start else {
        return;
    };
    let sa = camera.world_to_screen(start_world, canvas_min);
    let sb = mouse;
    let fill = super::super::super::utils::color::pack_color_f32(colors.selection_fill);
    let outline = super::super::super::utils::color::pack_color_f32(colors.selection_outline);
    draw.add_rect(sa, sb, fill).filled(true).build();
    draw.add_rect(sa, sb, outline).filled(false).build();
}

// ─── Keyboard shortcuts ────────────────────────────────────────────────────────

/// Handle keyboard shortcuts when the canvas is active.
///
/// - Arrow keys: pan camera
/// - `+` / `-`: zoom in/out
/// - `F`: fit to graph
/// - `Esc`: clear selection
/// - `Space`: toggle simulation
/// - `P`: pin/unpin selected
/// - `Delete`/`Backspace`: emit delete request
pub(crate) fn handle_keyboard(
    ui: &Ui,
    camera: &mut Camera,
    canvas_hovered: bool,
    selection: &mut HashSet<NodeId>,
    graph: &mut GraphData,
    sim: &mut Simulation,
    canvas_size: [f32; 2],
    events: &mut Vec<GraphEvent>,
) {
    if !canvas_hovered && !ui.is_window_focused() {
        return;
    }

    let io = ui.io();
    let dt = io.delta_time().max(1.0 / 240.0);
    let pan_speed = 200.0 * dt;
    let mut panned = false;

    // Arrow key pan.
    if ui.is_key_down(dear_imgui_rs::Key::LeftArrow)  { camera.pan([pan_speed, 0.0]);   panned = true; }
    if ui.is_key_down(dear_imgui_rs::Key::RightArrow) { camera.pan([-pan_speed, 0.0]);  panned = true; }
    if ui.is_key_down(dear_imgui_rs::Key::UpArrow)    { camera.pan([0.0, pan_speed]);   panned = true; }
    if ui.is_key_down(dear_imgui_rs::Key::DownArrow)  { camera.pan([0.0, -pan_speed]);  panned = true; }
    if panned { events.push(GraphEvent::CameraChanged); }

    // + / = zoom in; - zoom out.
    if ui.is_key_down(dear_imgui_rs::Key::Equal) || ui.is_key_down(dear_imgui_rs::Key::KeypadAdd) {
        let pivot = [canvas_size[0] * 0.5, canvas_size[1] * 0.5];
        camera.zoom_at(1.0 + 2.0 * dt, pivot, [0.0, 0.0]);
        events.push(GraphEvent::CameraChanged);
    }
    if ui.is_key_down(dear_imgui_rs::Key::Minus) || ui.is_key_down(dear_imgui_rs::Key::KeypadSubtract) {
        let pivot = [canvas_size[0] * 0.5, canvas_size[1] * 0.5];
        camera.zoom_at(1.0 - 2.0 * dt, pivot, [0.0, 0.0]);
        events.push(GraphEvent::CameraChanged);
    }

    // Esc: clear selection.
    if ui.is_key_pressed(dear_imgui_rs::Key::Escape) && !selection.is_empty() {
        selection.clear();
        events.push(GraphEvent::SelectionChanged(selection.clone()));
    }

    // Space: request simulation toggle — mod.rs post-processor owns the actual flip.
    if ui.is_key_pressed(dear_imgui_rs::Key::Space) {
        events.push(GraphEvent::SimulationToggled(!sim.asleep));
    }

    // F: fit to screen.
    if ui.is_key_pressed(dear_imgui_rs::Key::F) {
        events.push(GraphEvent::FitToScreen);
    }

    // P: pin/unpin selected nodes.
    if ui.is_key_pressed(dear_imgui_rs::Key::P) {
        for &id in selection.iter() {
            let new_pin = !graph.node(id).is_some_and(|n| n.pinned);
            graph.node_set_pinned(id, new_pin);
            events.push(GraphEvent::NodePinned(id, new_pin));
        }
        sim.wake();
    }

    // Delete / Backspace: request deletion.
    if (ui.is_key_pressed(dear_imgui_rs::Key::Delete)
        || ui.is_key_pressed(dear_imgui_rs::Key::Backspace))
        && !selection.is_empty()
    {
        events.push(GraphEvent::SelectionDeleteRequested(selection.clone()));
    }
}
