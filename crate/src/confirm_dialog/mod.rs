//! # confirm_dialog
//!
//! Reusable modal confirmation dialog for Dear ImGui.
//!
//! ## Features
//! - Theme-aware via the unified [`crate::theme::Theme`] selector, plus a
//!   per-instance custom palette through [`DialogConfig::with_colors`]
//! - 4 icon types drawn via draw-list primitives (Warning, Error, Info, Question)
//! - Fullscreen dim overlay behind the dialog
//! - Keyboard shortcuts: Escape to cancel, Enter to confirm
//! - Destructive / Normal confirm button styles
//! - Builder-pattern configuration
//! - Font-independent: all icons drawn as crisp draw-list primitives
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::confirm_dialog::*;
//!
//! let cfg = DialogConfig::new("Close Application", "Are you sure you want to close?")
//!     .with_icon(DialogIcon::Warning)
//!     .with_confirm_label("Close")
//!     .with_cancel_label("Cancel")
//!     .with_theme(Theme::Dark);
//!
//! let mut open = true;
//!
//! match render_confirm_dialog(ui, &cfg, &mut open) {
//!     DialogResult::Confirmed => { /* do the action */ }
//!     DialogResult::Cancelled => { /* user cancelled */ }
//!     DialogResult::Open      => { /* still showing */ }
//! }
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
mod buttons;
pub mod config;
mod icons;
pub mod theme;

pub use config::{ConfirmStyle, DialogConfig, DialogIcon};
pub use theme::DialogColors;

use buttons::{ButtonGlyph, icon_button};
use icons::{draw_icon_error, draw_icon_info, draw_icon_question, draw_icon_warning};

use dear_imgui_rs::{Condition, DrawListMut, Key, StyleColor, StyleVar, Ui, WindowFlags};

use crate::utils::color::rgba_f32;
use crate::utils::text::{calc_text_size, line_height};

/// Result of rendering the confirm dialog for one frame.
///
/// Dropping this result means ignoring whether the user accepted or
/// cancelled — `#[must_use]` surfaces that at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "the caller must react to DialogResult — Confirmed triggers the destructive action, Cancelled dismisses"]
pub enum DialogResult {
    /// User confirmed (clicked confirm button or pressed Enter).
    Confirmed,
    /// User cancelled (clicked cancel button or pressed Escape).
    Cancelled,
    /// Dialog is still open, no action taken this frame.
    Open,
}

// ── Color helper ─────────────────────────────────────────────────────────────

#[inline]
fn c32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── Main public function ─────────────────────────────────────────────────────

