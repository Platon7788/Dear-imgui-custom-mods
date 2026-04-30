//! # TabControl — modern, focused tab controller
//!
//! Pure tab strip + content area. Inspired by DevExpress XtraTabControl with
//! contemporary touches: smooth scroll, drag-reorder, animated open/close,
//! overflow dropdown, status indicators, badges, and a single-pass hit-test.
//!
//! Implement [`TabItem`] on your data type and feed instances to a
//! [`TabControl<T>`]. The component handles all UI mechanics — tab strip
//! layout, scrolling, close confirmation, drag-and-drop — while delegating
//! content rendering to your trait implementation.
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::tab_control::*;
//!
//! struct MyTab { name: String }
//!
//! impl TabItem for MyTab {
//!     fn title(&self) -> &str { &self.name }
//!     fn render_content(&mut self, ui: &Ui) {
//!         ui.text("Hello from tab content");
//!     }
//! }
//!
//! let mut tc: TabControl<MyTab> = TabControl::new("##my_tabs");
//! tc.add(MyTab { name: "First".into() });
//!
//! // In your render loop:
//! if let Some(action) = tc.render(ui) {
//!     match action {
//!         TabAction::Closed(id) => println!("closed {}", id),
//!         _ => {}
//!     }
//! }
//! ```
//!
//! ## Nested controllers
//!
//! Because `TabControl<T>` is generic, nesting is trivial — your `TabItem`
//! holds another `TabControl` and forwards `render_content`:
//!
//! ```rust,ignore
//! struct OuterTab { inner: TabControl<InnerTab> }
//!
//! impl TabItem for OuterTab {
//!     fn title(&self) -> &str { "Outer" }
//!     fn render_content(&mut self, ui: &Ui) {
//!         self.inner.render(ui);
//!     }
//! }
//! ```
//!
//! ## Zero per-frame allocations
//!
//! After construction, the controller reuses internal scratch buffers
//! (`hit_scratch`, `tab_widths_cache`, `fmt_buf`) and caches popup IDs.
//! Steady-state rendering allocates nothing.

pub mod config;
pub(crate) mod layout;
pub(crate) mod render;

#[cfg(test)]
mod tests;

pub use config::*;

use dear_imgui_rs::Ui;

// ─── TabItem trait ──────────────────────────────────────────────────────────

/// Trait implemented by user types to define a tab's metadata and content.
pub trait TabItem {
    /// Title shown on the tab.
    fn title(&self) -> &str;

    /// Optional MDI icon glyph rendered before the title.
    fn icon(&self) -> Option<&str> {
        None
    }

    /// Optional badge pill rendered after the title.
    fn badge(&self) -> Option<Badge> {
        None
    }

    /// Visual status indicator (small dot on the tab).
    fn status(&self) -> TabStatus {
        TabStatus::Active
    }

    /// Optional tooltip when hovering the tab.
    fn tooltip(&self) -> Option<&str> {
        None
    }

    /// Optional accent color override `[R, G, B]`. If `None`, the tab uses
    /// the status color from the configured [`TabColors`] palette.
    fn tab_color(&self) -> Option<[u8; 3]> {
        None
    }

    /// Optional override color for the status dot — `[R, G, B]`. If `None`
    /// (default), the dot color is taken from the configured [`TabColors`]
    /// palette via [`Self::status`]. Useful when you want a custom-colored
    /// indicator without inventing a new `TabStatus`.
    fn dot_color(&self) -> Option<[u8; 3]> {
        None
    }

    /// Optional override for the **title text** color — `[R, G, B]`.
    ///
    /// `None` (default) → text uses [`TabColors::text`] when the tab is
    /// active and [`TabColors::text_muted`] when inactive. When `Some`,
    /// the override is applied in *both* states (inactive tabs still
    /// fade slightly via the open-animation alpha, but the hue is
    /// preserved).
    ///
    /// Use this to color tabs by domain — e.g. project files green,
    /// system files amber, error tabs red — without inventing new
    /// [`TabStatus`] variants. Stacks cleanly with [`Self::tab_color`]
    /// (accent override) and [`Self::dot_color`] (status indicator).
    fn text_color(&self) -> Option<[u8; 3]> {
        None
    }

    /// Whether this tab can be closed. Cooperatively gated by
    /// [`TabControlConfig::closable`].
    fn is_closable(&self) -> bool {
        true
    }

    /// Whether this tab is pinned. Pinned tabs sit in a compact, non-scrolling
    /// strip on the left, never show a close button (use [`TabControl::remove`]
    /// to programmatically remove them), and ignore drag-reorder across the
    /// pinned/regular boundary. Default: `false`.
    fn is_pinned(&self) -> bool {
        false
    }

