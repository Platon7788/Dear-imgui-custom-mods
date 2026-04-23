//! Process monitoring types — shared between core and UI.
//!
//! Serde-serializable, no external dependencies beyond `serde`.

use serde::{Deserialize, Serialize};
use std::fmt::Write;

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

/// Snapshot of a single OS process with full metrics.
///
/// All sizes are in bytes. Times are in 100-nanosecond units (NT FILETIME).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    // ─── Identity ──────────────────────────────────────────────
    /// Process ID.
    pub pid: u32,
    /// Process name (image name without path).
    pub name: String,
    /// Process bitness: 32 or 64.
    pub bits: u8,
    /// Parent process ID.
    pub ppid: u32,
    /// Terminal Services session ID.
    pub session_id: u32,

    // ─── State ─────────────────────────────────────────────────
    /// Running or suspended.
    pub status: ProcStatus,
    /// Process creation time (NT FILETIME: 100-ns ticks since 1601-01-01).
    pub create_time: i64,
    /// Base priority class (2=Low, 4=BelowNormal, 8=Normal, 13=High, 24=Realtime).
    pub priority: i32,

    // ─── Memory ────────────────────────────────────────────────
    /// Working set size (physical memory used, bytes).
    pub working_set: usize,
    /// Private memory usage (bytes not shared with other processes).
    pub private_bytes: usize,
    /// Virtual memory size (bytes).
    pub virtual_size: usize,
    /// Peak working set size (bytes).
    pub peak_working_set: usize,

    // ─── CPU ───────────────────────────────────────────────────
    /// Time spent in kernel mode (100-ns units).
    pub kernel_time: i64,
    /// Time spent in user mode (100-ns units).
    pub user_time: i64,
    /// CPU cycle count (platform-specific, may be 0 on some systems).
    pub cycle_time: u64,

    // ─── Threads & Handles ─────────────────────────────────────
    /// Number of threads in the process.
    pub thread_count: u32,
    /// Number of handles held by the process.
    pub handle_count: u32,

    // ─── I/O ───────────────────────────────────────────────────
    /// Total bytes read from I/O operations.
    pub io_read_bytes: u64,
    /// Total bytes written to I/O operations.
    pub io_write_bytes: u64,

    // ─── Derived ───────────────────────────────────────────────
    /// CPU usage in percent (0.0–100.0, normalized across all logical cores).
    ///
    /// Computed by `ProcessEnumerator` as `Δ(kernel+user) / (Δwall × cores) × 100`.
    /// Always `0.0` on the first tick after a process appears (no baseline).
    pub cpu_percent: f32,
}

