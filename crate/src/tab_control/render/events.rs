//! Pointer interaction dispatch: click / middle-click / right-click / hover /
//! preview.
//!
//! Reads the pre-computed `pc.hit_scratch` filled by
//! [`super::hittest::fill_hit_scratch`] — geometry is never recomputed here.
//! Drag-and-drop lives in [`super::drag`], keyboard nav in [`super::keyboard`].

use dear_imgui_rs::Ui;

use super::super::types::*;
use super::super::{TabControl, TabItem};
use super::DOUBLE_CLICK_THRESHOLD_SECS;
use super::hittest::scroll_into_view;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_tab_events<T: TabItem>(
    pc: &mut TabControl<T>,
    ui: &Ui,
    action: &mut Option<TabAction>,
    clicked: bool,
    middle_clicked: bool,
    right_clicked: bool,
    hover_activate_ms: Option<u32>,
    preview_hover_ms: Option<u32>,
    scroll_area_w: f32,
) {
    let cfg_closable = pc.config.closable;
    let cfg_middle_close = pc.config.middle_click_close;
    let cfg_context = pc.config.context_menu;
    let cfg_confirm = pc.config.confirm_close;
    let cfg_animate = pc.config.animate_close;

    let mut close_target: Option<TabId> = None;
    let mut activate_target: Option<TabId> = None;
    let mut activate_idx: Option<usize> = None;
    let mut context_target: Option<TabId> = None;
    let mut double_click_target: Option<TabId> = None;
    let mut tooltip_idx: Option<usize> = None;
    let mut drag_idx: Option<(usize, f32)> = None;

    // Track which tab the mouse is currently over (any), and the inactive one
    // separately (used for hover-delayed activation).
    let mut hovered_id: Option<TabId> = None;
    let mut hovered_inactive_id: Option<TabId> = None;

    for &(idx, _x0, _x1, _tw, hovered, close_hit) in &pc.hit_scratch {
        if !hovered {
            continue;
        }
        let tab = &pc.tabs[idx];
        let can_close = cfg_closable && tab.item.is_closable();

        hovered_id = Some(tab.id);
        if pc.active != Some(tab.id) {
            hovered_inactive_id = Some(tab.id);
        }

        if clicked {
            if close_hit && can_close {
                close_target = Some(tab.id);
            } else {
                let now = ui.time();
                if pc.last_click_tab == Some(tab.id)
                    && (now - pc.last_click_time) < DOUBLE_CLICK_THRESHOLD_SECS
                {
                    double_click_target = Some(tab.id);
                    pc.last_click_tab = None;
                } else {
                    pc.last_click_time = now;
                    pc.last_click_tab = Some(tab.id);
                }
                drag_idx = Some((idx, ui.io().mouse_pos()[0]));
                if pc.active != Some(tab.id) {
                    activate_target = Some(tab.id);
                    activate_idx = Some(idx);
                }
            }
        }

        if middle_clicked && cfg_middle_close && can_close {
            close_target = Some(tab.id);
        }

        if right_clicked && cfg_context {
            context_target = Some(tab.id);
        }

        if !clicked && !middle_clicked && !right_clicked && tab.item.tooltip().is_some() {
            tooltip_idx = Some(idx);
        }
    }

    // Update hover_target based on which tab is hovered this frame.
    // hover_target = Some((id, time when hover began)).
    let now = ui.time();
    pc.hover_target = match (hovered_id, pc.hover_target) {
        (Some(id), Some((tracked, started))) if tracked == id => Some((tracked, started)),
        (Some(id), _) => Some((id, now)),
        (None, _) => None,
    };

    // Hover-delayed activation (Edge / Win11 style)
    if let Some(ms) = hover_activate_ms
        && activate_target.is_none()
        && let Some(id) = hovered_inactive_id
        && let Some((tracked, started)) = pc.hover_target
        && tracked == id
        && (now - started) * 1000.0 >= ms as f64
    {
        activate_target = Some(id);
        activate_idx = pc.tabs.iter().position(|t| t.id == id);
    }

    // Hover preview popup (Windows-taskbar-peek style). Triggered for
    // *inactive* tabs only — peeking the active tab is pointless since the
    // user is already looking at its content.
    //
    // We render directly into the tooltip window (no child_window) so the
    // tooltip auto-sizes vertically to the actual content. A
    // `SetNextWindowSizeConstraints` call locks the *width* to
    // `cfg.preview_size[0]`, which makes `text_wrapped` and similar widgets
    // wrap consistently while letting height grow freely up to the safety
    // ceiling. Result: full content visible, no scrollbar, ever.
    if let Some(ms) = preview_hover_ms
        && pc.drag_source_idx.is_none()              // never popup during drag
        && pc.active != hovered_id
        && let Some((tracked, started)) = pc.hover_target
        && hovered_id == Some(tracked)
        && (now - started) * 1000.0 >= ms as f64
        && let Some(idx) = pc.tabs.iter().position(|t| t.id == tracked)
        && pc.tabs[idx].item.show_preview()
    // per-tab opt-out
    {
        let preview_w = pc.config.preview_size[0];
        let preview_h_max = pc.config.preview_size[1].max(64.0) * 8.0; // generous upper bound
        let preview_scale = pc.config.preview_font_scale;
        let title = pc.tabs[idx].item.title().to_string();
        let tab_id = pc.tabs[idx].id;
        let item: &mut T = &mut pc.tabs[idx].item;

        // SAFETY: `igSetNextWindowSizeConstraints` is a side-effect-only ImGui
        // call that records the constraint for the *next* `Begin*`. Both
        // `ImVec2_c` values are plain `repr(C)` floats with no invariants;
        // the optional callback pointer is `None` and the userdata is null.
        // No ImGui state is mutated across-thread (single-threaded UI).
        unsafe {
            dear_imgui_rs::sys::igSetNextWindowSizeConstraints(
                dear_imgui_rs::sys::ImVec2_c {
                    x: preview_w,
                    y: 0.0,
                },
                dear_imgui_rs::sys::ImVec2_c {
                    x: preview_w,
                    y: preview_h_max,
                },
                None,
                std::ptr::null_mut(),
            );
        }

        crate::utils::themed_tooltip(ui, || {
            // ID stack push isolates widget IDs in the preview from any other
            // place rendering the same content tree (e.g. an inner TabControl
            // re-rendered as a thumbnail).
            let _id = ui.push_id(tab_id as usize);
            // `set_window_font_scale` is a per-window write; the tooltip is its
            // own window so the scale doesn't leak outside this closure when
            // the tooltip closes. The explicit `1.0` restore at the tail covers
            // nested `render_preview` impls that opened sibling tooltips during
            // the body — those would otherwise inherit the shrunk scale (M5
            // from session 034 audit). Real save / restore would need unsafe
            // `igGetCurrentWindow` access; the `1.0` clamp is the practical
            // compromise.
            ui.set_window_font_scale(preview_scale);
            ui.text(title);
            ui.separator();
            item.render_preview(ui);
            ui.set_window_font_scale(1.0);
        });
    }

    // Apply collected actions
    if let Some(id) = close_target {
        if cfg_confirm {
            pc.pending_close = Some(id);
            pc.pending_close_new = true;
        } else if cfg_animate {
            pc.closing_tab = Some((id, 1.0));
        } else if let Some(t) = pc.tabs.iter_mut().find(|t| t.id == id) {
            t.open = false;
        }
    }

    if let (Some(new_id), Some(idx)) = (activate_target, activate_idx) {
        if let Some(old_id) = pc.active
            && let Some(old) = pc.tabs.iter_mut().find(|t| t.id == old_id)
        {
            old.item.on_deactivated();
        }
        pc.active = Some(new_id);
        if let Some(entry) = pc.tabs.iter_mut().find(|t| t.id == new_id) {
            entry.item.on_activated();
        }
        *action = Some(TabAction::Activated(new_id));
        // Snap scroll to the clicked tab — the tab was probably visible
        // already, but partial-overlap clicks (tab half-clipped at the right
        // edge) feel more decisive when the strip jumps the few remaining
        // pixels in one frame.
        scroll_into_view(pc, idx, scroll_area_w);
    }

    if let Some((idx, x)) = drag_idx {
        pc.drag_source_idx = Some(idx);
        pc.drag_start_x = x;
        pc.dragging = false;
    }

    if let Some(id) = context_target {
        pc.context_tab = Some(id);
        pc.open_context_menu = true;
    }

    if let Some(id) = double_click_target {
        *action = Some(TabAction::DoubleClicked(id));
    }

    if let Some(idx) = tooltip_idx
        && let Some(tip) = pc.tabs[idx].item.tooltip()
    {
        crate::utils::themed_tooltip(ui, || ui.text(tip));
    }
}
