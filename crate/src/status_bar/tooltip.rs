//! Foreground-draw-list tooltip painter for the overlay-foreground
//! render path. Split out of `mod.rs` so the rendering and tooltip
//! geometry live in cohesive files under the 500-line cap.

use super::*;

/// 2026-05-25 (vex0r session 130) — paint a tooltip body directly
/// into a foreground draw list, positioned ABOVE the cursor so the
/// status bar (which lives on the same foreground draw list) cannot
/// overlap it.
///
/// Used by `render_overlay_foreground` because a normal
/// `ui.tooltip(..)` body would land in a TopLayer ImGui window —
/// which paints BELOW the foreground draw list and gets sliced by
/// the bar strip the user is hovering. Painting into the SAME
/// foreground draw list with `cursor.y - box_h - 8 px gap` puts
/// the tooltip body above the cursor and outside the bar strip.
///
/// Geometry mirrors the crate-wide `themed_tooltip` look: 10×8 px
/// padding, 4 px corner rounding, 1 px border in the bar's
/// separator colour, background in the bar's bg colour but at
/// `0.95` alpha so the chrome under it bleeds through a touch and
/// the tooltip reads as floating rather than baked-in.
pub(super) fn paint_foreground_tooltip(
    ui: &Ui,
    draw: &dear_imgui_rs::DrawListMut,
    cfg: &StatusBarConfig,
    text: &str,
) {
    const PAD_X: f32 = 10.0;
    const PAD_Y: f32 = 8.0;
    const ROUND: f32 = 4.0;
    const CURSOR_GAP_Y: f32 = 8.0; // gap between cursor and tooltip bottom

    let text_size = calc_text_size(text);
    let box_w = text_size[0] + 2.0 * PAD_X;
    let box_h = text_size[1] + 2.0 * PAD_Y;

    let mouse = ui.io().mouse_pos();
    let display = ui.io().display_size();

    // Anchor box above the cursor; clamp inside the viewport.
    let mut tip_y = mouse[1] - box_h - CURSOR_GAP_Y;
    if tip_y < 4.0 {
        // No room above — fall back to below the cursor. Bar's
        // foreground body lives at the very bottom strip, so anything
        // above the cursor is fine; below-cursor would only be needed
        // if cursor is near the TOP of the viewport, where there's no
        // bar to overlap anyway.
        tip_y = (mouse[1] + CURSOR_GAP_Y).min(display[1] - box_h - 4.0);
    }
    let tip_x = mouse[0].min(display[0] - box_w - 4.0).max(4.0);

    // Background — slight transparency so the tooltip floats above
    // the chrome rather than baking in as part of the bar.
    let mut bg = cfg.colors.bg;
    bg[3] = 0.95;
    draw.add_rect([tip_x, tip_y], [tip_x + box_w, tip_y + box_h], col32(bg))
        .filled(true)
        .rounding(ROUND)
        .build();
    draw.add_rect(
        [tip_x, tip_y],
        [tip_x + box_w, tip_y + box_h],
        col32(cfg.colors.separator),
    )
    .rounding(ROUND)
    .thickness(1.0)
    .build();
    draw.add_text([tip_x + PAD_X, tip_y + PAD_Y], col32(cfg.colors.text), text);
}
