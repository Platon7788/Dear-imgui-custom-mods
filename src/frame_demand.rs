//! # frame_demand
//!
//! Per-frame "render-budget" signal used by event-driven hosts to decide
//! whether to schedule another paint after the current one.
//!
//! ## How it works
//!
//! - Widgets that have *ongoing* work — fade animations, scale tweens,
//!   countdown timers, blinking cursors — call [`request(N)`] from inside
//!   their per-frame render to mean *"I need at least N more frames after
//!   this one to finish what I'm doing."*
//! - The host (e.g. [`crate::app_window`]) reads the value with
//!   [`take()`] **after** running the user's render closure for one frame
//!   and uses it to bump its own pending-frame counter, then calls
//!   `Window::request_redraw()`.
//!
//! Storage is a `thread_local!` `Cell<u8>` — the GUI thread writes the max
//! of all requests during a frame, the host reads & resets at the end of
//! that frame. No locking, no allocation; cost per call is one branch + a
//! `Cell::set`.
//!
//! ## Why not pass `&mut AppState` through every widget?
//!
//! Widgets render with `(&Ui, …)` and do not own the host's state. A free
//! function with thread-local storage keeps the call site terse:
//!
//! ```ignore
//! // Inside a notification's render():
//! crate::frame_demand::request(1);  // "give me one more frame to fade"
//! ```
//!
//! ## Saturation
//!
//! `u8` (max 255) is plenty — any animation that needs more than ~4 s on a
//! 60 Hz refresh should drive its own keep-alive each frame anyway.
//!
//! ## Continuous-render hosts
//!
//! Hosts that always render at full FPS (game-style) can simply ignore
//! [`take()`] — calling [`request`] becomes a cheap no-op.

use std::cell::Cell;

thread_local! {
    static FRAMES_DEMANDED: Cell<u8> = const { Cell::new(0) };
}

/// Request that the host render at least `frames` more frames after the
/// current one. Calls accumulate by **max**, not sum, so spamming
/// `request(1)` every render is safe and idempotent.
#[inline]
pub fn request(frames: u8) {
    if frames == 0 {
        return;
    }
    FRAMES_DEMANDED.with(|c| {
        let cur = c.get();
        if frames > cur {
            c.set(frames);
        }
    });
}

/// Read the current demand and reset it to 0. Hosts call this once per
/// frame after the user's render closure returns.
#[inline]
pub fn take() -> u8 {
    FRAMES_DEMANDED.with(|c| c.replace(0))
}

/// Peek without resetting — useful for diagnostics.
#[inline]
pub fn peek() -> u8 {
    FRAMES_DEMANDED.with(|c| c.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_then_take() {
        let _ = take(); // clear from any previous test
        assert_eq!(peek(), 0);
        request(2);
        assert_eq!(peek(), 2);
        assert_eq!(take(), 2);
        assert_eq!(peek(), 0);
    }

    #[test]
    fn request_uses_max() {
        let _ = take();
        request(1);
        request(3);
        request(2);
        assert_eq!(take(), 3);
    }

    #[test]
    fn request_zero_is_noop() {
        let _ = take();
        request(0);
        assert_eq!(peek(), 0);
        request(2);
        request(0);
        assert_eq!(take(), 2);
    }
}