    /// Called when this tab becomes active.
    fn on_activated(&mut self) {}

    /// Called when this tab is no longer active.
    fn on_deactivated(&mut self) {}

    /// Render this tab's content area. Called once per frame on the active
    /// tab unless [`TabControlConfig::external_content`] is `true`.
    fn render_content(&mut self, ui: &Ui);

    /// Whether the hover preview is allowed for *this* tab. Per-tab opt-out
    /// for the global [`TabControlConfig::preview_hover_ms`] feature.
    ///
    /// Useful for tabs whose content is expensive, sensitive, or just doesn't
    /// preview meaningfully (e.g. a settings page with side-effecting
    /// widgets). Default: `true`.
    fn show_preview(&self) -> bool {
        true
    }

    /// Render the hover-preview popup body. Default: re-renders the tab's
    /// content via [`Self::render_content`] inside a small scaled-down child
    /// window — produces a live thumbnail that matches what the user would
    /// see if they activated the tab.
    ///
    /// Override for a cheaper or differently-shaped preview (e.g. a plain
    /// text summary) when re-rendering full content is too expensive.
    ///
    /// Only triggered when [`TabControlConfig::preview_hover_ms`] is `Some`
    /// and [`Self::show_preview`] returns `true`.
    fn render_preview(&mut self, ui: &Ui) {
        self.render_content(ui);
    }
}

// ─── Internal tab wrapper ───────────────────────────────────────────────────

/// Internal wrapper for a single tab. Not exposed as `pub` fields.
pub(crate) struct TabEntry<T> {
    pub(crate) id: TabId,
    pub(crate) item: T,
    pub(crate) open: bool,
    pub(crate) request_focus: bool,
    /// Open animation progress: `0.0` → `1.0`. Drives width and alpha.
    pub(crate) open_anim: f32,
}

// ─── TabControl ─────────────────────────────────────────────────────────────

/// Generic tabbed container.
///
/// `T` must implement [`TabItem`] to define what each tab is and how it draws
/// its content. See the [module docs](self) for usage.
pub struct TabControl<T: TabItem> {
    pub(crate) tabs: Vec<TabEntry<T>>,
    pub(crate) active: Option<TabId>,
    next_id: TabId,

    // Cached popup IDs — built once at construction
    pub(crate) close_popup_id: String,
    pub(crate) overflow_popup_id: String,

    /// Public configuration — modify freely between frames.
    ///
    /// **Cache invalidation:** changes to `tab_min_width`, `tab_max_width`,
    /// `tab_padding_h`, `pinned_tab_width`, `close_btn_size`, `close_btn_gap`,
    /// or `icons_available` affect tab width calculation. After mutating any
    /// of those, call [`Self::force_invalidate`] so the layout cache is
    /// rebuilt on the next render. All other fields take effect immediately.
    pub config: TabControlConfig,

    /// Right-clicked tab ID. Set when `config.context_menu = true`.
    /// Read after `render()` to drive a custom context menu.
    ///
    /// The controller never resets this on its own — once you've consumed
    /// the right-click event, set `context_tab = None` yourself. Pair with
    /// the per-frame [`Self::open_context_menu`] flag to detect "this is
    /// the frame to call `ui.open_popup(...)`".
    pub context_tab: Option<TabId>,

    /// `true` for the single frame on which the user right-clicked a tab.
    /// Pair with `ui.open_popup(...)` to open your context menu.
    pub open_context_menu: bool,

    /// Tab pending close confirmation. `Read` this before `render()` to
    /// snapshot data from the tab before it's removed.
    pub pending_close: Option<TabId>,

    // ── Internal render state ──
    pub(crate) pending_close_new: bool,
    pub(crate) scroll_offset: f32,
    pub(crate) scroll_target: f32,

    pub(crate) tab_widths_cache: Vec<f32>,
    pub(crate) tab_widths_gen: u64,
    pub(crate) tab_gen: u64,

    pub(crate) fmt_buf: String,

    // ── Click bookkeeping ──
    pub(crate) last_click_time: f64,
    pub(crate) last_click_tab: Option<TabId>,

    // ── Drag-and-drop ──
    pub(crate) drag_source_idx: Option<usize>,
    pub(crate) drag_start_x: f32,
    pub(crate) dragging: bool,

    // ── Close animation ──
    /// `(tab_id, remaining_fraction 1.0 → 0.0)` while a tab is shrinking out.
    pub(crate) closing_tab: Option<(TabId, f32)>,

    /// Set by `add()` / `set_active()` / `scroll_to_active()` to defer the
    /// actual scroll computation to the next render. This avoids invoking
    /// ImGui text measurement (which requires an initialized context) from
    /// non-render code paths — important when constructing a `TabControl`
    /// before the ImGui context is ready (e.g. inside `Default::default()`).
    pub(crate) pending_scroll_to_active: bool,

