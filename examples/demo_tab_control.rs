//! Demo: TabControl — modern tab controller showcase.
//!
//! Run: `cargo run --example demo_tab_control`
//!
//! Demonstrates:
//!   - Outer TabControl with mixed tab types
//!   - Nested TabControl inside one of the outer tabs
//!   - All four styles (Card / Pill / Underline / Square) selectable live
//!   - Status indicators (Active / Warning / Error / Dirty), badges, icons
//!   - Drag-reorder, scroll, overflow dropdown, close confirmation
//!   - Add (+) button hooked up to spawn new tabs

use dear_imgui_custom_mod::app_window::{AppConfig, AppHandler, AppState, AppWindow};
use dear_imgui_custom_mod::icons;
use dear_imgui_custom_mod::tab_control::{
    Badge, CloseGlyph, TabAction, TabControl, TabControlConfig, TabItem, TabStatus, TabStyle,
};
use dear_imgui_rs::Ui;

// ─── Tab variants ───────────────────────────────────────────────────────────

/// Plain home tab — pinned, non-closable, fits a single letter ("H").
struct HomeTab;

impl TabItem for HomeTab {
    fn title(&self) -> &str { "Home" }
    fn icon(&self) -> Option<&str> { Some(icons::HOME) }
    fn is_closable(&self) -> bool { false }
    fn is_pinned(&self) -> bool { true }
    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();
        ui.text("Welcome!");
        ui.spacing();
        ui.separator();
        ui.spacing();
        ui.text_wrapped(
            "TabControl showcase. Home and Settings tabs are PINNED (compact, \
             icon-only on the left) and never close. Try the keyboard:",
        );
        ui.spacing();
        ui.bullet_text("Ctrl+T  — request a new tab");
        ui.bullet_text("Ctrl+W  — close the active tab");
        ui.bullet_text("Ctrl+Tab / Ctrl+Shift+Tab — cycle through tabs");
        ui.bullet_text("Ctrl+1..8  — jump to N-th tab,  Ctrl+9 — jump to last");
        ui.bullet_text("Left/Right — step between tabs");
        ui.bullet_text("Hover an inactive tab for ~350 ms — it activates itself");
        ui.spacing();
        ui.separator();
        ui.spacing();
        ui.text_wrapped(
            "Editor tabs gain a dirty indicator (cyan dot) once edited; \
             clicking close shows a stronger confirmation popup.",
        );
    }
}

/// Editor tab with a dirty indicator that toggles via a button.
struct EditorTab {
    name: String,
    text: String,
    dirty: bool,
}

impl EditorTab {
    fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), text: String::from("Edit me\u{2026}"), dirty: false }
    }
}

impl TabItem for EditorTab {
    fn title(&self) -> &str { &self.name }
    fn icon(&self) -> Option<&str> { Some(icons::FILE_DOCUMENT_OUTLINE) }
    fn status(&self) -> TabStatus {
        if self.dirty { TabStatus::Dirty } else { TabStatus::Active }
    }
    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();
        if ui.input_text_multiline("##editor", &mut self.text, [-1.0, 200.0]).build() {
            self.dirty = true;
        }
        ui.spacing();
        ui.text(if self.dirty { "Status: unsaved changes" } else { "Status: saved" });
        ui.same_line();
        if ui.button("Save") {
            self.dirty = false;
        }
    }
}

/// Notification-feed tab with a numeric badge.
struct InboxTab {
    unread: u32,
}

impl TabItem for InboxTab {
    fn title(&self) -> &str { "Inbox" }
    fn icon(&self) -> Option<&str> { Some(icons::EMAIL_OUTLINE) }
    fn badge(&self) -> Option<Badge> {
        if self.unread == 0 { None } else { Some(Badge::count(self.unread, [0xd6, 0x4a, 0x4a])) }
    }
    fn status(&self) -> TabStatus {
        if self.unread > 5 { TabStatus::Warning } else { TabStatus::Active }
    }
    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();
        ui.text(format!("Unread: {}", self.unread));
        ui.spacing();
        if ui.button("Mark one read") && self.unread > 0 {
            self.unread -= 1;
        }
        ui.same_line();
        if ui.button("Receive new") {
            self.unread += 1;
        }
    }
}

/// Diagnostics tab — error status + tooltip.
struct DiagnosticsTab;

