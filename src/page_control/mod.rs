//! # PageControl — generic tabbed container with dashboard/tile view
//!
//! A fully generic, trait-based UI component providing two view modes:
//!
//! - **Dashboard**: scrollable grid of interactive tiles (overview)
//! - **Tabs**: pill-shaped tab strip with a content area rendered by the active page
//!
//! ## Design
//!
//! Implement [`PageItem`] on your data type, then add instances to a
//! [`PageControl<T>`]. The component handles all UI mechanics — tab scrolling,
//! close confirmation, dashboard layout — while delegating tile and content
//! rendering to your trait implementation.
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::page_control::*;
//!
//! struct MyPage { name: String }
//!
//! impl PageItem for MyPage {
//!     fn title(&self) -> &str { &self.name }
//!     fn render_tile_body(&self, ui: &Ui, area: [f32; 4]) -> Option<u64> {
//!         let draw = ui.get_window_draw_list();
//!         draw.add_text([area[0], area[1]], 0xFFFFFFFF, "Hello from tile");
//!         None
//!     }
//!     fn render_content(&mut self, ui: &Ui) {
//!         ui.text("Hello from content area");
//!     }
//! }
//!
//! let mut pc: PageControl<MyPage> = PageControl::new("##tabs");
//! let id = pc.add(MyPage { name: "Page 1".into() });
//!
//! // Render loop:
//! if let Some(action) = pc.render(ui) {
//!     match action {
//!         PageAction::TileClicked(id) => { pc.set_active(id); pc.view = ContentView::Tabs; }
//!         _ => {}
//!     }
//! }
//! ```
//!
//! ## Zero per-frame allocations
//!
//! After initialization, `PageControl` reuses internal scratch buffers
//! (`fmt_buf`, `tab_widths_cache`) and performs no heap allocations
//! during rendering.

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
pub(crate) mod render;

pub use config::*;

use crate::utils::text::calc_text_size;
use dear_imgui_rs::Ui;

/// Single tile's pre-computed layout data: (index, x, y, hovered, active, color, closable, accent).
pub(crate) type TileScratchRow = (usize, f32, f32, bool, bool, [f32; 4], bool, [f32; 4]);

// ─── PageItem trait ─────────────────────────────────────────────────────────

/// Trait that defines a page within a [`PageControl`].
///
/// Implement this on your data type to control what is displayed
/// in dashboard tiles and tab content areas.
pub trait PageItem {
    /// Title shown on the tab and tile header.
    fn title(&self) -> &str;

    /// Optional MDI icon shown before the title.
    fn icon(&self) -> Option<&str> {
        None
    }

    /// Whether this page can be closed. Checked together with
    /// [`PageControlConfig::closable`].
    fn is_closable(&self) -> bool {
        true
    }

    /// Visual status indicator (color dot on tile/tab).
    fn status(&self) -> PageStatus {
        PageStatus::Active
    }

    /// Optional badge on the tab (e.g. notification count).
    fn badge(&self) -> Option<Badge> {
        None
    }

    /// Optional tooltip when hovering the tab or tile header.
    fn tooltip(&self) -> Option<&str> {
        None
    }

    /// Optional per-tab accent color override `[R, G, B]`.
    /// If `None`, uses the status color from the palette.
    fn tab_color(&self) -> Option<[u8; 3]> {
        None
    }

    /// Optional subtitle shown below the title on dashboard tiles.
    /// Multiple lines are supported (separated by `\n`).
    fn subtitle(&self) -> Option<&str> {
        None
    }

    /// Called when this page becomes the active tab.
    fn on_activated(&mut self) {}

    /// Called when this page is no longer the active tab.
    fn on_deactivated(&mut self) {}

    /// Whether this page uses fully custom tile rendering via [`render_tile`](Self::render_tile).
    ///
    /// When `true`, the default tile header (icon + title + subtitle) and body
    /// layout is skipped — only the tile background, hover border, and close
    /// button are drawn by the component. Everything else is delegated to
    /// [`render_tile`](Self::render_tile).
    fn has_custom_tile(&self) -> bool {
        false
    }

    /// Render fully custom tile content (replaces both header and body).
    ///
    /// Only called when [`has_custom_tile`](Self::has_custom_tile) returns `true`.
    /// `area` = `[x, y, width, height]` of the tile interior (inside background rect).
    /// Return `Some(action_id)` if a custom click occurred inside the tile.
    ///
    /// The component still handles: tile background, hover border, close button,
    /// grid layout, and overall click detection.
    fn render_tile(&self, ui: &Ui, area: [f32; 4]) -> Option<u64> {
        let _ = (ui, area);
        None
    }

