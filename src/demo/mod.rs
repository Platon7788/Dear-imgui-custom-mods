//! Demo module — interactive showcase of all custom components.
//!
//! Call `render_demo_window(ui)` every frame to display a window
//! with tabs for each component module.

use crate::file_manager::FileManager;
use crate::icons;
use crate::page_control::{
    Badge, ContentView, PageAction, PageControl, PageControlConfig, PageItem, PageStatus,
    PcColors, TabStyle, draw_mini_tile,
};
use crate::virtual_table::{
    CellAlignment, CellValue, ColumnDef, TableConfig, VirtualTable, VirtualTableRow,
};
use dear_imgui_rs::Ui;
use std::cmp::Ordering;
use std::fmt::Write;

// ═══════════════════════════════════════════════════════════════════════════
// VirtualTable demo row
// ═══════════════════════════════════════════════════════════════════════════

struct DemoRow {
    id: usize,
    name: String,
    value: f64,
    status: &'static str,
}

impl VirtualTableRow for DemoRow {
    fn cell_value(&self, col: usize) -> CellValue {
        match col {
            0 => CellValue::Int(self.id as i64),
            1 => CellValue::Text(self.name.clone()),
            2 => CellValue::Float(self.value),
            3 => CellValue::Text(self.status.to_string()),
            _ => CellValue::Text(String::new()),
        }
    }

    fn set_cell_value(&mut self, col: usize, value: &CellValue) {
        match col {
            1 => {
                if let CellValue::Text(s) = value {
                    self.name = s.clone();
                }
            }
            2 => {
                if let CellValue::Float(v) = value {
                    self.value = *v;
                }
            }
            _ => {}
        }
    }

    fn row_style(&self) -> Option<crate::virtual_table::RowStyle> {
        if self.status == "Error" {
            Some(crate::virtual_table::RowStyle {
                bg_color: Some([0.4, 0.15, 0.15, 1.0]),
                ..Default::default()
            })
        } else {
            None
        }
    }

    fn cell_style(&self, col: usize) -> Option<crate::virtual_table::CellStyle> {
        if col == 3 {
            let color = match self.status {
                "OK" => Some([0.3, 0.8, 0.4, 1.0]),
                "Warning" => Some([0.9, 0.7, 0.2, 1.0]),
                "Error" => Some([0.9, 0.3, 0.3, 1.0]),
                _ => None,
            };
            color.map(|c| crate::virtual_table::CellStyle {
                text_color: Some(c),
                ..Default::default()
            })
        } else {
            None
        }
    }

