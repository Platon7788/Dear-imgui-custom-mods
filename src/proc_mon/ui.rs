//! Process monitor UI component.
//!
//! Displays a virtualized table of processes with search and selection.
//! Context menu is NOT rendered by this component — the caller handles
//! `MonitorEvent::ContextMenuRequested` and renders their own popup.

use crate::proc_mon::config::MonitorConfig;
use crate::proc_mon::types::{
    format_bytes, format_cpu_percent, format_cpu_time, format_create_time, ColumnConfig,
    MonitorEvent, ProcStatus, ProcessDelta, ProcessInfo,
};
use crate::virtual_table::{
    CellAlignment, CellValue, ColumnDef, RowDensity, RowStyle, SelectionMode, TableConfig,
    VirtualTable, VirtualTableRow,
};
use dear_imgui_rs::Ui;
use std::collections::HashMap;
use std::fmt::Write;

// ─── Fast hasher alias (u32-keyed PID map) ───────────────────────────────────

type FxMap<K, V> = HashMap<K, V, foldhash::fast::FixedState>;

// ─── ProcessRow adapter ──────────────────────────────────────────────────────

/// Row adapter for VirtualTable with cached formatted strings.
///
/// All volatile numeric columns (memory, I/O, CPU%, CPU-time) are pre-formatted
/// once per upsert — the table's `cell_display_text` only pushes bytes into
/// the scratch buffer, no allocation on render hot path.
pub struct ProcessRow {
    info: ProcessInfo,
    // Cached formatted columns.
    mem_str: String,
    private_str: String,
    virtual_str: String,
    peak_mem_str: String,
    io_read_str: String,
    io_write_str: String,
    cpu_time_str: String,
    cpu_pct_str: String,
    create_time_str: String,
}

impl From<ProcessInfo> for ProcessRow {
    fn from(info: ProcessInfo) -> Self {
        let mut row = Self {
            info,
            mem_str: String::new(),
            private_str: String::new(),
            virtual_str: String::new(),
            peak_mem_str: String::new(),
            io_read_str: String::new(),
            io_write_str: String::new(),
            cpu_time_str: String::new(),
            cpu_pct_str: String::new(),
            create_time_str: String::new(),
        };
        row.update_cached_strings();
        row
    }
}

impl ProcessRow {
    /// Update cached formatted strings.
    fn update_cached_strings(&mut self) {
        format_bytes(self.info.working_set, &mut self.mem_str);
        format_bytes(self.info.private_bytes, &mut self.private_str);
        format_bytes(self.info.virtual_size, &mut self.virtual_str);
        format_bytes(self.info.peak_working_set, &mut self.peak_mem_str);
        format_bytes(self.info.io_read_bytes as usize, &mut self.io_read_str);
        format_bytes(self.info.io_write_bytes as usize, &mut self.io_write_str);
        format_cpu_time(
            self.info.kernel_time.saturating_add(self.info.user_time),
            &mut self.cpu_time_str,
        );
        format_cpu_percent(self.info.cpu_percent, &mut self.cpu_pct_str);
        format_create_time(self.info.create_time, &mut self.create_time_str);
    }

    /// Update only the volatile caches that change per tick (memory, I/O, CPU).
    /// Avoids reformatting `create_time_str` which never changes.
    fn update_volatile(&mut self) {
        format_bytes(self.info.working_set, &mut self.mem_str);
        format_bytes(self.info.private_bytes, &mut self.private_str);
        format_bytes(self.info.virtual_size, &mut self.virtual_str);
        format_bytes(self.info.peak_working_set, &mut self.peak_mem_str);
        format_bytes(self.info.io_read_bytes as usize, &mut self.io_read_str);
        format_bytes(self.info.io_write_bytes as usize, &mut self.io_write_str);
        format_cpu_time(
            self.info.kernel_time.saturating_add(self.info.user_time),
            &mut self.cpu_time_str,
        );
        format_cpu_percent(self.info.cpu_percent, &mut self.cpu_pct_str);
    }

    /// Get the PID.
    pub const fn pid(&self) -> u32 {
        self.info.pid
    }