    /// Render custom body inside a dashboard tile.
    ///
    /// `area` = `[x, y, width, height]` in screen coordinates.
    /// Use `ui.get_window_draw_list()` for custom drawing.
    /// Called only in [`ContentView::Dashboard`] mode when
    /// [`has_custom_tile`](Self::has_custom_tile) is `false`.
    ///
    /// Return `Some(action_id)` to report a custom click inside the tile body.
    /// This will be returned as [`PageAction::TileBodyAction`].
    fn render_tile_body(&self, ui: &Ui, area: [f32; 4]) -> Option<u64> {
        let _ = (ui, area);
        None
    }

    /// Render content when this page's tab is active.
    ///
    /// Called only in [`ContentView::Tabs`] mode (and when
    /// [`external_content`](PageControlConfig::external_content) is `false`),
    /// after the tab strip. The full remaining area is available for ImGui widgets.
    fn render_content(&mut self, ui: &Ui) {
        let _ = ui;
    }
}

// ─── Internal page wrapper ──────────────────────────────────────────────────

pub struct PageEntry<T> {
    pub id: PageId,
    pub item: T,
    pub open: bool,
    pub request_focus: bool,
}

// ─── PageControl ────────────────────────────────────────────────────────────

/// Generic tabbed container with dashboard/tile view.
///
/// `T` must implement [`PageItem`] to define how each page is rendered.
pub struct PageControl<T: PageItem> {
    pub(crate) imgui_id: String,
    pub pages: Vec<PageEntry<T>>,
    pub(crate) active: Option<PageId>,
    next_id: PageId,

    /// Public configuration — modify freely between frames.
    pub config: PageControlConfig,

    /// Current view mode — switch between Dashboard and Tabs.
    pub view: ContentView,

    /// Right-clicked page ID (set when `config.context_menu = true`).
    /// Check this after `render()` to show your own context menu.
    pub context_page: Option<PageId>,

    /// Set to `true` on the frame a right-click occurs.
    /// Use with `ui.open_popup()` to show a context menu.
    pub open_context_menu: bool,

    // ── Internal render state ──
    pub(crate) scroll_offset: f32,
    /// Page ID pending close confirmation (if any). Read this before `render()`
    /// to capture data from the page before it is removed.
    pub pending_close: Option<PageId>,
    pub(crate) pending_close_new: bool,
    pub(crate) tab_widths_cache: Vec<f32>,
    pub(crate) tab_widths_gen: u64,
    pub(crate) fmt_buf: String,

    // ── Smooth scroll state ──
    /// Target scroll offset (animated towards this value).
    pub(crate) scroll_target: f32,

    // ── Double-click detection ──
    pub(crate) last_click_time: f64,
    pub(crate) last_click_tab: Option<PageId>,

    // ── Tab width generation counter ──
    /// Monotonically increasing counter, incremented on any change that
    /// affects tab widths (add, remove, title change, badge change).
    pub(crate) tab_gen: u64,

    // ── Drag-and-drop reorder state ──
    /// Index of the tab currently being dragged (if any).
    pub(crate) drag_source_idx: Option<usize>,
    /// Mouse X when drag started.
    pub(crate) drag_start_x: f32,
    /// Whether a drag is currently in progress (mouse held after initial threshold).
    pub(crate) dragging: bool,

    // ── Tab close animation ──
    /// Tab currently being animated closed (id, remaining_width_fraction 1.0→0.0).
    #[allow(dead_code)]
    pub(crate) closing_tab: Option<(PageId, f32)>,

    // ── Dashboard tile scratch (reused each frame) ──
    pub(crate) tile_scratch: Vec<TileScratchRow>,
}

impl<T: PageItem> PageControl<T> {
    /// Create a new PageControl with default configuration.
    ///
    /// `id` is the ImGui ID (e.g. `"##my_tabs"`).
    pub fn new(id: impl Into<String>) -> Self {
        Self::with_config(id, PageControlConfig::default())
    }

    /// Create a new PageControl with custom configuration.
    pub fn with_config(id: impl Into<String>, config: PageControlConfig) -> Self {
        Self {
            imgui_id: id.into(),
            pages: Vec::with_capacity(16),
            active: None,
            next_id: 1,
            config,
            view: ContentView::default(),
            context_page: None,
            open_context_menu: false,
            scroll_offset: 0.0,
            pending_close: None,
            pending_close_new: false,
            tab_widths_cache: Vec::new(),
            tab_widths_gen: u64::MAX,
            fmt_buf: String::with_capacity(128),
            scroll_target: 0.0,
            last_click_time: 0.0,
            last_click_tab: None,
            tab_gen: 0,
            drag_source_idx: None,
            drag_start_x: 0.0,
            dragging: false,
            closing_tab: None,
            tile_scratch: Vec::new(),
        }
    }

