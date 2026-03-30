//! Configuration types for PageControl.
//!
//! Contains all public types, enums, and configuration structs used by
//! [`PageControl`](super::PageControl). Separated from rendering logic
//! to keep concerns focused and imports clean.

// ─── Page identifier ────────────────────────────────────────────────────────

/// Opaque, auto-incrementing page identifier.
///
/// Assigned internally by [`PageControl::add`](super::PageControl::add).
/// Stable across removals — never reused within a single `PageControl` instance.
pub type PageId = u64;

// ─── Page status ────────────────────────────────────────────────────────────

/// Visual status of a page — controls indicator color on tiles and tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PageStatus {
    /// Green indicator (default).
    #[default]
    Active,
    /// Muted/gray indicator.
    Inactive,
    /// Amber indicator.
    Warning,
    /// Red indicator.
    Error,
}

// ─── Badge ──────────────────────────────────────────────────────────────────

/// Small badge shown on a tab (notification count, status label, etc.).
#[derive(Debug, Clone)]
pub struct Badge {
    pub text: String,
    pub color: [u8; 3],
}

impl Badge {
    /// Numeric badge (e.g. unread count).
    pub fn count(n: u32, color: [u8; 3]) -> Self {
        Self {
            text: n.to_string(),
            color,
        }
    }

    /// Text label badge.
    pub fn label(text: impl Into<String>, color: [u8; 3]) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

// ─── View mode ──────────────────────────────────────────────────────────────

/// Display mode for the PageControl.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentView {
    /// Grid of interactive tiles (overview).
    #[default]
    Dashboard,
    /// Tab strip with content area.
    Tabs,
    /// Custom view — the component renders nothing; caller handles all content.
    /// The `u8` is a user-defined view index for distinguishing multiple custom views.
    Custom(u8),
}

// ─── Tab style ──────────────────────────────────────────────────────────────

/// Visual style for tabs in the tab strip.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TabStyle {
    /// Fully rounded pill-shaped tabs (default).
    #[default]
    Pill,
    /// Flat background, accent underline only (Material Design style).
    Underline,
    /// Card-style tabs with top rounding (Chrome/browser style).
    Card,
    /// Rectangular tabs with small top rounding (classic style).
    Square,
}

// ─── Actions ────────────────────────────────────────────────────────────────

/// Actions returned by [`PageControl::render`](super::PageControl::render).
///
/// At most one action is returned per frame. The caller can match on it
/// to react to user interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageAction {
    /// A page tab was clicked — it is now the active page.
    Activated(PageId),
    /// A page was closed (after optional confirmation).
    Closed(PageId),
    /// A dashboard tile was clicked.
    TileClicked(PageId),
    /// The "+" add button was clicked (requires `show_add_button = true`).
    AddRequested,
    /// A custom action from inside a dashboard tile body.
    /// The `u64` is a user-defined action payload returned by
    /// [`PageItem::render_tile_body`](super::PageItem::render_tile_body) or
    /// [`PageItem::render_tile`](super::PageItem::render_tile).
    TileBodyAction(PageId, u64),
    /// Tab was double-clicked (e.g. for rename or detach).
    DoubleClicked(PageId),
    /// Tabs were reordered via drag-and-drop. The PageId is the tab that was moved.
    Reordered(PageId),
    /// View toggle button was clicked (Dashboard ↔ Tabs).
    ViewToggled,
}

// ─── Localization strings ───────────────────────────────────────────────────

/// User-facing strings for the PageControl UI.
///
/// Override with translations for localization.
pub struct PcStrings {
    pub cancel: &'static str,
    pub close: &'static str,
    pub close_confirm: &'static str,
    pub no_pages: &'static str,
    pub empty_hint: &'static str,
    pub overflow_tooltip: &'static str,
    pub view_dashboard: &'static str,
    pub view_tabs: &'static str,
    pub add_page: &'static str,
}

impl Default for PcStrings {
    fn default() -> Self {
        Self {
            cancel: "Cancel",
            close: "Close",
            close_confirm: "Close this page?",
            no_pages: "No pages",
            empty_hint: "Add a page to begin\u{2026}",
            overflow_tooltip: "All tabs",
            view_dashboard: "Dashboard",
            view_tabs: "Tabs",
            add_page: "Add page",
        }
    }
}

// ─── Color palette ──────────────────────────────────────────────────────────

/// Color palette for PageControl elements.
///
/// All colors are `[R, G, B]` in 0–255 range. Alpha is applied per-use.
pub struct PcColors {
    pub tab_bg: [u8; 3],
    pub tab_hover: [u8; 3],
    pub tab_active: [u8; 3],
    pub accent: [u8; 3],
    pub text: [u8; 3],
    pub text_muted: [u8; 3],
    pub close_hover: [u8; 3],
    pub strip_bg: [u8; 3],
    pub separator: [u8; 3],
    pub tile_bg: [u8; 3],
    pub tile_hover: [u8; 3],
    pub status_active: [u8; 3],
    pub status_inactive: [u8; 3],
    pub status_warning: [u8; 3],
    pub status_error: [u8; 3],
}

impl Default for PcColors {
    fn default() -> Self {
        Self {
            tab_bg: [0x35, 0x3a, 0x44],
            tab_hover: [0x3f, 0x45, 0x52],
            tab_active: [0x42, 0x48, 0x55],
            accent: [0x5b, 0x9b, 0xd5],
            text: [0xe0, 0xe4, 0xea],
            text_muted: [0x8a, 0x92, 0xa1],
            close_hover: [0xe0, 0x60, 0x60],
            strip_bg: [0x2a, 0x2e, 0x37],
            separator: [0x3f, 0x46, 0x54],
            tile_bg: [0x35, 0x3a, 0x44],
            tile_hover: [0x3f, 0x45, 0x52],
            status_active: [0x5f, 0xb8, 0x70],
            status_inactive: [0x8a, 0x92, 0xa1],
            status_warning: [0xd0, 0x7a, 0x30],
            status_error: [0xd0, 0x45, 0x45],
        }
    }
}