    /// Get the process name.
    pub fn name(&self) -> &str {
        &self.info.name
    }

    /// Get the process bitness.
    pub const fn bits(&self) -> u8 {
        self.info.bits
    }

    /// Get the process status.
    pub const fn status(&self) -> ProcStatus {
        self.info.status
    }

    /// Get the CPU usage percent.
    pub const fn cpu_percent(&self) -> f32 {
        self.info.cpu_percent
    }
}

// ─── Table cell dispatch ──────────────────────────────────────────────────────

/// Column index is canonical (0..=17): `ProcessMonitor` always registers
/// every column in the same order and uses `ColumnDef::visible(flag)` to
/// hide the ones disabled in `ColumnConfig`. ImGui's `table_set_column_enabled`
/// suppresses rendering but does not reorder indices, so the match below
/// stays correct regardless of which columns are hidden.
impl VirtualTableRow for ProcessRow {
    fn cell_value(&self, _col: usize) -> CellValue {
        CellValue::Custom
    }

    fn set_cell_value(&mut self, _col: usize, _value: &CellValue) {
        // Read-only.
    }

    /// Default dispatch assumes no columns are hidden (column index = key).
    /// When columns are hidden `ProcessMonitor` routes through its own mapper,
    /// but `VirtualTable` also calls this for tooltip fallbacks — use the
    /// canonical layout order.
    fn cell_display_text(&self, col: usize, buf: &mut String) {
        // Canonical order matches `build_columns` when everything is visible.
        match col {
            0 => buf.push_str(&self.info.name),
            1 => {
                let _ = write!(buf, "{}", self.info.pid);
            }
            2 => {
                let _ = write!(buf, "x{}", self.info.bits);
            }
            3 => buf.push_str(self.info.status.label()),
            4 => buf.push_str(&self.mem_str),
            5 => buf.push_str(&self.cpu_pct_str),
            6 => {
                let _ = write!(buf, "{}", self.info.ppid);
            }
            7 => {
                let _ = write!(buf, "{}", self.info.session_id);
            }
            8 => {
                let _ = write!(buf, "{}", self.info.priority);
            }
            9 => {
                let _ = write!(buf, "{}", self.info.thread_count);
            }
            10 => {
                let _ = write!(buf, "{}", self.info.handle_count);
            }
            11 => buf.push_str(&self.private_str),
            12 => buf.push_str(&self.virtual_str),
            13 => buf.push_str(&self.peak_mem_str),
            14 => buf.push_str(&self.io_read_str),
            15 => buf.push_str(&self.io_write_str),
            16 => buf.push_str(&self.cpu_time_str),
            17 => buf.push_str(&self.create_time_str),
            _ => {}
        }
    }

    fn row_style(&self) -> Option<RowStyle> {
        match self.info.status {
            ProcStatus::Suspended => Some(RowStyle {
                bg_color: Some([0.88, 0.55, 0.10, 0.20]),
                ..RowStyle::default()
            }),
            ProcStatus::Running => None,
        }
    }
}

// ─── Monitor state ───────────────────────────────────────────────────────────

/// Process monitor UI state.
pub struct ProcessMonitor {
    /// Table renderer.
    table: VirtualTable<ProcessRow>,
    /// All processes indexed by PID for O(1) upsert/remove (foldhash).
    processes: FxMap<u32, ProcessRow>,
    /// Sorted view for rendering — stores only PIDs.
    sorted_pids: Vec<u32>,
    /// Whether sorted view needs rebuild.
    dirty: bool,
    /// Total process count.
    pub total_count: usize,
    /// Search input buffer.
    pub search_buf: String,
    /// Cached lowercase search query.
    search_lower: String,
    /// Cached PID formatter buffer (reusable across rebuild_sorted ticks).
    pid_scratch: String,
    /// Reusable format buffer for header/footer.
    fmt_buf: String,
    /// Column configuration.
    columns: ColumnConfig,
    /// Window title.
    window_title: String,
    /// Whether to show search bar.
    show_search: bool,
    /// Cached system-wide CPU% (sum across processes). Recomputed only on
    /// `apply_delta` / `set_full_list` — avoids per-frame iteration.
    cached_system_cpu: f32,
}

