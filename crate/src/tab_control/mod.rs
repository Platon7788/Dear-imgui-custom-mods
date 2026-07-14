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

mod api;
pub mod colors;
pub mod config;
pub(crate) mod layout;
pub(crate) mod render;
pub mod strings;
pub mod types;

#[cfg(test)]
mod tests;

pub use colors::TabColors;
pub use config::*;
pub use strings::TabStrings;
pub use types::*;

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
    ///
    /// **Nested controllers caveat**: when `T` itself contains another
    /// `TabControl<U>` and the default preview body recursively calls
    /// `render_content` → inner `tc.render(ui)`, the inner controller's
    /// `drag_source_idx` can latch into a dragging state inside the
    /// preview tooltip if the user happens to hold the mouse button
    /// while hovering. Override with a static / cheap snapshot to
    /// avoid re-entering the inner state machine
    /// (M4 from session 034 audit).
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
    /// Hover-fade progress: `0.0` (idle) → `1.0` (fully hovered). Eased each
    /// frame toward [`Self::hovered`] when `config.animate_hover` is on; drives
    /// the smooth inactive-background lerp in the draw path.
    pub(crate) hover_anim: f32,
    /// This tab's hovered state as of the last hit-test pass. Written by
    /// `fill_hit_scratch`, read by the next frame's `hover_anim` tick.
    pub(crate) hovered: bool,
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
    pub fn with_config(id: impl Into<String>, mut config: TabControlConfig) -> Self {
        // Sync `strings` to the resolved locale so a config loaded
        // from ron with `locale: Ru` already comes up Russian without
        // the host having to call `set_locale` first. Hosts that
        // pre-populated `config.strings` with a custom catalogue
        // should also set `locale` accordingly (or call `set_strings`
        // / `set_locale` after construction).
        config.strings = TabStrings::for_locale(config.locale);
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

    // ── Localisation ────────────────────────────────────────────────────

    /// Override the user-visible language on construction. Default
    /// is English; pass [`crate::i18n::Locale::Ru`] for Russian. The
    /// host must bake `GlyphRanges::Cyrillic` (or a superset) into
    /// the active font atlas — without that, Cyrillic characters
    /// render as `?`.
    ///
    /// The locale is stored on [`TabControlConfig::locale`], so it
    /// round-trips through `ron::to_string` / `ron::from_str`.
    #[must_use]
    pub fn with_locale(mut self, locale: crate::i18n::Locale) -> Self {
        self.set_locale(locale);
        self
    }

    /// Mid-flight language switch — refreshes both `config.locale`
    /// and `config.strings`.
    pub fn set_locale(&mut self, locale: crate::i18n::Locale) {
        self.config.locale = locale;
        self.config.strings = TabStrings::for_locale(locale);
    }

    /// Currently-active locale.
    pub fn locale(&self) -> crate::i18n::Locale {
        self.config.locale
    }

    // ── Tab management ──────────────────────────────────────────────────
    //
    // `add` / `remove` / `clear` / `move_tab` / `get` / `set_active` /
    // `iter` / `force_invalidate` / `scroll_to_active` live in
    // [`super::api`] (sibling `impl` block) to keep `mod.rs` focused on
    // the type definition + construction + rendering entry point.

    // ── Rendering ───────────────────────────────────────────────────────

    /// Render the tab control at the current cursor position.
    ///
    /// Wrap in a `child_window` or window for placement and sizing. Returns
    /// at most one [`TabAction`] per frame.
    pub fn render(&mut self, ui: &Ui) -> Option<TabAction> {
        render::render_tab_control(self, ui)
    }

    // ── Internal helpers ────────────────────────────────────────────────

    pub(crate) fn invalidate_tab_layout_cache(&mut self) {
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
