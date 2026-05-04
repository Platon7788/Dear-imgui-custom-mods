//! Canvas grid rendering.

use crate::utils::color::rgb_arr as c32;

use super::super::config::NodeGraphConfig;
use super::super::state::Viewport;

/// Render the canvas background grid with support for rotation.
pub(super) fn render_grid(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    config: &NodeGraphConfig,
    vp: &Viewport,
    canvas_pos: [f32; 2],
    canvas_size: [f32; 2],
) {
    let colors = &config.colors;
    let grid = config.grid_size * vp.zoom;
    if grid < 4.0 {
        return; // Too small to see
    }

    let thick_every = config.grid_thick_every as i32;
    let rotation = config.grid_rotation;

    if rotation.abs() < 0.01 {
        // ── Fast path: axis-aligned grid (no rotation) ────────────────
        let off_x = vp.offset[0] % grid;
        let off_y = vp.offset[1] % grid;
        let first_col = ((-vp.offset[0]) / grid).floor() as i32;
        let first_row = ((-vp.offset[1]) / grid).floor() as i32;

        let x_end = canvas_pos[0] + canvas_size[0];
        let y_end = canvas_pos[1] + canvas_size[1];

        let mut i = 0;
        let mut x = canvas_pos[0] + off_x;
        while x < x_end {
            let col_idx = first_col + i;
            let is_thick = thick_every > 0 && col_idx % thick_every == 0;
            let color = if is_thick {
                c32(colors.grid_line_thick, 255)
            } else {
                c32(colors.grid_line, 255)
            };
            draw.add_line([x, canvas_pos[1]], [x, y_end], color).build();
            x += grid;
            i += 1;
        }

        let mut j = 0;
        let mut y = canvas_pos[1] + off_y;
        while y < y_end {
            let row_idx = first_row + j;
            let is_thick = thick_every > 0 && row_idx % thick_every == 0;
            let color = if is_thick {
                c32(colors.grid_line_thick, 255)
            } else {
                c32(colors.grid_line, 255)
            };
            draw.add_line([canvas_pos[0], y], [x_end, y], color).build();
            y += grid;
            j += 1;
        }
    } else {
        // ── Rotated grid ──────────────────────────────────────────────
        let rad = rotation.to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();

        // Canvas center in canvas-local coords
        let cx = canvas_size[0] * 0.5;
        let cy = canvas_size[1] * 0.5;
        // Diagonal of canvas — max reach of a rotated line
        let diag = (canvas_size[0] * canvas_size[0] + canvas_size[1] * canvas_size[1]).sqrt();
        let half_diag = diag * 0.5;

        // How many grid lines we need in each direction from center
        let n = (half_diag / grid).ceil() as i32 + 1;

        // Direction vectors for the two line families
        // Family 1: lines along direction (cos, sin), spaced perpendicular
        // Family 2: lines along direction (-sin, cos), spaced perpendicular

        for family in 0..2 {
            let (dx, dy) = if family == 0 {
                (cos_a, sin_a)
            } else {
                (-sin_a, cos_a)
            };
            // Perpendicular direction for spacing
            let (px, py) = (-dy, dx);

            // Offset in perpendicular direction due to pan
            let pan_proj = vp.offset[0] * px + vp.offset[1] * py;
            let pan_off = pan_proj % grid;

            for i in -n..=n {
                let perp_dist = i as f32 * grid + pan_off;
                // Center of line in canvas-local space
                let lx = cx + px * perp_dist;
                let ly = cy + py * perp_dist;
                // Line endpoints extending along the direction
                let x0 = canvas_pos[0] + lx - dx * half_diag;
                let y0 = canvas_pos[1] + ly - dy * half_diag;
                let x1 = canvas_pos[0] + lx + dx * half_diag;
                let y1 = canvas_pos[1] + ly + dy * half_diag;

                let line_idx = ((i as f32 * grid + pan_proj) / grid).round() as i32;
                let is_thick = thick_every > 0 && line_idx % thick_every == 0;
                let color = if is_thick {
                    c32(colors.grid_line_thick, 255)
                } else {
                    c32(colors.grid_line, 255)
                };
                draw.add_line([x0, y0], [x1, y1], color).build();
            }
        }
    }
}
