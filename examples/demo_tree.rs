#![allow(clippy::field_reassign_with_default)]
//! Demo: VirtualTree — comprehensive feature showcase.
//!
//! Features demonstrated:
//!   - Nested tree with expand/collapse, tree lines
//!   - Multiple columns: Name, Status (checkbox), Progress, Priority (color), Size, Actions (button)
//!   - Per-node colored icons (GlyphColored)
//!   - Badge (children count on collapsed folders)
//!   - Inline editing (text, checkbox, progress slider)
//!   - Selection (multi, Ctrl+Click, Shift+Click)
//!   - Sorting (sibling-scoped, folders first)
//!   - Filter/search with auto-expand
//!   - Drag-and-drop node reparenting
//!   - Keyboard navigation (Up/Down/Left/Right)
//!   - Context menu (rename, add child, delete)
//!   - Scroll-to-node
//!   - Stress test (10k+ nodes)
//!
//! Run: cargo run --example demo_tree --release

use dear_imgui_custom_mod::virtual_tree::*;
use dear_imgui_rs::{Condition, StyleColor, Ui};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::cmp::Ordering;
use std::fmt::Write as FmtWrite;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

// ─── Node data ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct TaskNode {
    name: String,
    kind: TaskKind,
    done: bool,
    progress: f32, // 0.0..1.0
    priority: Priority,
    size_bytes: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum TaskKind {
    Folder,
    RustFile,
    Config,
    Document,
    Test,
    Asset,
}

#[derive(Clone, Copy, PartialEq)]
enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl Priority {
    fn label(self) -> &'static str {
        match self {
            Priority::Low => "Low",
            Priority::Medium => "Medium",
            Priority::High => "High",
            Priority::Critical => "Critical",
        }
    }
    fn color(self) -> [f32; 4] {
        match self {
            Priority::Low => [0.45, 0.70, 0.45, 1.0],
            Priority::Medium => [0.85, 0.75, 0.30, 1.0],
            Priority::High => [0.90, 0.50, 0.25, 1.0],
            Priority::Critical => [0.95, 0.25, 0.25, 1.0],
        }
    }
    fn from_index(idx: usize) -> Self {
        match idx {
            0 => Priority::Low,
            1 => Priority::Medium,
            2 => Priority::High,
            _ => Priority::Critical,
        }
    }
    fn to_index(self) -> usize {
        match self {
            Priority::Low => 0,
            Priority::Medium => 1,
            Priority::High => 2,
            Priority::Critical => 3,
        }
    }
}

impl TaskKind {
    #[allow(dead_code)]
    fn label(self) -> &'static str {
        match self {
            TaskKind::Folder => "Folder",
            TaskKind::RustFile => ".rs",
            TaskKind::Config => ".toml",
            TaskKind::Document => ".md",
            TaskKind::Test => "test",
            TaskKind::Asset => "asset",
        }
    }

    fn icon_char(self) -> char {
        match self {
            TaskKind::Folder => '\u{F024B}',   // folder
            TaskKind::RustFile => '\u{F0214}', // code
            TaskKind::Config => '\u{F0219}',   // settings
            TaskKind::Document => '\u{F022E}', // text
            TaskKind::Test => '\u{F0293}',     // bug
            TaskKind::Asset => '\u{F021A}',    // image
        }
    }

    fn icon_color(self) -> [f32; 4] {
        match self {
            TaskKind::Folder => [0.90, 0.75, 0.30, 1.0],   // gold
            TaskKind::RustFile => [0.85, 0.45, 0.20, 1.0], // rust orange
            TaskKind::Config => [0.55, 0.75, 0.85, 1.0],   // light blue
            TaskKind::Document => [0.60, 0.80, 0.60, 1.0], // green
            TaskKind::Test => [0.80, 0.55, 0.85, 1.0],     // purple
            TaskKind::Asset => [0.85, 0.65, 0.75, 1.0],    // pink
        }
    }
}

// Column indices
const COL_NAME: usize = 0;
const COL_DONE: usize = 1;
const COL_PROGRESS: usize = 2;
const COL_PRIORITY: usize = 3;
const COL_SIZE: usize = 4;
const COL_ACTION: usize = 5;

impl VirtualTreeNode for TaskNode {
    fn cell_value(&self, col: usize) -> CellValue {
        match col {
            COL_NAME => CellValue::Text(self.name.clone()),
            COL_DONE => CellValue::Bool(self.done),
            COL_PROGRESS => CellValue::Progress(self.progress),
            COL_PRIORITY => CellValue::Choice(self.priority.to_index()),
            COL_SIZE => {
                if self.kind == TaskKind::Folder {
                    CellValue::Text(String::new())
                } else {
                    CellValue::Text(format_size(self.size_bytes))
                }
            }
            COL_ACTION => CellValue::Text(String::new()),
            _ => CellValue::Text(String::new()),
        }
    }

    fn set_cell_value(&mut self, col: usize, value: &CellValue) {
        match col {
            COL_NAME => {
                if let CellValue::Text(s) = value {
                    self.name = s.clone();
                }
            }
            COL_DONE => {
                if let CellValue::Bool(b) = value {
                    self.done = *b;
                }
            }
            COL_PROGRESS => {
                if let CellValue::Float(f) = value {
                    self.progress = *f as f32;
                }
            }
            COL_PRIORITY => {
                if let CellValue::Choice(idx) = value {
                    self.priority = Priority::from_index(*idx);
                }
            }
            _ => {}
        }
    }

    fn has_children(&self) -> bool {
        self.kind == TaskKind::Folder
    }

    fn icon(&self) -> NodeIcon {
        NodeIcon::GlyphColored(self.kind.icon_char(), self.kind.icon_color())
    }

    fn compare(&self, other: &Self, col: usize) -> Ordering {
        // Folders always first
        let folder_ord = other.has_children().cmp(&self.has_children());
        if folder_ord != Ordering::Equal {
            return folder_ord;
        }
        match col {
            COL_NAME => self.name.to_lowercase().cmp(&other.name.to_lowercase()),
            COL_DONE => self.done.cmp(&other.done),
            COL_PROGRESS => self
                .progress
                .partial_cmp(&other.progress)
                .unwrap_or(Ordering::Equal),
            COL_PRIORITY => self.priority.to_index().cmp(&other.priority.to_index()),
            COL_SIZE => self.size_bytes.cmp(&other.size_bytes),
            _ => Ordering::Equal,
        }
    }

