//! Per-toast rendering + layout helpers (require a live `Ui`).
//!
//! Holds the layout-pass height estimator, the animated position resolver,
//! and the single-toast draw routine. Split out of `mod.rs` (was 912 lines)
//! so every file in the module stays under the 500-line cap.

use super::*;

// ─── Per-toast render result ─────────────────────────────────────────────────

pub(super) struct ToastOutcome {
    pub(super) hovered: bool,
    pub(super) close_clicked: bool,
    pub(super) action_clicked: Option<u32>,
    pub(super) body_clicked: bool,
}

// ─── Rendering helpers ───────────────────────────────────────────────────────

/// Height in pixels for a toast — computed before rendering so the stack can
/// lay itself out in a single pass.
pub(super) fn estimate_height(n: &Notification, cfg: &CenterConfig, ui: &dear_imgui_rs::Ui) -> f32 {
    let pad_x = cfg.padding[0];
    let pad_y = cfg.padding[1];
    let title_h = line_height(ui);
    let body_line_h = title_h;

    // Content width = toast width - accent strip - left/right padding - close button.
    let close_slot = if n.closable { 18.0 } else { 0.0 };
    let content_w = cfg.width - cfg.accent_strip - pad_x * 2.0 - close_slot;

    let body_h = if n.body.is_empty() {
        0.0
    } else {
        // Approximate wrap: avg char width × len / content_w lines.
        let tw = calc_text_size(n.body.as_str())[0];
        let lines = ((tw / content_w.max(1.0)).ceil()).max(1.0);
        lines * body_line_h + 4.0
    };

    let actions_h = if n.actions.is_empty() { 0.0 } else { 28.0 };
    let progress_h = if n.show_progress && matches!(n.duration, Duration::Timed(_)) {
        cfg.progress_height + 2.0
    } else {
        0.0
    };

    pad_y * 2.0 + title_h + body_h + actions_h + progress_h
}

/// Resolve per-frame animated position + alpha for a notification.
pub(super) fn animated_pos(
    n: &Notification,
    cfg: &CenterConfig,
    anchor_x: f32,
    base_y: f32,
    cum_y: f32,
    est_h: f32,
    grows_up: bool,
) -> (f32, f32, f32) {
    // Eased enter: decelerates as it arrives (feels like it "lands").
    // Eased exit: accelerates as it leaves (feels like it "flies off").
    let alpha = match cfg.animation {
        AnimationKind::None => {
            if n.dismissing {
                0.0
            } else {
                1.0
            }
        }
        AnimationKind::Fade | AnimationKind::SlideIn => {
            ease_out_cubic(n.enter_t) * (1.0 - ease_in_cubic(n.exit_t))
        }
    };

    let slide_dx = if matches!(cfg.animation, AnimationKind::SlideIn) {
        let from = if cfg.placement.slides_from_left() {
            -(cfg.width + cfg.margin[0])
        } else if cfg.placement.slides_from_right() {
            cfg.width + cfg.margin[0]
        } else {
            0.0
        };
        // Entry: slide from edge, decelerating to rest position.
        // Exit: accelerate back toward the same edge.
        from * (1.0 - ease_out_cubic(n.enter_t)) + from * ease_in_cubic(n.exit_t) * 0.6
    } else {
        0.0
    };

    let px = anchor_x + slide_dx;
    let py = if grows_up {
        base_y - cum_y - est_h
    } else {
        base_y + cum_y
    };
    (px, py, alpha)
}

