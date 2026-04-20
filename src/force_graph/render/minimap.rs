//! Minimap overlay for the force-graph canvas.
//!
//! Renders a thumbnail of the full graph in the bottom-right corner, with a
//! viewport rectangle that shows the currently visible area. Clicking or
//! dragging on the minimap pans the main camera to that world position.

use dear_imgui_rs::{DrawListMut, MouseButton, Ui};

use super::super::data::GraphData;
use super::camera::Camera;
use crate::utils::color::pack_color_f32;

const MINIMAP_W: f32 = 160.0;
const MINIMAP_H: f32 = 100.0;
const MINIMAP_PAD: f32 = 8.0;

/// Draw the minimap overlay and handle click/drag panning.
///
/// Returns `true` if the minimap consumed mouse input this frame (callers
/// should suppress main canvas interaction when this is the case).
pub(crate) fn render_minimap(
    ui: &Ui,
    graph: &GraphData,
    camera: &mut Camera,
    canvas_min: [f32; 2],
    canvas_size: [f32; 2],
    draw: &DrawListMut<'_>,
) -> bool {
    // Anchor: bottom-right of the canvas.
    let mm_min = [
        canvas_min[0] + canvas_size[0] - MINIMAP_W - MINIMAP_PAD,
        canvas_min[1] + canvas_size[1] - MINIMAP_H - MINIMAP_PAD,
    ];
    let mm_max = [mm_min[0] + MINIMAP_W, mm_min[1] + MINIMAP_H];

    // ── Background ────────────────────────────────────────────────────────────
    draw.add_rect(mm_min, mm_max, pack_color_f32([0.06, 0.06, 0.08, 0.92]))
        .filled(true)
        .build();
    draw.add_rect(mm_min, mm_max, pack_color_f32([0.35, 0.35, 0.45, 1.0]))
        .filled(false)
        .build();

    if graph.node_count() == 0 {
        return false;
    }

    // ── Graph world-space bounding box ────────────────────────────────────────
    let mut gmin = [f32::INFINITY; 2];
    let mut gmax = [f32::NEG_INFINITY; 2];
    for (_, node) in graph.nodes.iter() {
        gmin[0] = gmin[0].min(node.pos[0]);
        gmin[1] = gmin[1].min(node.pos[1]);
        gmax[0] = gmax[0].max(node.pos[0]);
        gmax[1] = gmax[1].max(node.pos[1]);
    }
    let pad = 20.0_f32;
    gmin[0] -= pad;
    gmin[1] -= pad;
    gmax[0] += pad;
    gmax[1] += pad;
    let gw = (gmax[0] - gmin[0]).max(1.0);
    let gh = (gmax[1] - gmin[1]).max(1.0);

    // Closure: world → minimap screen coords.
    let world_to_mm = |p: [f32; 2]| -> [f32; 2] {
        [
            mm_min[0] + ((p[0] - gmin[0]) / gw) * MINIMAP_W,
            mm_min[1] + ((p[1] - gmin[1]) / gh) * MINIMAP_H,
        ]
    };

    // ── Node dots ─────────────────────────────────────────────────────────────
    for (_, node) in graph.nodes.iter() {
        let sp = world_to_mm(node.pos);
        if sp[0] < mm_min[0] || sp[0] > mm_max[0] || sp[1] < mm_min[1] || sp[1] > mm_max[1] {
            continue;
        }
        let dot_col = node.style.color.unwrap_or([0.55, 0.70, 0.95, 0.85]);
        draw.add_circle(sp, 1.8, pack_color_f32(dot_col))
            .filled(true)
            .num_segments(4)
            .build();
    }

    // ── Viewport rectangle ────────────────────────────────────────────────────
    // The visible world region is what screen_to_world maps the canvas corners to.
    let vp_wmin = camera.screen_to_world(canvas_min, canvas_min);
    let vp_wmax = camera.screen_to_world(
        [
            canvas_min[0] + canvas_size[0],
            canvas_min[1] + canvas_size[1],
        ],
        canvas_min,
    );
    let vp_mm_min = world_to_mm(vp_wmin);
    let vp_mm_max = world_to_mm(vp_wmax);

    // Clamp to minimap bounds for large zoom-outs.
    let vp_mm_min = [vp_mm_min[0].max(mm_min[0]), vp_mm_min[1].max(mm_min[1])];
    let vp_mm_max = [vp_mm_max[0].min(mm_max[0]), vp_mm_max[1].min(mm_max[1])];

    draw.add_rect(
        vp_mm_min,
        vp_mm_max,
        pack_color_f32([0.95, 0.95, 0.95, 0.15]),
    )
    .filled(true)
    .build();
    draw.add_rect(
        vp_mm_min,
        vp_mm_max,
        pack_color_f32([0.95, 0.95, 0.95, 0.70]),
    )
    .filled(false)
    .build();

    // ── Click / drag → pan main camera ────────────────────────────────────────
    let io = ui.io();
    let mouse = io.mouse_pos();
    let in_mm = mouse[0] >= mm_min[0]
        && mouse[0] <= mm_max[0]
        && mouse[1] >= mm_min[1]
        && mouse[1] <= mm_max[1];

    if in_mm && (ui.is_mouse_clicked(MouseButton::Left) || ui.is_mouse_down(MouseButton::Left)) {
        let tx = (mouse[0] - mm_min[0]) / MINIMAP_W;
        let ty = (mouse[1] - mm_min[1]) / MINIMAP_H;
        let world_target = [gmin[0] + tx * gw, gmin[1] + ty * gh];
        camera.animate_to_node(world_target, canvas_size, camera.zoom);
        return true;
    }

    false
}
