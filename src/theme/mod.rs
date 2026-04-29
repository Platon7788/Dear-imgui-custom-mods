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
//! - [`midnight`]   — near-black OLED-friendly, Tokyo Night blue accent
//! - [`solarized`]  — Solarized Dark (Ethan Schoonover), warm teal surfaces
//! - [`monokai`]    — Monokai Pro, warm charcoal + neon accents
//! - [`catppuccin`] — Catppuccin Mocha — soft pastel palette
//! - [`nord`]       — Nord — cool desaturated arctic minimalism
//!
//! The color tokens below are legacy constants kept for callers that tint
//! their own widgets by the Dark palette (e.g. `code_editor`, `file_manager`).
//! Prefer the full theme modules above for new code.

// The legacy color constants below are semantic tokens named by their
// intent (BG_WINDOW, ACCENT, …) so a rustdoc description would be
// circular. Opt out of `missing_docs` for this module; the unified
// `Theme` enum (below) is fully documented.
#![allow(missing_docs)]

pub mod catppuccin;
pub mod dark;
pub mod light;
pub mod midnight;
pub mod monokai;
pub mod nord;
pub mod palettes;
pub mod solarized;

// ─── Palette types — single source of truth ─────────────────────────────────
//
// Re-exported from [`palettes`] so the standard import path is
// `crate::theme::TitlebarColors` etc. The consumer modules (
// `app_window`, `confirm_dialog`, `nav_panel`, `notifications`)
// re-export these too for backwards-compatible user paths like
// `crate::theme::TitlebarColors`.
pub use palettes::{
    DialogColors, DisasmFlowKind, DisasmViewColors, HexViewerColors, NavColors, NotificationColors,
    StatusBarColors, TitlebarColors,
};

// ─── Dark theme palette (NxT-inspired) — LEGACY ─────────────────────────────
//
// **Legacy semantic tokens.** Pre-date the [`Theme`] enum below; kept for
// `code_editor` / `file_manager` which still tint widgets by the Dark
// palette directly. New code should use the typed sub-palette accessors:
//   - `Theme::Dark.titlebar()`     for borderless-window colors,
//   - `Theme::Dark.nav()`          for nav panel,
//   - `Theme::Dark.dialog()`       for confirm dialog,
//   - `Theme::Dark.statusbar()`    for status bar,
//   - `Theme::Dark.notifications()` for notification toasts,
//   - `Theme::Dark.apply_imgui_style(style)` to push the full ImGui theme.
//
// `#[deprecated]` is intentionally NOT applied here — that would emit
// hundreds of warnings inside the crate itself. The migration is tracked
// in the v1.0 milestone (per-module palette accessors only).

// Base backgrounds
pub const BG_WINDOW: [f32; 4] = [0.12, 0.13, 0.16, 1.0];
pub const BG_CHILD: [f32; 4] = [0.14, 0.15, 0.19, 1.0];
pub const BG_FRAME: [f32; 4] = [0.16, 0.18, 0.22, 1.0];

// Accent
pub const ACCENT: [f32; 4] = [0.36, 0.61, 0.84, 1.0];
pub const ACCENT_HOVER: [f32; 4] = [0.42, 0.67, 0.90, 1.0];
pub const ACCENT_ACTIVE: [f32; 4] = [0.30, 0.55, 0.78, 1.0];

// Success (green)
pub const SUCCESS: [f32; 4] = [0.37, 0.72, 0.44, 1.0];
pub const SUCCESS_HOVER: [f32; 4] = [0.43, 0.78, 0.50, 1.0];
pub const SUCCESS_ACTIVE: [f32; 4] = [0.31, 0.66, 0.38, 1.0];

// Danger (red)
pub const DANGER: [f32; 4] = [0.82, 0.27, 0.27, 1.0];
pub const DANGER_HOVER: [f32; 4] = [0.88, 0.33, 0.33, 1.0];
pub const DANGER_ACTIVE: [f32; 4] = [0.76, 0.21, 0.21, 1.0];

// Warning (amber/orange)
pub const WARNING: [f32; 4] = [0.85, 0.65, 0.25, 1.0];

// Text
pub const TEXT_PRIMARY: [f32; 4] = [0.88, 0.90, 0.92, 1.0];
pub const TEXT_SECONDARY: [f32; 4] = [0.54, 0.57, 0.63, 1.0];
pub const TEXT_MUTED: [f32; 4] = [0.40, 0.42, 0.48, 1.0];
pub const TEXT_ERROR: [f32; 4] = [0.92, 0.38, 0.35, 1.0];