    fn matches_filter(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(&query.to_lowercase())
    }

    fn badge(&self) -> &str {
        "" // We'll use a custom approach via the demo
    }

    fn accepts_drop(&self, _dragged: &Self) -> bool {
        self.kind == TaskKind::Folder
    }

    fn is_draggable(&self) -> bool {
        true
    }

    fn row_style(&self) -> Option<RowStyle> {
        if self.done && self.kind != TaskKind::Folder {
            Some(RowStyle {
                text_color: Some([0.50, 0.52, 0.55, 0.7]),
                ..Default::default()
            })
        } else {
            None
        }
    }

    fn cell_style(&self, col: usize) -> Option<CellStyle> {
        if col == COL_PRIORITY {
            Some(CellStyle {
                text_color: Some(self.priority.color()),
                ..Default::default()
            })
        } else {
            None
        }
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ─── Demo state ─────────────────────────────────────────────────────────────

struct DemoState {
    tree: VirtualTree<TaskNode>,
    filter_buf: String,
    show_tree_lines: bool,
    show_striped: bool,
    drag_drop_enabled: bool,
    badge_buf: String,
    next_id: u32,
}

impl DemoState {
    fn new() -> Self {
        let columns = vec![
            ColumnDef::new("Name")
                .stretch(3.0)
                .editor(CellEditor::TextInput),
            ColumnDef::new("Done")
                .fixed(50.0)
                .editor(CellEditor::Checkbox),
            ColumnDef::new("Progress")
                .fixed(120.0)
                .editor(CellEditor::SliderFloat { min: 0.0, max: 1.0 }),
            ColumnDef::new("Priority")
                .fixed(80.0)
                .editor(CellEditor::ComboBox {
                    items: vec![
                        "Low".into(),
                        "Medium".into(),
                        "High".into(),
                        "Critical".into(),
                    ],
                }),
            ColumnDef::new("Size")
                .fixed(80.0)
                .align(CellAlignment::Right),
            ColumnDef::new("Action")
                .fixed(70.0)
                .editor(CellEditor::Button {
                    label: "Open".into(),
                }),
        ];

        let config = TreeConfig {
            table: {
                let mut tc = dear_imgui_custom_mod::virtual_table::config::TableConfig::default();
                tc.selection_mode = SelectionMode::Multi;
                tc.edit_trigger = EditTrigger::DoubleClick;
                tc.sortable = true;
                tc
            },
            indent_width: 18.0,
            show_tree_lines: true,
            tree_line_color: [0.35, 0.40, 0.50, 0.45],
            drag_drop_enabled: true,
            expand_on_double_click: true,
            ..Default::default()
        };

        let mut tree = VirtualTree::new("##demo_tree", columns, config);

        // Build sample project tree
        let src = tree
            .insert_root(TaskNode {
                name: "src".into(),
                kind: TaskKind::Folder,
                done: false,
                progress: 0.0,
                priority: Priority::High,
                size_bytes: 0,
            })
            .unwrap();
        let examples = tree
            .insert_root(TaskNode {
                name: "examples".into(),
                kind: TaskKind::Folder,
                done: false,
                progress: 0.0,
                priority: Priority::Medium,
                size_bytes: 0,
            })
            .unwrap();
        let docs = tree
            .insert_root(TaskNode {
                name: "docs".into(),
                kind: TaskKind::Folder,
                done: true,
                progress: 1.0,
                priority: Priority::Low,
                size_bytes: 0,
            })
            .unwrap();
        let tests = tree
            .insert_root(TaskNode {
                name: "tests".into(),
                kind: TaskKind::Folder,
                done: false,
                progress: 0.4,
                priority: Priority::High,
                size_bytes: 0,
            })
            .unwrap();
        let assets = tree
            .insert_root(TaskNode {
                name: "assets".into(),
                kind: TaskKind::Folder,
                done: false,
                progress: 0.75,
                priority: Priority::Low,
                size_bytes: 0,
            })
            .unwrap();
        tree.insert_root(TaskNode {
            name: "Cargo.toml".into(),
            kind: TaskKind::Config,
            done: true,
            progress: 1.0,
            priority: Priority::Medium,
            size_bytes: 892,
        });
        tree.insert_root(TaskNode {
            name: "README.md".into(),
            kind: TaskKind::Document,
            done: false,
            progress: 0.6,
            priority: Priority::Medium,
            size_bytes: 4521,
        });

        // src/ subfolders
        let vt = Self::add_folder(&mut tree, src, "virtual_table", Priority::High);
        let vtree = Self::add_folder(&mut tree, src, "virtual_tree", Priority::Critical);
        let ng = Self::add_folder(&mut tree, src, "node_graph", Priority::Medium);
        let fm = Self::add_folder(&mut tree, src, "file_manager", Priority::Medium);

        tree.insert_child(
            src,
            TaskNode {
                name: "lib.rs".into(),
                kind: TaskKind::RustFile,
                done: true,
                progress: 1.0,
                priority: Priority::Low,
                size_bytes: 1024,
            },
        );
        tree.insert_child(
            src,
            TaskNode {
                name: "icons.rs".into(),
                kind: TaskKind::RustFile,
                done: true,
                progress: 1.0,
                priority: Priority::Low,
                size_bytes: 8192,
            },
        );

        // src/virtual_table/
        Self::add_file(&mut tree, vt, "mod.rs", 28540, Priority::High, 1.0);
        Self::add_file(&mut tree, vt, "column.rs", 4890, Priority::Medium, 1.0);
        Self::add_file(&mut tree, vt, "row.rs", 3560, Priority::Medium, 1.0);
        Self::add_file(&mut tree, vt, "config.rs", 5120, Priority::Low, 1.0);
        Self::add_file(&mut tree, vt, "edit.rs", 2340, Priority::Low, 0.95);
        Self::add_file(&mut tree, vt, "ring_buffer.rs", 6780, Priority::Low, 1.0);

        // src/virtual_tree/
        Self::add_file(&mut tree, vtree, "mod.rs", 22100, Priority::Critical, 0.85);
        Self::add_file(&mut tree, vtree, "arena.rs", 8900, Priority::High, 0.9);
        Self::add_file(&mut tree, vtree, "node.rs", 3200, Priority::Medium, 0.95);
        Self::add_file(
            &mut tree,
            vtree,
            "flat_view.rs",
            2100,
            Priority::Medium,
            0.9,
        );
        Self::add_file(&mut tree, vtree, "config.rs", 1500, Priority::Low, 1.0);
        Self::add_file(&mut tree, vtree, "drag.rs", 1200, Priority::Medium, 0.5);
        Self::add_file(&mut tree, vtree, "filter.rs", 1800, Priority::Medium, 0.8);
        Self::add_file(&mut tree, vtree, "sort.rs", 1600, Priority::Low, 1.0);

        // src/node_graph/
        Self::add_file(&mut tree, ng, "mod.rs", 12300, Priority::High, 1.0);
        Self::add_file(&mut tree, ng, "render.rs", 38900, Priority::High, 0.95);
        Self::add_file(&mut tree, ng, "graph.rs", 5600, Priority::Medium, 1.0);
        Self::add_file(&mut tree, ng, "types.rs", 4200, Priority::Low, 1.0);

        // src/file_manager/
        Self::add_file(&mut tree, fm, "mod.rs", 15600, Priority::Medium, 1.0);
        Self::add_file(&mut tree, fm, "render.rs", 22300, Priority::Medium, 0.9);
        Self::add_file(&mut tree, fm, "config.rs", 3800, Priority::Low, 1.0);

        // examples/
        Self::add_file(
            &mut tree,
            examples,
            "demo_tree.rs",
            9500,
            Priority::High,
            0.7,
        );
        Self::add_file(
            &mut tree,
            examples,
            "demo_table.rs",
            12400,
            Priority::Medium,
            1.0,
        );
        Self::add_file(
            &mut tree,
            examples,
            "demo_node_graph.rs",
            18700,
            Priority::Medium,
            1.0,
        );

        // docs/
        Self::add_file_kind(
            &mut tree,
            docs,
            "node_graph.md",
            7800,
            TaskKind::Document,
            Priority::Low,
            1.0,
        );
        Self::add_file_kind(
            &mut tree,
            docs,
            "virtual_table.md",
            5400,
            TaskKind::Document,
            Priority::Low,
            1.0,
        );

        // tests/
        Self::add_file_kind(
            &mut tree,
            tests,
            "arena_tests.rs",
            3200,
            TaskKind::Test,
            Priority::High,
            0.85,
        );
        Self::add_file_kind(
            &mut tree,
            tests,
            "flat_view_tests.rs",
            1800,
            TaskKind::Test,
            Priority::Medium,
            0.6,
        );
        Self::add_file_kind(
            &mut tree,
            tests,
            "integration.rs",
            5400,
            TaskKind::Test,
            Priority::Critical,
            0.3,
        );

        // assets/
        Self::add_file_kind(
            &mut tree,
            assets,
            "logo.png",
            24500,
            TaskKind::Asset,
            Priority::Low,
            1.0,
        );
        Self::add_file_kind(
            &mut tree,
            assets,
            "icon_atlas.png",
            128000,
            TaskKind::Asset,
            Priority::Low,
            0.9,
        );
        Self::add_file_kind(
            &mut tree,
            assets,
            "font.ttf",
            342000,
            TaskKind::Asset,
            Priority::Low,
            1.0,
        );

        // Expand src and virtual_tree by default
        tree.expand(src);
        tree.expand(vtree);

        Self {
            tree,
            filter_buf: String::new(),
            show_tree_lines: true,
            show_striped: true,
            drag_drop_enabled: true,
            badge_buf: String::new(),
            next_id: 0,
        }
    }

    fn add_folder(
        tree: &mut VirtualTree<TaskNode>,
        parent: NodeId,
        name: &str,
        prio: Priority,
    ) -> NodeId {
        tree.insert_child(
            parent,
            TaskNode {
                name: name.into(),
                kind: TaskKind::Folder,
                done: false,
                progress: 0.0,
                priority: prio,
                size_bytes: 0,
            },
        )
        .unwrap()
    }

    fn add_file(
        tree: &mut VirtualTree<TaskNode>,
        parent: NodeId,
        name: &str,
        size: u64,
        prio: Priority,
        progress: f32,
    ) {
        tree.insert_child(
            parent,
            TaskNode {
                name: name.into(),
                kind: TaskKind::RustFile,
                done: progress >= 1.0,
                progress,
                priority: prio,
                size_bytes: size,
            },
        );
    }

    fn add_file_kind(
        tree: &mut VirtualTree<TaskNode>,
        parent: NodeId,
        name: &str,
        size: u64,
        kind: TaskKind,
        prio: Priority,
        progress: f32,
    ) {
        tree.insert_child(
            parent,
            TaskNode {
                name: name.into(),
                kind,
                done: progress >= 1.0,
                progress,
                priority: prio,
                size_bytes: size,
            },
        );
    }

    fn stress_test(&mut self) {
        // Add 10,000 nodes in a deep hierarchy
        let root = match self.tree.insert_root(TaskNode {
            name: format!("stress_test_{}", self.next_id),
            kind: TaskKind::Folder,
            done: false,
            progress: 0.0,
            priority: Priority::Low,
            size_bytes: 0,
        }) {
            Some(id) => id,
            None => return, // at capacity
        };
        self.next_id += 1;

        let priorities = [
            Priority::Low,
            Priority::Medium,
            Priority::High,
            Priority::Critical,
        ];
        let kinds = [
            TaskKind::RustFile,
            TaskKind::Config,
            TaskKind::Document,
            TaskKind::Test,
            TaskKind::Asset,
        ];

        for i in 0..100 {
            let folder = match self.tree.insert_child(
                root,
                TaskNode {
                    name: format!("folder_{i:03}"),
                    kind: TaskKind::Folder,
                    done: false,
                    progress: (i as f32) / 100.0,
                    priority: priorities[i % 4],
                    size_bytes: 0,
                },
            ) {
                Some(id) => id,
                None => return, // at capacity
            };

            for j in 0..100 {
                let idx = i * 100 + j;
                if self
                    .tree
                    .insert_child(
                        folder,
                        TaskNode {
                            name: format!("item_{idx:05}"),
                            kind: kinds[j % 5],
                            done: j % 3 == 0,
                            progress: (j as f32) / 100.0,
                            priority: priorities[j % 4],
                            size_bytes: (idx as u64 + 1) * 128,
                        },
                    )
                    .is_none()
                {
                    return; // at capacity
                }
            }
        }
    }

    fn render(&mut self, ui: &Ui) {
        let vp = ui.main_viewport();
        let size = vp.size();
        let pos = vp.pos();

        ui.window("VirtualTree Demo")
            .position(pos, Condition::Always)
            .size(size, Condition::Always)
            .flags(
                dear_imgui_rs::WindowFlags::NO_TITLE_BAR
                    | dear_imgui_rs::WindowFlags::NO_RESIZE
                    | dear_imgui_rs::WindowFlags::NO_MOVE
                    | dear_imgui_rs::WindowFlags::NO_COLLAPSE,
            )
            .build(|| {
                self.render_toolbar(ui);
                ui.separator();
                self.render_tree(ui);
                self.handle_context_menu(ui);
                self.handle_button_clicks(ui);
            });
    }

    fn render_toolbar(&mut self, ui: &Ui) {
        ui.text("VirtualTree Demo");
        ui.same_line_with_spacing(0.0, 20.0);
        ui.text_colored(
            [0.55, 0.60, 0.68, 1.0],
            "Drag & drop nodes | Right-click for context menu | Double-click to edit",
        );
        ui.spacing();

        // Row 1: Filter + buttons
        ui.set_next_item_width(220.0);
        if ui
            .input_text("##filter", &mut self.filter_buf)
            .hint("Filter by name...")
            .build()
        {
            self.tree.set_filter(&self.filter_buf);
        }
        ui.same_line();
        if ui.button("Clear") {
            self.filter_buf.clear();
            self.tree.clear_filter();
        }
        ui.same_line_with_spacing(0.0, 16.0);
        if ui.button("Expand All") {
            self.tree.expand_all();
        }
        ui.same_line();
        if ui.button("Collapse All") {
            self.tree.collapse_all();
        }
        ui.same_line_with_spacing(0.0, 16.0);

        if ui.button("+ Stress 10K") {
            self.stress_test();
        }
        ui.same_line();
        if ui.button("+ Add Root") {
            self.next_id += 1;
            let _ = self.tree.insert_root(TaskNode {
                name: format!("new_folder_{}", self.next_id),
                kind: TaskKind::Folder,
                done: false,
                progress: 0.0,
                priority: Priority::Medium,
                size_bytes: 0,
            });
        }

        // Row 2: Options + stats
        if ui.checkbox("Tree Lines", &mut self.show_tree_lines) {
            self.tree.config.show_tree_lines = self.show_tree_lines;
        }
        ui.same_line();
        if ui.checkbox("Striped", &mut self.show_striped) {
            self.tree.config.striped = self.show_striped;
        }
        ui.same_line();
        if ui.checkbox("Drag & Drop", &mut self.drag_drop_enabled) {
            self.tree.config.drag_drop_enabled = self.drag_drop_enabled;
        }

        ui.same_line_with_spacing(0.0, 20.0);
        let status = format!(
            "{} nodes | {} visible | {} selected",
            self.tree.node_count(),
            self.tree.flat_row_count(),
            self.tree.selected_count(),
        );
        ui.text_colored([0.55, 0.60, 0.68, 1.0], &status);
    }

    fn render_tree(&mut self, ui: &Ui) {
        self.tree.render(ui);
    }

    fn handle_context_menu(&mut self, ui: &Ui) {
        if self.tree.open_context_menu {
            ui.open_popup("##tree_ctx");
            self.tree.open_context_menu = false;
        }

        if let Some(_popup) = ui.begin_popup("##tree_ctx")
            && let Some(id) = self.tree.context_node
        {
            if let Some(node) = self.tree.get(id) {
                ui.text_disabled(&node.name);
                ui.separator();
            }

            // Add child (only for folders)
            if self
                .tree
                .get(id)
                .is_some_and(|n| n.kind == TaskKind::Folder)
            {
                if ui.selectable("Add Child File") {
                    self.next_id += 1;
                    self.tree.insert_child(
                        id,
                        TaskNode {
                            name: format!("new_file_{}.rs", self.next_id),
                            kind: TaskKind::RustFile,
                            done: false,
                            progress: 0.0,
                            priority: Priority::Medium,
                            size_bytes: 1024,
                        },
                    );
                    self.tree.expand(id);
                }
                if ui.selectable("Add Subfolder") {
                    self.next_id += 1;
                    self.tree.insert_child(
                        id,
                        TaskNode {
                            name: format!("new_folder_{}", self.next_id),
                            kind: TaskKind::Folder,
                            done: false,
                            progress: 0.0,
                            priority: Priority::Medium,
                            size_bytes: 0,
                        },
                    );
                    self.tree.expand(id);
                }
                ui.separator();
            }

            // Toggle done
            if ui.selectable("Toggle Done")
                && let Some(data) = self.tree.get_mut(id)
            {
                data.done = !data.done;
                if data.done {
                    data.progress = 1.0;
                }
            }

            // Set priority submenu
            if let Some(_sub) = ui.begin_menu("Set Priority") {
                for p in [
                    Priority::Low,
                    Priority::Medium,
                    Priority::High,
                    Priority::Critical,
                ] {
                    if ui.selectable(p.label()) {
                        let selected: Vec<NodeId> = self.tree.selected_nodes().collect();
                        if selected.contains(&id) {
                            for &sel_id in &selected {
                                if let Some(data) = self.tree.get_mut(sel_id) {
                                    data.priority = p;
                                }
                            }
                        } else if let Some(data) = self.tree.get_mut(id) {
                            data.priority = p;
                        }
                    }
                }
            }

            ui.separator();
            if ui.selectable("Delete") {
                let selected: Vec<NodeId> = self.tree.selected_nodes().collect();
                if selected.contains(&id) {
                    for sel_id in selected {
                        self.tree.remove(sel_id);
                    }
                } else {
                    self.tree.remove(id);
                }
            }
        }
    }

    fn handle_button_clicks(&mut self, _ui: &Ui) {
        if let Some((node_id, _col)) = self.tree.button_clicked
            && let Some(data) = self.tree.get(node_id)
        {
            self.badge_buf.clear();
            let _ = write!(self.badge_buf, "Opened: {}", data.name);
            // In a real app this would open the file; here just log to tooltip
        }
    }
}

// ─── GPU boilerplate ────────────────────────────────────────────────────────

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_cfg: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    context: dear_imgui_rs::Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    demo: DemoState,
}

struct App {
    gpu: Option<GpuState>,
}

impl App {
    fn new() -> Self {
        Self { gpu: None }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_inner_size(LogicalSize::new(1100.0, 700.0))
                        .with_title("VirtualTree Demo"),
                )
                .expect("window"),
        );

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("adapter");
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");

