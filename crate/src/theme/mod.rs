//! Theme module — per-theme bundles + shared color tokens.
//!
//! Each built-in theme lives in its own file and owns the full stack for
//! that theme: titlebar colors, nav colors, dialog colors, status-bar
//! config, and the Dear ImGui style palette. This keeps the "one theme =
//! one file" rule so a single change stays contained.
//!
//! Module layout — one theme per file, each owning a full stack:
//! - [`dark`]       — NxT native dark palette (default)
//! - [`light`]      — readable light palette with clearly visible borders
//!
//! Supporting modules:
//! - [`palettes`] — crate-wide palette TYPES (shared colour-token structs)
//! - [`tokens`] — legacy semantic colour constants, re-exported here for
//!   callers that tint their own widgets by the Dark palette (e.g.
//!   `code_editor`, `file_manager`)
//!
//! Prefer the full theme modules above for new code.

// The re-exported legacy colour constants are semantic tokens named by
// their intent (BG_WINDOW, ACCENT, …) so a rustdoc description would be
// circular. Opt out of `missing_docs` for this module; the unified
// `Theme` enum (below) is fully documented.
#![allow(missing_docs)]

pub mod dark;
pub mod light;
pub mod palettes;
mod tokens;

// ─── Palette types — single source of truth ─────────────────────────────────
//
// Re-exported from [`palettes`] so the standard import path is
// `crate::theme::TitlebarColors` etc. The consumer modules
// (`chrome`, `confirm_dialog`, `nav_panel`, `notifications`)
// re-export these too for backwards-compatible user paths.
pub use palettes::{
    DialogColors, DisasmFlowKind, DisasmViewColors, HexViewerColors, NavColors, NotificationColors,
    StatusBarColors, TitlebarColors,
};

// ─── Legacy semantic colour tokens ──────────────────────────────────────────
//
// The Dark/Light palette constants (`BG_WINDOW`, `ACCENT`, `LIGHT_ACCENT`, …)
// pre-date the [`Theme`] enum and are kept for `code_editor` /
// `file_manager`, which still tint widgets by the Dark palette directly.
// They live in [`tokens`] and are re-exported here so callers keep their
// historic `crate::theme::ACCENT` import paths. New code should prefer the
// typed sub-palette accessors on [`Theme`] (`Theme::Dark.titlebar()` etc.).
pub use tokens::*;

// ─── Unified Theme selector ──────────────────────────────────────────────────

// Palette tokens (`TitlebarColors`, `NavColors`, `DialogColors`,
// `NotificationColors`) are now defined in `theme::palettes` and
// re-exported at the top of this module — no upward import needed.
//
// `StatusBarConfig` still lives in `crate::status_bar` (it bundles
// layout fields with colour tokens — splitting it out is scheduled
// separately). Until then this is the one cross-module palette
// import.
#[cfg(feature = "status_bar")]
use crate::status_bar::StatusBarConfig;
use dear_imgui_rs::Style;

/// Single application-wide theme selector.
///
/// Every built-in theme owns the full stack (titlebar / nav / dialog /
/// statusbar / Dear ImGui style) through its per-theme module
/// (`crate::theme::{dark, light}`). Components
/// take this value by reference and pull the matching sub-palette via the
/// methods below — there is no per-component theme enum any more.
///
/// ```rust,no_run
/// use dear_imgui_custom_mod::theme::Theme;
/// let t = Theme::Dark.next();    // compile-time variants
/// let tb = Theme::Dark.titlebar();
/// let cols = Theme::default().nav();
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum Theme {
    /// NxT native dark palette (warm grey + blue accent).
    #[default]
    Dark,
    /// Readable light palette with visible borders.
    Light,
}

impl Theme {
    /// All built-in themes, ordered as they appear in Settings UIs.
    pub const ALL: &'static [Theme] = &[Theme::Dark, Theme::Light];

    /// Human-readable English name — used in menus / combo boxes.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }

