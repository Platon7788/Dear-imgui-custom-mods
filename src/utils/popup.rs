//! Crate-wide popup styling + small layout helpers.
//!
//! Sibling of [`super::tooltip`]: tooltips and popups share the
//! "breathing room" design philosophy but live in semantically
//! distinct spaces — tooltips are read-only floating annotations,
//! popups host interactive widgets (input fields, buttons, menu
//! items). Each gets its own helper file so a future tweak to
//! popup geometry can't accidentally drift the tooltip look (and
//! vice versa).
//!
//! Two flavours of helper:
//!
//! - [`themed_popup_style`] — pushes the standard StyleVar guards
//!   (window padding, item spacing, frame padding, rounding) for
//!   the duration of a body closure. Wrap your `BeginPopup` /
//!   `BeginPopupModal` body with it so the popup matches the rest
//!   of the chrome stack.
//! - [`button_with_color`] / [`success_button`] / [`danger_button`]
//!   — tiny convenience wrappers around `ui.button` that push a
//!   semantic colour stack (Button + Hovered + Active) before
//!   rendering. Save the 3 lines of `push_style_color` boilerplate
//!   per button and keep the green / red shades consistent across
//!   every popup that uses them.
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::utils::popup::{themed_popup_style, success_button};
//!
//! themed_popup_style(ui, || {
//!     if let Some(_popup) = ui.begin_popup(&id) {
//!         ui.text("Save changes?");
//!         if success_button(ui, "Save", [80.0, 0.0]) { /* … */ }
//!     }
//! });
//! ```

use dear_imgui_rs::{StyleColor, StyleVar, Ui};

// ── Popup geometry ──────────────────────────────────────────────────────────

/// Padding inside popup bodies — slightly more generous than the
/// tooltip variant because popups host interactive widgets (input
/// fields, buttons) that need room to breathe.
const POPUP_PADDING: [f32; 2] = [14.0, 12.0];

/// Vertical gap between rows inside a popup. Bigger than tooltip's
/// `4 px` so menu items don't read as glued together.
const POPUP_ITEM_SPACING: [f32; 2] = [10.0, 8.0];

/// Frame padding for buttons / inputs inside a popup. Wider than
/// the global default so the actionable widgets feel finger-sized
/// even on a dense theme.
const POPUP_FRAME_PADDING: [f32; 2] = [10.0, 6.0];

/// Corner rounding shared by the popup window AND its frame
/// widgets so the visual rhythm stays consistent.
const POPUP_ROUNDING: f32 = 5.0;

/// Push the crate-wide popup style vars (padding / spacing /
/// rounding / frame padding / frame rounding) for the duration
/// of `body`. Use this to wrap any `BeginPopup` / `BeginPopupModal`
/// body so popups across the codebase look consistent.
///
/// ```rust,ignore
/// themed_popup_style(ui, || {
///     if let Some(_popup) = ui.begin_popup(&id) {
///         ui.text("…");
///         if ui.button("OK") { ui.close_current_popup(); }
///     }
/// });
/// ```
pub fn themed_popup_style<F: FnOnce()>(ui: &Ui, body: F) {
    let _pad = ui.push_style_var(StyleVar::WindowPadding(POPUP_PADDING));
    let _spc = ui.push_style_var(StyleVar::ItemSpacing(POPUP_ITEM_SPACING));
    let _fp = ui.push_style_var(StyleVar::FramePadding(POPUP_FRAME_PADDING));
    let _wr = ui.push_style_var(StyleVar::WindowRounding(POPUP_ROUNDING));
    let _fr = ui.push_style_var(StyleVar::FrameRounding(POPUP_ROUNDING));
    body();
}

// ── Coloured buttons ────────────────────────────────────────────────────────

/// Semantic green — confirm / save / "go ahead". Tuned to read on
/// every built-in theme without going neon.
pub const GREEN: [f32; 4] = [0.18, 0.52, 0.35, 1.0];

/// Semantic red — destructive / delete / "abort with damage". Same
/// hue family as `confirm_dialog`'s confirm-red so destructive
/// affordances stay visually identical across the app.
pub const RED: [f32; 4] = [0.70, 0.22, 0.22, 1.0];

/// Semantic blue — "selected" / "active option" state for
/// pseudo-radio button rows (mode-toggle pills, BPR pickers, etc.).
/// Reads as a clear "this one is picked" cue without the implication
/// of confirm (green) or destructive (red).
pub const BLUE: [f32; 4] = [0.30, 0.50, 0.90, 1.0];

/// Hover / active deltas for the auto-derived button stack. `+0.06`
/// per channel for hover (slightly brighter), `-0.06` for active
/// (slightly darker). Channels clamped to `0.0..=1.0` by [`lift`].
const HOVER_DELTA: f32 = 0.06;
const ACTIVE_DELTA: f32 = -0.06;

