//! # notifications
//!
//! Modern, flexible toast-notification center for Dear ImGui.
//!
//! ## Features
//! - 5 severity levels: Info, Success, Warning, Error, Debug — each with a
//!   distinct color and font-independent draw-list icon.
//! - 6 stack placements (4 corners + top/bottom center) with customizable
//!   margin and spacing.
//! - Auto-dismiss timer with optional bottom progress bar; `sticky` mode
//!   keeps the toast until the user closes it.
//! - Pause-on-hover so reading long text does not miss the window.
//! - `Fade` and `SlideIn` entry / exit animations.
//! - Action buttons with caller-defined IDs surfaced via
//!   [`NotificationEvent::ActionClicked`].
//! - Manual `×` close, custom per-toast accent color, max-visible cap with
//!   graceful overflow.
//! - Fully themeable through the crate-wide [`crate::theme::Theme`] enum or
//!   a [`NotificationColors`] override.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::notifications::*;
//!
//! // Persistent state — typically lives in your app/handler struct.
//! let mut center = NotificationCenter::new();
//!
//! // Push from anywhere — events / buttons / async callbacks.
//! center.push(Notification::success("Saved"));
//! center.push(Notification::error("Failed").with_body("disk full"));
//!
//! # fn frame(ui: &dear_imgui_rs::Ui, center: &mut NotificationCenter, dt: f32) {
//! // Render every frame — last, so toasts are on top.
//! for event in center.render(ui, dt) {
//!     match event {
//!         NotificationEvent::Dismissed(id) => { let _ = id; }
//!         NotificationEvent::ActionClicked { id, action_id } => {
//!             let _ = (id, action_id);
//!         }
//!         NotificationEvent::Clicked(id) => { let _ = id; }
//!     }
//! }
//! # }
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass

pub mod config;
pub mod enums;
pub mod icons;
mod logic;
pub mod notification;
mod render;
pub mod theme;

pub use config::CenterConfig;
pub use enums::{AnimationKind, Duration, Placement, Severity};
pub use notification::{Notification, NotificationAction};
pub use theme::NotificationColors;

use dear_imgui_rs::{Condition, MouseButton, StyleColor, StyleVar, Ui, WindowFlags};

use crate::utils::color::rgba_f32;
use crate::utils::text::{calc_text_size, line_height};

// Pure lifecycle helpers (logic.rs) and per-toast rendering (render.rs) live
// in sibling submodules; the orchestration in `render` below and the unit
// tests call them by bare name.
use logic::{
    advance_animations, ease_in_cubic, ease_out_cubic, needs_frame, reap_dismissed, tick_timers,
};
use render::{animated_pos, estimate_height, render_toast};

// ─── Events ──────────────────────────────────────────────────────────────────

/// Event emitted by [`NotificationCenter::render`] during a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEvent {
    /// Toast was dismissed (timer expired, close button, or action click).
    Dismissed(u64),
    /// An action button inside a toast was clicked.
    ActionClicked {
        /// Notification id.
        id: u64,
        /// Caller-defined action id (see [`Notification::with_action`]).
        action_id: u32,
    },
    /// The toast body (not a button) was clicked.
    Clicked(u64),
}

// ─── Notification center ─────────────────────────────────────────────────────

/// Maximum per-frame `dt` accepted by [`NotificationCenter::render`]. Prevents
/// catastrophic timer / animation jumps after the host app is suspended
/// (Alt-Tab away, debugger pause, sleeping system) — without the clamp a
/// 5-second pause makes every active notification fast-forward through its
/// entire lifecycle in one frame.
const MAX_FRAME_DT: f32 = 0.5;

/// Holds the live stack of notifications between frames.
///
/// `NotificationCenter` is not `Copy` and persists across frames — keep it in
/// your application state struct. Every frame call
/// [`render`](Self::render) to advance animations, honor timers, and draw
/// the stack.
#[derive(Debug)]
pub struct NotificationCenter {
    /// Active notifications, oldest first.
    queue: Vec<Notification>,
    /// Configuration.
    config: CenterConfig,
    /// Monotonic id counter.
    next_id: u64,
}