impl Default for ProcessInfo {
    fn default() -> Self {
        Self {
            pid: 0,
            name: String::new(),
            bits: 64,
            ppid: 0,
            session_id: 0,
            status: ProcStatus::Running,
            create_time: 0,
            priority: 8,
            working_set: 0,
            private_bytes: 0,
            virtual_size: 0,
            peak_working_set: 0,
            kernel_time: 0,
            user_time: 0,
            cycle_time: 0,
            thread_count: 0,
            handle_count: 0,
            io_read_bytes: 0,
            io_write_bytes: 0,
            cpu_percent: 0.0,
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
/// Default shows the most useful columns: name, PID, bits, status, memory.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ColumnConfig {
    // ─── Always visible (cannot disable) ───────────────────────
    // (name and pid are always shown, not configurable)

    // ─── Default visible ───────────────────────────────────────
    /// Show process bitness (32/64). Default: true.
    pub bits: bool,
    /// Show process status (Running/Suspended). Default: true.
    pub status: bool,
    /// Show working set (RAM usage). Default: true.
    pub memory: bool,
    /// Show CPU usage percent (normalized across cores). Default: true.
    pub cpu_percent: bool,

    // ─── Optional (default hidden) ─────────────────────────────
    /// Show parent PID. Default: false.
    pub ppid: bool,
    /// Show session ID. Default: false.
    pub session_id: bool,
    /// Show process priority. Default: false.
    pub priority: bool,
    /// Show thread count. Default: false.
    pub threads: bool,
    /// Show handle count. Default: false.
    pub handles: bool,
    /// Show private bytes. Default: false.
    pub private_bytes: bool,
    /// Show virtual memory size. Default: false.
    pub virtual_size: bool,
    /// Show peak working set. Default: false.
    pub peak_memory: bool,
    /// Show I/O read bytes. Default: false.
    pub io_read: bool,
    /// Show I/O write bytes. Default: false.
    pub io_write: bool,
    /// Show CPU time (kernel + user). Default: false.
    pub cpu_time: bool,
    /// Show creation time. Default: false.
    pub create_time: bool,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            // Default visible — minimal essentials only. Keeps per-frame
            // imgui draw calls low and matches the NxT reference UI layout.
            // Enable heavier columns via `MonitorConfig::all_columns()` or
            // hand-rolled `ColumnConfig { memory: true, cpu_percent: true, .. }`.
            bits: true,
            status: true,
            memory: false,
            cpu_percent: false,
            // Default hidden
            ppid: false,
            session_id: false,
            priority: false,
            threads: false,
            handles: false,
            private_bytes: false,
            virtual_size: false,
            peak_memory: false,
            io_read: false,
            io_write: false,
            cpu_time: false,
            create_time: false,
        }
    }
}