    fn compare(&self, other: &Self, col: usize) -> Ordering {
        match col {
            0 => self.id.cmp(&other.id),
            1 => self.name.cmp(&other.name),
            2 => self.value.partial_cmp(&other.value).unwrap_or(Ordering::Equal),
            3 => self.status.cmp(other.status),
            _ => Ordering::Equal,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PageControl demo page types
// ═══════════════════════════════════════════════════════════════════════════

// ─── ServerPage: simulated server monitor ───────────────────────────────────

struct ServerPage {
    name: String,
    cpu: f32,
    memory: f32,
    connections: u32,
    alive: bool,
    uptime_secs: u64,
    log_lines: Vec<String>,
}

impl ServerPage {
    fn new(name: impl Into<String>, cpu: f32, mem: f32, conn: u32, alive: bool) -> Self {
        let n = name.into();
        let mut log_lines = Vec::new();
        for i in 0..20 {
            log_lines.push(format!("[{}] INFO  {} — heartbeat #{}", n, "system", i + 1));
        }
        Self {
            name: n,
            cpu,
            memory: mem,
            connections: conn,
            alive,
            uptime_secs: 86400 + (conn as u64 * 3600),
            log_lines,
        }
    }
}

impl PageItem for ServerPage {
    fn title(&self) -> &str {
        &self.name
    }

    fn icon(&self) -> Option<&str> {
        Some(icons::SERVER)
    }

    fn status(&self) -> PageStatus {
        if !self.alive {
            PageStatus::Error
        } else if self.cpu > 80.0 {
            PageStatus::Warning
        } else {
            PageStatus::Active
        }
    }

    fn badge(&self) -> Option<Badge> {
        if self.connections > 100 {
            Some(Badge::count(self.connections, [0xd0, 0x7a, 0x30]))
        } else {
            None
        }
    }

    fn render_tile_body(&self, ui: &Ui, area: [f32; 4]) -> Option<u64> {
        let [x, y, _w, _h] = area;
        let muted = [0x8a, 0x92, 0xa1];
        let text = [0xe0, 0xe4, 0xea];

        // CPU / Memory / Connections as text lines
        let mut buf = String::with_capacity(64);
        {
            let draw = ui.get_window_draw_list();
            let _ = write!(buf, "CPU: {:.0}%", self.cpu);
            draw.add_text([x, y], crate::utils::color::rgb_arr(
                if self.cpu > 80.0 { [0xd0, 0x7a, 0x30] } else { text }, 255), &buf);

            buf.clear();
            let _ = write!(buf, "MEM: {:.0}%", self.memory);
            draw.add_text([x, y + 14.0], crate::utils::color::rgb_arr(text, 255), &buf);

            buf.clear();
            let _ = write!(buf, "Conn: {}", self.connections);
            draw.add_text([x, y + 28.0], crate::utils::color::rgb_arr(muted, 200), &buf);
        }

        // Mini-tiles for services (draw_mini_tile gets its own DrawListMut)
        let mini_y = y + 48.0;
        let colors = PcColors::default();
        draw_mini_tile(ui, [x, mini_y], [72.0, 20.0],
            icons::SHIELD_LOCK, "Auth",
            [0x5b, 0x9b, 0xd5], &colors);
        draw_mini_tile(ui, [x + 76.0, mini_y], [72.0, 20.0],
            icons::RADAR, "Game",
            [0x5f, 0xb8, 0x70], &colors);
        None
    }

    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();

        // Server stats header
        ui.text_colored([0.54, 0.57, 0.63, 1.0], "Server Details");
        ui.separator();
        ui.spacing();

        ui.text(format!("{} CPU:  {:.1}%", icons::CHART_LINE, self.cpu));
        ui.text(format!("{} MEM:  {:.1}%", icons::DATABASE, self.memory));
        ui.text(format!("{} Conn: {}", icons::CONNECTION, self.connections));

        let h = self.uptime_secs / 3600;
        let m = (self.uptime_secs % 3600) / 60;
        ui.text(format!("{} Uptime: {}h {:02}m", icons::CLOCK_OUTLINE, h, m));

        ui.spacing();
        ui.separator();
        ui.spacing();

        // Simulated log output
        ui.text_colored([0.54, 0.57, 0.63, 1.0], "Recent Logs");
        ui.spacing();

        ui.child_window("##server_logs")
            .size([0.0, 0.0])
            .build(ui, || {
                for line in &self.log_lines {
                    ui.text_colored([0.65, 0.68, 0.73, 1.0], line);
                }
            });
    }
}

// ─── NestedPage: contains a nested PageControl ─────────────────────────────

struct SubPage {
    name: String,
    detail: String,
}

impl PageItem for SubPage {
    fn title(&self) -> &str {
        &self.name
    }

    fn icon(&self) -> Option<&str> {
        Some(icons::TEXT_BOX_OUTLINE)
    }

    fn status(&self) -> PageStatus {
        PageStatus::Active
    }

    fn render_tile_body(&self, ui: &Ui, area: [f32; 4]) -> Option<u64> {
        let draw = ui.get_window_draw_list();
        let [x, y, _w, _h] = area;
        draw.add_text(
            [x, y],
            crate::utils::color::rgb_arr([0x8a, 0x92, 0xa1], 200),
            &self.detail,
        );
        None
    }

    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();
        ui.text_colored([0.88, 0.90, 0.92, 1.0], &self.name);
        ui.spacing();
        ui.text_wrapped(&self.detail);
    }
}

struct NestedPage {
    name: String,
    inner: PageControl<SubPage>,
    sub_counter: u64,
}

impl NestedPage {
    fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let config = PageControlConfig {
            tile_width: 180.0,
            tile_header_height: 32.0,
            tile_body_height: 50.0,
            tile_gap: 8.0,
            confirm_close: false,
            ..Default::default()
        };
        let mut inner = PageControl::with_config(format!("##nested_{}", name), config);
        let mut counter = 0u64;

        // Pre-populate with a few sub-pages
        for i in 1..=3 {
            counter += 1;
            inner.add(SubPage {
                name: format!("Sub-{}", i),
                detail: format!("Nested sub-page {} inside '{}'.", i, name),
            });
        }

