//! # notifications
//!
//! Modern, flexible toast-notification center for Dear ImGui.
//!
//! ## Features
//! - 5 severity levels: Info, Success, Warning, Error, Debug — each with a
//!   distinct color and font-independent draw-list icon.
//! - 6 stack placements (4 corners + top/bottom center) with customizable
//!   margin and spacing.
//! - Auto-dismiss timer with optional bottom progress bar; `sticky` mode
//!   keeps the toast until the user closes it.
//! - Pause-on-hover so reading long text does not miss the window.
//! - `Fade` and `SlideIn` entry / exit animations.
//! - Action buttons with caller-defined IDs surfaced via
//!   [`NotificationEvent::ActionClicked`].
//! - Manual `×` close, custom per-toast accent color, max-visible cap with
//!   graceful overflow.
//! - Fully themeable through the crate-wide [`crate::theme::Theme`] enum or
//!   a [`NotificationColors`] override.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::notifications::*;
//!
//! // Persistent state — typically lives in your app/handler struct.
//! let mut center = NotificationCenter::new();
//!
//! // Push from anywhere — events / buttons / async callbacks.
//! center.push(Notification::success("Saved"));
//! center.push(Notification::error("Failed").with_body("disk full"));
//!
//! # fn frame(ui: &dear_imgui_rs::Ui, center: &mut NotificationCenter, dt: f32) {
//! // Render every frame — last, so toasts are on top.
//! for event in center.render(ui, dt) {
//!     match event {
//!         NotificationEvent::Dismissed(id) => { let _ = id; }
//!         NotificationEvent::ActionClicked { id, action_id } => {
//!             let _ = (id, action_id);
//!         }
//!         NotificationEvent::Clicked(id) => { let _ = id; }
//!     }
//! }
//! # }
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass

pub mod config;
pub mod icons;
pub mod theme;

pub use config::{
    AnimationKind, CenterConfig, Duration, Notification, NotificationAction, Placement, Severity,
};
pub use theme::NotificationColors;

use dear_imgui_rs::{Condition, MouseButton, StyleColor, StyleVar, Ui, WindowFlags};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

// ─── Events ──────────────────────────────────────────────────────────────────

/// Event emitted by [`NotificationCenter::render`] during a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEvent {
    /// Toast was dismissed (timer expired, close button, or action click).
    Dismissed(u64),
    /// An action button inside a toast was clicked.
    ActionClicked {
        /// Notification id.
        id: u64,
        /// Caller-defined action id (see [`Notification::with_action`]).
        action_id: u32,
    },
    /// The toast body (not a button) was clicked.
    Clicked(u64),
}

// ─── Notification center ─────────────────────────────────────────────────────

/// Holds the live stack of notifications between frames.
///
/// `NotificationCenter` is not `Copy` and persists across frames — keep it in
/// your application state struct. Every frame call
/// [`render`](Self::render) to advance animations, honor timers, and draw
/// the stack.
#[derive(Debug)]
pub struct NotificationCenter {
    /// Active notifications, oldest first.
    queue: Vec<Notification>,
    /// Configuration.
    config: CenterConfig,
    /// Monotonic id counter.
    next_id: u64,
}

impl Default for NotificationCenter {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            config: CenterConfig::default(),
            next_id: 1,
        }
    }
}

impl NotificationCenter {
    /// Create a center with default config (top-right, fade, 5 visible).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a center from an explicit config.
    pub fn with_config(config: CenterConfig) -> Self {
        Self {
            queue: Vec::new(),
            config,
            next_id: 1,
        }
    }

    /// Mutable access to the configuration — changes take effect next frame.
    pub fn config_mut(&mut self) -> &mut CenterConfig {
        &mut self.config
    }

    /// Read-only view of the configuration.
    pub fn config(&self) -> &CenterConfig {
        &self.config
    }