impl Default for NotificationCenter {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            config: CenterConfig::default(),
            next_id: 1,
        }
    }
}

impl NotificationCenter {
    /// Create a center with default config (top-right, fade, 5 visible).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a center from an explicit config.
    pub fn with_config(config: CenterConfig) -> Self {
        Self {
            queue: Vec::new(),
            config,
            next_id: 1,
        }
    }

    /// Mutable access to the configuration — changes take effect next frame.
    pub fn config_mut(&mut self) -> &mut CenterConfig {
        &mut self.config
    }

    /// Read-only view of the configuration.
    pub fn config(&self) -> &CenterConfig {
        &self.config
    }

    /// Push a notification onto the stack and return its id.
    ///
    /// On `u64` wrap-around (after ~10^19 pushes — practically
    /// impossible but defensively handled) we scan the live queue
    /// for the next unused id so a sticky long-lived toast that
    /// happens to occupy the post-wrap range can't collide with a
    /// fresh one. Pre-wrap behaviour is unchanged.
    pub fn push(&mut self, mut n: Notification) -> u64 {
        let mut id = self.next_id;
        // Skip 0 (reserved as "no id") and any id already in flight.
        while id == 0 || self.queue.iter().any(|n| n.id == id) {
            id = id.wrapping_add(1);
        }
        self.next_id = id.wrapping_add(1).max(1);
        n.id = id;
        // Pre-format ALL ImGui IDs once at push() time — saves
        // per-frame `format!` allocations on every visible toast.
        // The hot render path then reads these pre-built strings
        // instead of allocating each frame.
        n.win_id = format!("##toast_{id}");
        n.close_id = format!("##close_{id}");
        n.action_ids = n
            .actions
            .iter()
            .enumerate()
            .map(|(idx, act)| format!("{}##act_{id}_{idx}", act.label))
            .collect();
        self.queue.push(n);
        id
    }

    /// Mark the notification with `id` for dismissal (triggers exit animation).
    pub fn dismiss(&mut self, id: u64) {
        if let Some(n) = self.queue.iter_mut().find(|n| n.id == id)
            && !n.dismissing
        {
            n.dismissing = true;
        }
    }

    /// Dismiss every active notification. Returns the number of toasts that
    /// were not already in the dismissing state — useful for "Cleared N"
    /// status feedback.
    pub fn dismiss_all(&mut self) -> usize {
        let mut count = 0;
        for n in &mut self.queue {
            if !n.dismissing {
                n.dismissing = true;
                count += 1;
            }
        }
        count
    }

    /// Number of notifications currently on the stack — **including** ones
    /// that are fading out. For "how many are still alive and counting
    /// down" use [`active_count`](Self::active_count).
    pub fn count(&self) -> usize {
        self.queue.len()
    }

    /// Number of notifications that are still active (not yet dismissed).
    /// Excludes toasts that are mid-exit-animation.
    pub fn active_count(&self) -> usize {
        self.queue.iter().filter(|n| !n.dismissing).count()
    }