    // ── Page management ─────────────────────────────────────────────────

    /// Add a page and return its [`PageId`].
    ///
    /// The new page is automatically activated (brought to front).
    pub fn add(&mut self, mut item: T) -> PageId {
        let id = self.next_id;
        self.next_id += 1;
        // Deactivate previous active page
        if let Some(old_id) = self.active
            && let Some(old) = self.pages.iter_mut().find(|p| p.id == old_id)
        {
            old.item.on_deactivated();
        }
        item.on_activated();
        self.pages.push(PageEntry {
            id,
            item,
            open: true,
            request_focus: true,
        });
        self.active = Some(id);
        self.invalidate_tab_widths();
        id
    }

    /// Remove a page by ID. Returns the item if found.
    pub fn remove(&mut self, id: PageId) -> Option<T> {
        let idx = self.pages.iter().position(|p| p.id == id)?;
        let entry = self.pages.remove(idx);
        if self.active == Some(id) {
            self.active = self.pages.last().map(|p| p.id);
        }
        self.invalidate_tab_widths();
        Some(entry.item)
    }

    /// Get a shared reference to a page item.
    pub fn get(&self, id: PageId) -> Option<&T> {
        self.pages.iter().find(|p| p.id == id).map(|p| &p.item)
    }

    /// Get a mutable reference to a page item.
    pub fn get_mut(&mut self, id: PageId) -> Option<&mut T> {
        self.pages
            .iter_mut()
            .find(|p| p.id == id)
            .map(|p| &mut p.item)
    }

    /// Currently active page ID (if any).
    pub fn active_id(&self) -> Option<PageId> {
        self.active
    }

    /// Force recalculation of tab widths on the next frame.
    ///
    /// Call this when a page's title, badge, or icon changes dynamically,
    /// since the component cannot detect trait method return value changes.
    pub fn force_invalidate(&mut self) {
        self.invalidate_tab_widths();
    }

    /// Scroll the tab strip to make the active tab visible.
    ///
    /// Useful after programmatically activating a tab that may be off-screen.
    pub fn scroll_to_active(&mut self) {
        self.ensure_tab_widths();
        if let Some(active_id) = self.active
            && let Some(idx) = self.pages.iter().position(|p| p.id == active_id)
        {
            let cfg = &self.config;
            let mut tx: f32 = 0.0;
            for w in self.tab_widths_cache.iter().take(idx) {
                tx += w + cfg.tab_gap;
            }
            let tw = self.tab_widths_cache.get(idx).copied().unwrap_or(0.0);
            // Set scroll target so that the active tab is visible
            if tx < self.scroll_target {
                self.scroll_target = tx;
            } else {
                // We don't know scroll_area_w here, so just set target to tx
                // The render pass will clamp it appropriately
                let total_tabs_w: f32 = self.tab_widths_cache.iter().sum::<f32>()
                    + cfg.tab_gap * (self.tab_widths_cache.len() as f32 - 1.0).max(0.0);
                if tx + tw > self.scroll_target + total_tabs_w * 0.5 {
                    self.scroll_target = (tx + tw - total_tabs_w * 0.3).max(0.0);
                }
            }
        }
    }

    /// Set the active page. No-op if `id` doesn't exist.
    pub fn set_active(&mut self, id: PageId) {
        if self.pages.iter().any(|p| p.id == id) {
            // Notify old page
            if let Some(old_id) = self.active
                && old_id != id
                && let Some(old) = self.pages.iter_mut().find(|p| p.id == old_id)
            {
                old.item.on_deactivated();
            }
            self.active = Some(id);
            if let Some(entry) = self.pages.iter_mut().find(|p| p.id == id) {
                entry.item.on_activated();
            }
            self.scroll_to_active();
        }
    }

