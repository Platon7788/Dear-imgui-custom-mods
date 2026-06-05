//! Mouse and keyboard input handling for the node graph.

use dear_imgui_rs::{Key, MouseButton, Ui};

use super::super::config::NodeGraphConfig;
use super::super::graph::Graph;
use super::super::state::{
    CommentDrag, CommentDragKind, HoveredElement, InteractionState, NewWire, NodeDrag, RectSelect,
};
use super::super::types::*;
use super::super::viewer::NodeGraphViewer;
use super::comments;
use super::input_hittest;
use super::math;

// ─── Input handling ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_input<T>(
    graph: &Graph<T>,
    state: &mut InteractionState,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
    ui: &Ui,
    canvas_pos: [f32; 2],
    canvas_size: [f32; 2],
    canvas_hovered: bool,
    wire_aabbs: &[math::NodeAABB],
    actions: &mut Vec<GraphAction>,
) {
    let mouse = ui.io().mouse_pos();
    let io = ui.io();

    let in_canvas = canvas_hovered;

    if !in_canvas {
        return;
    }

    // ── Hit testing ──────────────────────────────────────────────────────
    let hit = input_hittest::hit_test(graph, state, config, viewer, mouse, wire_aabbs);
    state.hovered = hit;

    // ── Zoom ─────────────────────────────────────────────────────────────
    if config.zoom_with_wheel {
        let wheel = io.mouse_wheel();
        if wheel != 0.0 {
            if config.smooth_zoom {
                // Set target — interpolation happens at top of render_graph
                let new_target = (state.zoom_target + wheel * config.zoom_speed)
                    .clamp(config.zoom_min, config.zoom_max);
                // Adjust offset for the target zoom (keep cursor point fixed)
                let graph_mouse = state.viewport.screen_to_graph(mouse);
                let co = state.viewport.canvas_origin;
                state.zoom_target = new_target;
                state.viewport.offset[0] = mouse[0] - graph_mouse[0] * new_target - co[0];
                state.viewport.offset[1] = mouse[1] - graph_mouse[1] * new_target - co[1];
            } else {
                let old_zoom = state.viewport.zoom;
                let new_zoom =
                    (old_zoom + wheel * config.zoom_speed).clamp(config.zoom_min, config.zoom_max);
                let graph_mouse = state.viewport.screen_to_graph(mouse);
                let co = state.viewport.canvas_origin;
                state.viewport.zoom = new_zoom;
                state.zoom_target = new_zoom;
                state.viewport.offset[0] = mouse[0] - graph_mouse[0] * new_zoom - co[0];
                state.viewport.offset[1] = mouse[1] - graph_mouse[1] * new_zoom - co[1];
            }
        }
    }

    // ── Left mouse button ────────────────────────────────────────────────
    let shift = io.key_shift();
    // When Shift+LMB pan is active, suppress normal LMB actions
    let shift_panning = config.pan_shift_lmb && shift;
    let lmb_pressed = ui.is_mouse_clicked(MouseButton::Left) && !shift_panning;
    let lmb_released = ui.is_mouse_released(MouseButton::Left) && !shift_panning;
    let lmb_down = ui.is_mouse_down(MouseButton::Left) && !shift_panning;
    let lmb_double = ui.is_mouse_double_clicked(MouseButton::Left) && !shift_panning;
    let ctrl = io.key_ctrl();

    // Double click
    if lmb_double
        && let HoveredElement::Node(nid) = hit
        && config.node_double_click
    {
        actions.push(GraphAction::NodeDoubleClicked(nid));
    }

    // Press
    if lmb_pressed && !lmb_double {
        // If a wire is already being built (click-to-click mode),
        // a click completes or cancels it.
        let had_wire = state.new_wire.is_some();
        if had_wire {
            let completed = match (&state.new_wire, hit) {
                (Some(NewWire::FromOutput(out_pin)), HoveredElement::InputPin(in_pin))
                    if graph.can_connect_basic(*out_pin, in_pin)
                        && viewer.can_connect(*out_pin, in_pin, graph) =>
                {
                    actions.push(GraphAction::Connected(Wire {
                        out_pin: *out_pin,
                        in_pin,
                    }));
                    true
                }
                (Some(NewWire::FromInput(in_pin)), HoveredElement::OutputPin(out_pin))
                    if graph.can_connect_basic(out_pin, *in_pin)
                        && viewer.can_connect(out_pin, *in_pin, graph) =>
                {
                    actions.push(GraphAction::Connected(Wire {
                        out_pin,
                        in_pin: *in_pin,
                    }));
                    true
                }
                // Clicking another pin of the same direction — restart wire from new pin
                (Some(NewWire::FromOutput(_)), HoveredElement::OutputPin(pin)) => {
                    state.new_wire = Some(NewWire::FromOutput(pin));
                    false
                }
                (Some(NewWire::FromInput(_)), HoveredElement::InputPin(pin)) => {
                    state.new_wire = Some(NewWire::FromInput(pin));
                    false
                }
                _ => false,
            };

            if completed {
                state.new_wire = None;
            } else if !matches!(
                hit,
                HoveredElement::OutputPin(_) | HoveredElement::InputPin(_)
            ) {
                // Clicked on empty space / node — cancel wire + fire dropped wire action
                if config.drop_wire_menu && matches!(hit, HoveredElement::None) {
                    let graph_pos = state.viewport.screen_to_graph(mouse);
                    match &state.new_wire {
                        Some(NewWire::FromOutput(pin)) => {
                            actions.push(GraphAction::DroppedWireOut(*pin, graph_pos));
                        }
                        Some(NewWire::FromInput(pin)) => {
                            actions.push(GraphAction::DroppedWireIn(*pin, graph_pos));
                        }
                        None => {}
                    }
                }
                state.new_wire = None;
            }
        }

        // Normal press actions (only if we didn't already have a wire in progress)
        if !had_wire {
            match hit {
                HoveredElement::OutputPin(pin) => {
                    state.new_wire = Some(NewWire::FromOutput(pin));
                }
                HoveredElement::InputPin(pin) => {
                    state.new_wire = Some(NewWire::FromInput(pin));
                }
                HoveredElement::Wire(out_pin, in_pin) if ctrl && config.wire_yanking => {
                    // Wire yanking: Ctrl+click detaches the input end and starts dragging
                    actions.push(GraphAction::Disconnected(Wire { out_pin, in_pin }));
                    state.new_wire = Some(NewWire::FromOutput(out_pin));
                }
                HoveredElement::Node(nid) => {
                    // Check if collapse button was clicked
                    if config.node_collapsible
                        && input_hittest::is_collapse_button_hit(graph, state, config, nid, mouse)
                    {
                        actions.push(GraphAction::NodeToggled(nid));
                    } else {
                        // Start dragging
                        if let Some(node) = graph.get_node(nid) {
                            let screen_pos = state.viewport.graph_to_screen(node.pos);
                            state.node_drag = Some(NodeDrag {
                                node: nid,
                                offset: [mouse[0] - screen_pos[0], mouse[1] - screen_pos[1]],
                                moved: false,
                            });
                            state.node_to_top(nid);

                            if ctrl && config.multi_select {
                                state.toggle_select(nid);
                            } else if !state.is_selected(nid) {
                                state.select_node(nid, false);
                                actions.push(GraphAction::NodeSelected(nid));
                            }
                        }
                    }
                }
                HoveredElement::CommentTitle(index) => {
                    // Begin moving the comment: grab offset from its top-left.
                    if let Some(comment) = graph.comments().get(index) {
                        let rect = comments::screen_rect(state, comment.pos, comment.size);
                        state.comment_drag = Some(CommentDrag {
                            index,
                            kind: CommentDragKind::Move,
                            offset: [mouse[0] - rect[0], mouse[1] - rect[1]],
                            moved: false,
                        });
                    }
                }
                HoveredElement::CommentResize(index) => {
                    // Begin resizing: grab offset from its bottom-right corner.
                    if let Some(comment) = graph.comments().get(index) {
                        let rect = comments::screen_rect(state, comment.pos, comment.size);
                        state.comment_drag = Some(CommentDrag {
                            index,
                            kind: CommentDragKind::Resize,
                            offset: [mouse[0] - rect[2], mouse[1] - rect[3]],
                            moved: false,
                        });
                    }
                }
                HoveredElement::CommentBody(_) => {
                    // Clicking a comment body is inert (no move, no rect-select,
                    // no deselect) — it only matters for right-click menus.
                }
                HoveredElement::None => {
                    if config.rect_select {
                        state.rect_select = Some(RectSelect {
                            start: mouse,
                            end: mouse,
                        });
                    }
                    if !ctrl {
                        state.deselect_all();
                    }
                }
                _ => {}
            }
        }
    }

    // Drag
    if lmb_down {
        let delta = io.mouse_delta();

        if let Some(ref mut drag) = state.node_drag
            && (delta[0] != 0.0 || delta[1] != 0.0)
        {
            drag.moved = true;
        }

        if let Some(ref mut rect_sel) = state.rect_select {
            rect_sel.end = mouse;
        }
    }

    // Release — handles drag-and-drop wire completion (not click-to-click)
    if lmb_released {
        // Complete wire via drag-and-drop (mouse was dragging, not click mode).
        // IMPORTANT: is_mouse_dragging() returns false on the release frame because
        // MouseDown is already false. Use mouse_drag_delta() which persists until
        // the next press — non-zero means the mouse actually moved during the drag.
        let dd = ui.mouse_drag_delta(MouseButton::Left);
        let dragged = dd[0] * dd[0] + dd[1] * dd[1] > 25.0; // 5 px threshold
        if dragged && let Some(ref nw) = state.new_wire {
            let completed = match nw {
                NewWire::FromOutput(out_pin) => {
                    if let HoveredElement::InputPin(in_pin) = hit
                        && graph.can_connect_basic(*out_pin, in_pin)
                        && viewer.can_connect(*out_pin, in_pin, graph)
                    {
                        actions.push(GraphAction::Connected(Wire {
                            out_pin: *out_pin,
                            in_pin,
                        }));
                        true
                    } else {
                        false
                    }
                }
                NewWire::FromInput(in_pin) => {
                    if let HoveredElement::OutputPin(out_pin) = hit
                        && graph.can_connect_basic(out_pin, *in_pin)
                        && viewer.can_connect(out_pin, *in_pin, graph)
                    {
                        actions.push(GraphAction::Connected(Wire {
                            out_pin,
                            in_pin: *in_pin,
                        }));
                        true
                    } else {
                        false
                    }
                }
            };

            if completed {
                state.new_wire = None;
            } else if matches!(hit, HoveredElement::None) {
                // Dropped wire on empty canvas via drag
                if config.drop_wire_menu {
                    let graph_pos = state.viewport.screen_to_graph(mouse);
                    match nw {
                        NewWire::FromOutput(pin) => {
                            actions.push(GraphAction::DroppedWireOut(*pin, graph_pos));
                        }
                        NewWire::FromInput(pin) => {
                            actions.push(GraphAction::DroppedWireIn(*pin, graph_pos));
                        }
                    }
                }
                state.new_wire = None;
            }
            // If released on a node or other element but not a valid pin,
            // keep the wire alive for click-to-click mode
        }

        // Complete node drag
        if let Some(drag) = state.node_drag.take()
            && drag.moved
        {
            actions.push(GraphAction::NodeMoved(drag.node));
        }

        // Complete comment move/resize. The per-frame mutation + CommentChanged
        // action are emitted by NodeGraph::apply_comment_drag while the drag is
        // active; here we just end the gesture.
        state.comment_drag = None;

        // Complete rectangle selection
        if let Some(rect_sel) = state.rect_select.take() {
            let r = rect_sel.rect();
            for (nid, node) in graph.nodes() {
                let sp = state.viewport.graph_to_screen(node.pos);
                let node_w = viewer
                    .node_width(&node.value)
                    .unwrap_or(config.node_min_width)
                    * state.viewport.zoom;
                let ni = viewer.inputs(&node.value);
                let no = viewer.outputs(&node.value);
                let hb = viewer.has_body(&node.value);
                let node_h =
                    config.node_height(ni, no, hb, node.open, viewer.body_height(&node.value))
                        * state.viewport.zoom;

                if sp[0] + node_w >= r[0]
                    && sp[0] <= r[2]
                    && sp[1] + node_h >= r[1]
                    && sp[1] <= r[3]
                {
                    state.select_node(nid, true);
                }
            }
        }
    }

    // ── Panning ────────────────────────────────────────────────────────
    let mid_down = config.pan_button_middle && ui.is_mouse_down(MouseButton::Middle);
    let right_dragging = config.pan_button_right
        && ui.is_mouse_dragging(MouseButton::Right)
        && matches!(hit, HoveredElement::None);
    let shift_lmb = config.pan_shift_lmb && io.key_shift() && ui.is_mouse_down(MouseButton::Left);

    if mid_down || right_dragging || shift_lmb {
        let delta = io.mouse_delta();
        state.viewport.offset[0] += delta[0];
        state.viewport.offset[1] += delta[1];
    }

    // ── Right click context menu (only on click, not drag) ───────────────
    if ui.is_mouse_released(MouseButton::Right) && !ui.is_mouse_dragging(MouseButton::Right) {
        match hit {
            HoveredElement::Node(nid) if config.node_context_menu => {
                actions.push(GraphAction::NodeMenu(nid));
            }
            HoveredElement::None if config.canvas_context_menu => {
                let gp = state.viewport.screen_to_graph(mouse);
                actions.push(GraphAction::CanvasMenu(gp));
            }
            HoveredElement::Wire(out_pin, in_pin) => {
                actions.push(GraphAction::Disconnected(Wire { out_pin, in_pin }));
            }
            HoveredElement::CommentTitle(index)
            | HoveredElement::CommentResize(index)
            | HoveredElement::CommentBody(index) => {
                actions.push(GraphAction::CommentMenu(index));
            }
            _ => {}
        }
    }

    // ── Keyboard shortcuts ───────────────────────────────────────────────
    if config.keyboard_delete && ui.is_key_pressed(Key::Delete) && !state.selected.is_empty() {
        actions.push(GraphAction::DeleteSelected);
    }

    if config.keyboard_select_all && ui.is_key_pressed(Key::A) && io.key_ctrl() {
        actions.push(GraphAction::SelectAll);
    }

    if config.keyboard_escape_cancel && ui.is_key_pressed(Key::Escape) {
        // Cancel wire drag
        state.new_wire = None;
        // Cancel rect selection
        state.rect_select = None;
    }

    // ── Interactive minimap ──────────────────────────────────────────────
    if config.show_minimap && config.minimap_interactive {
        input_hittest::handle_minimap_input(
            graph,
            state,
            config,
            viewer,
            ui,
            canvas_pos,
            canvas_size,
        );
    }
}

// Hit testing (`hit_test`, `comment_hit_test`, `is_collapse_button_hit`) and
// interactive minimap navigation live in the sibling `input_hittest` module to
// keep this file under the 500-line cap.