    /// One-line description shown in Settings tooltips / about dialogs.
    pub fn description(self) -> &'static str {
        match self {
            Self::Dark => "Warm grey with blue accent — NxT default.",
            Self::Light => "Neutral light, optimised for daylight reading.",
        }
    }

    /// Whether this theme uses dark surfaces (for choosing contrasting glyphs / shadows).
    pub const fn is_dark(self) -> bool {
        !matches!(self, Self::Light)
    }

    /// Whether this theme uses light surfaces.
    pub const fn is_light(self) -> bool {
        matches!(self, Self::Light)
    }

    /// Titlebar colours for this theme. The palette type lives in
    /// [`palettes`] and is always available; consumed by [`crate::chrome`]
    /// when rendering the borderless titlebar.
    pub fn titlebar(self) -> TitlebarColors {
        match self {
            Self::Dark => dark::titlebar_colors(),
            Self::Light => light::titlebar_colors(),
        }
    }

    /// Nav-panel colours for this theme. Always available — the palette
    /// type lives in [`palettes`].
    pub fn nav(self) -> NavColors {
        match self {
            Self::Dark => dark::nav_colors(),
            Self::Light => light::nav_colors(),
        }
    }

    /// Confirm-dialog colours for this theme. Always available — the
    /// palette type lives in [`palettes`].
    pub fn dialog(self) -> DialogColors {
        match self {
            Self::Dark => dark::dialog_colors(),
            Self::Light => light::dialog_colors(),
        }
    }

    /// Notification-center colours for this theme. Always available —
    /// the palette type lives in [`palettes`].
    pub fn notifications(self) -> NotificationColors {
        match self {
            Self::Dark => NotificationColors::dark(),
            Self::Light => NotificationColors::light(),
        }
    }

    /// Status-bar **colour** subset for this theme. Always available —
    /// the [`StatusBarColors`] type lives in [`palettes`]. Use this when
    /// you only need the palette (e.g. building a custom widget that
    /// re-uses the same indicator colours) without the layout fields
    /// of [`StatusBarConfig`].
    pub fn statusbar_colors(self) -> StatusBarColors {
        match self {
            Self::Dark => dark::statusbar_colors(),
            Self::Light => light::statusbar_colors(),
        }
    }

    /// Status-bar config (colours + default geometry) for this theme.
    /// Available only with the `status_bar` feature — `StatusBarConfig`
    /// lives in the `status_bar` widget module. For palette-only access
    /// see [`Self::statusbar_colors`].
    #[cfg(feature = "status_bar")]
    pub fn statusbar(self) -> StatusBarConfig {
        match self {
            Self::Dark => dark::statusbar_config(),
            Self::Light => light::statusbar_config(),
        }
    }

    /// Hex-viewer palette for this theme — 18 colour tokens used by
    /// [`crate::hex_viewer::HexViewer`], synthesised from the same
    /// `accent` / `success` / `warning` / `danger` / surface tokens
    /// the rest of the chrome stack uses, so the byte gutter, ASCII
    /// column, inspector and category tints all stay in the same
    /// visual family as `Theme::nav()` / `Theme::statusbar_colors()`.
    ///
    /// Apply via [`crate::hex_viewer::HexViewerConfig::with_theme`] or
    /// [`crate::hex_viewer::HexViewerConfig::apply_theme_colors`].
    pub fn hex_viewer_colors(self) -> HexViewerColors {
        match self {
            Self::Dark => dark::hex_viewer_colors(),
            Self::Light => light::hex_viewer_colors(),
        }
    }

    /// Disassembly-view palette for this theme — 26 colour tokens
    /// used by [`crate::disasm_view::DisasmView`], synthesised from
    /// the same `accent` / `success` / `warning` / `danger` /
    /// `purple` / `orange` / `cyan` semantic tokens the rest of the
    /// chrome stack uses, so the address gutter, mnemonic colouring,
    /// operand syntax tinting, branch arrows and breakpoint markers
    /// all stay in the same visual family as `Theme::nav()` /
    /// `Theme::statusbar_colors()`.
    ///
    /// Apply via [`crate::disasm_view::DisasmViewConfig::with_theme`]
    /// or
    /// [`crate::disasm_view::DisasmViewConfig::apply_theme_colors`].
    pub fn disasm_view_colors(self) -> DisasmViewColors {
        match self {
            Self::Dark => dark::disasm_view_colors(),
            Self::Light => light::disasm_view_colors(),
        }
    }

    /// Primary window-background colour for this theme — the same
    /// value the theme installs as `StyleColor::WindowBg` in
    /// [`Self::apply_imgui_style`]. Hosts that paint the framebuffer
    /// directly (wgpu / vulkan clear pass) should use this so the
    /// visible page surface stays in sync with whatever ImGui itself
    /// paints into ordinary windows.
    pub fn window_bg(self) -> [f32; 4] {
        // Hex literals match the per-theme `BG` / `BASE` / `NORD0`
        // constants. Inlined here so this accessor doesn't require a
        // dispatch function in every theme module.
        const fn rgb(rgb: u32) -> [f32; 4] {
            [
                ((rgb >> 16) & 0xFF) as f32 / 255.0,
                ((rgb >> 8) & 0xFF) as f32 / 255.0,
                (rgb & 0xFF) as f32 / 255.0,
                1.0,
            ]
        }
        match self {
            Self::Dark => rgb(0x2f343e),
            Self::Light => rgb(0xf1f2f5),
        }
    }

    /// Tab-strip colours synthesized from this theme's nav + status-bar
    /// palettes. Use it to keep [`crate::tab_control::TabControl`] in
    /// the same visual ecosystem as `nav_panel` / `status_bar` —
    /// `tab_strip_bg = nav.bg`, hover/active surfaces from
    /// `nav.btn_hover` / `nav.btn_active`, status indicators
    /// (`status_active` / `_warning` / `_error`) from
    /// `statusbar_colors().{success,warning,error}`.
    ///
    /// Available only with the `tab_control` feature; the
    /// [`crate::tab_control::TabColors`] type lives in that widget.
    #[cfg(feature = "tab_control")]
    pub fn tab_colors(self) -> crate::tab_control::TabColors {
        crate::tab_control::TabColors::from_palettes(&self.nav(), &self.statusbar_colors())
    }

    /// Code-editor syntax palette for this theme. Maps the crate-wide
    /// [`Theme`] selector onto the closest [`crate::code_editor::EditorTheme`]
    /// preset (`Theme::Dark → EditorTheme::DarkDefault`,
    /// `Theme::Light → EditorTheme::GithubLight`) so the editor reads
    /// in the same visual family as the rest of the chrome stack.
    ///
    /// Apply via [`crate::code_editor::EditorConfig::with_crate_theme`]
    /// or [`crate::code_editor::EditorConfig::set_crate_theme`].
    #[cfg(feature = "code_editor")]
    pub fn code_editor_colors(self) -> crate::code_editor::SyntaxColors {
        crate::code_editor::EditorTheme::from_crate_theme(self).colors()
    }

    /// Diff-viewer config for this theme — a [`crate::diff_viewer::DiffViewerConfig`]
    /// with every `color_*` field synthesized from the same `nav` /
    /// `statusbar` / semantic-token palette the rest of the chrome stack
    /// uses, so an added line reads as `theme.success()`, removed as
    /// `theme.danger()`, etc.
    ///
    /// Apply via [`crate::diff_viewer::DiffViewerConfig::with_theme`] or
    /// [`crate::diff_viewer::DiffViewerConfig::apply_theme`] if you need
    /// to keep custom layout overrides.
    #[cfg(feature = "diff_viewer")]
    pub fn diff_viewer_config(self) -> crate::diff_viewer::DiffViewerConfig {
        crate::diff_viewer::DiffViewerConfig::default().with_theme(self)
    }

    /// Force-graph palette for this theme — synthesized via
    /// [`crate::force_graph::style::GraphColors::from_theme`]. The graph
    /// canvas, node fills, edge default + highlight, label text and
    /// box-selection surfaces all stay in the same visual family as the
    /// rest of the chrome stack (background = `window_bg()`, accents
    /// derived from `theme.accent()`).
    ///
    /// Apply via [`crate::force_graph::ViewerConfig::with_theme`].
    #[cfg(feature = "force_graph")]
    pub fn force_graph_colors(self) -> crate::force_graph::style::GraphColors {
        crate::force_graph::style::GraphColors::from_theme(self)
    }

    /// Node-graph palette for this theme — synthesized via
    /// [`crate::node_graph::NgColors::from_theme`]. Canvas, node body /
    /// header / border, pins, wires, selection rect and minimap surfaces
    /// derive from `nav` + `accent` so the editor stays in the same visual
    /// family as the rest of the chrome stack.
    ///
    /// Apply via [`crate::node_graph::NodeGraphConfig::with_theme`] or
    /// [`crate::node_graph::NodeGraphConfig::apply_theme`].
    #[cfg(feature = "node_graph")]
    pub fn node_graph_colors(self) -> crate::node_graph::NgColors {
        crate::node_graph::NgColors::from_theme(self)
    }

    /// Timeline config for this theme — a [`crate::timeline::TimelineConfig`]
    /// with every `color_*` field synthesized from the same `nav` /
    /// `statusbar` / semantic-token palette the rest of the chrome stack
    /// uses. The per-span hue rotation (`span_palette`) stays constant so
    /// flame-chart hues read identically across themes; surfaces,
    /// rulers, labels and tooltips track the theme.
    ///
    /// Apply via [`crate::timeline::TimelineConfig::with_theme`] or
    /// [`crate::timeline::TimelineConfig::apply_theme`] when you need
    /// to keep custom layout overrides.
    #[cfg(feature = "timeline")]
    pub fn timeline_config(self) -> crate::timeline::TimelineConfig {
        crate::timeline::TimelineConfig::default().with_theme(self)
    }

    /// Toolbar config for this theme — a [`crate::toolbar::ToolbarConfig`]
    /// with every `color_*` field synthesized from `nav` so the
    /// horizontal toolbar reads as the same chrome surface as the
    /// vertical [`crate::nav_panel`].
    ///
    /// Apply via [`crate::toolbar::ToolbarConfig::with_theme`] or
    /// [`crate::toolbar::ToolbarConfig::apply_theme`].
    #[cfg(feature = "toolbar")]
    pub fn toolbar_config(self) -> crate::toolbar::ToolbarConfig {
        crate::toolbar::ToolbarConfig::default().with_theme(self)
    }

    /// Property-inspector config for this theme — a
    /// [`crate::property_inspector::InspectorConfig`] with every
    /// `color_*` field synthesized from `nav` + `statusbar` + semantic
    /// tokens, so the key / value / category-header surfaces match the
    /// rest of the chrome stack.
    ///
    /// Apply via [`crate::property_inspector::InspectorConfig::with_theme`]
    /// or [`crate::property_inspector::InspectorConfig::apply_theme`].
    #[cfg(feature = "property_inspector")]
    pub fn inspector_config(self) -> crate::property_inspector::InspectorConfig {
        crate::property_inspector::InspectorConfig::default().with_theme(self)
    }

    /// Apply this theme's Dear ImGui style (rounding + sizing + colours)
    /// to the supplied style object. Call once at startup and any time
    /// after a theme change.
    pub fn apply_imgui_style(self, style: &mut Style) {
        match self {
            Self::Dark => dark::apply_imgui_style(style),
            Self::Light => light::apply_imgui_style(style),
        }
    }

    /// Cycle to the next theme in `Theme::ALL` (wraps around).
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&t| t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Cycle to the previous theme in `Theme::ALL` (wraps around).
    pub fn prev(self) -> Self {
        let n = Self::ALL.len();
        let i = Self::ALL.iter().position(|&t| t == self).unwrap_or(0);
        Self::ALL[(i + n - 1) % n]
    }

    /// Primary brand accent colour — usually the same hue as
    /// `StyleColor::Button`. Useful for tinting custom widgets that aren't
    /// covered by the ImGui style stack.
    pub fn accent(self) -> [f32; 4] {
        match self {
            Self::Dark | Self::Light => [0.36, 0.61, 0.84, 1.0], // ACCENT
        }
    }

    /// Destructive / error accent — the colour shown by `confirm_dialog`'s
    /// Error icon and used for destructive button bgs.
    pub fn danger(self) -> [f32; 4] {
        match self {
            Self::Dark | Self::Light => [0.82, 0.27, 0.27, 1.0],
        }
    }

    /// Success / positive accent — green-family across all themes.
    pub fn success(self) -> [f32; 4] {
        match self {
            Self::Dark | Self::Light => [0.37, 0.72, 0.44, 1.0],
        }
    }

    /// Warning / attention accent — amber/yellow/orange family.
    pub fn warning(self) -> [f32; 4] {
        match self {
            Self::Dark | Self::Light => [0.85, 0.65, 0.25, 1.0],
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod widget_tests;
