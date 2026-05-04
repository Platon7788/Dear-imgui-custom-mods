//! Crate-wide tooltip styling helper.
//!
//! Every custom widget that wants a hover tooltip should go through
//! [`themed_tooltip`] instead of calling [`dear_imgui_rs::Ui::tooltip`]
//! directly. The helper:
//!
//! 1. Pushes `WindowPadding` / `ItemSpacing` / `WindowRounding`
//!    [`dear_imgui_rs::StyleVar`]s before the tooltip body so the
//!    contents always have generous breathing room — independent of
//!    whatever the host theme set globally for ordinary windows.
//! 2. Defers to [`dear_imgui_rs::Ui::tooltip`], which itself wraps
//!    Dear ImGui's `BeginTooltip` / `EndTooltip`. Tooltips opened that
//!    way live in the **top-most popup stack**: nothing else in the
//!    same frame can be drawn over them. They cannot be hidden by
//!    sibling windows, child-windows, or even other popups.
//! 3. Pops the style vars when the helper returns, so the override is
//!    confined to this single tooltip — sibling widgets keep the
//!    host-theme look.
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::utils::tooltip::themed_tooltip;
//!
//! if hovered {
//!     themed_tooltip(ui, || {
//!         ui.text("Offset: 0x42");
//!         ui.text("Hex: 0x90");
//!     });
//! }
//! ```

use dear_imgui_rs::{StyleVar, Ui};

/// Padding inside the tooltip body, in pixels.
const TOOLTIP_PADDING: [f32; 2] = [10.0, 8.0];

/// Vertical gap between successive `ui.text(...)` lines, in pixels.
const TOOLTIP_ITEM_SPACING: [f32; 2] = [8.0, 4.0];

/// Corner rounding of the tooltip frame, in pixels.
const TOOLTIP_ROUNDING: f32 = 4.0;

/// Open a tooltip with the crate-wide visual styling applied.
///
/// `content` runs while three [`StyleVar`] guards are alive:
/// `WindowPadding`, `ItemSpacing`, `WindowRounding`. The guards drop
/// in reverse order at the end of this call, so neighbouring widgets
/// see no style change.
///
/// Equivalent to:
///
/// ```rust,ignore
/// let _p = ui.push_style_var(StyleVar::WindowPadding([10.0, 8.0]));
/// let _s = ui.push_style_var(StyleVar::ItemSpacing([8.0, 4.0]));
/// let _r = ui.push_style_var(StyleVar::WindowRounding(4.0));
/// ui.tooltip(content);
/// ```
pub fn themed_tooltip<F: FnOnce()>(ui: &Ui, content: F) {
    let _pad = ui.push_style_var(StyleVar::WindowPadding(TOOLTIP_PADDING));
    let _spc = ui.push_style_var(StyleVar::ItemSpacing(TOOLTIP_ITEM_SPACING));
    let _rnd = ui.push_style_var(StyleVar::WindowRounding(TOOLTIP_ROUNDING));
    ui.tooltip(content);
}
