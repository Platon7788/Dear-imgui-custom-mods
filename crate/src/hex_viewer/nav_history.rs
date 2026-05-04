//! Navigation history for hex_viewer address navigation.

// ── Navigation History ──────────────────────────────────────────────────────

/// Back/forward navigation stack for address history.
///
/// Uses `VecDeque` for O(1) eviction of oldest entries.
#[derive(Debug, Clone, Default)]
pub struct NavHistory {
    back: std::collections::VecDeque<u64>,
    forward: Vec<u64>,
    /// Maximum stack depth.
    max_depth: usize,
}

impl NavHistory {
    pub fn new(max_depth: usize) -> Self {
        Self {
            back: std::collections::VecDeque::new(),
            forward: Vec::new(),
            max_depth,
        }
    }

    /// Record a navigation to `addr`. Call *before* changing the address.
    pub fn push(&mut self, current_addr: u64) {
        self.back.push_back(current_addr);
        self.forward.clear();
        if self.max_depth > 0 && self.back.len() > self.max_depth {
            self.back.pop_front(); // O(1) instead of Vec::remove(0) O(n)
        }
    }

    /// Go back. Returns previous address, or `None`.
    pub fn go_back(&mut self, current_addr: u64) -> Option<u64> {
        let addr = self.back.pop_back()?;
        self.forward.push(current_addr);
        Some(addr)
    }

    /// Go forward. Returns next address, or `None`.
    pub fn go_forward(&mut self, current_addr: u64) -> Option<u64> {
        let addr = self.forward.pop()?;
        self.back.push_back(current_addr);
        Some(addr)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
    pub fn clear(&mut self) {
        self.back.clear();
        self.forward.clear();
    }
}
