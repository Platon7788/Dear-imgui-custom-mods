//! Shared rendering helpers: button color presets, icon-label scratch writer,
//! and the `with_btn_style` style-stack guard.

use std::fmt::Write;

use dear_imgui_rs::{StyleColor, Ui};

use crate::theme;

/// Warm gold text color for folder names in the file table.
pub(super) const CLR_FOLDER_TEXT: [f32; 4] = [0.88, 0.82, 0.55, 1.0];

/// Pack three button state colors into `[base, hovered, active]`.
#[inline]
pub(super) fn btn_colors(base: [f32; 4], hover: [f32; 4], active: [f32; 4]) -> [[f32; 4]; 3] {
    [base, hover, active]
}

/// Navigation button colors (Back, Forward, Up, Refresh) — subtle background.
#[inline]
pub(super) fn nav_btn() -> [[f32; 4]; 3] {
    btn_colors(theme::BG_CHILD, theme::BG_CHILD_HOVER, theme::BG_FRAME)
}

/// Confirm/success button colors (Open, Save, Select Folder, Create, Yes).
#[inline]
pub(super) fn confirm_btn() -> [[f32; 4]; 3] {
    btn_colors(theme::SUCCESS, theme::SUCCESS_HOVER, theme::SUCCESS_ACTIVE)
}

/// Cancel/danger button colors (Cancel, No).
#[inline]
pub(super) fn cancel_btn() -> [[f32; 4]; 3] {
    btn_colors(theme::DANGER, theme::DANGER_HOVER, theme::DANGER_ACTIVE)
}

/// Apply a 3-color button style (base, hovered, active) for the duration of `f`.
pub(super) fn with_btn_style<R>(ui: &Ui, colors: [[f32; 4]; 3], f: impl FnOnce() -> R) -> R {
    let _c0 = ui.push_style_color(StyleColor::Button, colors[0]);
    let _c1 = ui.push_style_color(StyleColor::ButtonHovered, colors[1]);
    let _c2 = ui.push_style_color(StyleColor::ButtonActive, colors[2]);
    f()
}

/// Write `"icon text"` into the scratch buffer, return a borrowed `&str`.
/// Reuses the same allocation across all calls — zero per-frame alloc.
pub(super) fn icon_label<'a>(buf: &'a mut String, icon: &str, text: &str) -> &'a str {
    buf.clear();
    buf.push_str(icon);
    buf.push(' ');
    buf.push_str(text);
    buf.as_str()
}

/// Write `"text##suffix"` into the scratch buffer (used for unique button IDs).
pub(super) fn label_with_id<'a>(buf: &'a mut String, text: &str, id_suffix: &str) -> &'a str {
    buf.clear();
    let _ = write!(buf, "{text}##{id_suffix}");
    buf.as_str()
}
