//! Configuration for process monitor.

use crate::proc_mon::types::{ColumnConfig, MonitorColors};

/// Process monitor configuration.
///
/// With the minimal 5-field `ProcessInfo` the only user-visible knobs are
/// the two optional columns (`Bits`, `Status`), the row-highlight palette,
/// and the poll interval — everything else is UI chrome.
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Which columns to display.
    pub columns: ColumnConfig,
    /// Row-highlight palette — suspended tint, self-process, per-name, per-PID.
    /// Default: only `Suspended` is tinted (amber, matches pre-0.9 behavior).
    pub colors: MonitorColors,
    /// Monitor update interval in milliseconds.
    /// Range: 1-5000ms. Default: 1000ms (1 update per second).
    pub interval_ms: u32,
    /// Maximum number of processes to keep in memory for delta tracking.
    /// Default: 10000.
    pub max_processes: usize,
    /// Whether to show search bar.
    pub show_search: bool,
    /// Window title (can be overridden for localization).
    pub window_title: &'static str,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            columns: ColumnConfig::default(),
            colors: MonitorColors::default(),
            interval_ms: 1000,
            max_processes: 10_000,
            show_search: true,
            window_title: "Process Monitor",
        }
    }
}

impl MonitorConfig {
    /// Minimal config: only `Name` + `PID` visible.
    ///
    /// Useful for dense list UIs where status / bitness aren't needed.
    pub fn minimal() -> Self {
        Self {
            columns: ColumnConfig {
                bits: false,
                status: false,
            },
            ..Default::default()
        }
    }

    /// All four columns visible (Name / PID / Bits / Status).
    ///
    /// Equivalent to [`ColumnConfig::default`] — kept as a named preset for
    /// call-site readability.
    pub fn all_columns() -> Self {
        Self {
            columns: ColumnConfig {
                bits: true,
                status: true,
            },
            ..Default::default()
        }
    }

    /// Get interval as Duration, clamped to valid range (1-5000ms).
    pub fn interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.interval_ms.clamp(1, 5000) as u64)
    }
}
