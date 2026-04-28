//! Cross-thread wake-up handle for the [`AppWindowV2`](super::AppWindowV2)
//! event loop.
//!
//! In event-driven mode the loop sleeps in `ControlFlow::Wait` until an OS
//! event arrives. Background threads (HTTP, file watch, IPC, async runtime)
//! cannot deliver work to the UI without a way to *wake* the loop —
//! [`AppProxyV2::wake`] is exactly that, calling [`winit::event_loop::EventLoopProxy::send_event`]
//! under the hood.
//!
//! The proxy is `Send + Sync + Clone` and may be shared across threads, futures
//! and channels freely. It is exposed via [`super::AppStateV2::proxy`] inside
//! `AppHandlerV2` callbacks.

use winit::event_loop::EventLoopProxy;

/// Thread-safe handle that wakes the event loop on demand.
///
/// `wake()` is idempotent and cheap — multiple calls between two iterations
/// of the event loop coalesce into a single redraw cycle.
#[derive(Clone)]
pub struct AppProxyV2 {
    inner: EventLoopProxy<()>,
}

impl AppProxyV2 {
    pub(super) fn new(inner: EventLoopProxy<()>) -> Self {
        Self { inner }
    }

    /// Wake the event loop. Triggers a single redraw cycle on the UI thread.
    /// Idempotent across calls within the same loop iteration.
    ///
    /// Returns [`WakeError::EventLoopClosed`] if the event loop has already
    /// exited (the application is shutting down). Most callers can ignore
    /// the error — a closed loop simply means there is nothing to wake.
    pub fn wake(&self) -> Result<(), WakeError> {
        self.inner
            .send_event(())
            .map_err(|_| WakeError::EventLoopClosed)
    }
}

impl std::fmt::Debug for AppProxyV2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppProxyV2").finish_non_exhaustive()
    }
}

/// Failure mode of [`AppProxyV2::wake`].
///
/// Currently only one variant — kept as an enum so future failure modes
/// (e.g. cross-process proxy invalidation) stay backwards-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WakeError {
    /// The event loop has exited; the proxy is permanently inert.
    EventLoopClosed,
}

impl std::fmt::Display for WakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventLoopClosed => f.write_str("event loop has exited"),
        }
    }
}

impl std::error::Error for WakeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_error_display_is_human_readable() {
        let s = format!("{}", WakeError::EventLoopClosed);
        assert!(s.contains("event loop"), "got: {s}");
    }

    #[test]
    fn wake_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WakeError>();
    }

    #[test]
    fn wake_error_implements_std_error() {
        let _: &dyn std::error::Error = &WakeError::EventLoopClosed;
    }
}