impl TabItem for DiagnosticsTab {
    fn title(&self) -> &str { "Diagnostics" }
    fn icon(&self) -> Option<&str> { Some(icons::PULSE) }
    fn status(&self) -> TabStatus { TabStatus::Error }
    fn tooltip(&self) -> Option<&str> { Some("3 errors detected — click for details") }
    // Demo of per-tab preview opt-out — the tooltip carries the summary.
    fn show_preview(&self) -> bool { false }
    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();
        ui.text_colored([1.0, 0.45, 0.45, 1.0], "Three errors detected:");
        ui.bullet_text("E001: connection timed out");
        ui.bullet_text("E107: invalid configuration");
        ui.bullet_text("E312: out of memory");
    }
}

/// Settings tab — controls the parent style live.
struct SettingsTab {
    style_idx: usize,
}

impl TabItem for SettingsTab {
    fn title(&self) -> &str { "Settings" }
    fn icon(&self) -> Option<&str> { Some(icons::COG_OUTLINE) }
    fn is_pinned(&self) -> bool { true }
    fn is_closable(&self) -> bool { false }
    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();
        ui.text("Tab style (applies to outer & nested):");
        ui.spacing();
        let labels = ["Pill", "Underline", "Square"];
        for (i, name) in labels.iter().enumerate() {
            if ui.radio_button_bool(name, self.style_idx == i) {
                self.style_idx = i;
            }
            ui.same_line();
        }
        ui.new_line();
        ui.spacing();
        ui.separator();
        ui.spacing();
        ui.text_wrapped(
            "Tips: drag-reorder, middle-click to close, Ctrl+W on the active tab. \
             Pinned tabs (Home, Settings) stay on the left even with many tabs open.",
        );
    }
}

/// Container tab that hosts another TabControl — demonstrates nesting.
struct NestedTab {
    inner: TabControl<EditorTab>,
}

impl NestedTab {
    fn new() -> Self {
        let mut inner: TabControl<EditorTab> = TabControl::with_config(
            "##nested_tabs",
            TabControlConfig {
                show_add_button: true,
                tab_style: TabStyle::Underline, // visually distinct from outer
                ..Default::default()
            },
        );
        inner.add(EditorTab::new("note_a.txt"));
        inner.add(EditorTab::new("note_b.txt"));
        Self { inner }
    }
}

impl TabItem for NestedTab {
    fn title(&self) -> &str { "Nested" }
    fn icon(&self) -> Option<&str> { Some(icons::FOLDER_MULTIPLE_OUTLINE) }
    fn render_content(&mut self, ui: &Ui) {
        ui.spacing();
        ui.text("This tab contains its own TabControl:");
        ui.spacing();
        if let Some(TabAction::AddRequested) = self.inner.render(ui) {
            let n = self.inner.tab_count();
            self.inner.add(EditorTab::new(format!("note_{}.txt", n + 1)));
        }
    }
}

// ─── Outer tab — boxed enum so heterogeneous tabs share one TabControl ──────

enum OuterTab {
    Home(HomeTab),
    Editor(EditorTab),
    Inbox(InboxTab),
    Diag(DiagnosticsTab),
    Settings(SettingsTab),
    /// Boxed because the nested controller is much larger than the other variants.
    Nested(Box<NestedTab>),
}

impl TabItem for OuterTab {
    fn title(&self) -> &str {
        match self {
            Self::Home(t) => t.title(),
            Self::Editor(t) => t.title(),
            Self::Inbox(t) => t.title(),
            Self::Diag(t) => t.title(),
            Self::Settings(t) => t.title(),
            Self::Nested(t) => t.title(),
        }
    }
    fn icon(&self) -> Option<&str> {
        match self {
            Self::Home(t) => t.icon(),
            Self::Editor(t) => t.icon(),
            Self::Inbox(t) => t.icon(),
            Self::Diag(t) => t.icon(),
            Self::Settings(t) => t.icon(),
            Self::Nested(t) => t.icon(),
        }
    }
    fn badge(&self) -> Option<Badge> {
        match self {
            Self::Inbox(t) => t.badge(),
            _ => None,
        }
    }
    fn status(&self) -> TabStatus {
        match self {
            Self::Editor(t) => t.status(),
            Self::Inbox(t) => t.status(),
            Self::Diag(t) => t.status(),
            _ => TabStatus::Active,
        }
    }
    fn tooltip(&self) -> Option<&str> {
        match self {
            Self::Diag(t) => t.tooltip(),
            _ => None,
        }
    }
    fn is_closable(&self) -> bool {
        match self {
            Self::Home(t) => t.is_closable(),
            _ => true,
        }
    }
    fn render_content(&mut self, ui: &Ui) {
        match self {
            Self::Home(t) => t.render_content(ui),
            Self::Editor(t) => t.render_content(ui),
            Self::Inbox(t) => t.render_content(ui),
            Self::Diag(t) => t.render_content(ui),
            Self::Settings(t) => t.render_content(ui),
            Self::Nested(t) => t.render_content(ui),
        }
    }
    fn show_preview(&self) -> bool {
        match self {
            Self::Home(t) => t.show_preview(),
            Self::Editor(t) => t.show_preview(),
            Self::Inbox(t) => t.show_preview(),
            Self::Diag(t) => t.show_preview(),
            Self::Settings(t) => t.show_preview(),
            Self::Nested(t) => t.show_preview(),
        }
    }
    // render_preview: default (re-renders content via render_content) is fine.
}

