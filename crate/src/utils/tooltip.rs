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

use dear_imgui_rs::{StyleColor, StyleVar, Ui};

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

/// Themed tooltip with hard size constraints — the tooltip window is
/// forced into a `[min_size, max_size]` box before Dear ImGui measures
/// its content. Useful for tooltips carrying large blocks of text (like
/// packet-structure dumps) that would otherwise auto-fit to a size
/// bigger than the viewport and get clipped by the OS status bar.
///
/// Combined with a scrollable child inside `content`, this guarantees
/// every line stays reachable regardless of packet size.
///
/// `min_size` / `max_size` are in **pixels**. Pass `f32::MAX` (or any
/// large sentinel) on an axis you want unconstrained. ImGui will still
/// place the tooltip so it doesn't overflow the viewport — moving it
/// above / left of the cursor when it doesn't fit below / right.
pub fn sized_tooltip<F: FnOnce()>(
    ui: &Ui,
    min_size: [f32; 2],
    max_size: [f32; 2],
    content: F,
) {
    // SAFETY: `igSetNextWindowSizeConstraints` records the constraint
    // for the NEXT `Begin*` (in our case: the `BeginTooltip` inside
    // `ui.tooltip`). Both `ImVec2_c` fields are plain floats with no
    // invariants; the callback pointer is `None` and the userdata is
    // null. Single-threaded UI, no cross-thread state access.
    unsafe {
        dear_imgui_rs::sys::igSetNextWindowSizeConstraints(
            dear_imgui_rs::sys::ImVec2_c {
                x: min_size[0],
                y: min_size[1],
            },
            dear_imgui_rs::sys::ImVec2_c {
                x: max_size[0],
                y: max_size[1],
            },
            None,
            std::ptr::null_mut(),
        );
    }
    themed_tooltip(ui, content);
}

/// Translucency the tooltip's background is pushed to. Matches the alpha
/// used by the NxT license modal (`crates/ui/src/license_dialog.rs:96`)
/// so tooltips read as part of the same visual family as other floating
/// panels — you can see the editor bg peeking through faintly instead
/// of a flat opaque plate.
const TOOLTIP_POPUP_BG: [f32; 4] = [0.13, 0.15, 0.17, 0.92];

/// Chrome-aware tooltip — flips **above** the cursor when there isn't
/// enough room below, and never lets its bottom edge intersect the
/// caller's own reserved chrome (e.g. a status bar the host draws below
/// Dear ImGui's viewport).
///
/// Placement logic:
/// - `reserved_bottom_px` tells the picker how many pixels at the bottom
///   of the viewport are perpetually covered by host chrome. Dear ImGui
///   does NOT know about them, so its default "grow down until clip"
///   would land under that chrome.
/// - If the cursor is closer to the bottom than `min_flip_up_px`, the
///   tooltip anchors with `pivot = [0, 1]` — its bottom-left snaps to
///   the cursor and the box unfolds upward.
/// - The `max_size[1]` constraint is tightened to whichever side
///   (`above` / `below`) the tooltip is unfolding into, so a huge
///   packet dump still fits and the tooltip's own scrollable child
///   handles anything larger.
///
/// The background is pushed to a translucent [`TOOLTIP_POPUP_BG`] for
/// the duration of the tooltip so hosts don't need to know about the
/// alpha.
///
/// # Parameters
/// - `min_size` — floor for the tooltip's constrained size.
/// - `max_width` — cap on tooltip width; height cap is derived from
///   free space on the chosen side.
/// - `reserved_bottom_px` — height of any host chrome below the
///   ImGui viewport (0 when there isn't any).
/// - `min_flip_up_px` — threshold: if `free_below < this`, flip up.
///   Also acts as the minimum comfortable height for a "grow-down"
///   tooltip — passes below that, we assume the tooltip won't fit
///   comfortably and go up instead.
/// - `content` — the tooltip body.
pub fn smart_positioned_tooltip<F: FnOnce()>(
    ui: &Ui,
    min_size: [f32; 2],
    max_width: f32,
    reserved_bottom_px: f32,
    min_flip_up_px: f32,
    content: F,
) {
    let [mx, my] = ui.io().mouse_pos();
    let [_vw, vh] = ui.io().display_size();

    // Effective vertical envelope Dear ImGui may place the tooltip in.
    // Trim the reserved band off the bottom so we never overlap the
    // host chrome ImGui doesn't know about.
    let usable_bottom = (vh - reserved_bottom_px.max(0.0)).max(0.0);
    let free_below = (usable_bottom - my).max(0.0);
    let free_above = my.max(0.0);

    let flip_up = free_below < min_flip_up_px && free_above > free_below;

    // Compute the height budget on whichever side we chose. Reserve a
    // small margin on each end so the tooltip never sits flush against
    // the viewport / chrome edge.
    let side_margin = 8.0;
    let max_h = if flip_up {
        (free_above - side_margin).max(min_size[1].max(80.0))
    } else {
        (free_below - side_margin).max(min_size[1].max(80.0))
    };

    // SAFETY: single-frame side-effects only; both calls are recorded
    // for the *next* `Begin*` (the BeginTooltip inside `ui.tooltip`).
    // All values are plain `f32`; single-threaded UI.
    unsafe {
        if flip_up {
            // Anchor: cursor position, pivot bottom-left → the box
            // grows upward and rightward from the cursor. A small
            // vertical nudge keeps the top of the tooltip a hair above
            // the cursor glyph so it doesn't touch the caret.
            #[allow(clippy::unnecessary_cast)]
            // `ImGuiCond_Always` is `u32` on Linux / `i32` on Windows.
            dear_imgui_rs::sys::igSetNextWindowPos(
                dear_imgui_rs::sys::ImVec2 {
                    x: mx,
                    y: (my - 4.0).max(0.0),
                },
                dear_imgui_rs::sys::ImGuiCond_Always as i32,
                dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 1.0 },
            );
        }
        dear_imgui_rs::sys::igSetNextWindowSizeConstraints(
            dear_imgui_rs::sys::ImVec2_c {
                x: min_size[0],
                y: min_size[1],
            },
            dear_imgui_rs::sys::ImVec2_c {
                x: max_width,
                y: max_h,
            },
            None,
            std::ptr::null_mut(),
        );
    }

    let _bg = ui.push_style_color(StyleColor::PopupBg, TOOLTIP_POPUP_BG);
    themed_tooltip(ui, content);
}