// Borders / Separators
pub const BORDER: [f32; 4] = [0.25, 0.28, 0.33, 1.0];
pub const SEPARATOR: [f32; 4] = [0.22, 0.25, 0.30, 1.0];

// Backgrounds — interactive
pub const BG_CHILD_HOVER: [f32; 4] = [0.18, 0.20, 0.25, 1.0];

// Selection
pub const SELECTION_BG: [f32; 4] = [0.35, 0.55, 0.80, 0.40];

// ─── Light theme palette ──────────────────────────────────────────────────────

// Base backgrounds
pub const LIGHT_BG_WINDOW: [f32; 4] = [0.96, 0.96, 0.98, 1.0];
pub const LIGHT_BG_CHILD: [f32; 4] = [0.92, 0.92, 0.95, 1.0];
pub const LIGHT_BG_FRAME: [f32; 4] = [0.87, 0.87, 0.91, 1.0];

// Accent
pub const LIGHT_ACCENT: [f32; 4] = [0.18, 0.48, 0.76, 1.0];
pub const LIGHT_ACCENT_HOVER: [f32; 4] = [0.24, 0.56, 0.86, 1.0];
pub const LIGHT_ACCENT_ACTIVE: [f32; 4] = [0.14, 0.40, 0.66, 1.0];

// Status
pub const LIGHT_SUCCESS: [f32; 4] = [0.20, 0.60, 0.28, 1.0];
pub const LIGHT_DANGER: [f32; 4] = [0.80, 0.18, 0.18, 1.0];
pub const LIGHT_WARNING: [f32; 4] = [0.76, 0.52, 0.04, 1.0];

// Text
pub const LIGHT_TEXT_PRIMARY: [f32; 4] = [0.10, 0.10, 0.14, 1.0];
pub const LIGHT_TEXT_SECONDARY: [f32; 4] = [0.36, 0.38, 0.44, 1.0];
pub const LIGHT_TEXT_MUTED: [f32; 4] = [0.55, 0.56, 0.62, 1.0];
pub const LIGHT_TEXT_ERROR: [f32; 4] = [0.82, 0.18, 0.16, 1.0];

// Borders
pub const LIGHT_BORDER: [f32; 4] = [0.70, 0.71, 0.76, 1.0];
pub const LIGHT_SEPARATOR: [f32; 4] = [0.76, 0.77, 0.82, 1.0];

// Selection
pub const LIGHT_SELECTION_BG: [f32; 4] = [0.25, 0.50, 0.82, 0.30];

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
/// (`crate::theme::{dark, light, midnight, solarized, monokai}`). Components
/// take this value by reference and pull the matching sub-palette via the
/// methods below — there is no per-component theme enum any more.
///
/// ```rust,no_run
/// use dear_imgui_custom_mod::theme::Theme;
/// let t = Theme::Dark.next();    // compile-time variants
/// let tb = Theme::Dark.titlebar();
/// let cols = Theme::default().nav();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Theme {
    /// NxT native dark palette (warm grey + blue accent).
    #[default]
    Dark,
    /// Readable light palette with visible borders.
    Light,
    /// Near-black OLED-friendly (Tokyo Night accent).
    Midnight,
    /// Solarized Precision Colors — dark variant.
    Solarized,
    /// Monokai Pro — warm charcoal + neon accents.
    Monokai,
    /// Catppuccin Mocha — soft pastel palette.
    Catppuccin,
    /// Nord — cool desaturated arctic minimalism.
    Nord,
}

