//! Theme module — per-theme bundles + shared color tokens.
//!
//! Each built-in theme lives in its own file and owns the full stack for
//! that theme: titlebar colors, nav colors, dialog colors, status-bar
//! config, and the Dear ImGui style palette. This keeps the "one theme =
//! one file" rule so a single change stays contained.
//!
//! Module layout — one theme per file, each owning a full stack:
//! - [`dark`]      — NxT native dark palette (default)
//! - [`light`]     — readable light palette with clearly visible borders
//! - [`midnight`]  — near-black OLED-friendly, Tokyo Night blue accent
//! - [`solarized`] — Solarized Dark (Ethan Schoonover), warm teal surfaces
//! - [`monokai`]   — Monokai Pro, warm charcoal + neon accents
//!
//! The color tokens below are legacy constants kept for callers that tint
//! their own widgets by the Dark palette (e.g. `code_editor`, `file_manager`).
//! Prefer the full theme modules above for new code.

pub mod dark;
pub mod light;
pub mod midnight;
pub mod monokai;
pub mod solarized;

// ─── Dark theme palette (NxT-inspired) ──────────────────────────────────────

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
pub const LIGHT_BG_CHILD: [f32; 4]  = [0.92, 0.92, 0.95, 1.0];
pub const LIGHT_BG_FRAME: [f32; 4]  = [0.87, 0.87, 0.91, 1.0];

// Accent
pub const LIGHT_ACCENT: [f32; 4]        = [0.18, 0.48, 0.76, 1.0];
pub const LIGHT_ACCENT_HOVER: [f32; 4]  = [0.24, 0.56, 0.86, 1.0];
pub const LIGHT_ACCENT_ACTIVE: [f32; 4] = [0.14, 0.40, 0.66, 1.0];

// Status
pub const LIGHT_SUCCESS: [f32; 4] = [0.20, 0.60, 0.28, 1.0];
pub const LIGHT_DANGER:  [f32; 4] = [0.80, 0.18, 0.18, 1.0];
pub const LIGHT_WARNING: [f32; 4] = [0.76, 0.52, 0.04, 1.0];

// Text
pub const LIGHT_TEXT_PRIMARY:   [f32; 4] = [0.10, 0.10, 0.14, 1.0];
pub const LIGHT_TEXT_SECONDARY: [f32; 4] = [0.36, 0.38, 0.44, 1.0];
pub const LIGHT_TEXT_MUTED:     [f32; 4] = [0.55, 0.56, 0.62, 1.0];
pub const LIGHT_TEXT_ERROR:     [f32; 4] = [0.82, 0.18, 0.16, 1.0];

// Borders
pub const LIGHT_BORDER:    [f32; 4] = [0.70, 0.71, 0.76, 1.0];
pub const LIGHT_SEPARATOR: [f32; 4] = [0.76, 0.77, 0.82, 1.0];

// Selection
pub const LIGHT_SELECTION_BG: [f32; 4] = [0.25, 0.50, 0.82, 0.30];

// ─── Theme selector ──────────────────────────────────────────────────────────

/// Select which built-in global color palette to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Dark NxT-inspired palette (default).
    #[default]
    Dark,
    /// Light Material-inspired palette.
    Light,
}