        Self {
            name,
            inner,
            sub_counter: counter,
        }
    }
}

impl PageItem for NestedPage {
    fn title(&self) -> &str {
        &self.name
    }

    fn icon(&self) -> Option<&str> {
        Some(icons::LAYERS_OUTLINE)
    }

    fn status(&self) -> PageStatus {
        PageStatus::Active
    }

    fn badge(&self) -> Option<Badge> {
        let n = self.inner.page_count() as u32;
        if n > 0 {
            Some(Badge::count(n, [0x5b, 0x9b, 0xd5]))
        } else {
            None
        }
    }

    fn render_tile_body(&self, ui: &Ui, area: [f32; 4]) -> Option<u64> {
        let draw = ui.get_window_draw_list();
        let [x, y, _w, _h] = area;
        let mut buf = String::with_capacity(32);
        let _ = write!(buf, "{} sub-pages", self.inner.page_count());
        draw.add_text(
            [x, y],
            crate::utils::color::rgb_arr([0x8a, 0x92, 0xa1], 200),
            &buf,
        );
        None
    }

    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();

        // Controls for the nested PageControl
        if ui.button("Add Sub-Page") {
            self.sub_counter += 1;
            self.inner.add(SubPage {
                name: format!("Sub-{}", self.sub_counter),
                detail: format!("Dynamically added sub-page #{}.", self.sub_counter),
            });
        }
        ui.same_line();
        let view_label = match self.inner.view {
            ContentView::Dashboard => "View: Dashboard",
            ContentView::Tabs => "View: Tabs",
            ContentView::Custom(_) => "View: Custom",
        };
        if ui.button(view_label) {
            self.inner.view = match self.inner.view {
                ContentView::Dashboard => ContentView::Tabs,
                _ => ContentView::Dashboard,
            };
        }
        ui.same_line();
        ui.text_colored(
            [0.54, 0.57, 0.63, 1.0],
            format!("({} pages)", self.inner.page_count()),
        );

        ui.spacing();
        ui.separator();
        ui.spacing();

        // Render the nested PageControl
        ui.child_window("##nested_content")
            .size([0.0, 0.0])
            .build(ui, || {
                if let Some(PageAction::TileClicked(id)) = self.inner.render(ui) {
                    self.inner.set_active(id);
                    self.inner.view = ContentView::Tabs;
                }
            });
    }
}

// ─── InfoPage: simple static page ──────────────────────────────────────────

struct InfoPage {
    name: String,
    description: String,
    closable: bool,
}

impl PageItem for InfoPage {
    fn title(&self) -> &str {
        &self.name
    }

    fn icon(&self) -> Option<&str> {
        Some(icons::INFORMATION)
    }

    fn is_closable(&self) -> bool {
        self.closable
    }

    fn status(&self) -> PageStatus {
        PageStatus::Active
    }

    fn render_tile_body(&self, ui: &Ui, area: [f32; 4]) -> Option<u64> {
        let draw = ui.get_window_draw_list();
        let [x, y, w, _h] = area;
        // Truncate text to tile width
        let text = self.description.char_indices()
            .nth(30)
            .map(|(i, _)| &self.description[..i])
            .unwrap_or(&self.description);
        let _ = w;
        draw.add_text(
            [x, y],
            crate::utils::color::rgb_arr([0x8a, 0x92, 0xa1], 180),
            text,
        );
        None
    }

    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();
        ui.text_colored([0.88, 0.90, 0.92, 1.0], &self.name);
        ui.spacing();
        ui.text_wrapped(&self.description);