    /// Advance state and draw the stack. Returns events that fired this frame.
    pub fn render(&mut self, ui: &Ui, dt: f32) -> Vec<NotificationEvent> {
        let mut events = Vec::new();
        // Sanitise `dt`: NaN / negative → 0 (skip frame); huge `dt` (host
        // suspended for several seconds) → clamp so animations don't
        // collapse to a single tick, swallowing the entry / exit phases.
        if dt.is_nan() || dt < 0.0 {
            return events;
        }
        let dt = dt.min(MAX_FRAME_DT);

        // Clone the config for the duration of the frame so we can freely
        // interleave `&self.queue` reads with `&mut self` mutations (dismiss).
        let cfg = self.config.clone();
        let colors = cfg.resolved_colors();
        let anim_dur = cfg.animation_duration.max(0.0001);

        // ── Pass 1: advance animations for every notification ───────────────
        advance_animations(&mut self.queue, dt, anim_dur);

        // ── Layout parameters ───────────────────────────────────────────────
        let [dw, dh] = ui.io().display_size();
        let anchor_x = match cfg.placement {
            Placement::TopRight | Placement::BottomRight => dw - cfg.margin[0] - cfg.width,
            Placement::TopLeft | Placement::BottomLeft => cfg.margin[0],
            Placement::TopCenter | Placement::BottomCenter => (dw - cfg.width) * 0.5,
        };
        let grows_up = cfg.placement.grows_up();
        let base_y = if grows_up {
            dh - cfg.margin[1]
        } else {
            cfg.margin[1]
        };

        // ── Determine visible slice: the newest `max_visible` ────────────────
        let visible_count = self.queue.len().min(cfg.max_visible);
        let start = self.queue.len().saturating_sub(visible_count);

        // Newest first (closest to anchor edge).
        let indices: Vec<usize> = (start..self.queue.len()).rev().collect();

        // ── Pass 2: render each visible toast ───────────────────────────────
        let mut cum_y = 0.0_f32;
        let mut hover_flags: Vec<(u64, bool)> = Vec::with_capacity(indices.len());
        let mut to_dismiss: Vec<u64> = Vec::new();
        // Single-frame click-consumed latch — guards against the
        // (theoretical) case where overlapping or stacked toasts
        // each report `is_window_hovered() && is_mouse_clicked()`
        // for the same physical click. ImGui normally returns
        // hover for the topmost window only, but defensive
        // gating ensures *one* click → *one* event regardless of
        // window stacking flags.
        let mut click_consumed = false;

        for &i in &indices {
            let n = &self.queue[i];

            let est_h = estimate_height(n, &cfg, ui);

            // Slot fraction drives how much vertical space this toast claims
            // in the stack — animating from 0→1 (enter) or 1→0 (exit) so
            // neighboring toasts glide rather than jump.
            let slot_frac = ease_out_cubic(n.enter_t) * (1.0 - ease_in_cubic(n.exit_t));

            let (px, py, alpha) = animated_pos(n, &cfg, anchor_x, base_y, cum_y, est_h, grows_up);

            // Always advance by animated slot so the gap opens/closes smoothly.
            cum_y += (est_h + cfg.spacing) * slot_frac;

            if alpha <= 0.001 && n.dismissing {
                continue;
            }

            let outcome = render_toast(ui, n, &colors, &cfg, px, py, alpha, est_h);

            hover_flags.push((n.id, outcome.hovered));

            if outcome.close_clicked && !click_consumed {
                to_dismiss.push(n.id);
                click_consumed = true;
            }
            if let Some(aid) = outcome.action_clicked
                && !click_consumed
            {
                events.push(NotificationEvent::ActionClicked {
                    id: n.id,
                    action_id: aid,
                });
                to_dismiss.push(n.id);
                click_consumed = true;
            }
            if outcome.body_clicked && !click_consumed {
                events.push(NotificationEvent::Clicked(n.id));
                click_consumed = true;
            }
        }

        // ── Pass 3: advance elapsed timers (paused while hovered) ───────────
        tick_timers(&mut self.queue, dt, &hover_flags, cfg.pause_on_hover);

        // ── Pass 4: apply requested dismissals ──────────────────────────────
        for id in to_dismiss {
            self.dismiss(id);
        }

        // ── Pass 5: reap notifications whose exit animation has finished ────
        reap_dismissed(&mut self.queue, cfg.animation, &mut events);

        // ── Pass 6: keep the renderer alive while toasts are animating ──
        // In event-driven hosts (e.g. `app_window` default) the loop
        // would otherwise sleep mid-fade or stop ticking the auto-dismiss
        // countdown. A toast mid-animation, or a live toast with a *ticking*
        // `Duration::Timed` countdown, demands the next frame. A fully-entered
        // `Sticky` toast does NOT — nothing about it changes until the user
        // acts, and input events already wake the host. See `logic::needs_frame`.
        if needs_frame(&self.queue) {
            crate::frame_demand::request(1);
        }

        events
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