impl Theme {
    /// All built-in themes, ordered as they appear in Settings UIs.
    pub const ALL: &'static [Theme] = &[
        Theme::Dark,
        Theme::Light,
        Theme::Midnight,
        Theme::Solarized,
        Theme::Monokai,
        Theme::Catppuccin,
        Theme::Nord,
    ];

    /// Human-readable English name — used in menus / combo boxes.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Midnight => "Midnight",
            Self::Solarized => "Solarized",
            Self::Monokai => "Monokai",
            Self::Catppuccin => "Catppuccin",
            Self::Nord => "Nord",
        }
    }

    /// One-line description shown in Settings tooltips / about dialogs.
    pub fn description(self) -> &'static str {
        match self {
            Self::Dark => "Warm grey with blue accent — NxT default.",
            Self::Light => "Neutral light, optimised for daylight reading.",
            Self::Midnight => "Near-black OLED-friendly with deep Tokyo-Night blues.",
            Self::Solarized => "Schoonover's Solarized Dark — warm teal surfaces.",
            Self::Monokai => "Monokai Pro — charcoal surfaces, neon accents.",
            Self::Catppuccin => "Catppuccin Mocha — soft pastel palette, easy on the eyes.",
            Self::Nord => "Nord — cool, low-contrast arctic minimalism.",
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
    /// [`palettes`] and is always available — the consumer module
    /// `app_window` is feature-gated, but inspecting / building
    /// a `TitlebarColors` does not require it.
    pub fn titlebar(self) -> TitlebarColors {
        match self {
            Self::Dark => dark::titlebar_colors(),
            Self::Light => light::titlebar_colors(),
            Self::Midnight => midnight::titlebar_colors(),
            Self::Solarized => solarized::titlebar_colors(),
            Self::Monokai => monokai::titlebar_colors(),
            Self::Catppuccin => catppuccin::titlebar_colors(),
            Self::Nord => nord::titlebar_colors(),
        }
    }

    /// Nav-panel colours for this theme. Always available — the palette
    /// type lives in [`palettes`].
    pub fn nav(self) -> NavColors {
        match self {
            Self::Dark => dark::nav_colors(),
            Self::Light => light::nav_colors(),
            Self::Midnight => midnight::nav_colors(),
            Self::Solarized => solarized::nav_colors(),
            Self::Monokai => monokai::nav_colors(),
            Self::Catppuccin => catppuccin::nav_colors(),
            Self::Nord => nord::nav_colors(),
        }
    }

    /// Confirm-dialog colours for this theme. Always available — the
    /// palette type lives in [`palettes`].
    pub fn dialog(self) -> DialogColors {
        match self {
            Self::Dark => dark::dialog_colors(),
            Self::Light => light::dialog_colors(),
            Self::Midnight => midnight::dialog_colors(),
            Self::Solarized => solarized::dialog_colors(),
            Self::Monokai => monokai::dialog_colors(),
            Self::Catppuccin => catppuccin::dialog_colors(),
            Self::Nord => nord::dialog_colors(),
        }
    }

    /// Notification-center colours for this theme. Always available —
    /// the palette type lives in [`palettes`].
    pub fn notifications(self) -> NotificationColors {
        match self {
            Self::Dark => NotificationColors::dark(),
            Self::Light => NotificationColors::light(),
            Self::Midnight => NotificationColors::midnight(),
            Self::Solarized => NotificationColors::solarized(),
            Self::Monokai => NotificationColors::monokai(),
            // The new themes don't ship custom notification palettes yet —
            // pick the closest-fitting existing one. Catppuccin's mauve/pink
            // accents read naturally on Monokai's neon stack; Nord's cool
            // greys land closest to Midnight.
            Self::Catppuccin => NotificationColors::monokai(),
            Self::Nord => NotificationColors::midnight(),
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
            Self::Midnight => midnight::statusbar_colors(),
            Self::Solarized => solarized::statusbar_colors(),
            Self::Monokai => monokai::statusbar_colors(),
            Self::Catppuccin => catppuccin::statusbar_colors(),
            Self::Nord => nord::statusbar_colors(),
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
            Self::Midnight => midnight::statusbar_config(),
            Self::Solarized => solarized::statusbar_config(),
            Self::Monokai => monokai::statusbar_config(),
            Self::Catppuccin => catppuccin::statusbar_config(),
            Self::Nord => nord::statusbar_config(),
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
            Self::Midnight => midnight::hex_viewer_colors(),
            Self::Solarized => solarized::hex_viewer_colors(),
            Self::Monokai => monokai::hex_viewer_colors(),
            Self::Catppuccin => catppuccin::hex_viewer_colors(),
            Self::Nord => nord::hex_viewer_colors(),
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
            Self::Midnight => midnight::disasm_view_colors(),
            Self::Solarized => solarized::disasm_view_colors(),
            Self::Monokai => monokai::disasm_view_colors(),
            Self::Catppuccin => catppuccin::disasm_view_colors(),
            Self::Nord => nord::disasm_view_colors(),
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
            Self::Midnight => rgb(0x0e0f13),
            Self::Solarized => rgb(0x002b36),
            Self::Monokai => rgb(0x2d2a2e),
            Self::Catppuccin => rgb(0x1e1e2e),
            Self::Nord => rgb(0x2e3440),
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

    /// Apply this theme's Dear ImGui style (rounding + sizing + colours)
    /// to the supplied style object. Call once at startup and any time
    /// after a theme change.
    pub fn apply_imgui_style(self, style: &mut Style) {
        match self {
            Self::Dark => dark::apply_imgui_style(style),
            Self::Light => light::apply_imgui_style(style),
            Self::Midnight => midnight::apply_imgui_style(style),
            Self::Solarized => solarized::apply_imgui_style(style),
            Self::Monokai => monokai::apply_imgui_style(style),
            Self::Catppuccin => catppuccin::apply_imgui_style(style),
            Self::Nord => nord::apply_imgui_style(style),
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
            Self::Midnight => [0.49, 0.65, 1.0, 1.0],            // Tokyo blue
            Self::Solarized => [0.15, 0.55, 0.82, 1.0],          // BLUE
            Self::Monokai => [0.47, 0.86, 0.91, 1.0],            // CYAN
            Self::Catppuccin => [0.54, 0.71, 0.98, 1.0],         // BLUE
            Self::Nord => [0.53, 0.75, 0.82, 1.0],               // NORD8
        }
    }

    /// Destructive / error accent — the colour shown by `confirm_dialog`'s
    /// Error icon and used for destructive button bgs.
    pub fn danger(self) -> [f32; 4] {
        match self {
            Self::Dark | Self::Light => [0.82, 0.27, 0.27, 1.0],
            Self::Midnight => [0.97, 0.46, 0.50, 1.0],
            Self::Solarized => [0.86, 0.20, 0.18, 1.0],
            Self::Monokai => [1.0, 0.38, 0.53, 1.0], // hot pink
            Self::Catppuccin => [0.95, 0.55, 0.66, 1.0], // pastel red
            Self::Nord => [0.75, 0.38, 0.42, 1.0],   // NORD11
        }
    }

    /// Success / positive accent — green-family across all themes.
    pub fn success(self) -> [f32; 4] {
        match self {
            Self::Dark | Self::Light => [0.37, 0.72, 0.44, 1.0],
            Self::Midnight => [0.62, 0.76, 0.51, 1.0],
            Self::Solarized => [0.52, 0.60, 0.0, 1.0],
            Self::Monokai => [0.66, 0.86, 0.46, 1.0],
            Self::Catppuccin => [0.65, 0.89, 0.63, 1.0],
            Self::Nord => [0.64, 0.75, 0.55, 1.0],
        }
    }

    /// Warning / attention accent — amber/yellow/orange family.
    pub fn warning(self) -> [f32; 4] {
        match self {
            Self::Dark | Self::Light => [0.85, 0.65, 0.25, 1.0],
            Self::Midnight => [0.88, 0.69, 0.32, 1.0],
            Self::Solarized => [0.71, 0.54, 0.0, 1.0],
            Self::Monokai => [0.99, 0.60, 0.40, 1.0],
            Self::Catppuccin => [0.98, 0.70, 0.53, 1.0], // peach
            Self::Nord => [0.82, 0.53, 0.44, 1.0],       // NORD12
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_cycles_through_all_variants() {
        let start = Theme::Dark;
        let mut t = start;
        for _ in 0..Theme::ALL.len() {
            t = t.next();
        }
        assert_eq!(t, start, "Theme::next() should cycle around in N steps");
    }

    #[test]
    fn prev_inverts_next() {
        for &theme in Theme::ALL {
            assert_eq!(
                theme.next().prev(),
                theme,
                "{theme:?}: prev∘next ≠ identity"
            );
            assert_eq!(
                theme.prev().next(),
                theme,
                "{theme:?}: next∘prev ≠ identity"
            );
        }
    }

    #[test]
    fn next_visits_every_variant_exactly_once() {
        let mut t = Theme::Dark;
        let mut seen: Vec<Theme> = vec![t];
        for _ in 0..Theme::ALL.len() - 1 {
            t = t.next();
            seen.push(t);
        }
        for &expected in Theme::ALL {
            assert!(seen.contains(&expected), "{expected:?} missing from cycle");
        }
        assert_eq!(seen.len(), Theme::ALL.len());
    }

    #[test]
    fn display_name_is_unique_per_variant() {
        let names: Vec<&'static str> = Theme::ALL.iter().map(|t| t.display_name()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "display_name collision");
    }

    #[test]
    fn description_is_non_empty_per_variant() {
        for &t in Theme::ALL {
            assert!(!t.description().is_empty(), "{t:?} has empty description");
        }
    }

    #[test]
    fn default_is_dark() {
        assert_eq!(Theme::default(), Theme::Dark);
    }

    #[test]
    fn is_dark_matches_only_light_as_light() {
        assert!(!Theme::Light.is_dark());
        assert!(Theme::Light.is_light());
        for &t in Theme::ALL {
            if t != Theme::Light {
                assert!(t.is_dark(), "{t:?} should be dark");
                assert!(!t.is_light(), "{t:?} should not be light");
            }
        }
    }

    // ── Semantic colour helpers ─────────────────────────────────────────────

    #[test]
    fn semantic_colors_have_full_alpha() {
        // accent / danger / success / warning are intended for opaque tinting
        // — anything below 0.99 is almost certainly a typo.
        for &t in Theme::ALL {
            for (name, c) in [
                ("accent", t.accent()),
                ("danger", t.danger()),
                ("success", t.success()),
                ("warning", t.warning()),
            ] {
                assert!(c[3] >= 0.99, "{t:?}::{name}: alpha {} < 0.99", c[3]);
            }
        }
    }

    #[test]
    fn semantic_hues_are_perceptually_distinct() {
        // accent ≠ danger ≠ success ≠ warning per theme. Compare via raw
        // RGB Euclidean distance. Threshold = 0.15 (Solarized's GREEN /
        // YELLOW pair sits at ~0.20 by Schoonover's design — both anchored
        // near the yellow-green band).
        fn dist(a: [f32; 4], b: [f32; 4]) -> f32 {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        }
        const MIN: f32 = 0.15;
        for &t in Theme::ALL {
            let pairs = [
                ("accent vs danger", t.accent(), t.danger()),
                ("accent vs success", t.accent(), t.success()),
                ("danger vs success", t.danger(), t.success()),
                ("danger vs warning", t.danger(), t.warning()),
                ("success vs warning", t.success(), t.warning()),
            ];
            for (label, a, b) in pairs {
                let d = dist(a, b);
                assert!(d > MIN, "{t:?}: {label} distance {d:.3} ≤ {MIN}");
            }
        }
    }

    // ── WCAG contrast (informational baseline — not strict AA) ─────────────
    //
    // Computes W3C "relative luminance" + contrast ratio for the primary
    // text-on-window-bg pair sourced from `titlebar()` (under the
    // `app_window` feature). The threshold is set to 4.5 ("AA body
    // text") for the Light theme and 3.0 (relaxed for dark dim contrasts
    // — many popular dark themes flunk strict AA but read well in practice)
    // for everything else. Catches accidental black-on-black regressions.

    #[cfg(feature = "app_window")]
    fn relative_luminance(c: [f32; 4]) -> f32 {
        // sRGB → linear conversion per WCAG 2.x.
        fn lin(c: f32) -> f32 {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        let r = lin(c[0]);
        let g = lin(c[1]);
        let b = lin(c[2]);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    #[cfg(feature = "app_window")]
    fn contrast_ratio(a: [f32; 4], b: [f32; 4]) -> f32 {
        let la = relative_luminance(a);
        let lb = relative_luminance(b);
        let (l1, l2) = if la >= lb { (la, lb) } else { (lb, la) };
        (l1 + 0.05) / (l2 + 0.05)
    }

    #[cfg(feature = "app_window")]
    #[test]
    fn primary_text_meets_minimum_contrast() {
        // Floor levels chosen as regression detectors, NOT strict WCAG
        // compliance. Titlebar title text is allowed to be muted by design
        // (Solarized's BASE00 on BASE02 sits at ~2.9 — Schoonover
        // intentionally desaturates chrome) — we just want to flag any
        // accidental black-on-black or near-identical-luminance pairs.
        for &theme in Theme::ALL {
            let palette = theme.titlebar();
            let ratio = contrast_ratio(palette.title, palette.bg);
            let min = if theme == Theme::Light { 4.5 } else { 2.5 };
            assert!(
                ratio >= min,
                "{theme:?}: title-on-bg contrast {ratio:.2} < {min}",
            );
        }
    }

    #[cfg(feature = "app_window")]
    #[test]
    fn button_glyphs_pop_against_titlebar_bg() {
        // The per-button accent palette (amber/cyan/red) MUST stand out
        // from the titlebar background — if a future tweak made
        // `btn_close ≈ bg`, the close button would silently disappear.
        for &theme in Theme::ALL {
            let p = theme.titlebar();
            for (label, glyph) in [
                ("minimize", p.btn_minimize),
                ("maximize", p.btn_maximize),
                ("close", p.btn_close),
            ] {
                let ratio = contrast_ratio(glyph, p.bg);
                assert!(
                    ratio >= 1.8,
                    "{theme:?}: {label} glyph contrast {ratio:.2} < 1.8 — too subtle",
                );
            }
        }
    }

    #[cfg(feature = "tab_control")]
    #[test]
    fn tab_colors_track_nav_and_statusbar() {
        // `Theme::tab_colors()` must compose tab strip surfaces from
        // the nav + status-bar palettes for that theme — keeps the
        // chrome ecosystem coherent. Pin the wiring so an accidental
        // edit of either palette doesn't desync the tab strip.
        for &theme in Theme::ALL {
            let nav = theme.nav();
            let sb = theme.statusbar_colors();
            let tabs = theme.tab_colors();
            let to_u8 = |c: [f32; 4]| {
                [
                    (c[0] * 255.0).round().clamp(0.0, 255.0) as u8,
                    (c[1] * 255.0).round().clamp(0.0, 255.0) as u8,
                    (c[2] * 255.0).round().clamp(0.0, 255.0) as u8,
                ]
            };
            assert_eq!(tabs.strip_bg, to_u8(nav.bg), "{theme:?}: strip_bg");
            assert_eq!(tabs.tab_hover, to_u8(nav.btn_hover), "{theme:?}: tab_hover");
            assert_eq!(tabs.text, to_u8(nav.icon_active), "{theme:?}: text");
            assert_eq!(
                tabs.text_muted,
                to_u8(nav.icon_default),
                "{theme:?}: text_muted"
            );
            assert_eq!(tabs.separator, to_u8(nav.separator), "{theme:?}: separator");
            assert_eq!(
                tabs.status_active,
                to_u8(sb.success),
                "{theme:?}: status_active"
            );
            assert_eq!(
                tabs.status_error,
                to_u8(sb.error),
                "{theme:?}: status_error"
            );
        }
    }

    #[test]
    fn hex_viewer_colors_resolve_for_every_theme() {
        // Every theme must produce a non-default-zeroed
        // `HexViewerColors`. Pin a few invariants so a regression in
        // `HexViewerColors::from_tokens` (or a token mistype in a
        // theme module) surfaces as a fail with the offending theme
        // name rather than a silently broken visual.
        for &theme in Theme::ALL {
            let p = theme.hex_viewer_colors();
            assert!(p.offset[3] > 0.0, "{theme:?}: offset alpha = 0");
            assert!(
                p.cat_printable[3] > 0.0,
                "{theme:?}: cat_printable alpha = 0"
            );
            // Offset and category-printable are different hues
            // (accent vs success) — they must not collapse to the
            // same RGB or the visual hierarchy of the gutter
            // disappears.
            assert_ne!(
                [p.offset[0], p.offset[1], p.offset[2]],
                [p.cat_printable[0], p.cat_printable[1], p.cat_printable[2]],
                "{theme:?}: offset and cat_printable share a hue",
            );
        }
    }

    #[test]
    fn disasm_view_colors_resolve_for_every_theme() {
        // Every theme must produce a non-default-zeroed
        // `DisasmViewColors`. Pin the per-flow uniqueness invariant —
        // call (success), jump (warning), return (danger), stack
        // (purple) and system (orange) MUST land on distinct hues so
        // the syntax highlighting actually reads.
        for &theme in Theme::ALL {
            let p = theme.disasm_view_colors();
            assert!(p.address[3] > 0.0, "{theme:?}: address alpha = 0");
            assert!(
                p.mnemonic_normal[3] > 0.0,
                "{theme:?}: mnemonic_normal alpha = 0"
            );
            assert!(!p.block_tints.is_empty(), "{theme:?}: empty block_tints");
            assert!(
                !p.breakpoint_colors.is_empty(),
                "{theme:?}: empty breakpoint_colors",
            );
            // Per-FlowKind mnemonic colours must be distinct so a jump
            // (yellow) does not visually collide with a call (green) or
            // return (red).
            let rgb = |c: [f32; 4]| [c[0], c[1], c[2]];
            assert_ne!(
                rgb(p.mnemonic_call),
                rgb(p.mnemonic_jump),
                "{theme:?}: call and jump share a hue",
            );
            assert_ne!(
                rgb(p.mnemonic_call),
                rgb(p.mnemonic_return),
                "{theme:?}: call and return share a hue",
            );
            assert_ne!(
                rgb(p.mnemonic_stack),
                rgb(p.mnemonic_system),
                "{theme:?}: stack and system share a hue",
            );
        }
    }

    #[test]
    fn disasm_view_colors_default_matches_dark_theme() {
        // Bare `palettes::DisasmViewColors::default()` must mirror
        // `Theme::Dark.disasm_view_colors()` — that's the value
        // `DisasmViewConfig::default()` ends up with when the caller
        // never goes through the `Theme` system.
        let default = palettes::DisasmViewColors::default();
        let dark = Theme::Dark.disasm_view_colors();
        assert_eq!(default.mnemonic_normal, dark.mnemonic_normal);
        assert_eq!(default.mnemonic_call, dark.mnemonic_call);
        assert_eq!(default.address, dark.address);
        assert_eq!(default.selection_bg, dark.selection_bg);
        assert_eq!(default.breakpoint, dark.breakpoint);
        assert_eq!(default.block_tints.len(), dark.block_tints.len());
    }

    #[test]
    fn hex_viewer_colors_default_matches_dark_theme() {
        // `HexViewerColors::default()` must be `Theme::Dark.hex_viewer_colors()`
        // exactly, so a bare `HexViewerConfig::default()` that
        // doesn't go through `Theme` still matches the rest of the
        // Dark chrome stack.
        let default = palettes::HexViewerColors::default();
        let dark = Theme::Dark.hex_viewer_colors();
        assert_eq!(default.offset, dark.offset);
        assert_eq!(default.cat_printable, dark.cat_printable);
        assert_eq!(default.hex, dark.hex);
        assert_eq!(default.cursor_bg, dark.cursor_bg);
        assert_eq!(default.changed, dark.changed);
    }

    #[test]
    fn statusbar_colors_default_matches_dark_theme() {
        // `StatusBarColors::default()` is the value `StatusBarConfig::default()`
        // hands out when the caller hasn't gone through `Theme`. It must
        // mirror `Theme::Dark.statusbar_colors()` exactly, otherwise a
        // status bar built from defaults will read as a different shade
        // than every neighbour palette (nav, titlebar, dialog) in the
        // same Dark stack.
        let default = palettes::StatusBarColors::default();
        let dark = Theme::Dark.statusbar_colors();
        assert_eq!(default.bg, dark.bg, "bg drift");
        assert_eq!(default.text, dark.text, "text drift");
        assert_eq!(default.text_dim, dark.text_dim, "text_dim drift");
        assert_eq!(default.separator, dark.separator, "separator drift");
        assert_eq!(default.hover, dark.hover, "hover drift");
        assert_eq!(default.active, dark.active, "active drift");
    }

    #[cfg(feature = "status_bar")]
    #[test]
    fn nav_and_statusbar_palettes_are_in_sync() {
        // The two strip-style chrome surfaces (vertical nav, horizontal
        // status) should read as one cohesive ecosystem — they share
        // the same surface tokens (background, separator, primary /
        // muted text). This test pins that contract per theme so a
        // future tweak to one palette can't silently desync the other.
        for &theme in Theme::ALL {
            let nav = theme.nav();
            let sb = theme.statusbar_colors();
            assert_eq!(nav.bg, sb.bg, "{theme:?}: nav.bg ≠ statusbar.bg");
            assert_eq!(
                nav.separator, sb.separator,
                "{theme:?}: nav.separator ≠ statusbar.separator",
            );
            assert_eq!(
                nav.icon_active, sb.text,
                "{theme:?}: nav.icon_active ≠ statusbar.text",
            );
            assert_eq!(
                nav.icon_default, sb.text_dim,
                "{theme:?}: nav.icon_default ≠ statusbar.text_dim",
            );
        }
    }
}