/// Render a modal confirmation dialog.
///
/// `open` controls visibility. Set to `true` to show, the function sets it to
/// `false` when the user confirms or cancels.
///
/// Returns [`DialogResult`] indicating the action taken this frame.
pub fn render_confirm_dialog(ui: &Ui, cfg: &DialogConfig, open: &mut bool) -> DialogResult {
    if !*open {
        return DialogResult::Cancelled;
    }

    let colors = cfg.resolved_colors();
    let [dw, dh] = ui.io().display_size();

    let fg_draw = ui.get_foreground_draw_list();

    // ── Dim overlay ──────────────────────────────────────────────────────────
    if cfg.dim_background {
        fg_draw
            .add_rect([0.0, 0.0], [dw, dh], c32(colors.overlay))
            .filled(true)
            .build();
    }

    // ── Keyboard shortcuts ───────────────────────────────────────────────────
    let mut result = DialogResult::Open;
    if cfg.keyboard_shortcuts {
        if ui.is_key_pressed(Key::Escape) {
            *open = false;
            return DialogResult::Cancelled;
        }
        if ui.is_key_pressed(Key::Enter) {
            *open = false;
            return DialogResult::Confirmed;
        }
    }

    // ── Dialog window ────────────────────────────────────────────────────────
    let dlg_x = (dw - cfg.width) * 0.5;
    let dlg_y = (dh - cfg.height) * 0.5;

    // Resolve the border color: derive from icon when accent_border is on,
    // fall back to the theme's neutral border otherwise.
    let border_color = if cfg.accent_border {
        match cfg.icon {
            DialogIcon::Warning => colors.icon_warning,
            DialogIcon::Error => colors.icon_error,
            DialogIcon::Info => colors.icon_info,
            DialogIcon::Question => colors.icon_question,
            DialogIcon::None => colors.border,
        }
    } else {
        colors.border
    };

    let _pad = ui.push_style_var(StyleVar::WindowPadding([cfg.padding, cfg.padding]));
    let _rnd = ui.push_style_var(StyleVar::WindowRounding(cfg.rounding));
    let _brd = ui.push_style_var(StyleVar::WindowBorderSize(cfg.border_thickness));
    let _bg = ui.push_style_color(StyleColor::WindowBg, colors.bg);
    let _brdc = ui.push_style_color(StyleColor::Border, border_color);

    ui.window("##confirm_dialog")
        .position([dlg_x, dlg_y], Condition::Always)
        .size([cfg.width, cfg.height], Condition::Always)
        .flags(
            WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_SCROLLBAR
                | WindowFlags::NO_COLLAPSE,
        )
        .build(|| {
            let content_w = cfg.width - cfg.padding * 2.0;
            let content_h = cfg.height - cfg.padding * 2.0;
            let win_pos = ui.window_pos();

            // ── Icon + Title row ─────────────────────────────────────────────
            let icon_size = cfg.header_icon_size;
            let has_icon = cfg.icon != DialogIcon::None;
            let title_start_x = if has_icon {
                icon_size * 2.0 + 10.0
            } else {
                0.0
            };

            // Scoped window draw list so it is released before `icon_button`
            // below tries to take its own — only one DrawListMut of a given
            // type (Window) may be live at a time.
            {
                let wdl = ui.get_window_draw_list();

                if has_icon {
                    let icon_cx = win_pos[0] + cfg.padding + icon_size;
                    let [_, cy_pos] = ui.cursor_pos();
                    let text_h = line_height(ui);
                    let icon_cy = win_pos[1] + cy_pos + text_h * 0.5;
                    // Single match resolves the per-icon colour *and* dispatches
                    // the draw call. `has_icon` already excludes `None`, so no
                    // `unreachable!()` panic path is needed here.
                    match cfg.icon {
                        DialogIcon::Warning => draw_icon_warning(
                            &wdl,
                            icon_cx,
                            icon_cy,
                            icon_size * 0.6,
                            c32(colors.icon_warning),
                            c32(colors.bg),
                        ),
                        DialogIcon::Error => draw_icon_error(
                            &wdl,
                            icon_cx,
                            icon_cy,
                            icon_size * 0.55,
                            c32(colors.icon_error),
                        ),
                        DialogIcon::Info => draw_icon_info(
                            &wdl,
                            icon_cx,
                            icon_cy,
                            icon_size * 0.55,
                            c32(colors.icon_info),
                        ),
                        DialogIcon::Question => draw_icon_question(
                            &wdl,
                            icon_cx,
                            icon_cy,
                            icon_size * 0.55,
                            c32(colors.icon_question),
                        ),
                        DialogIcon::None => {}
                    }
                }

                // Separator — drawn via the same wdl scope.
                if cfg.show_separator {
                    let sep_y_abs = win_pos[1] + content_h * 0.55;
                    wdl.add_line(
                        [win_pos[0] + cfg.padding, sep_y_abs],
                        [win_pos[0] + cfg.width - cfg.padding, sep_y_abs],
                        c32(colors.separator),
                    )
                    .thickness(1.0)
                    .build();
                }
            } // wdl drops here

            // Title text
            let [_, ty] = ui.cursor_pos();
            let title_tw = calc_text_size(cfg.title.as_str())[0];
            let title_x = if has_icon {
                title_start_x
            } else {
                ((content_w - title_tw) * 0.5).max(0.0)
            };
            ui.set_cursor_pos([title_x, ty]);
            ui.text_colored(colors.title, &cfg.title);

            ui.spacing();

            // ── Message ──────────────────────────────────────────────────────
            let msg_w = calc_text_size(cfg.message.as_str())[0];
            let msg_x = if has_icon {
                title_start_x
            } else {
                ((content_w - msg_w) * 0.5).max(0.0)
            };
            let [_, my] = ui.cursor_pos();
            ui.set_cursor_pos([msg_x, my]);
            ui.text_colored(colors.message, &cfg.message);

            // ── Buttons — anchored to bottom, centred horizontally ──────────
            let btn_h = cfg.button_height;
            let btn_bottom_margin = cfg.padding * cfg.button_bottom_factor;
            let btn_y = content_h - btn_h - btn_bottom_margin + cfg.padding;

            // Resolve glyphs for the cancel + confirm buttons.
            let (cancel_glyph, confirm_glyph) = if cfg.show_button_icons {
                let cg = match cfg.confirm_style {
                    ConfirmStyle::Destructive => ButtonGlyph::Power,
                    ConfirmStyle::Normal => ButtonGlyph::Check,
                };
                (ButtonGlyph::X, cg)
            } else {
                (ButtonGlyph::None, ButtonGlyph::None)
            };

            // Width = label + icon + horizontal padding; both buttons share max width.
            let icon_extra = if cfg.show_button_icons {
                btn_h * cfg.button_icon_scale * 2.0 + 8.0 // icon diameter + gap
            } else {
                0.0
            };
            let h_pad = cfg.button_padding_x;
            // A non-zero `button_width` pins both cells to a fixed width;
            // `0.0` falls back to content auto-sizing (label + icon + pad),
            // with both buttons sharing the wider of the two.
            let btn_w = if cfg.button_width > 0.0 {
                cfg.button_width
            } else {
                let cancel_w = calc_text_size(cfg.cancel_label.as_str())[0] + icon_extra + h_pad;
                let confirm_w = calc_text_size(cfg.confirm_label.as_str())[0] + icon_extra + h_pad;
                cancel_w.max(confirm_w)
            };

            // `button_gap` is the visible pixel gap between the two buttons —
            // no implicit multiplier. (Pre-0.9 we silently scaled by 1.6 here,
            // which forced callers to back-solve magic numbers.)
            let total = btn_w * 2.0 + cfg.button_gap;
            // Centre within full window width.
            let btn_start = (cfg.width - total) * 0.5;

            // Cancel button (green / safe)
            ui.set_cursor_pos([btn_start, btn_y]);
            if icon_button(
                ui,
                "##cd_cancel",
                &cfg.cancel_label,
                [btn_w, btn_h],
                cancel_glyph,
                colors.btn_cancel,
                colors.btn_cancel_hover,
                colors.btn_cancel_active,
                colors.btn_cancel_text,
                4.0,
                cfg.button_icon_scale,
            ) {
                result = DialogResult::Cancelled;
            }

            // Confirm button (red / destructive or green / normal)
            let (bg, hov, act, text_col) = match cfg.confirm_style {
                ConfirmStyle::Destructive => (
                    colors.btn_confirm,
                    colors.btn_confirm_hover,
                    colors.btn_confirm_active,
                    colors.btn_confirm_text,
                ),
                ConfirmStyle::Normal => (
                    colors.btn_cancel,
                    colors.btn_cancel_hover,
                    colors.btn_cancel_active,
                    colors.btn_cancel_text,
                ),
            };
            ui.set_cursor_pos([btn_start + btn_w + cfg.button_gap, btn_y]);
            if icon_button(
                ui,
                "##cd_confirm",
                &cfg.confirm_label,
                [btn_w, btn_h],
                confirm_glyph,
                bg,
                hov,
                act,
                text_col,
                4.0,
                cfg.button_icon_scale,
            ) {
                result = DialogResult::Confirmed;
            }
        });

    if result != DialogResult::Open {
        *open = false;
    }

    result
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
