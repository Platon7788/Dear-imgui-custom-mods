//! Process monitoring types — shared between core and UI.
//!
//! Minimal 5-field [`ProcessInfo`] matching the `IMGUI_NXT` reference
//! engine: `pid`, `name`, `bits`, `status`, `create_time`. Serializable
//! with `serde`; no external dependencies beyond that.

use serde::{Deserialize, Serialize};

// ─── Process status ───────────────────────────────────────────────────────────

/// Process running state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcStatus {
    /// Process is running normally.
    #[default]
    Running,
    /// All threads are suspended.
    Suspended,
}

impl ProcStatus {
    /// Returns a static label for the status.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Suspended => "Suspended",
        }
    }
}

// ─── Process information ───────────────────────────────────────────────────────

/// Snapshot of a single OS process — minimal 5-field version matching the
/// `IMGUI_NXT` reference engine. Anything beyond these five fields pays
/// memory and syscall-result-parsing cost for nothing when all the UI
/// wants is a classic "process list" dialog.
///
/// Times are in 100-nanosecond units (NT FILETIME).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// Process ID.
    pub pid: u32,
    /// Process name (image name without path).
    pub name: String,
    /// Process bitness: 32 or 64.
    pub bits: u8,
    /// Running or suspended.
    pub status: ProcStatus,
    /// Process creation time (NT FILETIME: 100-ns ticks since 1601-01-01).
    /// Used as a stable sort key — newest first — so rows keep their
    /// position across PID reuse.
    pub create_time: i64,
}

impl Default for ProcessInfo {
    fn default() -> Self {
        Self {
            pid: 0,
            name: String::new(),
            bits: 64,
            status: ProcStatus::Running,
            create_time: 0,
        }
    }
}

// ─── Delta update ──────────────────────────────────────────────────────────────

/// Incremental update for the process list.
///
/// Designed for efficient UI updates: only changed/new/removed processes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessDelta {
    /// New or changed processes (upsert).
    pub upsert: Vec<ProcessInfo>,
    /// PIDs that have exited (remove).
    pub removed: Vec<u32>,
    /// Total process count in the system.
    pub total: usize,
}

// ─── Column configuration ─────────────────────────────────────────────────────

/// Configuration for which columns to display in the process monitor.
///
/// Always visible: `Process Name`, `PID`. The two flags below let callers
/// hide `Bits` / `Status` if they only want a plain `name + PID` list.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ColumnConfig {
    /// Show process bitness (32/64). Default: `true`.
    pub bits: bool,
    /// Show process status (Running / Suspended). Default: `true`.
    pub status: bool,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            bits: true,
            status: true,
        }
    }
}

impl ColumnConfig {
    /// Total number of visible columns (including always-visible `Name` and `PID`).
    pub const fn visible_count(&self) -> usize {
        let mut n = 2;
        if self.bits {
            n += 1;
        }
        if self.status {
            n += 1;
        }
        n
    }
}

// ─── Row colors ────────────────────────────────────────────────────────────────

/// Highlight configuration for process rows.
///
/// Resolution priority when a row has multiple matches (first non-`None` wins):
///
/// 1. [`by_pid`](Self::by_pid) — explicit `u32 → color` map
/// 2. [`by_name`](Self::by_name) — case-insensitive `name → color` map
/// 3. [`self_process`](Self::self_process) — `Some(color)` highlights
///    the PID of the current process (`std::process::id()`)
/// 4. [`suspended`](Self::suspended) — tint rows whose status is `Suspended`
/// 5. No highlight (default row background)
///
/// All fields are serializable so full palettes can be shipped via
/// config files or a theme JSON.
///
/// # Example
///
/// ```rust,ignore
/// use dear_imgui_custom_mod::proc_mon::MonitorColors;
///
/// let colors = MonitorColors::default()
///     .with_self([0.20, 0.60, 0.35, 0.25])                 // soft green for my process
///     .with_name("chrome.exe", [0.25, 0.50, 0.85, 0.22])   // blue for browser
///     .with_name("svchost.exe", [0.40, 0.40, 0.45, 0.16])  // muted gray
///     .with_pid(4, [0.70, 0.20, 0.20, 0.20]);              // PID 4 = System
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorColors {
    /// Background tint for rows whose status is [`ProcStatus::Suspended`].
    /// Default: `Some([0.88, 0.55, 0.10, 0.20])` — amber at 20% opacity.
    /// Set to `None` to disable the tint entirely.
    pub suspended: Option<[f32; 4]>,
    /// Background tint for the current process (`std::process::id()`).
    /// Default: `None`. Useful to find yourself quickly in a long list.
    pub self_process: Option<[f32; 4]>,
    /// Per-name highlight map. Keys are **lowercase** process names
    /// (e.g. `"chrome.exe"`) — matching is case-insensitive at resolve time.
    /// Default: empty.
    #[serde(default)]
    pub by_name: std::collections::HashMap<String, [f32; 4]>,
    /// Per-PID highlight map. Highest-priority override.
    /// Default: empty.
    #[serde(default)]
    pub by_pid: std::collections::HashMap<u32, [f32; 4]>,
}

impl Default for MonitorColors {
    fn default() -> Self {
        Self {
            // Matches the historical hard-coded Suspended tint so upgrading
            // from an earlier version doesn't change the look.
            suspended: Some([0.88, 0.55, 0.10, 0.20]),
            self_process: None,
            by_name: std::collections::HashMap::new(),
            by_pid: std::collections::HashMap::new(),
        }
    }
}

impl MonitorColors {
    /// Set the tint for `Suspended` rows. `None` disables it.
    pub fn with_suspended(mut self, color: Option<[f32; 4]>) -> Self {
        self.suspended = color;
        self
    }

