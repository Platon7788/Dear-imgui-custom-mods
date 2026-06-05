//! Rendering for [`TabControl`](super::TabControl).
//!
//! Split into cohesive sub-modules around the single-pass design. One
//! pre-pass hit-test fills [`TabHitRow`]; both drawing and event handling
//! read from the same scratch buffer — no duplicate geometry.
//!
//! - [`strip`] — the per-frame tab-strip driver (layout + draw + dispatch).
//! - [`hittest`] — the hit-test pre-pass (`fill_hit_scratch`) and scroll math
//!   (`scroll_into_view`).
//! - [`body`] — the empty-state placeholder and the active-tab body frame.
//! - [`events`] — click / middle-click / right-click / hover / preview dispatch.
//! - [`drag`] — drag-and-drop reorder.
//! - [`keyboard`] — focus-gated keyboard navigation.
//! - [`buttons`] — scroll arrows, overflow `…` dropdown, add `+` button and
//!   the close-confirmation modal.
//! - [`draw`] — per-tab visual styles (pill / underline / square), tab content
//!   (icon / title / dot / badge / close) and the parametric close glyph.

mod body;
mod buttons;
mod drag;
mod draw;
mod events;
mod hittest;
mod keyboard;
mod strip;

// Re-export for the deterministic scroll-math unit tests (see
// `super::tests::scroll`). The renderer itself reaches `scroll_into_view`
// through the `hittest` submodule.
#[cfg(test)]
pub(crate) use hittest::scroll_into_view;

use dear_imgui_rs::Ui;

use super::config::TabControlConfig;
use super::types::*;
use super::{TabControl, TabItem};

// ─── Tunable constants ──────────────────────────────────────────────────────

/// Convert a `TabColors` `[u8; 3]` token to an RGBA `[f32; 4]` for
/// `Ui::text_colored` (which wants linear floats).
#[inline]
pub(super) fn rgba(c: [u8; 3], a: f32) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        a,
    ]
}

/// Duration of the tab close animation, seconds.
pub(super) const TAB_CLOSE_ANIMATION_SECS: f32 = 0.15;
/// Duration of the tab open animation, seconds.
pub(super) const TAB_OPEN_ANIMATION_SECS: f32 = 0.12;
/// Maximum time between two clicks to count as a double-click, seconds.
pub(super) const DOUBLE_CLICK_THRESHOLD_SECS: f64 = 0.35;
/// Pixels of horizontal mouse movement before a tab drag begins.
pub(super) const DRAG_START_THRESHOLD_PX: f32 = 5.0;
/// Smooth-scroll exponential coefficient (higher = snappier).
/// Exponential-decay coefficient for the smooth-scroll easing —
/// `scroll_offset += diff * (1 - exp(-COEF * dt))` per frame. Higher
/// = faster ease. `28.0` lands on the active tab in ~3 frames @ 60 fps
/// (1/e time ≈ 36 ms) — reads as "instant but not jarring", per user
/// feedback 2026-04-30 ("уменьшить анимацию"). Earlier `14.0` felt
/// sluggish on long jumps; the temporary hard-snap (commit `feea46f`)
/// felt too abrupt — this is the middle ground.
pub(super) const SMOOTH_SCROLL_COEF: f32 = 28.0;
/// Per-side hit-area padding for close buttons (matches the visual hover bg).
pub(super) const CLOSE_HIT_PAD: f32 = 2.0;

// ─── Hit-test scratch ───────────────────────────────────────────────────────

/// One row of the tab hit-test scratch buffer, computed once per frame.
///
/// Fields: `(idx, x0, x1, tw, tab_hovered, close_hit)`.
pub(crate) type TabHitRow = (usize, f32, f32, f32, bool, bool);

// ─── Tab draw context (for style dispatch) ─────────────────────────────────

/// Per-tab draw context handed to the style dispatch in [`draw`].
pub(super) struct TabDraw<'a, T: TabItem> {
    pub(super) draw: &'a dear_imgui_rs::DrawListMut<'a>,
    pub(super) item: &'a T,
    pub(super) cfg: &'a TabControlConfig,
    pub(super) is_active: bool,
    pub(super) hovered: bool,
    pub(super) close_hovered: bool,
    /// Combined animation alpha (0..1) — fades tab content during open/close.
    pub(super) anim_alpha: u8,
    pub(super) accent: [u8; 3],
    pub(super) x0: f32,
    pub(super) y0: f32,
    pub(super) x1: f32,
    pub(super) y1: f32,
    pub(super) time: f32,
}

// ─── Main entry point ───────────────────────────────────────────────────────

pub(crate) fn render_tab_control<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) -> Option<TabAction> {
    pc.open_context_menu = false;

    tick_animations(pc, ui);

    let mut action: Option<TabAction> = None;

    if pc.tabs.is_empty() {
        if pc.config.show_empty_placeholder {
            body::render_empty_placeholder(ui, &pc.config);
        }
    } else {
        action = strip::render_strip(pc, ui);
    }

    buttons::render_close_popup(pc, ui);

    // Process deferred closes (animation finished → tab.open=false)
    let mut closed_id: Option<TabId> = None;
    pc.tabs.retain(|t| {
        if t.open {
            true
        } else {
            closed_id = Some(t.id);
            false
        }
    });
    if let Some(id) = closed_id {
        if pc.active == Some(id) {
            pc.active = pc.tabs.last().map(|t| t.id);
            if let Some(new_id) = pc.active
                && let Some(entry) = pc.tabs.iter_mut().find(|t| t.id == new_id)
            {
                entry.item.on_activated();
            }
        }
        pc.invalidate_tab_layout_cache();
        action = Some(TabAction::Closed(id));
    }

    action
}

// ─── Animation tick ─────────────────────────────────────────────────────────

fn tick_animations<T: TabItem>(pc: &mut TabControl<T>, ui: &Ui) {
    let dt = ui.io().delta_time();

    // Close animation
    if let Some((closing_id, ref mut frac)) = pc.closing_tab {
        *frac -= dt / TAB_CLOSE_ANIMATION_SECS;
        if *frac <= 0.0 {
            if let Some(tab) = pc.tabs.iter_mut().find(|t| t.id == closing_id) {
                tab.open = false;
            }
            pc.closing_tab = None;
        }
    }

    // Open animation: every tab with open_anim < 1 advances toward 1.
    // The animation only multiplies the rendered width inside
    // `fill_hit_scratch` — the cached *base* width doesn't change, so we
    // don't need to invalidate `tab_widths_cache` per frame. This saves a
    // `calc_text_size` per tab per frame for the duration of the animation.
    if pc.config.animate_open {
        let step = dt / TAB_OPEN_ANIMATION_SECS;
        for tab in &mut pc.tabs {
            if tab.open_anim < 1.0 {
                tab.open_anim = (tab.open_anim + step).min(1.0);
            }
        }
    }
}