    /// Hover-activation tracking: `(tab_id, ui.time() at first hover)`.
    /// Cleared as soon as the mouse leaves the tab. Used when
    /// [`TabControlConfig::hover_activate_ms`] is `Some`.
    pub(crate) hover_target: Option<(TabId, f64)>,

    // ── Hit-test scratch (reused each frame) ──
    pub(crate) hit_scratch: Vec<render::TabHitRow>,
}

impl<T: TabItem> TabControl<T> {
    /// Create a new `TabControl` with default configuration.
    ///
    /// `id` is a unique ImGui-style ID (e.g. `"##my_tabs"`) — used to scope
    /// internal popups so multiple instances coexist safely.
    pub fn new(id: impl Into<String>) -> Self {
        Self::with_config(id, TabControlConfig::default())
    }

    /// Create a new `TabControl` with custom configuration.
    pub fn with_config(id: impl Into<String>, config: TabControlConfig) -> Self {
        let imgui_id: String = id.into();
        let close_popup_id = format!("##tc_close_{}", imgui_id);
        let overflow_popup_id = format!("##tc_overflow_{}", imgui_id);
        Self {
            tabs: Vec::with_capacity(16),
            active: None,
            next_id: 1,
            close_popup_id,
            overflow_popup_id,
            config,
            context_tab: None,
            open_context_menu: false,
            pending_close: None,
            pending_close_new: false,
            scroll_offset: 0.0,
            scroll_target: 0.0,
            tab_widths_cache: Vec::new(),
            tab_widths_gen: u64::MAX,
            tab_gen: 0,
            fmt_buf: String::with_capacity(128),
            last_click_time: 0.0,
            last_click_tab: None,
            drag_source_idx: None,
            drag_start_x: 0.0,
            dragging: false,
            closing_tab: None,
            pending_scroll_to_active: false,
            hover_target: None,
            hit_scratch: Vec::with_capacity(16),
        }
    }

    // ── Tab management ──────────────────────────────────────────────────

    /// Add a tab and return its [`TabId`]. The new tab becomes active.
    ///
    /// Insertion respects the pinned/regular invariant: pinned tabs are
    /// inserted right after the last pinned (i.e. at the end of the pinned
    /// section), regular tabs are pushed to the end. This way pinned tabs
    /// always occupy a contiguous prefix of the internal list.
    ///
    /// Safe to call before any ImGui context exists (e.g. from
    /// `Default::default()`) — the auto-scroll-to-active is deferred to the
    /// next render so no text measurement happens here.
    pub fn add(&mut self, mut item: T) -> TabId {
        let id = self.next_id;
        self.next_id += 1;

        if let Some(old_id) = self.active
            && let Some(old) = self.tabs.iter_mut().find(|t| t.id == old_id)
        {
            old.item.on_deactivated();
        }
        item.on_activated();

        let open_anim = if self.config.animate_open { 0.0 } else { 1.0 };
        let entry = TabEntry {
            id,
            item,
            open: true,
            request_focus: false,
            open_anim,
        };

        // Maintain "pinned tabs occupy a contiguous prefix" invariant.
        if entry.item.is_pinned() {
            // Insert right after the last existing pinned tab.
            let insert_at = self
                .tabs
                .iter()
                .position(|t| !t.item.is_pinned())
                .unwrap_or(self.tabs.len());
            self.tabs.insert(insert_at, entry);
        } else {
            self.tabs.push(entry);
        }
        self.active = Some(id);
        self.invalidate_tab_widths();
        self.pending_scroll_to_active = true;
        id
    }