impl ProcessMonitor {
    /// Create a new process monitor with the given configuration.
    pub fn new(config: MonitorConfig) -> Self {
        let columns = Self::build_columns(&config.columns);
        let table_config = TableConfig {
            auto_scroll: false,
            selection_mode: SelectionMode::Single,
            sortable: false, // Fixed sort by create_time
            resizable: true,
            row_density: RowDensity::Dense,
            highlight_hovered: false,
            default_clip_tooltip: false,
            ..TableConfig::default()
        };
        let table = VirtualTable::new("##proc_table", columns, 10_000, table_config);

        Self {
            table,
            processes: FxMap::with_capacity_and_hasher(512, foldhash::fast::FixedState::default()),
            sorted_pids: Vec::with_capacity(512),
            dirty: false,
            total_count: 0,
            search_buf: String::with_capacity(128),
            search_lower: String::with_capacity(128),
            pid_scratch: String::with_capacity(12),
            fmt_buf: String::with_capacity(64),
            columns: config.columns,
            window_title: config.window_title.to_string(),
            show_search: config.show_search,
            cached_system_cpu: 0.0,
        }
    }

    /// Build column definitions from configuration.
    ///
    /// **Canonical layout**: indices 0..=17 are fixed. Hidden columns are still
    /// registered but marked `.visible(false)` — this keeps `col_idx` stable
    /// across configurations and lets `VirtualTableRow::cell_display_text`
    /// match on raw indices without a dynamic mapping.
    fn build_columns(cfg: &ColumnConfig) -> Vec<ColumnDef> {
        vec![
            // 0 — Name: stretch so it absorbs the remaining width; PID/Bits/
            //     Status stay pinned to the right edge at fixed widths.
            ColumnDef::new("Process Name").stretch(1.0).clip_tooltip(true),
            // 1 — PID: center-aligned numeric.
            ColumnDef::new("PID").fixed(70.0).align(CellAlignment::Center),
            // 2 — Bits: center-aligned (already was).
            ColumnDef::new("Bits")
                .fixed(45.0)
                .align(CellAlignment::Center)
                .visible(cfg.bits),
            // 3 — Status: center-aligned, 70 px per request.
            ColumnDef::new("Status")
                .fixed(70.0)
                .align(CellAlignment::Center)
                .visible(cfg.status),
            // 4
            ColumnDef::new("Memory")
                .fixed(90.0)
                .align(CellAlignment::Right)
                .visible(cfg.memory),
            // 5
            ColumnDef::new("CPU %")
                .fixed(65.0)
                .align(CellAlignment::Right)
                .visible(cfg.cpu_percent),
            // 6
            ColumnDef::new("PPID").fixed(70.0).visible(cfg.ppid),
            // 7
            ColumnDef::new("Session").fixed(70.0).visible(cfg.session_id),
            // 8
            ColumnDef::new("Priority").fixed(70.0).visible(cfg.priority),
            // 9
            ColumnDef::new("Threads").fixed(70.0).visible(cfg.threads),
            // 10
            ColumnDef::new("Handles").fixed(70.0).visible(cfg.handles),
            // 11
            ColumnDef::new("Private")
                .fixed(90.0)
                .align(CellAlignment::Right)
                .visible(cfg.private_bytes),
            // 12
            ColumnDef::new("VM Size")
                .fixed(90.0)
                .align(CellAlignment::Right)
                .visible(cfg.virtual_size),
            // 13
            ColumnDef::new("Peak Mem")
                .fixed(90.0)
                .align(CellAlignment::Right)
                .visible(cfg.peak_memory),
            // 14
            ColumnDef::new("I/O Read")
                .fixed(90.0)
                .align(CellAlignment::Right)
                .visible(cfg.io_read),
            // 15
            ColumnDef::new("I/O Write")
                .fixed(90.0)
                .align(CellAlignment::Right)
                .visible(cfg.io_write),
            // 16
            ColumnDef::new("CPU Time").fixed(100.0).visible(cfg.cpu_time),
            // 17
            ColumnDef::new("Created").fixed(120.0).visible(cfg.create_time),
        ]
    }