/// Render a single toast window. Returns user-interaction flags.
///
/// `est_h` is computed once by the caller (layout pass) and threaded
/// through here so we don't re-walk `calc_text_size` over the title /
/// body strings a second time per toast per frame.
pub(super) fn render_toast(
    ui: &Ui,
    n: &Notification,
    c: &NotificationColors,
    cfg: &CenterConfig,
    x: f32,
    y: f32,
    alpha: f32,
    est_h: f32,
) -> ToastOutcome {
    let mut outcome = ToastOutcome {
        hovered: false,
        close_clicked: false,
        action_clicked: None,
        body_clicked: false,
    };

    let _a = ui.push_style_var(StyleVar::Alpha(alpha));
    let _rnd = ui.push_style_var(StyleVar::WindowRounding(cfg.rounding));
    let _brd = ui.push_style_var(StyleVar::WindowBorderSize(1.0));
    let _pad = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0])); // manual inner layout
    let _bg = ui.push_style_color(StyleColor::WindowBg, c.bg);
    let _brdc = ui.push_style_color(StyleColor::Border, c.border);

    ui.window(&n.win_id)
        .position([x, y], Condition::Always)
        .size([cfg.width, est_h], Condition::Always)
        .flags(
            WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_COLLAPSE
                | WindowFlags::NO_SCROLLBAR
                | WindowFlags::NO_SAVED_SETTINGS
                | WindowFlags::NO_FOCUS_ON_APPEARING
                | WindowFlags::NO_NAV,
        )
        .build(|| {
            let win_pos = ui.window_pos();
            let accent = n.resolved_accent(c);

            // ── Accent strip (left edge) ────────────────────────────────────
            {
                let wdl = ui.get_window_draw_list();
                wdl.add_rect(
                    [win_pos[0], win_pos[1]],
                    [win_pos[0] + cfg.accent_strip, win_pos[1] + est_h],
                    rgba_f32(accent[0], accent[1], accent[2], accent[3] * alpha),
                )
                .filled(true)
                .build();

                // ── Severity icon ───────────────────────────────────────────
                if n.show_icon {
                    let icon_r = 9.0;
                    let icon_cx = win_pos[0] + cfg.accent_strip + cfg.padding[0] + icon_r;
                    let icon_cy = win_pos[1] + cfg.padding[1] + icon_r + 1.0;
                    icons::draw_severity(
                        &wdl,
                        n.severity,
                        icon_cx,
                        icon_cy,
                        icon_r,
                        rgba_f32(accent[0], accent[1], accent[2], accent[3] * alpha),
                        rgba_f32(c.bg[0], c.bg[1], c.bg[2], alpha),
                    );
                }

                // ── Progress bar (bottom) ───────────────────────────────────
                if n.show_progress
                    && let Duration::Timed(secs) = n.duration
                    && secs > 0.0
                {
                    let frac = (1.0 - n.elapsed / secs).clamp(0.0, 1.0);
                    let px0 = win_pos[0] + cfg.accent_strip;
                    let px1 = win_pos[0] + cfg.width;
                    let py0 = win_pos[1] + est_h - cfg.progress_height;
                    let py1 = win_pos[1] + est_h;
                    wdl.add_rect(
                        [px0, py0],
                        [px1, py1],
                        rgba_f32(
                            c.progress_bg[0],
                            c.progress_bg[1],
                            c.progress_bg[2],
                            c.progress_bg[3] * alpha,
                        ),
                    )
                    .filled(true)
                    .build();
                    wdl.add_rect(
                        [px0, py0],
                        [px0 + (px1 - px0) * frac, py1],
                        rgba_f32(accent[0], accent[1], accent[2], accent[3] * alpha),
                    )
                    .filled(true)
                    .build();
                }
            } // wdl dropped

            // ── Inner content laid out with manual cursor positioning ────────
            let content_left =
                cfg.accent_strip + cfg.padding[0] + if n.show_icon { 22.0 } else { 0.0 };

            // Pre-compute countdown label so we can reserve its width for title clipping.
            let countdown_label: Option<String> = if n.show_countdown
                && let Duration::Timed(secs) = n.duration
                && secs > 0.0
            {
                let rem = (secs - n.elapsed).max(0.0);
                Some(if rem >= 10.0 {
                    format!("{:.0}s", rem)
                } else {
                    format!("{:.1}s", rem)
                })
            } else {
                None
            };
            let countdown_w = countdown_label
                .as_deref()
                .map(|l| calc_text_size(l)[0] + 6.0) // 6 px gap before close
                .unwrap_or(0.0);

            let content_right =
                cfg.width - cfg.padding[0] - if n.closable { 18.0 } else { 0.0 } - countdown_w;
            let content_w = (content_right - content_left).max(1.0);

            // Title
            ui.set_cursor_pos([content_left, cfg.padding[1]]);
            let _tc = ui.push_style_color(StyleColor::Text, c.title);
            ui.text(&n.title);
            drop(_tc);

            // Countdown text — right-aligned, left of the close button.
            if let Some(label) = &countdown_label {
                let lw = calc_text_size(label.as_str())[0];
                let close_x = if n.closable {
                    cfg.width - cfg.padding[0] - 14.0
                } else {
                    cfg.width - cfg.padding[0]
                };
                let tx = close_x - lw - 4.0;
                let ty = cfg.padding[1] + 1.0; // nudge down 1px for optical alignment
                ui.set_cursor_pos([tx, ty]);
                let _dc = ui.push_style_color(StyleColor::Text, c.body);
                ui.text(label.as_str());
            }

            // Body
            if !n.body.is_empty() {
                ui.set_cursor_pos([content_left, cfg.padding[1] + line_height(ui) + 2.0]);
                let _bc = ui.push_style_color(StyleColor::Text, c.body);
                let _wrap = ui.push_text_wrap_pos(ui.window_pos()[0] + content_left + content_w);
                ui.text_wrapped(&n.body);
                drop(_bc);
            }

            // Action buttons row (below body) — anchored near bottom.
            if !n.actions.is_empty() {
                let row_y = est_h
                    - cfg.padding[1]
                    - if n.show_progress && matches!(n.duration, Duration::Timed(_)) {
                        cfg.progress_height + 2.0 + 22.0
                    } else {
                        22.0
                    };
                ui.set_cursor_pos([content_left, row_y]);

                let _bc = ui.push_style_color(StyleColor::Button, c.btn_action);
                let _bch = ui.push_style_color(StyleColor::ButtonHovered, c.btn_action_hover);
                let _bca = ui.push_style_color(StyleColor::ButtonActive, c.btn_action_active);
                let _btc = ui.push_style_color(StyleColor::Text, c.btn_action_text);

                // Use pre-formatted `n.action_ids[idx]` instead of
                // building a fresh `format!` per button per frame —
                // saves N allocations per visible toast where N is
                // the action count.
                for (idx, act) in n.actions.iter().enumerate() {
                    if idx > 0 {
                        ui.same_line();
                    }
                    let label = n
                        .action_ids
                        .get(idx)
                        .map(String::as_str)
                        .unwrap_or(act.label.as_str());
                    if ui.button(label) {
                        outcome.action_clicked = Some(act.id);
                    }
                }
            }

            // ── Close button (invisible hit target + custom × glyph) ────────
            if n.closable {
                let close_size = 14.0;
                ui.set_cursor_pos([cfg.width - cfg.padding[0] - close_size, cfg.padding[1]]);
                // Pre-formatted `n.close_id` instead of per-frame
                // `format!("##close_{id}")`.
                let clicked = ui.invisible_button(&n.close_id, [close_size, close_size]);
                let hov = ui.is_item_hovered();
                let col = if hov { c.close_hover } else { c.close };
                let cx = win_pos[0] + cfg.width - cfg.padding[0] - close_size * 0.5;
                let cy = win_pos[1] + cfg.padding[1] + close_size * 0.5;
                let wdl = ui.get_window_draw_list();
                icons::draw_close_x(
                    &wdl,
                    cx,
                    cy,
                    close_size * 0.30,
                    rgba_f32(col[0], col[1], col[2], col[3] * alpha),
                );
                if clicked {
                    outcome.close_clicked = true;
                }
            }

            // ── Whole-toast hover + click detection ─────────────────────────
            outcome.hovered = ui.is_window_hovered();
            if outcome.hovered
                && ui.is_mouse_clicked(MouseButton::Left)
                && !outcome.close_clicked
                && outcome.action_clicked.is_none()
            {
                // Only emit body_clicked if the click wasn't absorbed by a button.
                // Action + close button flags handled above.
                outcome.body_clicked = true;
            }
        });

    outcome
}