        if !self.closable {
            ui.spacing();
            ui.text_colored([0.54, 0.57, 0.63, 1.0], "(This page cannot be closed)");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// We need a single enum to hold different page types for PageControl<T>
// since PageControl is generic over a single T.
// ═══════════════════════════════════════════════════════════════════════════

enum DemoPage {
    Server(ServerPage),
    Nested(Box<NestedPage>),
    Info(InfoPage),
}

impl PageItem for DemoPage {
    fn title(&self) -> &str {
        match self {
            Self::Server(p) => p.title(),
            Self::Nested(p) => p.title(),
            Self::Info(p) => p.title(),
        }
    }

    fn icon(&self) -> Option<&str> {
        match self {
            Self::Server(p) => p.icon(),
            Self::Nested(p) => p.icon(),
            Self::Info(p) => p.icon(),
        }
    }

    fn is_closable(&self) -> bool {
        match self {
            Self::Server(p) => p.is_closable(),
            Self::Nested(p) => p.is_closable(),
            Self::Info(p) => p.is_closable(),
        }
    }

    fn status(&self) -> PageStatus {
        match self {
            Self::Server(p) => p.status(),
            Self::Nested(p) => p.status(),
            Self::Info(p) => p.status(),
        }
    }

    fn badge(&self) -> Option<Badge> {
        match self {
            Self::Server(p) => p.badge(),
            Self::Nested(p) => p.badge(),
            Self::Info(p) => p.badge(),
        }
    }

    fn tooltip(&self) -> Option<&str> {
        match self {
            Self::Server(p) => p.tooltip(),
            Self::Nested(p) => p.tooltip(),
            Self::Info(p) => p.tooltip(),
        }
    }

    fn render_tile_body(&self, ui: &Ui, area: [f32; 4]) -> Option<u64> {
        match self {
            Self::Server(p) => p.render_tile_body(ui, area),
            Self::Nested(p) => p.render_tile_body(ui, area),
            Self::Info(p) => p.render_tile_body(ui, area),
        }
    }

    fn render_content(&mut self, ui: &Ui) {
        match self {
            Self::Server(p) => p.render_content(ui),
            Self::Nested(p) => p.render_content(ui),
            Self::Info(p) => p.render_content(ui),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Second PageControl — simpler, with just InfoPages (tests independent
// instances side by side)
// ═══════════════════════════════════════════════════════════════════════════

// ─── Demo state ─────────────────────────────────────────────────────────────

/// Persistent state for the demo window.
pub struct DemoState {
    pub show: bool,
    file_manager: FileManager,
    table: VirtualTable<DemoRow>,
    table_populated: bool,

    // Main PageControl: mixed page types
    pc_main: PageControl<DemoPage>,
    pc_main_populated: bool,

    // Secondary PageControl: simple info pages (shown side-by-side)
    pc_side: PageControl<InfoPage>,
    pc_side_populated: bool,
}

impl Default for DemoState {
    fn default() -> Self {
        let config = TableConfig {
            sortable: true,
            resizable: true,
            ..Default::default()
        };
        Self {
            show: true,
            file_manager: FileManager::new(),
            table: VirtualTable::new(
                "##demo_table",
                vec![
                    ColumnDef::new("#").fixed(40.0).align(CellAlignment::Center).no_resize(),
                    ColumnDef::new("Name").stretch(1.0),
                    ColumnDef::new("Value").fixed(80.0).align(CellAlignment::Right),
                    ColumnDef::new("Status").fixed(70.0).align(CellAlignment::Center),
                ],
                10_000,
                config,
            ),
            table_populated: false,

            pc_main: PageControl::new("##pc_main"),
            pc_main_populated: false,

            pc_side: PageControl::with_config("##pc_side", PageControlConfig {
                tile_width: 180.0,
                tile_header_height: 32.0,
                tile_body_height: 60.0,
                tile_gap: 8.0,
                confirm_close: false,
                ..Default::default()
            }),
            pc_side_populated: false,
        }
    }
}

impl DemoState {
    fn populate_table(&mut self) {
        if self.table_populated {
            return;
        }
        self.table_populated = true;
        let statuses = ["OK", "Warning", "Error", "OK", "OK"];
        for i in 0..500 {
            self.table.push(DemoRow {
                id: i + 1,
                name: format!("Item_{:04}", i + 1),
                value: (i as f64 * 1.37) % 100.0,
                status: statuses[i % statuses.len()],
            });
        }
    }

    fn populate_page_controls(&mut self) {
        if self.pc_main_populated {
            return;
        }
        self.pc_main_populated = true;

        // Server pages
        self.pc_main.add(DemoPage::Server(
            ServerPage::new("prod-web-01", 45.2, 62.0, 230, true),
        ));
        self.pc_main.add(DemoPage::Server(
            ServerPage::new("prod-db-01", 88.5, 91.3, 42, true),
        ));
        self.pc_main.add(DemoPage::Server(
            ServerPage::new("staging-01", 12.0, 28.0, 5, true),
        ));
        self.pc_main.add(DemoPage::Server(
            ServerPage::new("legacy-api", 0.0, 0.0, 0, false),
        ));

        // Nested page (contains its own PageControl!)
        self.pc_main.add(DemoPage::Nested(Box::new(
            NestedPage::new("Workspaces"),
        )));

        // Info page (non-closable)
        self.pc_main.add(DemoPage::Info(InfoPage {
            name: "About".into(),
            description: "PageControl v2 — generic, trait-based, zero-alloc tabbed container \
                with dashboard and tab views. Supports nesting, badges, status indicators, \
                and full customization via PageControlConfig."
                .into(),
            closable: false,
        }));

        // Side panel pages
        if !self.pc_side_populated {
            self.pc_side_populated = true;
            self.pc_side.add(InfoPage {
                name: "Note 1".into(),
                description: "This is a secondary PageControl rendered side-by-side.".into(),
                closable: true,
            });
            self.pc_side.add(InfoPage {
                name: "Note 2".into(),
                description: "Each instance is fully independent with its own state.".into(),
                closable: true,
            });
            self.pc_side.add(InfoPage {
                name: "Note 3".into(),
                description: "Demonstrates multiple PageControl instances coexisting.".into(),
                closable: true,
            });
        }
    }
}

// ─── Render ─────────────────────────────────────────────────────────────────

/// Render the demo window. Call every frame.
pub fn render_demo_window(ui: &Ui, state: &mut DemoState) {
    if !state.show {
        return;
    }

    let mut show = state.show;
    ui.window("Custom Components Demo")
        .size([900.0, 650.0], dear_imgui_rs::Condition::FirstUseEver)
        .opened(&mut show)
        .build(|| {
            if let Some(tab_bar) = ui.tab_bar("##demo_tabs") {
                // ── FileManager tab ──
                if let Some(_tab) = ui.tab_item("File Manager") {
                    ui.spacing();
                    if ui.button("Open Folder Picker") {
                        state.file_manager.open_folder(None);
                    }
                    ui.same_line();
                    if ui.button("Open File Picker") {
                        state.file_manager.open_file(
                            None,
                            vec![
                                crate::file_manager::FileFilter::new(
                                    "Rust Files (*.rs)",
                                    &["rs"],
                                ),
                                crate::file_manager::FileFilter::new(
                                    "All Files (*.*)",
                                    &[],
                                ),
                            ],
                        );
                    }
                    ui.same_line();
                    if ui.button("Save File Dialog") {
                        state.file_manager.save_file(None, "untitled.rs", vec![]);
                    }

                    ui.spacing();
                    if !state.file_manager.selected_paths().is_empty() {
                        for p in state.file_manager.selected_paths() {
                            ui.text_colored(
                                [0.3, 0.8, 0.4, 1.0],
                                format!("Selected: {}", p.display()),
                            );
                        }
                    } else if let Some(ref path) = state.file_manager.selected_path {
                        ui.text_colored(
                            [0.3, 0.8, 0.4, 1.0],
                            format!("Selected: {}", path.display()),
                        );
                    }
                }

                // ── VirtualTable tab ──
                if let Some(_tab) = ui.tab_item("Virtual Table") {
                    state.populate_table();
                    ui.spacing();
                    ui.text(format!("Rows: {}", state.table.len()));
                    ui.same_line();
                    ui.checkbox("Auto-scroll", &mut state.table.config.auto_scroll);
                    ui.same_line();
                    if ui.button("Add 100 rows") {
                        let base = state.table.len();
                        let statuses = ["OK", "Warning", "Error"];
                        for i in 0..100 {
                            state.table.push(DemoRow {
                                id: base + i + 1,
                                name: format!("New_{:04}", base + i + 1),
                                value: (i as f64 * 2.71) % 100.0,
                                status: statuses[i % statuses.len()],
                            });
                        }
                    }
                    ui.same_line();
                    if ui.button("Clear") {
                        state.table.clear();
                        state.table_populated = false;
                    }
                    ui.spacing();

                    ui.child_window("##table_area")
                        .size([0.0, 0.0])
                        .build(ui, || {
                            state.table.render(ui);
                        });
                }

                // ── PageControl tab (main showcase) ──
                if let Some(_tab) = ui.tab_item("Page Control") {
                    state.populate_page_controls();
                    render_page_control_tab(ui, state);
                }

                drop(tab_bar);
            }
        });

    state.show = show;

    // FileManager renders as a separate window
    state.file_manager.render(ui);
}

/// Render the PageControl demo tab content.
fn render_page_control_tab(ui: &Ui, state: &mut DemoState) {
    ui.spacing();

    // ── Toolbar ──
    if ui.button("Add Server") {
        use std::sync::atomic::{AtomicU32, Ordering};
        static CTR: AtomicU32 = AtomicU32::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed) + 1;
        state.pc_main.add(DemoPage::Server(
            ServerPage::new(format!("new-srv-{:02}", n), 25.0, 40.0, 10, true),
        ));
    }
    ui.same_line();
    if ui.button("Add Nested") {
        use std::sync::atomic::{AtomicU32, Ordering};
        static CTR2: AtomicU32 = AtomicU32::new(0);
        let n = CTR2.fetch_add(1, Ordering::Relaxed) + 1;
        state.pc_main.add(DemoPage::Nested(Box::new(
            NestedPage::new(format!("Workspace-{}", n)),
        )));
    }
    ui.same_line();
    let view_label = match state.pc_main.view {
        ContentView::Dashboard => format!("{} Dashboard", icons::VIEW_DASHBOARD_OUTLINE),
        ContentView::Tabs => format!("{} Tabs", icons::TAB),
        ContentView::Custom(_) => format!("{} Custom", icons::VIEW_DASHBOARD_OUTLINE),
    };
    if ui.button(&view_label) {
        state.pc_main.view = match state.pc_main.view {
            ContentView::Dashboard => ContentView::Tabs,
            _ => ContentView::Dashboard,
        };
    }
    ui.same_line();
    let style_label = match state.pc_main.config.tab_style {
        TabStyle::Pill => "Style: Pill",
        TabStyle::Underline => "Style: Underline",
        TabStyle::Card => "Style: Card",
        TabStyle::Square => "Style: Square",
    };
    if ui.button(style_label) {
        state.pc_main.config.tab_style = match state.pc_main.config.tab_style {
            TabStyle::Pill => TabStyle::Underline,
            TabStyle::Underline => TabStyle::Card,
            TabStyle::Card => TabStyle::Square,
            TabStyle::Square => TabStyle::Pill,
        };
    }
    ui.same_line();
    ui.text_colored(
        [0.54, 0.57, 0.63, 1.0],
        format!("Pages: {}", state.pc_main.page_count()),
    );

    ui.spacing();
    ui.separator();
    ui.spacing();

    // ── Two PageControls side by side ──
    let avail = ui.content_region_avail();
    let main_w = avail[0] * 0.65;
    let side_w = avail[0] - main_w - 8.0;

    // Main PageControl (left)
    ui.child_window("##pc_main_area")
        .size([main_w, 0.0])
        .build(ui, || {
            if let Some(PageAction::TileClicked(id)) = state.pc_main.render(ui) {
                state.pc_main.set_active(id);
                state.pc_main.view = ContentView::Tabs;
            }
        });

    ui.same_line();

    // Side PageControl (right)
    ui.child_window("##pc_side_area")
        .size([side_w, 0.0])
        .build(ui, || {
            ui.text_colored([0.54, 0.57, 0.63, 1.0], "Secondary PageControl");
            ui.spacing();

            let side_view_label = match state.pc_side.view {
                ContentView::Dashboard => "Dashboard",
                ContentView::Tabs => "Tabs",
                ContentView::Custom(_) => "Custom",
            };
            if ui.button(side_view_label) {
                state.pc_side.view = match state.pc_side.view {
                    ContentView::Dashboard => ContentView::Tabs,
                    _ => ContentView::Dashboard,
                };
            }
            ui.same_line();
            if ui.button("Add Note") {
                let n = state.pc_side.page_count() + 1;
                state.pc_side.add(InfoPage {
                    name: format!("Note {}", n),
                    description: format!("Dynamically added note #{}", n),
                    closable: true,
                });
            }
            ui.spacing();

            ui.child_window("##pc_side_inner")
                .size([0.0, 0.0])
                .build(ui, || {
                    if let Some(PageAction::TileClicked(id)) = state.pc_side.render(ui) {
                        state.pc_side.set_active(id);
                        state.pc_side.view = ContentView::Tabs;
                    }
                });
        });
}