/// Lift each RGB channel of `c` by `d`, clamped to `0.0..=1.0`. Alpha
/// passes through. Used to derive hover / active button shades from a
/// single base colour — single source of truth for the colour stack
/// so [`success_button`] / [`danger_button`] / [`button_with_color`]
/// can't drift apart.
fn lift(c: [f32; 4], d: f32) -> [f32; 4] {
    [
        (c[0] + d).clamp(0.0, 1.0),
        (c[1] + d).clamp(0.0, 1.0),
        (c[2] + d).clamp(0.0, 1.0),
        c[3],
    ]
}

/// Internal: push the 3-colour Button stack derived from `base`,
/// then run `body`. Centralises the StyleColor push/pop sequence so
/// [`button_with_color`] / [`success_button`] / [`danger_button`]
/// share the exact same hover / active derivation.
fn with_button_stack<R>(ui: &Ui, base: [f32; 4], body: impl FnOnce() -> R) -> R {
    let _b = ui.push_style_color(StyleColor::Button, base);
    let _h = ui.push_style_color(StyleColor::ButtonHovered, lift(base, HOVER_DELTA));
    let _a = ui.push_style_color(StyleColor::ButtonActive, lift(base, ACTIVE_DELTA));
    body()
}

/// Internal: dispatch to `button_with_size` for non-zero sizes, fall
/// back to auto-sizing `button` otherwise. Same convention as
/// `button_with_color` and friends.
fn button_or_sized(ui: &Ui, label: &str, size: [f32; 2]) -> bool {
    if size[0] == 0.0 && size[1] == 0.0 {
        ui.button(label)
    } else {
        ui.button_with_size(label, size)
    }
}

/// Render a button with an explicit `[r, g, b, a]` background
/// (plus auto-derived hover / active variants — `+0.06` and
/// `-0.06` per channel respectively, clamped to `0..1`). Returns
/// the same `bool` as [`Ui::button_with_size`] / [`Ui::button`].
///
/// `size = [0.0, 0.0]` falls back to ImGui's auto-sizing.
pub fn button_with_color(ui: &Ui, label: &str, color: [f32; 4], size: [f32; 2]) -> bool {
    with_button_stack(ui, color, || button_or_sized(ui, label, size))
}

/// Confirm / "Go" / "OK" button — green semantic background. The
/// hover / active shades are derived from [`GREEN`] via the same
/// `±0.06` lift as [`button_with_color`], so the two helpers can't
/// drift out of sync when the base palette is tuned.
/// `size = [0.0, 0.0]` for auto-fit.
pub fn success_button(ui: &Ui, label: &str, size: [f32; 2]) -> bool {
    with_button_stack(ui, GREEN, || button_or_sized(ui, label, size))
}

/// Destructive / "Delete" / "Abort" button — red semantic background.
/// `size = [0.0, 0.0]` for auto-fit.
pub fn danger_button(ui: &Ui, label: &str, size: [f32; 2]) -> bool {
    with_button_stack(ui, RED, || button_or_sized(ui, label, size))
}

/// Pseudo-radio "selected option" pill — when `selected` is true,
/// pushes the [`BLUE`] semantic colour stack via the same unified
/// `lift()` derivation as [`success_button`] / [`danger_button`].
/// When `selected` is false, renders as a plain unstyled button.
/// Returns `true` on click.
///
/// Use for inline mode toggles (Hex / String, ASCII / UTF-8 / UTF-16LE,
/// bytes-per-row size pickers, etc.) where a full Combo widget would
/// be overkill for a 2–7 option set.
///
/// `size = [0.0, 0.0]` for auto-fit.
pub fn selected_button(ui: &Ui, label: &str, size: [f32; 2], selected: bool) -> bool {
    if selected {
        with_button_stack(ui, BLUE, || button_or_sized(ui, label, size))
    } else {
        button_or_sized(ui, label, size)
    }
}

// ── Layout helpers ──────────────────────────────────────────────────────────

/// Compact-body StyleVar guards shared by every "modal-style" popup
/// in the crate (goto / search / settings / context-style menus).
/// Pushes a tighter ruleset over an already-`themed_popup_style`'d
/// popup body so option rows / checklists don't read as towering.
///
/// - `ItemSpacing`      `[6, 4]`  (row gap)
/// - `FramePadding`     `[6, 3]`  (button / input padding)
/// - `ItemInnerSpacing` `[4, 4]`  (between checkbox square and label)
///
/// Pop order is reverse-LIFO when `body` returns — no leakage to
/// sibling popups.
///
/// ```rust,ignore
/// crate::utils::popup::themed_popup_style(ui, || {
///     if let Some(_p) = ui.begin_popup(&id) {
///         crate::utils::popup::compact_popup_body(ui, || {
///             ui.checkbox("Show ASCII", &mut cfg.show_ascii);
///             ui.checkbox("Uppercase",  &mut cfg.uppercase);
///         });
///     }
/// });
/// ```
pub fn compact_popup_body<F: FnOnce()>(ui: &Ui, body: F) {
    let _spc = ui.push_style_var(StyleVar::ItemSpacing([6.0, 4.0]));
    let _fp = ui.push_style_var(StyleVar::FramePadding([6.0, 3.0]));
    let _ip = ui.push_style_var(StyleVar::ItemInnerSpacing([4.0, 4.0]));
    body();
}