    /// Handle full process list (first snapshot).
    pub fn set_full_list(&mut self, procs: &[ProcessInfo]) {
        self.processes.clear();
        for p in procs {
            self.processes.insert(p.pid, ProcessRow::from(p.clone()));
        }
        self.total_count = procs.len();
        self.dirty = true;
        self.recompute_system_cpu();
    }

    /// Handle incremental delta update.
    ///
    /// In-place update: when a PID already exists, mutate its `ProcessInfo`
    /// and refresh only volatile cached strings — avoids dropping the whole
    /// row and re-formatting immutable columns (name, create_time).
    pub fn apply_delta(&mut self, delta: &ProcessDelta) {
        let mut changed = false;

        for pid in &delta.removed {
            if self.processes.remove(pid).is_some() {
                changed = true;
            }
        }

        for p in &delta.upsert {
            match self.processes.get_mut(&p.pid) {
                Some(row) => {
                    row.info = p.clone();
                    row.update_volatile();
                }
                None => {
                    self.processes.insert(p.pid, ProcessRow::from(p.clone()));
                }
            }
            changed = true;
        }

        self.total_count = delta.total;

        if changed {
            self.dirty = true;
        }
        self.recompute_system_cpu();
    }

    /// Recompute cached system-wide CPU aggregate. Called once per
    /// `apply_delta` / `set_full_list`, not per frame. Skipped when the
    /// CPU% column is hidden — all values would be `0.0` anyway.
    fn recompute_system_cpu(&mut self) {
        if !self.columns.cpu_percent {
            self.cached_system_cpu = 0.0;
            return;
        }
        let sum: f32 = self.processes.values().map(|r| r.info.cpu_percent).sum();
        self.cached_system_cpu = sum.clamp(0.0, 100.0);
    }

    /// Rebuild the sorted PID list if dirty.
    fn rebuild_sorted(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;

        self.sorted_pids.clear();

        let query = if self.search_lower.is_empty() {
            None
        } else {
            Some(self.search_lower.as_str())
        };

        for row in self.processes.values() {
            if let Some(q) = query {
                // PID → string via reusable scratch buffer (no cursor, no alloc).
                self.pid_scratch.clear();
                let _ = write!(&mut self.pid_scratch, "{}", row.info.pid);

                let name_bytes = row.info.name.as_bytes();
                let q_bytes = q.as_bytes();
                let name_match = q_bytes.len() <= name_bytes.len()
                    && name_bytes
                        .windows(q_bytes.len())
                        .any(|w| w.eq_ignore_ascii_case(q_bytes));

                if !name_match && !self.pid_scratch.contains(q) {
                    continue;
                }
            }
            self.sorted_pids.push(row.info.pid);
        }

        // Sort by CreateTime descending (newest first).
        let procs = &self.processes;
        self.sorted_pids.sort_by(|a, b| {
            let ta = procs.get(a).map_or(0, |r| r.info.create_time);
            let tb = procs.get(b).map_or(0, |r| r.info.create_time);
            tb.cmp(&ta)
        });
    }

    /// Get the currently selected PID (if any).
    pub fn selected_pid(&self) -> Option<u32> {
        self.table
            .selected_row()
            .and_then(|idx| self.sorted_pids.get(idx).copied())
    }

    /// Mark dirty to force rebuild (e.g., when search changes).
    pub fn invalidate(&mut self) {
        self.dirty = true;
        self.search_lower.clear();
        self.search_lower.push_str(&self.search_buf);
        self.search_lower.make_ascii_lowercase();
    }

    /// Update column visibility. Rebuilds the underlying `VirtualTable` with
    /// the new column set.
    pub fn set_columns(&mut self, columns: ColumnConfig) {
        self.columns = columns;
        let new_cols = Self::build_columns(&self.columns);
        let table_config = TableConfig {
            auto_scroll: false,
            selection_mode: SelectionMode::Single,
            sortable: false,
            resizable: true,
            row_density: RowDensity::Dense,
            highlight_hovered: false,
            default_clip_tooltip: false,
            ..TableConfig::default()
        };
        self.table = VirtualTable::new("##proc_table", new_cols, 10_000, table_config);
        self.dirty = true;
    }