    /// Remove a tab by ID, returning the item if found. The next-most-recent
    /// tab becomes active and receives `on_activated()`.
    pub fn remove(&mut self, id: TabId) -> Option<T> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;
        let entry = self.tabs.remove(idx);
        if self.active == Some(id) {
            self.active = self.tabs.last().map(|t| t.id);
            if let Some(new_id) = self.active
                && let Some(new_entry) = self.tabs.iter_mut().find(|t| t.id == new_id)
            {
                new_entry.item.on_activated();
            }
        }
        self.invalidate_tab_widths();
        Some(entry.item)
    }

    /// Remove all tabs. Calls `on_deactivated()` on the active tab if any.
    pub fn clear(&mut self) {
        if let Some(active_id) = self.active
            && let Some(entry) = self.tabs.iter_mut().find(|t| t.id == active_id)
        {
            entry.item.on_deactivated();
        }
        self.tabs.clear();
        self.active = None;
        self.pending_close = None;
        self.pending_close_new = false;
        self.closing_tab = None;
        self.scroll_offset = 0.0;
        self.scroll_target = 0.0;
        self.invalidate_tab_widths();
    }

    /// Move a tab from index `from` to index `to`.
    ///
    /// Cross-group moves (pinned ↔ regular) are clamped to the source group:
    /// a pinned tab cannot escape the pinned prefix, and a regular tab
    /// cannot enter it. This preserves the pinned/regular invariant.
    /// Returns `true` if a move actually happened.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() || to >= self.tabs.len() || from == to {
            return false;
        }
        let pinned_count = self.tabs.iter().take_while(|t| t.item.is_pinned()).count();
        let from_is_pinned = from < pinned_count;
        // Clamp `to` into the source's group.
        let clamped_to = if from_is_pinned {
            to.min(pinned_count.saturating_sub(1))
        } else {
            to.max(pinned_count)
        };
        if clamped_to == from {
            return false;
        }
        let entry = self.tabs.remove(from);
        self.tabs.insert(clamped_to, entry);
        self.invalidate_tab_widths();
        true
    }

    /// Re-establish the pinned/regular partition. Delegates to
    /// [`layout::enforce_pinned_partition`].
    pub(crate) fn enforce_pinned_partition(&mut self) {
        layout::enforce_pinned_partition(self);
    }

    /// Borrow a tab's item.
    pub fn get(&self, id: TabId) -> Option<&T> {
        self.tabs.iter().find(|t| t.id == id).map(|t| &t.item)
    }

    /// Mutably borrow a tab's item.
    pub fn get_mut(&mut self, id: TabId) -> Option<&mut T> {
        self.tabs
            .iter_mut()
            .find(|t| t.id == id)
            .map(|t| &mut t.item)
    }

    /// Currently active tab ID (if any).
    pub fn active_id(&self) -> Option<TabId> {
        self.active
    }

    /// Programmatically activate a tab. Calls `on_deactivated()` on the
    /// previously active tab and `on_activated()` on the new one. The
    /// scroll-into-view side effect is deferred to the next render.
    pub fn set_active(&mut self, id: TabId) {
        if !self.tabs.iter().any(|t| t.id == id) {
            return;
        }
        if let Some(old_id) = self.active
            && old_id != id
            && let Some(old) = self.tabs.iter_mut().find(|t| t.id == old_id)
        {
            old.item.on_deactivated();
        }
        self.active = Some(id);
        if let Some(entry) = self.tabs.iter_mut().find(|t| t.id == id) {
            entry.item.on_activated();
        }
        self.pending_scroll_to_active = true;
    }

    /// Number of tabs currently in the control.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Whether there are zero tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Iterate `(TabId, &T)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (TabId, &T)> {
        self.tabs.iter().map(|t| (t.id, &t.item))
    }

    /// Iterate `(TabId, &mut T)` pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (TabId, &mut T)> {
        self.tabs.iter_mut().map(|t| (t.id, &mut t.item))
    }

    /// Force the tab-width cache to be recomputed next frame. Call this if a
    /// tab's title, icon, or badge changes dynamically — the controller
    /// can't otherwise detect trait method return value changes.
    pub fn force_invalidate(&mut self) {
        self.invalidate_tab_widths();
    }

    /// Request that `scroll_target` be adjusted so the active tab is visible
    /// on the next render. The actual computation runs inside `render()` so
    /// it never invokes ImGui text measurement before the context is ready.
    pub fn scroll_to_active(&mut self) {
        self.pending_scroll_to_active = true;
    }

    // ── Rendering ───────────────────────────────────────────────────────

    /// Render the tab control at the current cursor position.
    ///
    /// Wrap in a `child_window` or window for placement and sizing. Returns
    /// at most one [`TabAction`] per frame.
    pub fn render(&mut self, ui: &Ui) -> Option<TabAction> {
        render::render_tab_control(self, ui)
    }

    // ── Internal helpers ────────────────────────────────────────────────

    pub(crate) fn invalidate_tab_widths(&mut self) {
        self.tab_gen = self.tab_gen.wrapping_add(1);
    }

    pub(crate) fn ensure_tab_widths(&mut self) {
        if self.tab_widths_gen == self.tab_gen && self.tab_widths_cache.len() == self.tabs.len() {
            return;
        }
        self.tab_widths_cache.clear();
        let cfg = &self.config;
        self.tab_widths_cache.extend(self.tabs.iter().map(|t| {
            let w = layout::compute_tab_width(cfg, &t.item);
            w.clamp(cfg.tab_min_width, cfg.tab_max_width)
        }));
        self.tab_widths_gen = self.tab_gen;
    }
}
