//! Drag-and-drop reorder.
//!
//! Walks tab midpoints in the dragged tab's group (pinned ↔ pinned, regular ↔
//! regular — crossing the boundary is forbidden), performs one neighbour swap
//! per frame as the cursor crosses a midpoint, and paints the drag indicator +
//! translucent ghost tab.

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::rgb_arr as c32;
use crate::utils::text::calc_text_size;

use super::super::types::*;
use super::super::{TabControl, TabItem};
use super::DRAG_START_THRESHOLD_PX;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_drag<T: TabItem>(
    pc: &mut TabControl<T>,
    ui: &Ui,
    mouse: [f32; 2],
    strip_x: f32,
    tabs_origin_x: f32,
    strip_y: f32,
    action: &mut Option<TabAction>,
) {
    let Some(src_idx) = pc.drag_source_idx else {
        return;
    };
    let mouse_held = ui.is_mouse_down(MouseButton::Left);
    if !mouse_held {
        pc.drag_source_idx = None;
        pc.dragging = false;
        return;
    }

    let dx = (mouse[0] - pc.drag_start_x).abs();
    if dx > DRAG_START_THRESHOLD_PX {
        pc.dragging = true;
    }
    if !pc.dragging {
        return;
    }

    // Pinned tabs only swap with other pinned, regular only with regular —
    // crossing the boundary is forbidden. This keeps the visual zones intact.
    let src_is_pinned = pc.tabs.get(src_idx).is_some_and(|t| t.item.is_pinned());

    // Find target index by walking midpoints in the appropriate origin.
    let cfg = &pc.config;
    let origin = if src_is_pinned {
        strip_x
    } else {
        tabs_origin_x - pc.scroll_offset
    };
    let mut tx2 = origin;
    let mut target_idx: Option<usize> = None;
    for j in 0..pc.tabs.len() {
        let same_group = pc.tabs[j].item.is_pinned() == src_is_pinned;
        if !same_group {
            continue;
        }
        let Some(&tw2) = pc.tab_widths_cache.get(j) else {
            break;
        };
        let mid = tx2 + tw2 * 0.5;
        if mouse[0] < mid {
            target_idx = Some(j);
            break;
        }
        tx2 += tw2 + cfg.tab_gap;
    }
    // Clamp target to last tab in the same group
    let last_in_group = pc
        .tabs
        .iter()
        .enumerate()
        .rev()
        .find(|(_, t)| t.item.is_pinned() == src_is_pinned)
        .map(|(i, _)| i);
    let target = target_idx.or(last_in_group).unwrap_or(src_idx);

    // Pinned invariant guarantees `pc.tabs` is partitioned (pinned prefix,
    // regular suffix). `target_idx` was selected from the same group as
    // `src_idx`, so `src_idx ± 1` stays within that group: a plain swap is
    // sufficient and never crosses the boundary.
    if target != src_idx && target < pc.tabs.len() {
        let new_src = if target < src_idx {
            pc.tabs.swap(src_idx, src_idx - 1);
            src_idx - 1
        } else {
            pc.tabs.swap(src_idx, src_idx + 1);
            src_idx + 1
        };
        // Release-mode guard (M3 from session 034 audit) — if a future change
        // to the `target_idx` selection ever returns an out-of-group index,
        // `enforce_pinned_partition` would observe corruption only on the next
        // frame. Bail out here so the swap is reverted before the partition
        // pass runs.
        if pc.tabs[new_src].item.is_pinned() != src_is_pinned {
            // Undo the offending swap.
            pc.tabs.swap(src_idx, new_src);
            return;
        }
        debug_assert_eq!(
            pc.tabs[new_src].item.is_pinned(),
            src_is_pinned,
            "drag swap escaped its group — pinned partition broken"
        );
        pc.drag_source_idx = Some(new_src);
        pc.tab_gen = pc.tab_gen.wrapping_add(1);
        let moved_id = pc.tabs[new_src].id;
        *action = Some(TabAction::Reordered(moved_id));
    }

    // Drag indicator + ghost tab
    let draw = ui.get_window_draw_list();
    let drag_y = strip_y + cfg.strip_padding_v;
    draw.add_line(
        [mouse[0], drag_y],
        [mouse[0], drag_y + cfg.tab_height],
        c32(cfg.colors.accent, 220),
    )
    .thickness(2.0)
    .build();

    if let Some(&tw) = pc.tab_widths_cache.get(src_idx) {
        let ghost_x0 = mouse[0] - tw * 0.5;
        let ghost_y0 = drag_y;
        let ghost_x1 = ghost_x0 + tw;
        let ghost_y1 = ghost_y0 + cfg.tab_height;
        let alpha_mul = 0.55_f32;
        let tab = &pc.tabs[src_idx];
        let accent = tab
            .item
            .tab_color()
            .unwrap_or_else(|| cfg.colors.status_color(tab.item.status()));
        let bg = c32(cfg.colors.tab_active, (235.0 * alpha_mul) as u8);
        draw.add_rect([ghost_x0, ghost_y0], [ghost_x1, ghost_y1], bg)
            .filled(true)
            .rounding(cfg.tab_rounding)
            .build();
        draw.add_rect([ghost_x0, ghost_y0], [ghost_x1, ghost_y1], c32(accent, 200))
            .filled(false)
            .thickness(1.5)
            .rounding(cfg.tab_rounding)
            .build();
        let title = tab.item.title();
        let ts = calc_text_size(title);
        let tx = ghost_x0 + (tw - ts[0]) * 0.5;
        let ty = ghost_y0 + (cfg.tab_height - ts[1]) * 0.5;
        // Drag ghost is a shadow of the dragged tab — honour the per-tab
        // `text_color()` override so a colored tab keeps its hue while being
        // dragged.
        let ghost_text = tab.item.text_color().unwrap_or(cfg.colors.text);
        let fg = c32(ghost_text, (240.0 * alpha_mul) as u8);
        draw.add_text([tx, ty], fg, title);
    }
}