    /// Render the process monitor window.
    ///
    /// Returns an event if the user interacted with a row.
    /// The caller should handle `ContextMenuRequested` by rendering their own popup.
    pub fn render(&mut self, ui: &Ui, show: &mut bool) -> Option<MonitorEvent> {
        let [dw, dh] = ui.io().display_size();
        let win_w = 600.0_f32;
        let win_h = 500.0_f32;

        self.rebuild_sorted();

        let mut opened = *show;
        let mut action: Option<MonitorEvent> = None;

        let title = self.window_title.clone();

        ui.window(&title)
            .size([win_w, win_h], dear_imgui_rs::Condition::Appearing)
            .position(
                [dw * 0.5 - win_w * 0.5, dh * 0.5 - win_h * 0.5],
                dear_imgui_rs::Condition::FirstUseEver,
            )
            .flags(dear_imgui_rs::WindowFlags::NO_COLLAPSE)
            .opened(&mut opened)
            .build(|| {
                // Suppress header hover/active highlight — keeps the header
                // row a flat, non-clickable strip (we're not sortable and
                // don't want imgui's default button-like header feedback).
                // Scope covers the entire window body including the table.
                let _hdr_hover = ui.push_style_color(
                    dear_imgui_rs::StyleColor::HeaderHovered,
                    [0.0, 0.0, 0.0, 0.0],
                );
                let _hdr_active = ui.push_style_color(
                    dear_imgui_rs::StyleColor::HeaderActive,
                    [0.0, 0.0, 0.0, 0.0],
                );

                // Header: total count (+ system CPU% only when tracked).
                self.fmt_buf.clear();
                if self.columns.cpu_percent {
                    let _ = write!(
                        &mut self.fmt_buf,
                        "Total: {}   |   System CPU: {:.1}%",
                        self.total_count, self.cached_system_cpu,
                    );
                } else {
                    let _ = write!(&mut self.fmt_buf, "Total: {}", self.total_count);
                }
                ui.text(&self.fmt_buf);
                ui.separator();

                // Search bar.
                if self.show_search {
                    let search_width = ui.content_region_avail()[0] - 140.0;
                    ui.set_next_item_width(search_width.max(100.0));
                    let search_changed = ui.input_text("##search", &mut self.search_buf).build();

                    if search_changed {
                        self.invalidate();
                    }

                    ui.same_line();
                    {
                        let _c = [
                            ui.push_style_color(
                                dear_imgui_rs::StyleColor::Button,
                                [0.24, 0.48, 0.28, 1.0],
                            ),
                            ui.push_style_color(
                                dear_imgui_rs::StyleColor::ButtonHovered,
                                [0.30, 0.58, 0.34, 1.0],
                            ),
                            ui.push_style_color(
                                dear_imgui_rs::StyleColor::ButtonActive,
                                [0.20, 0.42, 0.24, 1.0],
                            ),
                        ];
                        if ui.button("Search") {
                            self.invalidate();
                        }
                    }
                    ui.same_line();
                    if ui.button("Clear") {
                        self.search_buf.clear();
                        self.invalidate();
                    }
                    ui.separator();
                }

                // Render table from sorted view.
                let sorted_pids = &self.sorted_pids;
                let processes = &self.processes;
                let sorted_len = sorted_pids.len();

                self.table.render_lookup(ui, sorted_len, |idx| {
                    sorted_pids.get(idx).and_then(|pid| processes.get(pid))
                });

                // Check for events.
                if let Some(idx) = self.table.double_clicked_row
                    && let Some(pid) = sorted_pids.get(idx)
                {
                    action = Some(MonitorEvent::RowDoubleClicked(*pid));
                }

                if self.table.open_context_menu {
                    if let Some(idx) = self.table.context_row
                        && let Some(pid) = sorted_pids.get(idx)
                    {
                        action = Some(MonitorEvent::ContextMenuRequested(*pid));
                    }
                    // Reset flag so caller can render their own popup.
                    self.table.open_context_menu = false;
                }
            });

        if !opened {
            *show = false;
        }

        action
    }
}

