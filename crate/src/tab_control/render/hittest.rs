//! The single-pass hit-test pre-pass and scroll-into-view math.
//!
//! [`fill_hit_scratch`] computes every tab's screen rectangle + hover/close-hit
//! state once per frame into `pc.hit_scratch`; both [`super::draw`] (drawing)
//! and [`super::events`] (interaction) read from that buffer, so geometry is
//! never recomputed. [`scroll_into_view`] nudges `scroll_target` so a given
//! regular tab becomes visible.

use super::super::config::TabControlConfig;
use super::super::types::TabId;
use super::super::{TabControl, TabItem};
use super::CLOSE_HIT_PAD;

/// Two-pass fill: pinned tabs first (left-anchored, no scroll), then regular
/// tabs (with scroll, clipped to the regular area). Order in `hit_scratch`
/// matches the visual order, which keeps drawing/event code simple.
#[allow(clippy::too_many_arguments)]
pub(super) fn fill_hit_scratch<T: TabItem>(
    pc: &mut TabControl<T>,
    accept_clicks: bool,
    mouse: [f32; 2],
    strip_x: f32,
    strip_y_top: f32,
    pinned_max_x: f32,
    regular_origin_x: f32,
    regular_clip_min_x: f32,
    regular_clip_max_x: f32,
) {
    pc.hit_scratch.clear();
    let cfg = &pc.config;
    let y0 = strip_y_top + cfg.strip_padding_v;
    let tab_h = cfg.tab_height;

    // Helper to compute animated width for a tab
    let anim_width = |idx: usize, tab_id: TabId, base_tw: f32| -> f32 {
        let close_frac = match pc.closing_tab {
            Some((cid, frac)) if cid == tab_id => frac.max(0.0),
            _ => 1.0,
        };
        let open_frac = pc.tabs[idx].open_anim.clamp(0.0, 1.0);
        base_tw * close_frac * open_frac
    };

    // Pass 1 — pinned (no scroll, hit-tested against pinned strip area)
    let mut tx = strip_x;
    for (i, tab) in pc.tabs.iter().enumerate() {
        if !tab.item.is_pinned() {
            continue;
        }
        let Some(&base_tw) = pc.tab_widths_cache.get(i) else {
            break;
        };
        let tw = anim_width(i, tab.id, base_tw);
        if tw < 1.0 {
            tx += cfg.tab_gap;
            continue;
        }
        let x0 = tx;
        let x1 = tx + tw;
        let outside = x1 < strip_x || x0 > pinned_max_x;
        let hovered = !outside
            && accept_clicks
            && mouse[0] >= x0.max(strip_x)
            && mouse[0] < x1.min(pinned_max_x)
            && mouse[1] >= y0
            && mouse[1] < y0 + tab_h;
        // Pinned tabs never expose a close button.
        pc.hit_scratch.push((i, x0, x1, tw, hovered, false));
        tx += tw + cfg.tab_gap;
    }

    // Pass 2 — regular (with scroll, clipped to regular strip)
    let mut tx = regular_origin_x - pc.scroll_offset;
    for (i, tab) in pc.tabs.iter().enumerate() {
        if tab.item.is_pinned() {
            continue;
        }
        let Some(&base_tw) = pc.tab_widths_cache.get(i) else {
            break;
        };
        let tw = anim_width(i, tab.id, base_tw);
        if tw < 1.0 {
            tx += cfg.tab_gap;
            continue;
        }
        let x0 = tx;
        let x1 = tx + tw;
        let outside = x1 < regular_clip_min_x || x0 > regular_clip_max_x;
        let hovered = !outside
            && accept_clicks
            && mouse[0] >= x0.max(regular_clip_min_x)
            && mouse[0] < x1.min(regular_clip_max_x)
            && mouse[1] >= y0
            && mouse[1] < y0 + tab_h;
        let can_close = cfg.closable && tab.item.is_closable();
        let close_hit = hovered
            && can_close
            && is_close_hovered(mouse, x1, y0, cfg, regular_clip_min_x, regular_clip_max_x);
        pc.hit_scratch.push((i, x0, x1, tw, hovered, close_hit));
        tx += tw + cfg.tab_gap;
    }

    // Publish each tab's hovered state (from this frame's geometry) back onto
    // the entry so the next frame's `hover_anim` tick has a target to ease
    // toward. `hit_scratch` and `tabs` are disjoint fields, so this write-back
    // doesn't conflict with the `&pc.config`/`&pc.tabs` reads above.
    for &(idx, _, _, _, hovered, _) in &pc.hit_scratch {
        pc.tabs[idx].hovered = hovered;
    }
}

#[inline]
fn is_close_hovered(
    mouse: [f32; 2],
    x1: f32,
    y0: f32,
    cfg: &TabControlConfig,
    clip_min_x: f32,
    clip_max_x: f32,
) -> bool {
    let cx = x1 - cfg.tab_padding_h - cfg.close_btn_size;
    let cy_center = y0 + cfg.tab_height * 0.5;
    let half = cfg.close_btn_size * 0.5 + CLOSE_HIT_PAD;
    mouse[0] >= (cx - CLOSE_HIT_PAD).max(clip_min_x)
        && mouse[0] < (cx + cfg.close_btn_size + CLOSE_HIT_PAD).min(clip_max_x)
        && mouse[1] >= cy_center - half
        && mouse[1] < cy_center + half
}

// ─── Scroll-into-view ───────────────────────────────────────────────────────

// `pub(crate)` (not `pub(super)`) so the deterministic unit tests in
// `super::super::tests::scroll` can drive it directly — it reads only
// `tab_widths_cache` + `tabs`, never `calc_text_size`, so it's safe to call
// outside an ImGui context once the cache is hand-populated.
pub(crate) fn scroll_into_view<T: TabItem>(pc: &mut TabControl<T>, idx: usize, scroll_area_w: f32) {
    let cfg = &pc.config;
    let Some(&tw) = pc.tab_widths_cache.get(idx) else {
        return;
    };
    // Pinned tabs live in the fixed, non-scrolling left strip — they are
    // always visible, so scrolling toward one is meaningless and would in
    // fact corrupt `scroll_target` (the pinned coordinate space differs from
    // the regular scroll space). Bail out for pinned indices.
    if pc.tabs.get(idx).is_some_and(|t| t.item.is_pinned()) {
        return;
    }
    // Accumulate the offset of `idx` **within the regular section only**:
    // the regular scroll origin (`tabs_origin_x`) is the left edge of the
    // first regular tab, so a regular tab's scroll-space x is the sum of the
    // preceding *regular* widths + gaps. Including pinned widths here would
    // over-count by `pinned_total_w` and mis-target the scroll (pinned tabs
    // are drawn left-anchored, never scrolled).
    let mut tx: f32 = 0.0;
    for (w, tab) in pc.tab_widths_cache.iter().zip(pc.tabs.iter()).take(idx) {
        if tab.item.is_pinned() {
            continue;
        }
        tx += w + cfg.tab_gap;
    }
    if tx < pc.scroll_target {
        pc.scroll_target = tx;
    } else if tx + tw > pc.scroll_target + scroll_area_w {
        pc.scroll_target = tx + tw - scroll_area_w;
    }
}
