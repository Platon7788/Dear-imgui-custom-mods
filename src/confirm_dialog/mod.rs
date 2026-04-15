//! # confirm_dialog
//!
//! Reusable modal confirmation dialog for Dear ImGui.
//!
//! ## Features
//! - 6 built-in color themes (Dark, Light, Midnight, Nord, Solarized, Monokai)
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
//!     .with_theme(DialogTheme::Dark);
//!
//! let mut open = true;
//!
//! match render_confirm_dialog(ui, &cfg, &mut open) {
//!     DialogResult::Confirmed => { /* do the action */ }
//!     DialogResult::Cancelled => { /* user cancelled */ }
//!     DialogResult::Open      => { /* still showing */ }
//! }
//! ```

pub mod config;
pub mod theme;

pub use config::{ConfirmStyle, DialogConfig, DialogIcon};
pub use theme::{DialogColors, DialogTheme};

use dear_imgui_rs::{Condition, DrawListMut, Key, StyleColor, StyleVar, Ui, WindowFlags};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

/// Result of rendering the confirm dialog for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
fn c32(c: [f32; 4]) -> u32 { rgba_f32(c[0], c[1], c[2], c[3]) }

// ── Icon drawing primitives ──────────────────────────────────────────────────

fn draw_icon_warning(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32, bg_col: u32) {
    // Equilateral triangle pointing up, centred at (cx, cy).
    // Heights: top = cy - r, base = cy + r*0.6  (visually centred).
    let h = r * 1.6;
    let half_base = h * 0.577; // tan(30°) ≈ 0.577
    let top_y = cy - r;
    let base_y = top_y + h;

    let p_top = [cx, top_y];
    let p_bl  = [cx - half_base, base_y];
    let p_br  = [cx + half_base, base_y];

    // Filled triangle background
    draw.add_triangle(p_top, p_bl, p_br, col).filled(true).build();
    // "!" drawn in bg color on top of the filled triangle
    let bang_top = cy - r * 0.22;
    let bang_bot = cy + r * 0.20;
    let dot_y    = cy + r * 0.42;
    draw.add_line([cx, bang_top], [cx, bang_bot], bg_col).thickness(2.2).build();
    draw.add_circle([cx, dot_y], 1.6, bg_col).filled(true).build();
}

fn draw_icon_error(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).thickness(2.0).build();
    let d = r * 0.42;
    draw.add_line([cx - d, cy - d], [cx + d, cy + d], col).thickness(2.0).build();
    draw.add_line([cx + d, cy - d], [cx - d, cy + d], col).thickness(2.0).build();
}

fn draw_icon_info(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).thickness(2.0).build();
    draw.add_circle([cx, cy - r * 0.35], 1.8, col).filled(true).build();
    draw.add_line([cx, cy - r * 0.10], [cx, cy + r * 0.45], col).thickness(2.0).build();
}

fn draw_icon_question(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).thickness(2.0).build();
    let qx = cx;
    draw.add_line([qx - r * 0.20, cy - r * 0.35], [qx, cy - r * 0.50], col).thickness(2.0).build();
    draw.add_line([qx, cy - r * 0.50], [qx + r * 0.20, cy - r * 0.35], col).thickness(2.0).build();
    draw.add_line([qx + r * 0.20, cy - r * 0.35], [qx, cy - r * 0.10], col).thickness(2.0).build();
    draw.add_line([qx, cy - r * 0.10], [qx, cy + r * 0.05], col).thickness(2.0).build();
    draw.add_circle([qx, cy + r * 0.30], 1.8, col).filled(true).build();
}

// ── Main public function ─────────────────────────────────────────────────────