impl PcColors {
    /// Return the `[u8; 3]` color for a given [`PageStatus`].
    pub fn status_color(&self, status: PageStatus) -> [u8; 3] {
        match status {
            PageStatus::Active => self.status_active,
            PageStatus::Inactive => self.status_inactive,
            PageStatus::Warning => self.status_warning,
            PageStatus::Error => self.status_error,
        }
    }
}

// ─── Configuration ──────────────────────────────────────────────────────────

/// Full configuration for [`PageControl`](super::PageControl).
///
/// All fields have sensible defaults via [`Default`].
pub struct PageControlConfig {
    // ── Behavior ──
    /// Global override: allow closing pages. Individual pages can still
    /// override via [`PageItem::is_closable`](super::PageItem::is_closable).
    pub closable: bool,
    /// Show a confirmation popup before closing a page.
    pub confirm_close: bool,
    /// Middle-click on a tab closes it (browser-style).
    pub middle_click_close: bool,
    /// Scroll wheel on the tab strip scrolls tabs horizontally.
    pub scroll_with_wheel: bool,
    /// Left/Right arrow keys cycle tabs, Ctrl+W closes active tab.
    pub keyboard_nav: bool,
    /// Show a "+" button at the end of the tab strip.
    /// Returns [`PageAction::AddRequested`] when clicked.
    pub show_add_button: bool,
    /// Right-click on a tab/tile sets `context_page` and `open_context_menu`.
    pub context_menu: bool,
    /// When `true`, the tab strip is rendered but `render_content()` is NOT
    /// called on the active page. The caller is responsible for rendering
    /// content after `PageControl::render()` returns (use `active_id()` to
    /// determine which page is active). This allows passing extra context
    /// (localization, channels, etc.) that the `PageItem` trait doesn't carry.
    pub external_content: bool,

    // ── Tab strip ──
    /// Visual style for tabs.
    pub tab_style: TabStyle,
    /// Show an accent underline on the active tab (Pill/Square/Card styles).
    pub show_tab_underline: bool,
    pub tab_height: f32,
    pub tab_rounding: f32,
    pub tab_padding_h: f32,
    pub tab_gap: f32,
    pub close_btn_size: f32,
    pub close_btn_gap: f32,
    pub strip_padding_v: f32,
    pub scroll_btn_width: f32,
    pub scroll_speed: f32,
    /// Minimum tab width in pixels. Default: `60.0`.
    pub tab_min_width: f32,
    /// Maximum tab width in pixels. Default: `300.0`.
    pub tab_max_width: f32,
    /// Smooth scroll animation for the tab strip. Default: `true`.
    pub smooth_scroll: bool,
    /// Show overflow dropdown button when tabs don't fit. Default: `true`.
    /// Displays a list of all tabs for quick navigation.
    pub show_overflow_dropdown: bool,
    /// Show a Dashboard↔Tabs view toggle button at the end of the tab strip. Default: `false`.
    pub show_view_toggle: bool,

    // ── Dashboard tiles ──
    pub tile_width: f32,
    pub tile_header_height: f32,
    pub tile_body_height: f32,
    pub tile_gap: f32,
    pub tile_rounding: f32,
    pub tile_padding: f32,
    /// Fixed number of tile columns. `None` = auto-compute from tile_width. Default: `None`.
    pub dashboard_columns: Option<usize>,
    /// Show a "+" tile at the end of the dashboard grid. Default: `false`.
    /// Returns [`PageAction::AddRequested`] when clicked.
    pub show_add_tile: bool,

    // ── Dashboard header ──
    /// Optional title shown above the dashboard tile grid (e.g. "Connected Clients").
    pub dashboard_title: Option<String>,
    /// When `true` and `dashboard_title` is set, appends `(N)` with the page count.
    pub dashboard_show_count: bool,

    // ── Appearance ──
    pub colors: PcColors,
    pub strings: PcStrings,
}

impl Default for PageControlConfig {
    fn default() -> Self {
        Self {
            closable: true,
            confirm_close: true,
            middle_click_close: true,
            scroll_with_wheel: true,
            keyboard_nav: true,
            show_add_button: false,
            context_menu: true,
            external_content: false,

            tab_style: TabStyle::default(),
            show_tab_underline: true,
            tab_height: 24.0,
            tab_rounding: 12.0,
            tab_padding_h: 8.0,
            tab_gap: 4.0,
            close_btn_size: 12.0,
            close_btn_gap: 5.0,
            strip_padding_v: 3.0,
            scroll_btn_width: 22.0,
            scroll_speed: 200.0,
            tab_min_width: 60.0,
            tab_max_width: 300.0,
            smooth_scroll: true,
            show_overflow_dropdown: true,
            show_view_toggle: false,

            tile_width: 210.0,
            tile_header_height: 40.0,
            tile_body_height: 100.0,
            tile_gap: 10.0,
            tile_rounding: 8.0,
            tile_padding: 10.0,
            dashboard_columns: None,
            show_add_tile: false,

            dashboard_title: None,
            dashboard_show_count: false,

            colors: PcColors::default(),
            strings: PcStrings::default(),
        }
    }
}

impl PageControlConfig {
    /// Total tile height (header + body).
    #[inline]
    pub fn tile_height(&self) -> f32 {
        self.tile_header_height + self.tile_body_height
    }

    /// Tab strip total height (tab + vertical padding).
    #[inline]
    pub fn strip_height(&self) -> f32 {
        self.tab_height + self.strip_padding_v * 2.0
    }
}
