//! Configuration for process monitor.

use crate::proc_mon::types::ColumnConfig;

/// Process monitor configuration.
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Which columns to display.
    pub columns: ColumnConfig,
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
            interval_ms: 1000,
            max_processes: 10_000,
            show_search: true,
            window_title: "Process Monitor",
        }
    }
}

impl MonitorConfig {
    /// Create config with all optional columns visible.
    pub fn all_columns() -> Self {
        Self {
            columns: ColumnConfig {
                bits: true,
                status: true,
                memory: true,
                cpu_percent: true,
                ppid: true,
                session_id: true,
                priority: true,
                threads: true,
                handles: true,
                private_bytes: true,
                virtual_size: true,
                peak_memory: true,
                io_read: true,
                io_write: true,
                cpu_time: true,
                create_time: true,
            },
            ..Default::default()
        }
    }

    /// Create minimal config (only essential columns).
    pub fn minimal() -> Self {
        Self {
            columns: ColumnConfig {
                bits: true,
                status: true,
                memory: false,
                cpu_percent: false,
                ..ColumnConfig::default()
            },
            ..Default::default()
        }
    }

    /// Get interval as Duration, clamped to valid range (1-5000ms).
    pub fn interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.interval_ms.clamp(1, 5000) as u64)
    }
}
