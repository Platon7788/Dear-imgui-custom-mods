//! UI rendering submodules for the file manager dialog.
//!
//! Each rendering area lives in its own file and is exposed as a free-standing
//! `pub(crate)` function. Free functions instead of methods avoid borrow
//! conflicts: each function takes only the data it needs, allowing
//! [`FileManager`](super::FileManager) to hold disjoint `&mut self` field
//! references simultaneously.
//!
//! ## Render pipeline
//!
//! Called from [`FileManager::render`](super::FileManager::render) in this order:
//!
//! 1. [`render_drive_bar`] — row of drive/root buttons
//! 2. [`render_toolbar`] — Back / Forward / Up / New Folder / New File / Refresh / Hidden
//! 3. [`render_breadcrumb_bar`] — clickable path segments or text input
//! 4. [`render_favorites_panel`] — left sidebar (if enabled)
//! 5. [`render_file_table`] — 4-column ImGui Table with [`ListClipper`](dear_imgui_rs::ListClipper)
//! 6. [`render_footer`] — filter dropdown + Confirm/Cancel buttons
//! 7. [`render_overwrite_confirm`] / [`render_delete_confirm`] — nested modals
//!
//! Each function returns `Option<Action>` (or a tuple including one) for
//! deferred state mutations.

mod breadcrumb;
mod drive_bar;
mod favorites;
mod footer;
mod icon_map;
mod modals;
mod style;
mod table;
mod toolbar;

pub(crate) use breadcrumb::render_breadcrumb_bar;
pub(crate) use drive_bar::render_drive_bar;
pub(crate) use favorites::render_favorites_panel;
pub(crate) use footer::render_footer;
pub(crate) use modals::{render_delete_confirm, render_overwrite_confirm};
pub(crate) use table::{TableCtx, render_file_table};
pub(crate) use toolbar::render_toolbar;