    /// Highlight the current process with the given color.
    pub fn with_self(mut self, color: [f32; 4]) -> Self {
        self.self_process = Some(color);
        self
    }

    /// Add a process-name → color mapping (case-insensitive). Name is
    /// lowercased before insertion so all lookups are O(1) and layout-
    /// independent.
    pub fn with_name(mut self, name: impl AsRef<str>, color: [f32; 4]) -> Self {
        self.by_name.insert(name.as_ref().to_lowercase(), color);
        self
    }

    /// Add a PID → color mapping.
    pub fn with_pid(mut self, pid: u32, color: [f32; 4]) -> Self {
        self.by_pid.insert(pid, color);
        self
    }

    /// In-place name insert (lowercased).
    pub fn add_name(&mut self, name: impl AsRef<str>, color: [f32; 4]) {
        self.by_name.insert(name.as_ref().to_lowercase(), color);
    }

    /// In-place PID insert.
    pub fn add_pid(&mut self, pid: u32, color: [f32; 4]) {
        self.by_pid.insert(pid, color);
    }

    /// Remove a name mapping. Name is lowercased before lookup.
    pub fn remove_name(&mut self, name: impl AsRef<str>) -> Option<[f32; 4]> {
        self.by_name.remove(&name.as_ref().to_lowercase())
    }

    /// Remove a PID mapping.
    pub fn remove_pid(&mut self, pid: u32) -> Option<[f32; 4]> {
        self.by_pid.remove(&pid)
    }

    /// Clear every mapping (suspended + self + by_name + by_pid).
    pub fn clear_all(&mut self) {
        self.suspended = None;
        self.self_process = None;
        self.by_name.clear();
        self.by_pid.clear();
    }

    /// Resolve the highlight for a given process, following the priority
    /// order documented on the struct.  Returns `None` if nothing matches.
    ///
    /// `self_pid` is the PID to match against [`self_process`](Self::self_process);
    /// callers typically pass `std::process::id()` once at monitor creation.
    #[inline]
    pub fn resolve(&self, info: &ProcessInfo, self_pid: u32) -> Option<[f32; 4]> {
        // 1. by_pid — explicit override, highest priority.
        if let Some(&c) = self.by_pid.get(&info.pid) {
            return Some(c);
        }
        // 2. by_name — case-insensitive match. Skip the to_lowercase alloc
        //    entirely when the map is empty (the common case).
        if !self.by_name.is_empty() {
            let lower = info.name.to_lowercase();
            if let Some(&c) = self.by_name.get(&lower) {
                return Some(c);
            }
        }
        // 3. self_process — own PID.
        if let Some(c) = self.self_process
            && info.pid == self_pid
        {
            return Some(c);
        }
        // 4. suspended — fallback for suspended processes.
        if info.status == ProcStatus::Suspended {
            return self.suspended;
        }
        None
    }
}

// ─── Monitor event ─────────────────────────────────────────────────────────────

/// Event returned from the monitor UI after rendering.
///
/// Caller handles these events (e.g., show context menu, perform action).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorEvent {
    /// A row was selected (single click).
    RowSelected(u32),
    /// A row was double-clicked.
    RowDoubleClicked(u32),
    /// Context menu requested for a row (right-click).
    /// Caller should render their own popup menu.
    ContextMenuRequested(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_config_default() {
        let cfg = ColumnConfig::default();
        assert!(cfg.bits);
        assert!(cfg.status);
        assert_eq!(cfg.visible_count(), 4); // Name + PID + Bits + Status
    }

    #[test]
    fn test_column_config_visible_count() {
        let none = ColumnConfig {
            bits: false,
            status: false,
        };
        assert_eq!(none.visible_count(), 2);
        let bits_only = ColumnConfig {
            bits: true,
            status: false,
        };
        assert_eq!(bits_only.visible_count(), 3);
    }

    #[test]
    fn test_monitor_colors_priority() {
        // Priority order: by_pid > by_name > self > suspended > None.
        let info = ProcessInfo {
            pid: 1234,
            name: "myapp.exe".into(),
            status: ProcStatus::Running,
            ..ProcessInfo::default()
        };

        // Empty palette — no highlight.
        let empty = MonitorColors::default().with_suspended(None);
        assert_eq!(empty.resolve(&info, 9999), None);

        // Self match (not suspended, not named, PID matches `self_pid`).
        let c_self = [0.1, 0.2, 0.3, 0.4];
        let pal = MonitorColors::default()
            .with_suspended(None)
            .with_self(c_self);
        assert_eq!(pal.resolve(&info, 1234), Some(c_self));
        assert_eq!(pal.resolve(&info, 9999), None);

        // by_name wins over self.
        let c_name = [0.5, 0.6, 0.7, 0.8];
        let pal = pal.with_name("MYAPP.EXE", c_name);
        assert_eq!(pal.resolve(&info, 1234), Some(c_name));

        // by_pid wins over by_name.
        let c_pid = [0.9, 0.9, 0.9, 0.9];
        let pal = pal.with_pid(1234, c_pid);
        assert_eq!(pal.resolve(&info, 1234), Some(c_pid));

        // Suspended fallback when nothing else matches.
        let suspended = ProcessInfo {
            pid: 42,
            name: "other.exe".into(),
            status: ProcStatus::Suspended,
            ..ProcessInfo::default()
        };
        let susp = [0.88, 0.55, 0.10, 0.20];
        let only_susp = MonitorColors::default().with_suspended(Some(susp));
        assert_eq!(only_susp.resolve(&suspended, 0), Some(susp));
        // Running process under the same palette — no match.
        assert_eq!(only_susp.resolve(&info, 0), None);
    }
}
