//! Navigation history — back/forward stack for directory browsing.
//!
//! Implements browser-style navigation: `push()` records the current path before
//! navigating away, `go_back()` / `go_forward()` move through the stacks.
//! Both stacks are capped at a configurable limit to prevent unbounded memory growth.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

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
        Self::new(100)
    }
}
