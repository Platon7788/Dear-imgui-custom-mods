//! Layout helpers for [`TabControl`](super::TabControl).
//!
//! Pure functions and constants — no rendering, no ImGui state, no I/O. Lives
//! in its own module so `mod.rs` stays focused on the public API surface.
//!
//! ## What's here
//!
//! - **Layout constants** that both `compute_tab_width` and `draw_tab_content`
//!   read from. Keeping them in one place guarantees layout/draw stay in sync.
//! - [`compute_tab_width`] — pixel width of a tab from its `TabItem` traits.
//! - [`enforce_pinned_partition`] — keeps the pinned-prefix invariant on
//!   `TabControl::tabs` after user-driven `is_pinned()` flips.

use crate::utils::text::calc_text_size;

use super::config::TabControlConfig;
use super::types::TabStatus;
use super::{TabControl, TabItem};

// ─── Layout micro-constants ────────────────────────────────────────────────
//
// Single source of truth for sizing. Drawing (render.rs::draw_tab_content)
// and layout reservation (this file) must agree to within a pixel.

/// Diameter of the regular-tab status dot. Layout reserve = drawing diameter.
pub(crate) const STATUS_DOT_DIAM: f32 = 6.0;
/// Gap between status dot and the next element (icon or title).
pub(crate) const STATUS_DOT_GAP: f32 = 5.0;
/// Gap between icon and title.
pub(crate) const ICON_TITLE_GAP: f32 = 6.0;
/// Outer gap before the badge pill.
pub(crate) const BADGE_LEFT_GAP: f32 = 5.0;
/// Badge pill horizontal inner padding (per side).
pub(crate) const BADGE_INNER_PAD_X: f32 = 3.0;
/// Badge pill vertical inner padding.
pub(crate) const BADGE_INNER_PAD_Y: f32 = 1.0;
/// Outer gap after the badge pill (before the close-button slot).
pub(crate) const BADGE_RIGHT_GAP: f32 = 2.0;
/// Width of the pinned/regular vertical separator strip.
pub(crate) const PINNED_SEPARATOR_W: f32 = 8.0;
/// Vertical inset of the pinned/regular separator line (top + bottom).
pub(crate) const PINNED_SEPARATOR_VPAD: f32 = 4.0;

// ─── compute_tab_width ─────────────────────────────────────────────────────

/// Compute the natural pixel width of a tab.
///
/// Pinned tabs always return [`TabControlConfig::pinned_tab_width`]. Regular
/// tabs sum: padding + status-dot reserve + icon (only if `icons_available`)
/// + title + badge + close-or-dirty slot + padding.
///
/// **Status-dot reserve rules:**
/// - Skipped when [`TabControlConfig::show_status_dot`] is `false`.
/// - Skipped when [`TabItem::status`] returns [`TabStatus::None`] for this
///   tab (per-tab opt-out).
/// - Kept for `TabStatus::Dirty` even though the dot itself isn't drawn —
///   this prevents layout from jumping when the user toggles between
///   `Active` and `Dirty`. Note that toggling between `None` and `Dirty`
///   *does* cause an ~11 px shift; that's an accepted trade-off because
///   `Active ↔ Dirty` is the common path.
pub(crate) fn compute_tab_width<T: TabItem>(cfg: &TabControlConfig, item: &T) -> f32 {
    if item.is_pinned() {
        return cfg.pinned_tab_width;
    }

    let mut w = cfg.tab_padding_h;

    // Status dot reserve
    let reserve_dot = cfg.show_status_dot && item.status() != TabStatus::None;
    if reserve_dot {
        w += STATUS_DOT_DIAM + STATUS_DOT_GAP;
    }

    // Icon — only if the host registered the icon font
    if cfg.icons_available
        && let Some(icon) = item.icon()
    {
        w += calc_text_size(icon)[0] + ICON_TITLE_GAP;
    }

    // Title
    w += calc_text_size(item.title())[0];

    // Badge: outer gap + inner pad + text + inner pad + outer gap
    if let Some(ref badge) = item.badge() {
        w += BADGE_LEFT_GAP
            + BADGE_INNER_PAD_X * 2.0
            + calc_text_size(&badge.text)[0]
            + BADGE_RIGHT_GAP;
    }

    // Close button (or dirty indicator)
    let needs_right_button =
        (cfg.closable && item.is_closable()) || item.status() == TabStatus::Dirty;
    if needs_right_button {
        w += cfg.close_btn_gap + cfg.close_btn_size;
    }

    w + cfg.tab_padding_h
}

// ─── enforce_pinned_partition ──────────────────────────────────────────────

/// Re-establish the pinned/regular partition. O(n) check; if violated,
/// uses [`Vec::rotate_right`] passes for in-place repair (no allocations).
///
/// Called automatically every frame by the renderer to absorb `is_pinned()`
/// changes the user may have made between frames. The early-exit walk is
/// O(n) — for typical tab counts (<20) the overhead is sub-microsecond
/// per frame, so the audit-suggested dirty-flag skip (P1, session 034)
/// was deferred as a profile-evidence-required optimisation.
pub(crate) fn enforce_pinned_partition<T: TabItem>(pc: &mut TabControl<T>) {
    // Quick check: walk once, ensure no regular precedes a pinned.
    let mut seen_regular = false;
    let mut needs_fix = false;
    for t in &pc.tabs {
        if t.item.is_pinned() {
            if seen_regular {
                needs_fix = true;
                break;
            }
        } else {
            seen_regular = true;
        }
    }
    if !needs_fix {
        return;
    }

    // In-place stable repair, no allocation. We sweep left-to-right, and
    // every time we encounter a pinned tab that sits to the right of regular
    // tabs, we rotate it leftward into the pinned prefix. This is O(n²) in
    // the worst case but only over the *number of misplaced pinned tabs*,
    // which is typically tiny (1–2). For the common steady-state path
    // (already partitioned) the early-exit above keeps it O(n).
    let mut prefix_end = 0usize; // length of the maintained pinned prefix
    let mut i = 0usize;
    while i < pc.tabs.len() {
        if pc.tabs[i].item.is_pinned() {
            if i != prefix_end {
                // Rotate the pinned tab from `i` down to `prefix_end`,
                // shifting the regulars in between rightward by one slot.
                pc.tabs[prefix_end..=i].rotate_right(1);
            }
            prefix_end += 1;
        }
        i += 1;
    }
    pc.invalidate_tab_layout_cache();
}
