//! Pure animation / timer / lifecycle helpers for the notification center.
//!
//! Every function here is deliberately `Ui`-free so the queue mechanics
//! (enter/exit animation ramps, auto-dismiss countdowns, reaping, and the
//! event-driven frame-demand decision) can be unit-tested without an ImGui
//! context. Split out of `mod.rs` (was 912 lines) so every file in the
//! module stays under the 500-line cap.

use super::*;

/// Pass 1 — advance enter/exit animation timelines toward their targets.
pub(super) fn advance_animations(queue: &mut [Notification], dt: f32, anim_dur: f32) {
    for n in queue {
        if !n.dismissing && n.enter_t < 1.0 {
            n.enter_t = (n.enter_t + dt / anim_dur).min(1.0);
        }
        if n.dismissing && n.exit_t < 1.0 {
            n.exit_t = (n.exit_t + dt / anim_dur).min(1.0);
        }
    }
}

/// Pass 3 — tick auto-dismiss timers, skipping toasts the cursor is over
/// when `pause_on_hover` is enabled. Toasts whose elapsed time crosses
/// their `Timed(secs)` budget are flagged `dismissing`.
pub(super) fn tick_timers(
    queue: &mut [Notification],
    dt: f32,
    hover_flags: &[(u64, bool)],
    pause_on_hover: bool,
) {
    for n in queue {
        if n.dismissing {
            continue;
        }
        let hovered = pause_on_hover && hover_flags.iter().any(|&(id, h)| id == n.id && h);
        if hovered {
            continue;
        }
        if let Duration::Timed(secs) = n.duration {
            n.elapsed += dt;
            if n.elapsed >= secs {
                n.dismissing = true;
            }
        }
    }
}

/// Pass 5 — drop notifications whose exit animation has finished. Pushes
/// `Dismissed` events for each one removed.
pub(super) fn reap_dismissed(
    queue: &mut Vec<Notification>,
    animation: AnimationKind,
    events: &mut Vec<NotificationEvent>,
) {
    let none_anim = matches!(animation, AnimationKind::None);
    queue.retain(|n| {
        let done = n.dismissing && (none_anim || n.exit_t >= 1.0);
        if done {
            events.push(NotificationEvent::Dismissed(n.id));
        }
        !done
    });
}

/// Pass 6 — decide whether the event-driven host must be woken for another
/// frame. A frame is demanded when **any** toast is still doing work:
///
/// * mid enter animation (`enter_t < 1.0`),
/// * mid exit animation (`0.0 < exit_t < 1.0`), or
/// * a live (non-dismissing) toast with a *ticking* auto-dismiss timer
///   (`Duration::Timed`) — its countdown / progress bar must keep moving.
///
/// A fully-entered `Sticky` toast that is just sitting there waiting for
/// user interaction does **not** demand frames: nothing about it changes
/// between input events, so the host may sleep until the user acts. This
/// keeps idle CPU/GPU at zero even while a sticky toast is on screen.
pub(super) fn needs_frame(queue: &[Notification]) -> bool {
    queue.iter().any(|n| {
        let entering = n.enter_t < 1.0;
        let exiting = n.exit_t > 0.0 && n.exit_t < 1.0;
        let ticking = !n.dismissing && matches!(n.duration, Duration::Timed(_));
        entering || exiting || ticking
    })
}

// ─── Easing ──────────────────────────────────────────────────────────────────

/// Decelerates into the target — fast start, smooth landing.
#[inline]
pub(super) fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Accelerates away from the origin — slow start, fast finish.
#[inline]
pub(super) fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}