        let phys = window.inner_size();
        let surface_cfg = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: phys.width.max(1),
            height: phys.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };
        surface.configure(&device, &surface_cfg);

        let mut context = dear_imgui_rs::Context::create();
        let _ = context.set_ini_filename(None::<std::path::PathBuf>);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, HiDpiMode::Default, &mut context);

        let hidpi = window.scale_factor() as f32;
        let font_size = 15.0 * hidpi;
        context.io_mut().set_font_global_scale(1.0 / hidpi);

        let segoe_path = "C:\\Windows\\Fonts\\segoeui.ttf";
        if std::path::Path::new(segoe_path).exists() {
            let font_data = std::fs::read(segoe_path).expect("read font");
            let font_data: &'static [u8] = Box::leak(font_data.into_boxed_slice());
            context
                .fonts()
                .add_font(&[dear_imgui_rs::FontSource::TtfData {
                    data: font_data,
                    size_pixels: Some(font_size),
                    config: Some(
                        dear_imgui_rs::FontConfig::new()
                            .size_pixels(font_size)
                            .oversample_h(2),
                    ),
                }]);
        } else {
            context
                .fonts()
                .add_font(&[dear_imgui_rs::FontSource::DefaultFontData {
                    config: Some(
                        dear_imgui_rs::FontConfig::new()
                            .size_pixels(font_size)
                            .oversample_h(2),
                    ),
                    size_pixels: Some(font_size),
                }]);
        }

        apply_dark_theme(context.style_mut());

        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_cfg.format),
            &mut context,
        )
        .expect("renderer");

        self.gpu = Some(GpuState {
            device,
            queue,
            window,
            surface_cfg,
            surface,
            context,
            platform,
            renderer,
            demo: DemoState::new(),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };

        gpu.platform.handle_event::<()>(
            &mut gpu.context,
            &gpu.window,
            &Event::WindowEvent {
                window_id,
                event: event.clone(),
            },
        );

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                gpu.surface_cfg.width = new_size.width.max(1);
                gpu.surface_cfg.height = new_size.height.max(1);
                gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                        return;
                    }
                    other => {
                        eprintln!("Surface unavailable: {other:?}");
                        return;
                    }
                };

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);
                let ui = gpu.context.frame();
                gpu.demo.render(ui);
                gpu.platform.prepare_render_with_ui(ui, &gpu.window);
                let draw_data = gpu.context.render();

                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("imgui"),
                        });

                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("imgui_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.08,
                                    g: 0.09,
                                    b: 0.11,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });

                    if draw_data.total_vtx_count > 0 {
                        gpu.renderer
                            .render_draw_data(draw_data, &mut pass)
                            .expect("render");
                    }
                }

                gpu.queue.submit(Some(encoder.finish()));
                frame.present();
                gpu.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(gpu) = self.gpu.as_ref() {
            gpu.window.request_redraw();
        }
    }
}