    /// Push a notification onto the stack and return its id.
    pub fn push(&mut self, mut n: Notification) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        n.id = id;
        self.queue.push(n);
        id
    }

    /// Mark the notification with `id` for dismissal (triggers exit animation).
    pub fn dismiss(&mut self, id: u64) {
        if let Some(n) = self.queue.iter_mut().find(|n| n.id == id)
            && !n.dismissing
        {
            n.dismissing = true;
        }
    }

    /// Dismiss every active notification.
    pub fn dismiss_all(&mut self) {
        for n in &mut self.queue {
            n.dismissing = true;
        }
    }

    /// Number of notifications currently on the stack (including fading-out).
    pub fn count(&self) -> usize {
        self.queue.len()
    }

    /// Advance state and draw the stack. Returns events that fired this frame.
    pub fn render(&mut self, ui: &Ui, dt: f32) -> Vec<NotificationEvent> {
        let mut events = Vec::new();
        if dt.is_nan() || dt < 0.0 {
            return events;
        }

        // Clone the config for the duration of the frame so we can freely
        // interleave `&self.queue` reads with `&mut self` mutations (dismiss).
        let cfg = self.config.clone();
        let colors = cfg.resolved_colors();
        let anim_dur = cfg.animation_duration.max(0.0001);

        // ── Pass 1: advance animations for every notification ───────────────
        for n in &mut self.queue {
            if !n.dismissing && n.enter_t < 1.0 {
                n.enter_t = (n.enter_t + dt / anim_dur).min(1.0);
            }
            if n.dismissing && n.exit_t < 1.0 {
                n.exit_t = (n.exit_t + dt / anim_dur).min(1.0);
            }
        }

        // ── Layout parameters ───────────────────────────────────────────────
        let [dw, dh] = ui.io().display_size();
        let anchor_x = match cfg.placement {
            Placement::TopRight | Placement::BottomRight => dw - cfg.margin[0] - cfg.width,
            Placement::TopLeft | Placement::BottomLeft => cfg.margin[0],
            Placement::TopCenter | Placement::BottomCenter => (dw - cfg.width) * 0.5,
        };
        let grows_up = cfg.placement.grows_up();
        let base_y = if grows_up {
            dh - cfg.margin[1]
        } else {
            cfg.margin[1]
        };

        // ── Determine visible slice: the newest `max_visible` ────────────────
        let visible_count = self.queue.len().min(cfg.max_visible);
        let start = self.queue.len().saturating_sub(visible_count);

        // Newest first (closest to anchor edge).
        let indices: Vec<usize> = (start..self.queue.len()).rev().collect();

        // ── Pass 2: render each visible toast ───────────────────────────────
        let mut cum_y = 0.0_f32;
        let mut hover_flags: Vec<(u64, bool)> = Vec::with_capacity(indices.len());
        let mut to_dismiss: Vec<u64> = Vec::new();

        for &i in &indices {
            let n = &self.queue[i];

            let est_h = estimate_height(n, &cfg);

            // Slot fraction drives how much vertical space this toast claims
            // in the stack — animating from 0→1 (enter) or 1→0 (exit) so
            // neighboring toasts glide rather than jump.
            let slot_frac = ease_out_cubic(n.enter_t) * (1.0 - ease_in_cubic(n.exit_t));

            let (px, py, alpha) = animated_pos(n, &cfg, anchor_x, base_y, cum_y, est_h, grows_up);

            // Always advance by animated slot so the gap opens/closes smoothly.
            cum_y += (est_h + cfg.spacing) * slot_frac;

            if alpha <= 0.001 && n.dismissing {
                continue;
            }

            let outcome = render_toast(ui, n, &colors, &cfg, px, py, alpha);

            hover_flags.push((n.id, outcome.hovered));

            if outcome.close_clicked {
                to_dismiss.push(n.id);
            }
            if let Some(aid) = outcome.action_clicked {
                events.push(NotificationEvent::ActionClicked {
                    id: n.id,
                    action_id: aid,
                });
                to_dismiss.push(n.id);
            }
            if outcome.body_clicked {
                events.push(NotificationEvent::Clicked(n.id));
            }
        }

        // ── Pass 3: advance elapsed timers (paused while hovered) ───────────
        for n in &mut self.queue {
            if n.dismissing {
                continue;
            }
            let hovered = cfg.pause_on_hover && hover_flags.iter().any(|&(id, h)| id == n.id && h);
            if hovered {
                continue;
            }

            if let Duration::Timed(secs) = n.duration {
                n.elapsed += dt;
                if n.elapsed >= secs {
                    n.dismissing = true;
                }
            }
        }

        // ── Pass 4: apply requested dismissals ──────────────────────────────
        for id in to_dismiss {
            self.dismiss(id);
        }

        // ── Pass 5: reap notifications whose exit animation has finished ────
        let none_anim = matches!(cfg.animation, AnimationKind::None);
        self.queue.retain(|n| {
            let done = n.dismissing && (none_anim || n.exit_t >= 1.0);
            if done {
                events.push(NotificationEvent::Dismissed(n.id));
            }
            !done
        });

        events
    }
}

