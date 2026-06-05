//! Unit + property tests for the `hex_viewer` module.
//!
//! Tests reach into private fields (`v.cursor`, `v.selection`, etc.)
//! through `pub(super)` visibility — no API surface is exposed just
//! for testing. Free helper functions (`parse_address`, `format_bytes`,
//! …) are imported from their respective sub-modules.
//!
//! Split across themed files so each stays under the 500-line ceiling
//! (declared via `#[cfg(test)] mod tests;` in `mod.rs`):
//! - [`search_tests`]  — address / pattern parsing, search, encodings,
//!   clipboard formatting, parser property tests.
//! - [`edit_tests`]    — undo/redo, nav history, pending-nibble commit,
//!   cursor movement / select-all.
//! - [`layout_tests`]  — address-width, column geometry, address-literal
//!   formatting, inspector-height accessors, byte-colour overrides.
//! - [`config_tests`]  — config defaults / ron round-trip / locale
//!   guards, providers, byte category, popup-trigger API.

mod config_tests;
mod edit_tests;
mod layout_tests;
mod search_tests;