impl ColumnConfig {
    /// Returns the total number of visible columns.
    pub fn visible_count(&self) -> usize {
        // Always: name (0), pid (1) = 2 columns
        let mut count = 2;
        if self.bits {
            count += 1;
        }
        if self.status {
            count += 1;
        }
        if self.memory {
            count += 1;
        }
        if self.cpu_percent {
            count += 1;
        }
        if self.ppid {
            count += 1;
        }
        if self.session_id {
            count += 1;
        }
        if self.priority {
            count += 1;
        }
        if self.threads {
            count += 1;
        }
        if self.handles {
            count += 1;
        }
        if self.private_bytes {
            count += 1;
        }
        if self.virtual_size {
            count += 1;
        }
        if self.peak_memory {
            count += 1;
        }
        if self.io_read {
            count += 1;
        }
        if self.io_write {
            count += 1;
        }
        if self.cpu_time {
            count += 1;
        }
        if self.create_time {
            count += 1;
        }
        count
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

// ─── Helper functions ──────────────────────────────────────────────────────────

/// Format bytes as human-readable string (KB, MB, GB).
pub fn format_bytes(bytes: usize, buf: &mut String) {
    const KB: usize = 1024;
    const MB: usize = 1024 * KB;
    const GB: usize = 1024 * MB;

    buf.clear();
    if bytes >= GB {
        let _ = write!(buf, "{:.1} GB", bytes as f64 / GB as f64);
    } else if bytes >= MB {
        let _ = write!(buf, "{:.1} MB", bytes as f64 / MB as f64);
    } else if bytes >= KB {
        let _ = write!(buf, "{:.1} KB", bytes as f64 / KB as f64);
    } else {
        let _ = write!(buf, "{} B", bytes);
    }
}

/// Format CPU percent as a compact human-readable string.
///
/// Output: `"—"` for 0%, `"0.5%"` for sub-1%, `"42%"` for integer whole,
/// `"3.4%"` for non-whole ≤10%. Keeps the column narrow and avoids jitter.
pub fn format_cpu_percent(pct: f32, buf: &mut String) {
    buf.clear();
    if pct <= 0.0 || !pct.is_finite() {
        buf.push('—');
        return;
    }
    if pct < 10.0 {
        // Sub-10%: one decimal place ("0.3%", "3.5%").
        let _ = write!(buf, "{:.1}%", pct);
    } else {
        // ≥10%: whole-percent only, keeps column narrow and reduces jitter.
        let _ = write!(buf, "{:.0}%", pct);
    }
}

/// Format CPU time (100-ns units) as human-readable string (ms, s, m:s).
pub fn format_cpu_time(time_100ns: i64, buf: &mut String) {
    buf.clear();
    let total_ms = time_100ns / 10_000;
    let total_s = total_ms / 1000;
    let ms = total_ms % 1000;
    let m = total_s / 60;
    let s = total_s % 60;

    if m > 0 {
        let _ = write!(buf, "{}:{:02}.{:03}", m, s, ms);
    } else if total_s > 0 {
        let _ = write!(buf, "{}.{:03}s", total_s, ms);
    } else {
        let _ = write!(buf, "{}ms", total_ms);
    }
}

/// Format creation time (NT FILETIME) as local datetime string.
pub fn format_create_time(create_time: i64, buf: &mut String) {
    buf.clear();
    if create_time == 0 {
        buf.push_str("N/A");
        return;
    }

    // NT FILETIME is 100-ns ticks since 1601-01-01.
    // Convert to Unix timestamp (seconds since 1970-01-01).
    // Difference: 11644473600 seconds = 369 years.
    const EPOCH_DIFF: i64 = 116_444_736_000_000_000; // 100-ns units
    let unix_100ns = create_time - EPOCH_DIFF;
    let unix_secs = unix_100ns / 10_000_000;

    // Simple formatting without chrono dependency
    // This is a rough approximation; for production use `time` or `chrono`.
    let days = unix_secs / 86400;
    let secs = unix_secs % 86400;
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let seconds = secs % 60;

    // Approximate year (very rough, ignores leap years)
    let year = 1970 + (days / 365);
    let day_of_year = days % 365;

    let _ = write!(
        buf,
        "{:04}-{:03} {:02}:{:02}:{:02}",
        year, day_of_year, hours, mins, seconds
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        let mut buf = String::new();
        format_bytes(512, &mut buf);
        assert_eq!(buf, "512 B");

        format_bytes(2048, &mut buf);
        assert_eq!(buf, "2.0 KB");

        format_bytes(1024 * 1024 * 5, &mut buf);
        assert_eq!(buf, "5.0 MB");

        format_bytes(1024 * 1024 * 1024 * 3, &mut buf);
        assert_eq!(buf, "3.0 GB");
    }

    #[test]
    fn test_format_cpu_time() {
        let mut buf = String::new();
        // 500ms = 5_000_000 * 100ns
        format_cpu_time(5_000_000, &mut buf);
        assert_eq!(buf, "500ms");

        // 1.5s = 15_000_000 * 100ns
        format_cpu_time(15_000_000, &mut buf);
        assert_eq!(buf, "1.500s");

        // 90.5s = 905_000_000 * 100ns = 1:30.500
        format_cpu_time(905_000_000, &mut buf);
        assert_eq!(buf, "1:30.500");
    }

    #[test]
    fn test_column_config_default() {
        let cfg = ColumnConfig::default();
        // Minimal defaults — Name, PID, Bits, Status — match NxT reference UI.
        assert!(cfg.bits);
        assert!(cfg.status);
        assert!(!cfg.memory);
        assert!(!cfg.cpu_percent);
        assert!(!cfg.threads);
        assert!(!cfg.handles);
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

    #[test]
    fn test_format_cpu_percent() {
        let mut buf = String::new();
        format_cpu_percent(0.0, &mut buf);
        assert_eq!(buf, "—");

        format_cpu_percent(0.3, &mut buf);
        assert_eq!(buf, "0.3%");

        format_cpu_percent(3.5, &mut buf);
        assert_eq!(buf, "3.5%");

        format_cpu_percent(42.0, &mut buf);
        assert_eq!(buf, "42%");

        // NaN / Inf → fallback to em-dash
        format_cpu_percent(f32::NAN, &mut buf);
        assert_eq!(buf, "—");
    }
}