// ─── App handler ────────────────────────────────────────────────────────────

struct DemoApp {
    tc: TabControl<OuterTab>,
    next_extra: u32,
}

impl Default for DemoApp {
    fn default() -> Self {
        let mut tc: TabControl<OuterTab> = TabControl::with_config(
            "##outer_tabs",
            TabControlConfig {
                show_add_button: true,
                tab_style: TabStyle::Pill,
                // Don't auto-switch on hover — only on click / keyboard.
                hover_activate_ms: None,
                // Show a Windows-taskbar-peek-style preview after 450 ms hover.
                preview_hover_ms: Some(450),
                // Prominent close button (cross inside a thin square).
                close_glyph: CloseGlyph::SquareX,
                // The default ImGui font here doesn't carry MDI glyphs.
                icons_available: false,
                ..Default::default()
            },
        );
        tc.add(OuterTab::Home(HomeTab));
        tc.add(OuterTab::Editor(EditorTab::new("readme.md")));
        tc.add(OuterTab::Inbox(InboxTab { unread: 3 }));
        tc.add(OuterTab::Diag(DiagnosticsTab));
        tc.add(OuterTab::Nested(Box::new(NestedTab::new())));
        tc.add(OuterTab::Settings(SettingsTab { style_idx: 0 }));
        // Activate Home tab first so the first thing the user sees is the welcome
        let first_id = tc.iter().next().map(|(id, _)| id);
        if let Some(id) = first_id {
            tc.set_active(id);
        }
        Self { tc, next_extra: 1 }
    }
}

impl AppHandler for DemoApp {
    fn render(&mut self, ui: &Ui, _state: &mut AppState) {
        // Apply the style chosen in SettingsTab to both outer and inner controllers
        let outer_style = current_style(&self.tc);
        self.tc.config.tab_style = outer_style;
        propagate_style_to_nested(&mut self.tc, outer_style);

        ui.spacing();
        if let Some(TabAction::AddRequested) = self.tc.render(ui) {
            self.next_extra += 1;
            let name = format!("scratch_{}.txt", self.next_extra);
            self.tc.add(OuterTab::Editor(EditorTab::new(name)));
        }
    }
}

fn current_style(tc: &TabControl<OuterTab>) -> TabStyle {
    for (_, item) in tc.iter() {
        if let OuterTab::Settings(s) = item {
            return match s.style_idx {
                1 => TabStyle::Underline,
                2 => TabStyle::Square,
                _ => TabStyle::Pill,
            };
        }
    }
    TabStyle::Pill
}

fn propagate_style_to_nested(tc: &mut TabControl<OuterTab>, style: TabStyle) {
    for (_, item) in tc.iter_mut() {
        if let OuterTab::Nested(n) = item {
            // Keep nested visually distinct from outer if they happen to match.
            n.inner.config.tab_style = if n.inner.config.tab_style == style {
                match style {
                    TabStyle::Pill => TabStyle::Underline,
                    TabStyle::Underline => TabStyle::Square,
                    TabStyle::Square => TabStyle::Pill,
                }
            } else {
                n.inner.config.tab_style
            };
        }
    }
}

// ─── Entry point ────────────────────────────────────────────────────────────

fn main() -> Result<(), winit::error::EventLoopError> {
    AppWindow::new(AppConfig::new("TabControl Demo", 1100.0, 720.0))
        .run(DemoApp::default())
}