    /// Number of open pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Whether there are no pages.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Iterate over `(PageId, &T)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (PageId, &T)> {
        self.pages.iter().map(|p| (p.id, &p.item))
    }

    /// Iterate over `(PageId, &mut T)` pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (PageId, &mut T)> {
        self.pages.iter_mut().map(|p| (p.id, &mut p.item))
    }

    // ── Rendering ───────────────────────────────────────────────────────

    /// Render the PageControl in the current ImGui context.
    ///
    /// Call every frame. Returns an optional [`PageAction`] describing
    /// the most significant user interaction this frame.
    ///
    /// Renders inline at the current cursor position — wrap in a
    /// `ui.child_window` or `ui.window` to control placement and sizing.
    pub fn render(&mut self, ui: &Ui) -> Option<PageAction> {
        render::render_page_control(self, ui)
    }

    // ── Internal helpers ────────────────────────────────────────────────

    pub(crate) fn invalidate_tab_widths(&mut self) {
        self.tab_gen += 1;
    }

    pub(crate) fn ensure_tab_widths(&mut self) {
        if self.tab_widths_gen == self.tab_gen && self.tab_widths_cache.len() == self.pages.len() {
            return;
        }
        self.tab_widths_cache.clear();
        let cfg = &self.config;
        self.tab_widths_cache.extend(self.pages.iter().map(|p| {
            let w = compute_tab_width(cfg, &p.item);
            w.clamp(cfg.tab_min_width, cfg.tab_max_width)
        }));
        self.tab_widths_gen = self.tab_gen;
    }
}

/// Compute the pixel width of a tab for a given page item.
fn compute_tab_width<T: PageItem>(cfg: &PageControlConfig, item: &T) -> f32 {
    let mut w = cfg.tab_padding_h;

    // Status dot
    w += 10.0;

    // Icon
    if let Some(icon) = item.icon() {
        w += calc_text_size(icon)[0] + 4.0;
    }

    // Title
    w += calc_text_size(item.title())[0];

    // Close button
    if cfg.closable && item.is_closable() {
        w += cfg.close_btn_gap + cfg.close_btn_size;
    }

    // Badge
    if let Some(ref badge) = item.badge() {
        w += 4.0 + calc_text_size(&badge.text)[0] + 8.0;
    }

    w += cfg.tab_padding_h;
    w
}

// ─── Public helper: draw_mini_tile ──────────────────────────────────────────

/// Draw a small interactive labeled tile — utility for use inside
/// [`PageItem::render_tile_body`] implementations.
///
/// Draws a rounded rectangle with an accent bar, icon, and label
/// at the given screen position. Performs hit-testing and returns
/// `true` if the mini-tile was left-clicked this frame.
///
/// - `ui`: ImGui context (for mouse state)
/// - `pos`: `[x, y]` screen position (top-left)
/// - `size`: `[width, height]`
/// - `icon`: MDI icon string
/// - `label`: text label
/// - `accent`: accent color `[R, G, B]`
///
/// Returns `true` if the mini-tile was clicked.
pub fn draw_mini_tile(
    ui: &dear_imgui_rs::Ui,
    pos: [f32; 2],
    size: [f32; 2],
    icon: &str,
    label: &str,
    accent: [u8; 3],
    colors: &PcColors,
) -> bool {
    use crate::utils::color::rgb_arr as c32;

    let [x, y] = pos;
    let [w, h] = size;

    let mouse = ui.io().mouse_pos();
    let hovered = mouse[0] >= x && mouse[0] < x + w && mouse[1] >= y && mouse[1] < y + h;
    let clicked = hovered && ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Left);

    let draw = ui.get_window_draw_list();
    let bg: [u8; 3] = if hovered {
        colors.tile_hover
    } else {
        colors.tile_bg
    };

    draw.add_rect([x, y], [x + w, y + h], c32(bg, 220))
        .rounding(3.0)
        .filled(true)
        .build();

    if hovered {
        draw.add_rect([x, y], [x + w, y + h], c32(accent, 180))
            .rounding(3.0)
            .filled(false)
            .thickness(1.0)
            .build();
    }

    // Left accent bar
    draw.add_rect([x, y + 3.0], [x + 2.0, y + h - 3.0], c32(accent, 255))
        .filled(true)
        .rounding(1.0)
        .build();

    let icon_sz = calc_text_size(icon);
    let label_sz = calc_text_size(label);
    let total_w = icon_sz[0] + 3.0 + label_sz[0];
    let cx = x + (w - total_w) * 0.5;
    let cy = y + (h - icon_sz[1]) * 0.5;

    let alpha = if hovered { 255 } else { 220 };
    draw.add_text([cx, cy], c32(accent, alpha), icon);
    draw.add_text(
        [cx + icon_sz[0] + 3.0, cy],
        c32(colors.text_muted, alpha),
        label,
    );

    clicked
}