// ─── Per-toast render result ─────────────────────────────────────────────────

struct ToastOutcome {
    hovered: bool,
    close_clicked: bool,
    action_clicked: Option<u32>,
    body_clicked: bool,
}

// ─── Rendering helpers ───────────────────────────────────────────────────────

/// Height in pixels for a toast — computed before rendering so the stack can
/// lay itself out in a single pass.
fn estimate_height(n: &Notification, cfg: &CenterConfig) -> f32 {
    let pad_x = cfg.padding[0];
    let pad_y = cfg.padding[1];
    let title_h = calc_text_size("Mg")[1];
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
fn animated_pos(
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
fn render_toast(
    ui: &Ui,
    n: &Notification,
    c: &NotificationColors,
    cfg: &CenterConfig,
    x: f32,
    y: f32,
    alpha: f32,
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

    let win_id = format!("##toast_{}", n.id);
    let est_h = estimate_height(n, cfg);

    ui.window(&win_id)
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
            let countdown_label: Option<String> =
                if n.show_countdown && let Duration::Timed(secs) = n.duration && secs > 0.0 {
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

            let content_right = cfg.width
                - cfg.padding[0]
                - if n.closable { 18.0 } else { 0.0 }
                - countdown_w;
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
                ui.set_cursor_pos([content_left, cfg.padding[1] + calc_text_size("Mg")[1] + 2.0]);
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

                for (idx, act) in n.actions.iter().enumerate() {
                    if idx > 0 {
                        ui.same_line();
                    }
                    let label = format!("{}##act_{}_{}", act.label, n.id, act.id);
                    if ui.button(&label) {
                        outcome.action_clicked = Some(act.id);
                    }
                }
            }

            // ── Close button (invisible hit target + custom × glyph) ────────
            if n.closable {
                let close_size = 14.0;
                ui.set_cursor_pos([cfg.width - cfg.padding[0] - close_size, cfg.padding[1]]);
                let clicked =
                    ui.invisible_button(format!("##close_{}", n.id), [close_size, close_size]);
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

// ─── Easing ──────────────────────────────────────────────────────────────────

/// Decelerates into the target — fast start, smooth landing.
#[inline]
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Accelerates away from the origin — slow start, fast finish.
#[inline]
fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_assigns_unique_ids() {
        let mut c = NotificationCenter::new();
        let a = c.push(Notification::info("a"));
        let b = c.push(Notification::info("b"));
        assert_ne!(a, b);
        assert_eq!(c.count(), 2);
    }

    #[test]
    fn dismiss_sets_flag() {
        let mut c = NotificationCenter::new();
        let id = c.push(Notification::info("x"));
        c.dismiss(id);
        assert!(c.queue.iter().find(|n| n.id == id).unwrap().dismissing);
    }

    #[test]
    fn dismiss_all_sets_every_flag() {
        let mut c = NotificationCenter::new();
        c.push(Notification::info("a"));
        c.push(Notification::error("b"));
        c.dismiss_all();
        assert!(c.queue.iter().all(|n| n.dismissing));
    }

    #[test]
    fn severity_labels() {
        assert_eq!(Severity::Info.label(), "Info");
        assert_eq!(Severity::Error.label(), "Error");
    }

    #[test]
    fn placement_grows_up() {
        assert!(Placement::BottomRight.grows_up());
        assert!(Placement::BottomLeft.grows_up());
        assert!(Placement::BottomCenter.grows_up());
        assert!(!Placement::TopRight.grows_up());
    }

    #[test]
    fn builder_sticky_sets_duration() {
        let n = Notification::warning("w").sticky();
        assert_eq!(n.duration, Duration::Sticky);
    }

    #[test]
    fn builder_custom_color_override() {
        let n = Notification::info("x").with_custom_color([1.0, 0.5, 0.0, 1.0]);
        assert_eq!(n.custom_color, Some([1.0, 0.5, 0.0, 1.0]));
    }

    #[test]
    fn builder_actions_accumulate() {
        let n = Notification::info("x")
            .with_action(1, "One")
            .with_action(2, "Two");
        assert_eq!(n.actions.len(), 2);
        assert_eq!(n.actions[0].id, 1);
        assert_eq!(n.actions[1].label, "Two");
    }
}