fn apply_dark_theme(style: &mut dear_imgui_rs::Style) {
    style.set_window_rounding(6.0);
    style.set_frame_rounding(4.0);
    style.set_grab_rounding(4.0);
    style.set_tab_rounding(4.0);
    style.set_scrollbar_rounding(6.0);
    style.set_window_border_size(1.0);
    style.set_frame_border_size(0.0);
    style.set_popup_rounding(4.0);
    style.set_cell_padding([6.0, 2.0]);
    style.set_frame_padding([3.0, 2.0]);
    style.set_item_spacing([8.0, 4.0]);
    style.set_item_inner_spacing([6.0, 3.0]);

    let accent = [0.40, 0.63, 0.88, 1.0];
    let accent_dim = [0.30, 0.50, 0.75, 1.0];
    let accent_hi = [0.50, 0.73, 0.95, 1.0];

    style.set_color(StyleColor::WindowBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::ChildBg, [0.10, 0.10, 0.13, 1.0]);
    style.set_color(StyleColor::PopupBg, [0.11, 0.12, 0.15, 0.96]);
    style.set_color(StyleColor::Border, [0.20, 0.22, 0.27, 0.70]);
    style.set_color(StyleColor::FrameBg, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::FrameBgHovered, [0.19, 0.20, 0.26, 1.0]);
    style.set_color(StyleColor::FrameBgActive, [0.24, 0.26, 0.33, 1.0]);
    style.set_color(StyleColor::TitleBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::TitleBgActive, [0.12, 0.13, 0.17, 1.0]);
    style.set_color(StyleColor::ScrollbarBg, [0.08, 0.08, 0.10, 0.60]);
    style.set_color(StyleColor::ScrollbarGrab, [0.22, 0.24, 0.30, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabHovered, [0.30, 0.33, 0.40, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabActive, accent_dim);
    style.set_color(StyleColor::CheckMark, accent);
    style.set_color(StyleColor::SliderGrab, accent_dim);
    style.set_color(StyleColor::SliderGrabActive, accent);
    style.set_color(StyleColor::Button, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::ButtonHovered, [0.26, 0.29, 0.36, 1.0]);
    style.set_color(StyleColor::ButtonActive, accent_dim);
    style.set_color(StyleColor::Header, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::HeaderHovered, [0.24, 0.27, 0.34, 1.0]);
    style.set_color(StyleColor::HeaderActive, accent_dim);
    style.set_color(StyleColor::Separator, [0.20, 0.22, 0.27, 0.60]);
    style.set_color(StyleColor::Tab, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::TabHovered, accent_dim);
    style.set_color(StyleColor::TabSelected, [0.22, 0.24, 0.30, 1.0]);
    style.set_color(StyleColor::Text, [0.92, 0.93, 0.95, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.42, 0.45, 0.52, 1.0]);
    style.set_color(
        StyleColor::TextSelectedBg,
        [accent[0], accent[1], accent[2], 0.30],
    );
    style.set_color(StyleColor::PlotHistogram, accent_hi);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run");
}

// ─── Tests ──────────────────────────────────────────────────────────────────
//
// Run:  cargo test --example demo_tree
// Stress: cargo test --example demo_tree -- --ignored --nocapture

#[cfg(test)]
mod tests {
    use dear_imgui_custom_mod::virtual_table::row::CellValue;
    use dear_imgui_custom_mod::virtual_tree::arena::{MAX_TREE_NODES, TreeArena};
    use dear_imgui_custom_mod::virtual_tree::filter::FilterState;
    use dear_imgui_custom_mod::virtual_tree::flat_view::{FlatRow, FlatView};
    use dear_imgui_custom_mod::virtual_tree::node::{NodeIcon, VirtualTreeNode};

    // ── TreeArena unit tests ─────────────────────────────────────────

    #[test]
    fn insert_root_and_children() {
        let mut arena = TreeArena::new();
        let r1 = arena.insert_root("root1").unwrap();
        let r2 = arena.insert_root("root2").unwrap();
        assert_eq!(arena.node_count(), 2);
        assert_eq!(arena.roots().len(), 2);
        assert_eq!(arena.depth(r1), Some(0));

        let c1 = arena.insert_child(r1, "child1").unwrap();
        let c2 = arena.insert_child(r1, "child2").unwrap();
        assert_eq!(arena.node_count(), 4);
        assert_eq!(arena.children(r1).len(), 2);
        assert_eq!(arena.parent(c1), Some(r1));
        assert_eq!(arena.depth(c1), Some(1));

        let gc = arena.insert_child(c1, "grandchild").unwrap();
        assert_eq!(arena.depth(gc), Some(2));
        assert_eq!(arena.parent(gc), Some(c1));
        let _ = (r2, c2); // used
    }

    #[test]
    fn remove_subtree() {
        let mut arena = TreeArena::new();
        let r = arena.insert_root("root").unwrap();
        let c1 = arena.insert_child(r, "c1").unwrap();
        let _c2 = arena.insert_child(r, "c2").unwrap();
        let _gc = arena.insert_child(c1, "gc").unwrap();
        assert_eq!(arena.node_count(), 4);

        arena.remove(c1);
        assert_eq!(arena.node_count(), 2); // root + c2
        assert_eq!(arena.children(r).len(), 1);
    }

    #[test]
    fn generational_safety() {
        let mut arena = TreeArena::new();
        let id = arena.insert_root("hello").unwrap();
        arena.remove(id);
        assert!(arena.get_data(id).is_none()); // stale id

        let new_id = arena.insert_root("world").unwrap();
        // new_id reuses the freed slot but has a different generation
        assert_ne!(new_id, id);
        assert_eq!(arena.get_data(new_id), Some(&"world"));
        assert!(arena.get_data(id).is_none()); // old id still invalid
    }

    #[test]
    fn move_node_reparent() {
        let mut arena = TreeArena::new();
        let r1 = arena.insert_root("r1").unwrap();
        let r2 = arena.insert_root("r2").unwrap();
        let c = arena.insert_child(r1, "child").unwrap();

        assert!(arena.move_node(c, Some(r2), 0));
        assert_eq!(arena.children(r1).len(), 0);
        assert_eq!(arena.children(r2).len(), 1);
        assert_eq!(arena.parent(c), Some(r2));
        assert_eq!(arena.depth(c), Some(1));
    }

    #[test]
    fn move_to_root() {
        let mut arena = TreeArena::new();
        let r = arena.insert_root("root").unwrap();
        let c = arena.insert_child(r, "child").unwrap();

        assert!(arena.move_node(c, None, 0));
        assert_eq!(arena.roots().len(), 2);
        assert_eq!(arena.parent(c), None);
        assert_eq!(arena.depth(c), Some(0));
    }

    #[test]
    fn prevent_cycle() {
        let mut arena = TreeArena::new();
        let r = arena.insert_root("root").unwrap();
        let c = arena.insert_child(r, "child").unwrap();
        let gc = arena.insert_child(c, "grandchild").unwrap();

        // Cannot move root into its own grandchild
        assert!(!arena.move_node(r, Some(gc), 0));
        // Cannot move node into itself
        assert!(!arena.move_node(c, Some(c), 0));
    }

    #[test]
    fn expand_collapse() {
        let mut arena = TreeArena::new();
        let r = arena.insert_root("root").unwrap();
        assert!(!arena.is_expanded(r));

        arena.expand(r);
        assert!(arena.is_expanded(r));

        arena.toggle(r);
        assert!(!arena.is_expanded(r));
    }

    #[test]
    fn ensure_visible_expands_ancestors() {
        let mut arena = TreeArena::new();
        let r = arena.insert_root("root").unwrap();
        let c = arena.insert_child(r, "child").unwrap();
        let gc = arena.insert_child(c, "gc").unwrap();

        arena.ensure_visible(gc);
        assert!(arena.is_expanded(r));
        assert!(arena.is_expanded(c));
        assert!(!arena.is_expanded(gc)); // the node itself is not expanded
    }

    #[test]
    fn capacity_limit() {
        let mut arena = TreeArena::new();
        for i in 0..100 {
            assert!(arena.insert_root(i).is_some());
        }
        assert_eq!(arena.node_count(), 100);
    }

    #[test]
    fn custom_capacity_limit() {
        let mut arena = TreeArena::<i32>::with_capacity(5);
        assert_eq!(arena.capacity(), 5);

        for i in 0..5 {
            assert!(arena.insert_root(i).is_some());
        }
        // At capacity — should return None
        assert!(arena.insert_root(99).is_none());
        assert_eq!(arena.node_count(), 5);
    }

    #[test]
    fn set_capacity_at_runtime() {
        let mut arena = TreeArena::<i32>::new();
        assert_eq!(arena.capacity(), MAX_TREE_NODES);

        arena.set_capacity(3);
        assert_eq!(arena.capacity(), 3);

        for i in 0..3 {
            assert!(arena.insert_root(i).is_some());
        }
        assert!(arena.insert_root(99).is_none());
    }

    #[test]
    fn evict_oldest_root_on_overflow() {
        let mut arena = TreeArena::with_capacity(4);
        arena.set_evict_on_overflow(true);

        // Insert root "A" with 2 children → 3 nodes
        let r1 = arena.insert_root("A").unwrap();
        arena.insert_child(r1, "A1").unwrap();
        arena.insert_child(r1, "A2").unwrap();
        assert_eq!(arena.node_count(), 3);

        // Insert root "B" → 4 nodes (at capacity)
        let r2 = arena.insert_root("B").unwrap();
        assert_eq!(arena.node_count(), 4);

        // Insert root "C" → evicts "A" subtree (3 nodes), then inserts "C"
        let r3 = arena.insert_root("C").unwrap();
        assert_eq!(arena.node_count(), 2); // "B" + "C"
        assert!(arena.get_data(r1).is_none()); // "A" evicted
        assert_eq!(arena.get_data(r2), Some(&"B"));
        assert_eq!(arena.get_data(r3), Some(&"C"));
        assert_eq!(arena.roots().len(), 2);
    }

    #[test]
    fn evict_disabled_returns_none() {
        let mut arena = TreeArena::with_capacity(2);
        // evict_on_overflow defaults to false
        assert!(!arena.evict_on_overflow());

        arena.insert_root("A").unwrap();
        arena.insert_root("B").unwrap();
        assert!(arena.insert_root("C").is_none());
        assert_eq!(arena.node_count(), 2);
    }

    #[test]
    fn capacity_clamp_bounds() {
        // Capacity 0 clamped to 1
        let arena = TreeArena::<i32>::with_capacity(0);
        assert_eq!(arena.capacity(), 1);

        // Capacity above MAX clamped
        let arena = TreeArena::<i32>::with_capacity(MAX_TREE_NODES + 1000);
        assert_eq!(arena.capacity(), MAX_TREE_NODES);
    }

    // ── Stress test helpers ──────────────────────────────────────────

    struct BenchNode {
        name: String,
        is_folder: bool,
    }

    impl BenchNode {
        fn folder(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                is_folder: true,
            }
        }
        fn file(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                is_folder: false,
            }
        }
    }

    impl VirtualTreeNode for BenchNode {
        fn cell_value(&self, _col: usize) -> CellValue {
            CellValue::Text(self.name.clone())
        }
        fn set_cell_value(&mut self, _col: usize, _value: &CellValue) {}
        fn has_children(&self) -> bool {
            self.is_folder
        }
        fn icon(&self) -> NodeIcon {
            NodeIcon::None
        }
        fn matches_filter(&self, query: &str) -> bool {
            self.name.contains(query)
        }
    }

    fn elapsed_ms(start: std::time::Instant) -> f64 {
        start.elapsed().as_secs_f64() * 1000.0
    }

    /// Build a balanced tree: root folders, each with children.
    /// Repeats depth levels until `target` nodes are filled.
    fn build_balanced_tree(arena: &mut TreeArena<BenchNode>, target: usize) {
        let roots = 100;
        let children_per = 50;

        let mut count = 0;
        let mut parents = Vec::new();

        for i in 0..roots {
            if count >= target {
                break;
            }
            if let Some(id) = arena.insert_root(BenchNode::folder(format!("root_{i}"))) {
                parents.push(id);
                count += 1;
            }
        }

        while count < target {
            let current_parents = std::mem::take(&mut parents);
            if current_parents.is_empty() {
                break;
            }

            for parent in &current_parents {
                for j in 0..children_per {
                    if count >= target {
                        break;
                    }
                    let is_folder = j % 5 == 0;
                    let node = if is_folder {
                        BenchNode::folder(format!("dir_{count}"))
                    } else {
                        BenchNode::file(format!("file_{count}"))
                    };
                    if let Some(id) = arena.insert_child(*parent, node) {
                        if is_folder {
                            parents.push(id);
                        }
                        count += 1;
                    } else {
                        break;
                    }
                }
                if count >= target {
                    break;
                }
            }
        }
    }

    /// Build a deeply nested chain: root → child → grandchild → ...
    fn build_deep_chain(arena: &mut TreeArena<BenchNode>, depth: usize) {
        let mut parent = arena.insert_root(BenchNode::folder("deep_root")).unwrap();
        for i in 1..depth {
            let node = if i < depth - 1 {
                BenchNode::folder(format!("level_{i}"))
            } else {
                BenchNode::file(format!("leaf_{i}"))
            };
            if let Some(id) = arena.insert_child(parent, node) {
                parent = id;
            } else {
                break;
            }
        }
    }

    // ── 500K stress test ─────────────────────────────────────────────

    #[test]
    #[ignore]
    fn stress_test_500k() {
        run_stress_test(500_000, "500K");
    }

    // ── 1M stress test ───────────────────────────────────────────────

    #[test]
    #[ignore]
    fn stress_test_1m() {
        run_stress_test(1_000_000, "1M");
    }

    fn run_stress_test(target: usize, label: &str) {
        use dear_imgui_custom_mod::virtual_tree::arena::NodeSlot;
        use std::time::Instant;

        let sep = "=".repeat(60);
        println!("\n{sep}");
        println!("  STRESS TEST: {label} nodes");
        println!("{sep}\n");

        // ── 1. Bulk insert ───────────────────────────────────────────
        let mut arena = TreeArena::<BenchNode>::with_capacity(target);

        let t = Instant::now();
        build_balanced_tree(&mut arena, target);
        let insert_ms = elapsed_ms(t);
        let actual_count = arena.node_count();
        println!("[INSERT]  {actual_count} nodes in {insert_ms:.1} ms");

        // ── 2. Expand all ────────────────────────────────────────────
        let t = Instant::now();
        arena.expand_all();
        let expand_ms = elapsed_ms(t);
        println!("[EXPAND_ALL]  {expand_ms:.1} ms");

        // ── 3. Flat view rebuild (all expanded) ──────────────────────
        let filter = FilterState::new();
        let mut flat_view = FlatView::new();

        let t = Instant::now();
        flat_view.rebuild(&arena, &filter);
        let rebuild_ms = elapsed_ms(t);
        let flat_count = flat_view.len();
        println!("[FLAT_VIEW REBUILD]  {flat_count} rows in {rebuild_ms:.1} ms");

        // ── 4. Flat view index_of (random lookups) ───────────────────
        let sample_size = 10_000.min(flat_count);
        let step = flat_count / sample_size.max(1);
        let t = Instant::now();
        for i in (0..flat_count).step_by(step.max(1)) {
            let id = flat_view.rows[i].node_id;
            let _ = flat_view.index_of(id);
        }
        let lookup_ms = elapsed_ms(t);
        println!(
            "[INDEX_OF]  {sample_size} lookups in {lookup_ms:.3} ms ({:.0} ns/op)",
            lookup_ms * 1_000_000.0 / sample_size as f64
        );

        // ── 5. Collapse all + rebuild ────────────────────────────────
        arena.collapse_all();
        let t = Instant::now();
        flat_view.rebuild(&arena, &filter);
        let collapsed_ms = elapsed_ms(t);
        let collapsed_rows = flat_view.len();
        println!("[FLAT_VIEW COLLAPSED]  {collapsed_rows} rows in {collapsed_ms:.1} ms");

        // ── 6. Filter (10% match) ────────────────────────────────────
        arena.expand_all();
        let mut filter_state = FilterState::new();
        let t = Instant::now();
        filter_state.set_filter("file_1", &mut arena, true);
        let filter_ms = elapsed_ms(t);
        let visible = filter_state.visible_count();
        println!("[FILTER '\"file_1\"']  {visible} visible nodes in {filter_ms:.1} ms");

        // ── 7. Filter rebuild flat view ──────────────────────────────
        let t = Instant::now();
        flat_view.rebuild(&arena, &filter_state);
        let filter_rebuild_ms = elapsed_ms(t);
        let filter_rows = flat_view.len();
        println!("[FLAT_VIEW FILTERED]  {filter_rows} rows in {filter_rebuild_ms:.1} ms");

        // ── 8. Clear filter ──────────────────────────────────────────
        filter_state.clear();
        let t = Instant::now();
        flat_view.rebuild(&arena, &filter_state);
        let clear_ms = elapsed_ms(t);
        println!("[CLEAR FILTER + REBUILD]  {clear_ms:.1} ms");

        // ── 9. Remove 10% of roots ──────────────────────────────────
        let roots_to_remove: Vec<_> = arena.roots().iter().take(10).copied().collect();
        let t = Instant::now();
        for id in &roots_to_remove {
            arena.remove(*id);
        }
        let remove_ms = elapsed_ms(t);
        let remaining = arena.node_count();
        println!(
            "[REMOVE 10 root subtrees]  removed {} nodes in {remove_ms:.1} ms",
            actual_count - remaining
        );

        // ── 10. Deep chain test (stack depth) ────────────────────────
        let mut deep_arena = TreeArena::<BenchNode>::with_capacity(10_000);
        let t = Instant::now();
        build_deep_chain(&mut deep_arena, 10_000);
        let deep_ms = elapsed_ms(t);
        println!("[DEEP CHAIN]  10,000 depth in {deep_ms:.1} ms");

        deep_arena.expand_all();
        let deep_filter = FilterState::new();
        let mut deep_flat = FlatView::new();
        let t = Instant::now();
        deep_flat.rebuild(&deep_arena, &deep_filter);
        let deep_rebuild_ms = elapsed_ms(t);
        println!(
            "[DEEP FLAT_VIEW]  {} rows in {deep_rebuild_ms:.1} ms",
            deep_flat.len()
        );

        // ── 11. Memory estimate ──────────────────────────────────────
        let slot_size = std::mem::size_of::<Option<NodeSlot<BenchNode>>>();
        let flat_row_size = std::mem::size_of::<FlatRow>();
        let arena_mem_mb = (slot_size * actual_count + 4 * actual_count) as f64 / 1_048_576.0;
        let flat_mem_mb = (flat_row_size * flat_count + 16 * flat_count) as f64 / 1_048_576.0;
        let total_mb = arena_mem_mb + flat_mem_mb;
        println!("\n[MEMORY ESTIMATE]");
        println!("  NodeSlot size: {slot_size} bytes");
        println!("  FlatRow size:  {flat_row_size} bytes");
        println!("  Arena:         {arena_mem_mb:.1} MB");
        println!("  FlatView:      {flat_mem_mb:.1} MB");
        println!("  Total:         ~{total_mb:.0} MB");

        // ── Summary ──────────────────────────────────────────────────
        println!("\n--- VERDICT ---");
        let ok = rebuild_ms < 500.0 && filter_ms < 1000.0;
        if ok {
            println!("PASS: {label} is within interactive budgets (rebuild < 500ms, filter < 1s)");
        } else {
            println!("WARN: {label} exceeds interactive budgets");
            println!("  flat_view rebuild: {rebuild_ms:.1} ms (budget: 500 ms)");
            println!("  filter:            {filter_ms:.1} ms (budget: 1000 ms)");
        }
        println!();
    }
}
