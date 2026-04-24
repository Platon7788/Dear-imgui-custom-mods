//! Process monitor UI component.
//!
//! Displays a virtualized table of processes with search and selection.
//! Context menu is NOT rendered by this component — the caller handles
//! [`MonitorEvent::ContextMenuRequested`] and renders their own popup.
//!
//! # Layout
//!
//! 4 columns total, always in canonical order `Name | PID | Bits | Status`.
//! Name stretches with the window; PID / Bits / Status are fixed-width and
//! pinned to the right edge. `Bits` and `Status` can be hidden via
//! [`ColumnConfig`] (they stay registered on the table with `visible(false)`
//! so indices remain stable).

use crate::proc_mon::config::MonitorConfig;
use crate::proc_mon::types::{
    ColumnConfig, MonitorColors, MonitorEvent, ProcStatus, ProcessDelta, ProcessInfo,
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

/// Row adapter for [`VirtualTable`].
///
/// The minimal [`ProcessInfo`] only has three display fields beyond the
/// always-visible name (`pid` → integer, `bits` → `x32` / `x64`, `status` →
/// `"Running"` / `"Suspended"`). All three are cheap to format on demand
/// and immutable per-PID after the first upsert, so there are no cached
/// strings — `cell_display_text` formats directly into the shared scratch
/// buffer the table owns.
pub struct ProcessRow {
    info: ProcessInfo,
    /// Cached row background tint resolved from [`MonitorColors`] at upsert
    /// time — so rendering is pure lookup, no rule evaluation per frame.
    color_override: Option<[f32; 4]>,
}

impl From<ProcessInfo> for ProcessRow {
    fn from(info: ProcessInfo) -> Self {
        Self {
            info,
            color_override: None,
        }
    }
}

impl ProcessRow {
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

    /// Apply a pre-resolved color override (used by `ProcessMonitor` during
    /// upsert / `set_colors` so row rendering stays pure lookup).
    #[inline]
    fn set_color_override(&mut self, color: Option<[f32; 4]>) {
        self.color_override = color;
    }
}

// ─── Table cell dispatch ──────────────────────────────────────────────────────

/// Column index is canonical (0..=3): `ProcessMonitor` always registers
/// every column in the same order and uses `ColumnDef::visible(flag)` to
/// hide `Bits` / `Status` when disabled. ImGui's `table_set_column_enabled`
/// suppresses rendering but does not reorder indices, so the match below
/// stays correct regardless of which columns are hidden.
impl VirtualTableRow for ProcessRow {
    fn cell_value(&self, _col: usize) -> CellValue {
        CellValue::Custom
    }

    fn set_cell_value(&mut self, _col: usize, _value: &CellValue) {
        // Read-only.
    }

    fn cell_display_text(&self, col: usize, buf: &mut String) {
        match col {
            0 => buf.push_str(&self.info.name),
            1 => {
                let _ = write!(buf, "{}", self.info.pid);
            }
            2 => {
                let _ = write!(buf, "x{}", self.info.bits);
            }
            3 => buf.push_str(self.info.status.label()),
            _ => {}
        }
    }

    fn row_style(&self) -> Option<RowStyle> {
        // Pre-resolved in `ProcessMonitor::apply_delta` / `set_colors`.
        // Render path is pure lookup — no rule evaluation per frame.
        self.color_override.map(|bg| RowStyle {
            bg_color: Some(bg),
            ..RowStyle::default()
        })
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
    /// Row-highlight palette. Resolved into `ProcessRow::color_override` at
    /// upsert time (or on `set_colors` refresh) so rendering is pure lookup.
    colors: MonitorColors,
    /// PID of the current process — captured once at construction so
    /// [`MonitorColors::self_process`] matches without calling
    /// `std::process::id()` every frame.
    self_pid: u32,
    /// Window title.
    window_title: String,
    /// Whether to show search bar.
    show_search: bool,
}

impl ProcessMonitor {
    /// Create a new process monitor with the given configuration.
    pub fn new(config: MonitorConfig) -> Self {
        let columns = Self::build_columns(&config.columns);
        let table = VirtualTable::new("##proc_table", columns, 10_000, default_table_config());

        Self {
            table,
            processes: FxMap::with_capacity_and_hasher(512, foldhash::fast::FixedState::default()),
            sorted_pids: Vec::with_capacity(512),
            dirty: false,
            total_count: 0,
            search_buf: String::with_capacity(128),
            search_lower: String::with_capacity(128),
            pid_scratch: String::with_capacity(12),
            fmt_buf: String::with_capacity(32),
            columns: config.columns,
            colors: config.colors,
            self_pid: std::process::id(),
            window_title: config.window_title.to_string(),
            show_search: config.show_search,
        }
    }

    /// Build column definitions from configuration.
    ///
    /// **Canonical layout**: indices 0..=3 are fixed. Hidden columns are still
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
            // 2 — Bits: center-aligned `x32` / `x64`.
            ColumnDef::new("Bits")
                .fixed(45.0)
                .align(CellAlignment::Center)
                .visible(cfg.bits),
            // 3 — Status: center-aligned.
            ColumnDef::new("Status")
                .fixed(70.0)
                .align(CellAlignment::Center)
                .visible(cfg.status),
        ]
    }

    /// Handle full process list (first snapshot).
    pub fn set_full_list(&mut self, procs: &[ProcessInfo]) {
        self.processes.clear();
        for p in procs {
            let mut row = ProcessRow::from(p.clone());
            row.set_color_override(self.colors.resolve(&row.info, self.self_pid));
            self.processes.insert(p.pid, row);
        }
        self.total_count = procs.len();
        self.dirty = true;
    }

    /// Handle incremental delta update.
    ///
    /// In-place update: when a PID already exists, mutate its `ProcessInfo`
    /// and re-resolve the color override (status may have flipped Running ↔
    /// Suspended). No cached strings to refresh.
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
                    let c = self.colors.resolve(&row.info, self.self_pid);
                    row.set_color_override(c);
                }
                None => {
                    let mut row = ProcessRow::from(p.clone());
                    row.set_color_override(self.colors.resolve(&row.info, self.self_pid));
                    self.processes.insert(p.pid, row);
                }
            }
            changed = true;
        }

        self.total_count = delta.total;

        if changed {
            self.dirty = true;
        }
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

    /// Read-only access to the current highlight palette.
    pub const fn colors(&self) -> &MonitorColors {
        &self.colors
    }

    /// Mutable access for small edits (e.g. `monitor.colors_mut().add_pid(42, ...)`).
    /// Call [`refresh_colors`](Self::refresh_colors) afterwards so the change
    /// is reflected on already-tracked rows.
    pub fn colors_mut(&mut self) -> &mut MonitorColors {
        &mut self.colors
    }

    /// Replace the highlight palette and re-resolve overrides for every
    /// tracked row. This is the only way colors take effect without waiting
    /// for a fresh upsert.
    pub fn set_colors(&mut self, colors: MonitorColors) {
        self.colors = colors;
        self.refresh_colors();
    }

    /// Re-resolve [`MonitorColors`] against every tracked process. Call this
    /// after mutating via [`colors_mut`](Self::colors_mut) — otherwise only
    /// newly-upserted rows pick up the edit.
    pub fn refresh_colors(&mut self) {
        let colors = &self.colors;
        let self_pid = self.self_pid;
        for row in self.processes.values_mut() {
            row.set_color_override(colors.resolve(&row.info, self_pid));
        }
    }

    /// PID of the host process — what [`MonitorColors::self_process`] matches.
    pub const fn self_pid(&self) -> u32 {
        self.self_pid
    }

    /// Update column visibility (`Bits` / `Status`). Rebuilds the underlying
    /// `VirtualTable` with the new visibility flags — indices stay canonical.
    pub fn set_columns(&mut self, columns: ColumnConfig) {
        self.columns = columns;
        let new_cols = Self::build_columns(&self.columns);
        self.table = VirtualTable::new("##proc_table", new_cols, 10_000, default_table_config());
        self.dirty = true;
    }

    /// Look up a tracked process by PID. Returns `None` if the PID is not
    /// currently present in the monitor.
    pub fn process(&self, pid: u32) -> Option<&ProcessRow> {
        self.processes.get(&pid)
    }

    /// Render the monitor body (header + search + table) without opening
    /// an `ui.window`. Use this when embedding the widget inside an existing
    /// parent container with fixed layout (e.g. a panel inside a docked
    /// main-view window). The caller owns the outer window/positioning and
    /// is responsible for clipping / sizing the content region.
    pub fn render_contents(&mut self, ui: &Ui) -> Option<MonitorEvent> {
        self.rebuild_sorted();
        let mut action: Option<MonitorEvent> = None;

        // Flat headers (no hover/active tint on captions) come from the
        // underlying `VirtualTable` via `TableConfig::flat_headers = true`
        // in `default_table_config` — no window-wide style push is needed
        // here, and row selection highlight (which reuses the same style
        // colors) stays intact.

        // Header: total count only — no system-CPU aggregate any more.
        self.fmt_buf.clear();
        let _ = write!(&mut self.fmt_buf, "Total: {}", self.total_count);
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

        action
    }

    /// Render the process monitor window.
    ///
    /// Returns an event if the user interacted with a row.
    /// The caller should handle `ContextMenuRequested` by rendering their own popup.
    pub fn render(&mut self, ui: &Ui, show: &mut bool) -> Option<MonitorEvent> {
        let [dw, dh] = ui.io().display_size();
        let win_w = 600.0_f32;
        let win_h = 500.0_f32;

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
                action = self.render_contents(ui);
            });

        if !opened {
            *show = false;
        }

        action
    }
}

/// Default `TableConfig` shared between `new` and `set_columns`.
fn default_table_config() -> TableConfig {
    TableConfig {
        auto_scroll: false,
        selection_mode: SelectionMode::Single,
        sortable: false,       // Fixed sort by create_time
        flat_headers: true,    // Non-interactive captions — no hover tint
        resizable: true,
        row_density: RowDensity::Dense,
        highlight_hovered: false,
        default_clip_tooltip: false,
        ..TableConfig::default()
    }
}
