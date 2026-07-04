//! Navigation history — back/forward stack for directory browsing.
//!
//! Implements browser-style navigation: `push()` records the current path before
//! navigating away, `go_back()` / `go_forward()` move through the stacks.
//! Both stacks are capped at a configurable limit to prevent unbounded memory growth.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Default cap for back/forward stacks (P2-11: was duplicated as a literal `100`
/// in `FileManagerConfig::default` and `NavigationHistory::default`).
pub(super) const DEFAULT_MAX_HISTORY: usize = 100;

/// Simple back/forward navigation stack.
///
/// `push()` records the current path before navigating away.
/// `go_back()` / `go_forward()` move through the history.
/// Stacks are capped at `max_entries` entries.
pub(super) struct NavigationHistory {
    back_stack: VecDeque<PathBuf>,
    forward_stack: VecDeque<PathBuf>,
    max_entries: usize,
}

impl NavigationHistory {
    /// Create an empty history (no back/forward entries).
    /// `max_entries` is clamped to a minimum of 1.
    pub(super) fn new(max_entries: usize) -> Self {
        Self {
            back_stack: VecDeque::new(),
            forward_stack: VecDeque::new(),
            max_entries: max_entries.max(1),
        }
    }

    /// Record `current` before navigating to a new path.
    /// Clears the forward stack (new navigation branch).
    pub(super) fn push(&mut self, current: &Path) {
        if self.back_stack.len() >= self.max_entries {
            self.back_stack.pop_front();
        }
        self.back_stack.push_back(current.to_path_buf());
        self.forward_stack.clear();
    }

    /// Go back one step. Returns the path to navigate to.
    /// Pushes `current` onto the forward stack.
    pub(super) fn go_back(&mut self, current: &Path) -> Option<PathBuf> {
        let prev = self.back_stack.pop_back()?;
        self.forward_stack.push_back(current.to_path_buf());
        Some(prev)
    }

    /// Go forward one step. Returns the path to navigate to.
    /// Pushes `current` onto the back stack.
    pub(super) fn go_forward(&mut self, current: &Path) -> Option<PathBuf> {
        let next = self.forward_stack.pop_back()?;
        self.back_stack.push_back(current.to_path_buf());
        Some(next)
    }

    /// Peek the path a [`go_back`](Self::go_back) would navigate to, without
    /// mutating either stack. Lets the caller validate the target (e.g. try to
    /// list it) and only commit the stack move on success.
    pub(super) fn peek_back(&self) -> Option<&Path> {
        self.back_stack.back().map(PathBuf::as_path)
    }

    /// Peek the path a [`go_forward`](Self::go_forward) would navigate to,
    /// without mutating either stack.
    pub(super) fn peek_forward(&self) -> Option<&Path> {
        self.forward_stack.back().map(PathBuf::as_path)
    }

    /// Returns `true` if there is at least one entry in the back stack.
    pub(super) fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    /// Returns `true` if there is at least one entry in the forward stack.
    pub(super) fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    /// Clear both back and forward stacks.
    pub(super) fn clear(&mut self) {
        self.back_stack.clear();
        self.forward_stack.clear();
    }
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_HISTORY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_forward_round_trip() {
        let mut h = NavigationHistory::new(10);
        assert!(!h.can_go_back());
        h.push(Path::new("/a"));
        assert!(h.can_go_back());
        assert_eq!(h.go_back(Path::new("/b")).unwrap(), Path::new("/a"));
        assert!(h.can_go_forward());
        assert_eq!(h.go_forward(Path::new("/a")).unwrap(), Path::new("/b"));
    }

    #[test]
    fn push_clears_forward_branch() {
        let mut h = NavigationHistory::new(10);
        h.push(Path::new("/a"));
        let _ = h.go_back(Path::new("/b"));
        assert!(h.can_go_forward());
        h.push(Path::new("/c"));
        assert!(
            !h.can_go_forward(),
            "a new navigation branch clears forward"
        );
    }

    #[test]
    fn back_stack_respects_cap() {
        let mut h = NavigationHistory::new(2);
        h.push(Path::new("/1"));
        h.push(Path::new("/2"));
        h.push(Path::new("/3"));
        assert_eq!(h.go_back(Path::new("/x")).unwrap(), Path::new("/3"));
        assert_eq!(h.go_back(Path::new("/3")).unwrap(), Path::new("/2"));
        assert!(!h.can_go_back(), "oldest entry evicted at cap 2");
    }

    #[test]
    fn peek_does_not_consume() {
        let mut h = NavigationHistory::new(10);
        h.push(Path::new("/a"));
        assert_eq!(h.peek_back(), Some(Path::new("/a")));
        assert!(h.can_go_back(), "peek must not mutate the stack");
        assert_eq!(h.peek_forward(), None);
    }
}