/// Action-row layout shared by goto / search popups: `Cancel` pinned
/// `2 px` from the popup's left content edge, the green primary
/// button pinned the same distance from the right edge. Returns
/// `(primary_clicked, cancel_clicked)` so each popup body can
/// dispatch its own logic without re-implementing the geometry.
///
/// `body_w` is the reference width for the right anchor (pass the
/// width of the input field above so the row stays horizontally
/// aligned with it). `primary_label` is the green button body
/// (e.g. `"Go"`, `"Find"`).
///
/// `Enter` triggers `primary`, `Escape` triggers `cancel` — the
/// keyboard fallthrough happens here so each call site doesn't
/// have to thread `is_key_pressed` checks through its own button
/// expressions.
pub fn action_row(ui: &Ui, body_w: f32, primary_label: &str) -> (bool, bool) {
    const ACTION_BTN_WIDTH: f32 = 58.0;
    const ACTION_BTN_HEIGHT: f32 = 0.0; // ImGui auto-height
    const EDGE_GAP: f32 = 2.0;

    let row_origin_x = ui.cursor_pos()[0];
    let cancel_x = row_origin_x + EDGE_GAP;
    let primary_x = row_origin_x + body_w - ACTION_BTN_WIDTH - EDGE_GAP;

    ui.set_cursor_pos_x(cancel_x);
    let cancel_clicked = ui.button_with_size("Cancel", [ACTION_BTN_WIDTH, ACTION_BTN_HEIGHT])
        || ui.is_key_pressed(dear_imgui_rs::Key::Escape);

    ui.same_line();
    ui.set_cursor_pos_x(primary_x);
    let primary_clicked = success_button(ui, primary_label, [ACTION_BTN_WIDTH, ACTION_BTN_HEIGHT])
        || ui.is_key_pressed(dear_imgui_rs::Key::Enter);

    (primary_clicked, cancel_clicked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lift_brightens_each_channel() {
        let out = lift([0.5, 0.4, 0.3, 1.0], 0.10);
        // RGB go up by exactly the delta, alpha untouched.
        assert!((out[0] - 0.60).abs() < 1e-6);
        assert!((out[1] - 0.50).abs() < 1e-6);
        assert!((out[2] - 0.40).abs() < 1e-6);
        assert_eq!(out[3], 1.0);
    }

    #[test]
    fn lift_darkens_each_channel() {
        let out = lift([0.5, 0.4, 0.3, 0.8], -0.10);
        assert!((out[0] - 0.40).abs() < 1e-6);
        assert!((out[1] - 0.30).abs() < 1e-6);
        assert!((out[2] - 0.20).abs() < 1e-6);
        assert_eq!(out[3], 0.8);
    }

    #[test]
    fn lift_clamps_above_one() {
        // Pre-2026-04-29 the success / danger constants were
        // hand-rolled and drifted from the documented formula.
        // Pin the clamp behaviour so a future regression in the
        // delta or base colour can't smuggle invalid colours past.
        let out = lift([0.95, 0.50, 0.50, 1.0], 0.20);
        assert_eq!(out[0], 1.0);
        assert!((out[1] - 0.70).abs() < 1e-6);
    }

    #[test]
    fn lift_clamps_below_zero() {
        let out = lift([0.10, 0.50, 0.50, 1.0], -0.20);
        assert_eq!(out[0], 0.0);
        assert!((out[1] - 0.30).abs() < 1e-6);
    }

    #[test]
    fn green_red_constants_in_unit_range() {
        // The `pub const` semantic colours must stay in the
        // `0.0..=1.0` band — anything outside silently clips on the
        // GPU and reads wrong on the CPU side.
        for c in [GREEN, RED] {
            for &ch in &c {
                assert!((0.0..=1.0).contains(&ch), "channel {ch} out of range");
            }
            // Alpha pinned to 1.0 — semi-transparent semantic
            // buttons would read as ghosted, almost always a bug.
            assert_eq!(c[3], 1.0);
        }
    }

    #[test]
    fn success_and_danger_hue_distinct() {
        // Catches the hypothetical future "I tweaked GREEN by hand
        // and accidentally bumped it close to RED" regression.
        let dist =
            (GREEN[0] - RED[0]).powi(2) + (GREEN[1] - RED[1]).powi(2) + (GREEN[2] - RED[2]).powi(2);
        assert!(
            dist > 0.10,
            "GREEN and RED collapsed to similar hue (squared dist {dist})"
        );
    }
}
