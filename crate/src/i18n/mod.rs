//! Locale + per-module string catalogues.
//!
//! Single source for English / Russian text in widgets that render
//! user-facing labels, tooltips, popups, and context menus. Each
//! widget owns a `Locale` field (default [`Locale::En`]) which a
//! host can override at construction:
//!
//! ```rust,no_run
//! # use dear_imgui_custom_mod::i18n::Locale;
//! # use dear_imgui_custom_mod::hex_viewer::HexViewer;
//! let viewer = HexViewer::new("dump").with_locale(Locale::Ru);
//! ```
//!
//! Catalogues are static `&'static Strings` values resolved by a single
//! `match` — zero allocation, zero indirection, zero runtime overhead.
//!
//! ## Module layout
//!
//! This is a directory module: the [`Locale`] enum lives here in
//! `i18n/mod.rs`, and each widget owns its own file
//! (`i18n/<widget>.rs`) re-exported below via `pub mod <widget>;`.
//! The public paths (`crate::i18n::hex_viewer::strings`, etc.) are
//! unchanged by this split — `pub mod` keeps them byte-identical for
//! every external consumer.
//!
//! ## Adding a string
//!
//! 1. Add a `pub const_field: &'static str` to the relevant
//!    `Strings` struct in `i18n/<widget>.rs`.
//! 2. Add the same key to **both** `EN` and `RU` constants.
//! 3. Use `widget.strings().const_field` at the call site instead of
//!    a literal.
//! 4. Add an entry to the parity table in `i18n/tests.rs`.
//!
//! Format-template strings (`"Result {n}/{m}"`) live as helper
//! functions — see [`hex_viewer::result_n_of_m`].

use serde::{Deserialize, Serialize};

pub mod code_editor;
pub mod confirm_dialog;
pub mod diff_viewer;
pub mod disasm_view;
pub mod force_graph;
pub mod hex_viewer;
pub mod nav_panel;
pub mod timeline;

#[cfg(test)]
mod tests;

/// User-visible language. Default English.
///
/// `serde` for ron round-trip in saved widget configs (so a saved
/// `HexViewerConfig` ron file remembers the locale).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Locale {
    /// English. The default — no extra glyph ranges required, every
    /// label fits in Basic Latin.
    #[default]
    En,
    /// Russian. Requires the host to bake `GlyphRanges::Cyrillic`
    /// (or a superset) into the active font atlas — otherwise
    /// non-ASCII characters render as `?` placeholders.
    Ru,
}

impl Locale {
    /// Short tag (`"en"` / `"ru"`) — useful for logs / debug overlays.
    pub fn tag(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ru => "ru",
        }
    }
}