/// Render a modal confirmation dialog.
///
/// `open` controls visibility. Set to `true` to show, the function sets it to
/// `false` when the user confirms or cancels.
///
/// Returns [`DialogResult`] indicating the action taken this frame.
pub fn render_confirm_dialog(
    ui: &Ui,
    cfg: &DialogConfig,
    open: &mut bool,
) -> DialogResult {
    if !*open {
        return DialogResult::Cancelled;
    }

    let colors = cfg.theme.colors();
    let [dw, dh] = ui.io().display_size();

    let fg_draw = ui.get_foreground_draw_list();

    // ── Dim overlay ──────────────────────────────────────────────────────────
    if cfg.dim_background {
        fg_draw.add_rect([0.0, 0.0], [dw, dh], c32(colors.overlay))
            .filled(true).build();
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
    let dlg_x = (dw - cfg.width)  * 0.5;
    let dlg_y = (dh - cfg.height) * 0.5;

    let _pad  = ui.push_style_var(StyleVar::WindowPadding([cfg.padding, cfg.padding]));
    let _rnd  = ui.push_style_var(StyleVar::WindowRounding(cfg.rounding));
    let _brd  = ui.push_style_var(StyleVar::WindowBorderSize(1.0));
    let _bg   = ui.push_style_color(StyleColor::WindowBg, colors.bg);
    let _brdc = ui.push_style_color(StyleColor::Border, colors.border);

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
            let wdl = ui.get_window_draw_list();
            let win_pos = ui.window_pos();

            // ── Icon + Title row ─────────────────────────────────────────────
            let icon_size = 16.0_f32;
            let has_icon = cfg.icon != DialogIcon::None;
            let title_start_x = if has_icon { icon_size * 2.0 + 10.0 } else { 0.0 };

            if has_icon {
                let icon_cx = win_pos[0] + cfg.padding + icon_size;
                let [_, cy_pos] = ui.cursor_pos();
                let text_h = calc_text_size("Mg")[1];
                let icon_cy = win_pos[1] + cy_pos + text_h * 0.5;
                let icon_col = match cfg.icon {
                    DialogIcon::Warning  => colors.icon_warning,
                    DialogIcon::Error    => colors.icon_error,
                    DialogIcon::Info     => colors.icon_info,
                    DialogIcon::Question => colors.icon_question,
                    DialogIcon::None     => unreachable!(),
                };
                match cfg.icon {
                    DialogIcon::Warning  => draw_icon_warning(&wdl, icon_cx, icon_cy, icon_size * 0.6, c32(icon_col), c32(colors.bg)),
                    DialogIcon::Error    => draw_icon_error(&wdl, icon_cx, icon_cy, icon_size * 0.55, c32(icon_col)),
                    DialogIcon::Info     => draw_icon_info(&wdl, icon_cx, icon_cy, icon_size * 0.55, c32(icon_col)),
                    DialogIcon::Question => draw_icon_question(&wdl, icon_cx, icon_cy, icon_size * 0.55, c32(icon_col)),
                    DialogIcon::None     => {}
                }
            }

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

            // ── Separator — drawn at a fixed relative position ───────────────
            let sep_y_abs = win_pos[1] + content_h * 0.55;
            wdl.add_line(
                [win_pos[0] + cfg.padding, sep_y_abs],
                [win_pos[0] + cfg.width - cfg.padding, sep_y_abs],
                c32(colors.separator),
            ).thickness(1.0).build();

            // ── Buttons — anchored to bottom, centred horizontally ──────────
            let btn_h = cfg.button_height * 0.78;
            let btn_bottom_margin = cfg.padding * 0.35;
            let btn_y = content_h - btn_h - btn_bottom_margin + cfg.padding;

            let btn_w = ((content_w - cfg.button_gap) * 0.5) * 0.42;
            let gap = cfg.button_gap * 1.6;
            let total = btn_w * 2.0 + gap;
            // Centre within full window width (not content_w which excludes padding)
            let btn_start = (cfg.width - total) * 0.5;

            // Cancel button (green / safe)
            {
                let _c0 = ui.push_style_color(StyleColor::Button, colors.btn_cancel);
                let _c1 = ui.push_style_color(StyleColor::ButtonHovered, colors.btn_cancel_hover);
                let _c2 = ui.push_style_color(StyleColor::ButtonActive, colors.btn_cancel_active);
                let _c3 = ui.push_style_color(StyleColor::Text, colors.btn_cancel_text);
                let _r  = ui.push_style_var(StyleVar::FrameRounding(4.0));

                ui.set_cursor_pos([btn_start, btn_y]);
                if ui.button_with_size(&cfg.cancel_label, [btn_w, btn_h]) {
                    result = DialogResult::Cancelled;
                }
            }

            ui.same_line();

            // Confirm button (red / destructive)
            {
                let (bg, hov, act) = match cfg.confirm_style {
                    ConfirmStyle::Destructive => (
                        colors.btn_confirm,
                        colors.btn_confirm_hover,
                        colors.btn_confirm_active,
                    ),
                    ConfirmStyle::Normal => (
                        colors.btn_cancel,
                        colors.btn_cancel_hover,
                        colors.btn_cancel_active,
                    ),
                };
                let text_col = match cfg.confirm_style {
                    ConfirmStyle::Destructive => colors.btn_confirm_text,
                    ConfirmStyle::Normal => colors.btn_cancel_text,
                };
                let _c0 = ui.push_style_color(StyleColor::Button, bg);
                let _c1 = ui.push_style_color(StyleColor::ButtonHovered, hov);
                let _c2 = ui.push_style_color(StyleColor::ButtonActive, act);
                let _c3 = ui.push_style_color(StyleColor::Text, text_col);
                let _r  = ui.push_style_var(StyleVar::FrameRounding(4.0));

                ui.set_cursor_pos([btn_start + btn_w + gap, btn_y]);
                if ui.button_with_size(&cfg.confirm_label, [btn_w, btn_h]) {
                    result = DialogResult::Confirmed;
                }
            }
        });

    if result != DialogResult::Open {
        *open = false;
    }

    result
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = DialogConfig::default();
        assert_eq!(cfg.width, 340.0);
        assert_eq!(cfg.height, 160.0);
        assert!(cfg.dim_background);
        assert!(cfg.keyboard_shortcuts);
        assert_eq!(cfg.icon, DialogIcon::Warning);
        assert_eq!(cfg.confirm_style, ConfirmStyle::Destructive);
    }

    #[test]
    fn builder_chain() {
        let cfg = DialogConfig::new("Delete File", "This cannot be undone.")
            .with_theme(DialogTheme::Nord)
            .with_icon(DialogIcon::Error)
            .with_confirm_label("Delete")
            .with_cancel_label("Keep")
            .with_confirm_style(ConfirmStyle::Destructive)
            .with_width(400.0)
            .with_height(180.0)
            .with_rounding(8.0)
            .without_dim()
            .without_keyboard();

        assert_eq!(cfg.title, "Delete File");
        assert_eq!(cfg.message, "This cannot be undone.");
        assert_eq!(cfg.confirm_label, "Delete");
        assert_eq!(cfg.cancel_label, "Keep");
        assert_eq!(cfg.icon, DialogIcon::Error);
        assert_eq!(cfg.width, 400.0);
        assert_eq!(cfg.height, 180.0);
        assert_eq!(cfg.rounding, 8.0);
        assert!(!cfg.dim_background);
        assert!(!cfg.keyboard_shortcuts);
    }

    #[test]
    fn all_six_themes_resolve() {
        for theme in [
            DialogTheme::Dark, DialogTheme::Light, DialogTheme::Midnight,
            DialogTheme::Nord, DialogTheme::Solarized, DialogTheme::Monokai,
        ] {
            let c = theme.colors();
            assert!(c.bg.iter().all(|&v| (0.0..=1.0).contains(&v)));
            assert!(c.overlay[3] > 0.0);
            assert!(c.btn_confirm[3] > 0.0);
            assert!(c.btn_cancel[3] > 0.0);
        }
    }

    #[test]
    fn dialog_result_not_open_returns_cancelled() {
        assert_eq!(DialogResult::Confirmed, DialogResult::Confirmed);
        assert_ne!(DialogResult::Open, DialogResult::Cancelled);
    }

    #[test]
    fn icon_enum_variants() {
        assert_eq!(DialogIcon::default(), DialogIcon::Warning);
        assert_ne!(DialogIcon::None, DialogIcon::Error);
        assert_ne!(DialogIcon::Info, DialogIcon::Question);
    }
}
