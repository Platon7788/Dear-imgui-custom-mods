//! Per-frame rendering and input handling for [`Timeline`].
//!
//! This is the only file in the module that touches an ImGui [`Ui`];
//! all coordinate math it relies on lives in `coords.rs` so it can be
//! exercised in isolation by the unit tests.

use dear_imgui_rs::{MouseButton, Ui};

use super::{Timeline, TimelineEvent, adaptive_ticks, col32, format_duration};
use crate::utils::text::{calc_text_size, line_height};

impl Timeline {
    /// Render the timeline. Returns events from this frame.
    pub fn render(&mut self, ui: &Ui) -> Vec<TimelineEvent> {
        let mut events = Vec::new();
        self.hovered_span = None;

        let avail = ui.content_region_avail();
        let _id_tok = ui.push_id(&self.id);

        // Snapshot immutable config needed inside the closure.
        let cfg = self.config.clone();

        ui.child_window("##timeline_canvas")
            .size(avail)
            .build(ui, || {
                let win_pos = ui.cursor_screen_pos();
                let win_size = ui.content_region_avail();
                let draw = ui.get_window_draw_list();

                // ── Background ──────────────────────────────────────────
                draw.add_rect(
                    win_pos,
                    [win_pos[0] + win_size[0], win_pos[1] + win_size[1]],
                    col32(cfg.color_bg),
                )
                .filled(true)
                .build();

                // ── Layout ──────────────────────────────────────────────
                let label_w = if cfg.show_track_labels {
                    cfg.track_label_width
                } else {
                    0.0
                };
                let ruler_h = if cfg.show_ruler {
                    cfg.ruler_height
                } else {
                    0.0
                };
                let content_x = win_pos[0] + label_w;
                let content_w = (win_size[0] - label_w).max(1.0);
                let content_y = win_pos[1] + ruler_h;

                // ── Smooth zoom ─────────────────────────────────────────
                if cfg.smooth_zoom {
                    let dt = f64::from(ui.io().delta_time());
                    let diff = self.vp.zoom_target - self.vp.pixels_per_second;
                    if diff.abs() > 0.01 {
                        self.vp.pixels_per_second +=
                            diff * (1.0 - (-f64::from(cfg.smooth_zoom_speed) * dt).exp());
                    } else {
                        self.vp.pixels_per_second = self.vp.zoom_target;
                    }
                }

                // ── Input: pan, zoom & vertical scroll ──────────────────
                let mouse_pos = ui.io().mouse_pos();
                let in_content = mouse_pos[0] >= content_x
                    && mouse_pos[0] < win_pos[0] + win_size[0]
                    && mouse_pos[1] >= win_pos[1]
                    && mouse_pos[1] < win_pos[1] + win_size[1];
                let hovered = in_content && ui.is_window_hovered();

                if hovered {
                    let wheel = ui.io().mouse_wheel();
                    let shift_held = ui.io().key_shift();

                    if wheel.abs() > 0.01 {
                        if shift_held {
                            // Shift+Wheel scrolls vertically through tall
                            // track lists. Gated on `hovered` and the same
                            // wheel event as zoom so the two are mutually
                            // exclusive (no double-consume).
                            let total_h = self.total_content_height();
                            let visible_h = (win_size[1] - ruler_h).max(0.0);
                            if total_h > visible_h {
                                self.vp.scroll_y = (self.vp.scroll_y - wheel * 40.0)
                                    .clamp(0.0, total_h - visible_h);
                            }
                        } else {
                            // Plain wheel zooms toward the cursor.
                            let zoom_factor = 1.15_f64.powf(f64::from(wheel));
                            let mouse_time = self.x_to_time(mouse_pos[0], content_x);

                            let new_pps = (self.vp.zoom_target * zoom_factor)
                                .clamp(cfg.min_zoom, cfg.max_zoom);
                            self.vp.zoom_target = new_pps;

                            if !cfg.smooth_zoom {
                                self.vp.pixels_per_second = new_pps;
                            }

                            self.vp.time_start =
                                mouse_time - f64::from(mouse_pos[0] - content_x) / new_pps;

                            events.push(TimelineEvent::ViewChanged {
                                start: self.vp.time_start,
                                end: self.vp.time_start + f64::from(content_w) / new_pps,
                            });
                        }
                    }

                    // Pan with middle or right mouse button.
                    if ui.is_mouse_clicked(MouseButton::Middle)
                        || ui.is_mouse_clicked(MouseButton::Right)
                    {
                        self.panning = true;
                        self.pan_start_x = mouse_pos[0];
                        self.pan_start_time = self.vp.time_start;
                    }
                }

                if self.panning {
                    if ui.is_mouse_down(MouseButton::Middle) || ui.is_mouse_down(MouseButton::Right)
                    {
                        let dx = mouse_pos[0] - self.pan_start_x;
                        self.vp.time_start = self.pan_start_time
                            - f64::from(dx) / self.vp.pixels_per_second.max(1e-9);
                    } else {
                        self.panning = false;
                    }
                }

                // ── Ruler ───────────────────────────────────────────────
                if cfg.show_ruler {
                    draw.add_rect(
                        [content_x, win_pos[1]],
                        [content_x + content_w, win_pos[1] + ruler_h],
                        col32(cfg.color_ruler_bg),
                    )
                    .filled(true)
                    .build();

                    let pps = self.vp.pixels_per_second.max(1e-9);
                    let visible_duration = f64::from(content_w) / pps;
                    let (tick_interval, unit) = adaptive_ticks(visible_duration, content_w);

                    if tick_interval > 0.0 {
                        let first_tick =
                            (self.vp.time_start / tick_interval).floor() * tick_interval;
                        let end_time = self.vp.time_start + visible_duration;

                        let mut t = first_tick;
                        let mut safety = 0;
                        while t <= end_time && safety < 2000 {
                            safety += 1;
                            let x = self.time_to_x(t, content_x);

                            if x >= content_x && x <= content_x + content_w {
                                draw.add_line(
                                    [x, win_pos[1] + ruler_h - 6.0],
                                    [x, win_pos[1] + ruler_h],
                                    col32(cfg.color_ruler_text),
                                )
                                .build();

                                let val = t * unit.factor();
                                let label = if val.abs() < 0.001 {
                                    format!("0{}", unit.suffix())
                                } else if val.fract().abs() < 0.001 {
                                    format!("{:.0}{}", val, unit.suffix())
                                } else {
                                    format!("{:.1}{}", val, unit.suffix())
                                };

                                let text_size = calc_text_size(&label);
                                let tx = (x - text_size[0] * 0.5).max(content_x);
                                let ty = win_pos[1] + (ruler_h - 6.0 - text_size[1]) * 0.5;
                                draw.add_text([tx, ty], col32(cfg.color_ruler_text), &label);
                            }
                            t += tick_interval;
                        }
                    }
                }

                // ── Track label background ──────────────────────────────
                if cfg.show_track_labels && label_w > 0.0 {
                    draw.add_rect(
                        [win_pos[0], content_y],
                        [win_pos[0] + label_w, win_pos[1] + win_size[1]],
                        col32([
                            cfg.color_bg[0] + 0.02,
                            cfg.color_bg[1] + 0.02,
                            cfg.color_bg[2] + 0.03,
                            1.0,
                        ]),
                    )
                    .filled(true)
                    .build();
                }

                // ── Tracks & spans ──────────────────────────────────────
                // Pre-compute the global data time range ONCE per
                // frame so `span_color(.., ColorMode::ByDuration)`
                // doesn't re-walk every track per span (was O(N²),
                // measured in 2026-04-30 audit). Cheap when not in
                // ByDuration mode — just one extra range scan
                // amortised across all visible spans.
                let data_range = self.data_time_range();
                let mut y = content_y - self.vp.scroll_y;

                for (ti, track) in self.tracks.iter().enumerate() {
                    let rows = if track.collapsed {
                        0
                    } else {
                        track.depth_rows()
                    };
                    let track_h = if track.collapsed {
                        cfg.track_header_height
                    } else {
                        cfg.track_header_height + rows as f32 * (cfg.row_height + cfg.row_gap)
                    };

                    // Cull off-screen tracks.
                    if y + track_h < content_y || y > win_pos[1] + win_size[1] {
                        y += track_h;
                        continue;
                    }

                    // Track stripe.
                    if ti % 2 == 1 {
                        draw.add_rect(
                            [content_x, y],
                            [win_pos[0] + win_size[0], y + track_h],
                            col32(cfg.color_bg_alt),
                        )
                        .filled(true)
                        .build();
                    }

                    // Track label.
                    if cfg.show_track_labels {
                        let arrow = if track.collapsed {
                            "\u{25B8}"
                        } else {
                            "\u{25BE}"
                        };
                        // Two draw calls — avoids the `format!` heap alloc
                        // that the historic single-string path triggered
                        // every frame for every track.
                        let text_y = y + (cfg.track_header_height - 14.0) * 0.5;
                        let arrow_x = win_pos[0] + 4.0;
                        let track_col = col32(cfg.color_track_label);
                        draw.add_text([arrow_x, text_y], track_col, arrow);
                        let arrow_w = calc_text_size(arrow)[0] + 4.0;
                        draw.add_text([arrow_x + arrow_w, text_y], track_col, track.name.as_str());
                    }

                    // Track header separator.
                    draw.add_line(
                        [content_x, y + cfg.track_header_height - 1.0],
                        [win_pos[0] + win_size[0], y + cfg.track_header_height - 1.0],
                        col32(cfg.color_track_separator),
                    )
                    .build();

                    // Spans.
                    if !track.collapsed {
                        let span_base_y = y + cfg.track_header_height;

                        for span in &track.spans {
                            let sx = self.time_to_x(span.start, content_x);
                            let ex = self.time_to_x(span.end, content_x);

                            if ex < content_x || sx > win_pos[0] + win_size[0] {
                                continue;
                            }

                            let span_w = (ex - sx).max(cfg.min_span_width);
                            let sy =
                                span_base_y + span.depth as f32 * (cfg.row_height + cfg.row_gap);
                            let ey = sy + cfg.row_height;

                            if ey < content_y || sy > win_pos[1] + win_size[1] {
                                continue;
                            }

                            let span_color = self.span_color(span, data_range);
                            draw.add_rect([sx, sy], [sx + span_w, ey], col32(span_color))
                                .filled(true)
                                .build();

                            // Span text (only if wide enough).
                            if span_w > 20.0 {
                                let text_size = calc_text_size(&span.label);
                                if text_size[0] < span_w - 4.0 {
                                    let tx = sx + (span_w - text_size[0]) * 0.5;
                                    let ty = sy + (cfg.row_height - text_size[1]) * 0.5;
                                    draw.add_text(
                                        [tx, ty],
                                        col32(cfg.color_span_text),
                                        &span.label,
                                    );
                                } else if span_w > 6.0 {
                                    let ty = sy + (cfg.row_height - line_height(ui)) * 0.5;
                                    draw.add_text(
                                        [sx + 2.0, ty],
                                        col32(cfg.color_span_text),
                                        &span.label,
                                    );
                                }
                            }

                            // Hover / click detection.
                            if in_content
                                && mouse_pos[0] >= sx
                                && mouse_pos[0] < sx + span_w
                                && mouse_pos[1] >= sy
                                && mouse_pos[1] < ey
                            {
                                self.hovered_span = Some(span.id);

                                draw.add_rect([sx, sy], [sx + span_w, ey], col32(cfg.color_hover))
                                    .build();

                                if ui.is_mouse_clicked(MouseButton::Left) {
                                    self.selected_span = Some(span.id);
                                    events.push(TimelineEvent::SpanClicked { span_id: span.id });
                                }
                                if ui.is_mouse_double_clicked(MouseButton::Left) {
                                    events.push(TimelineEvent::SpanDoubleClicked {
                                        span_id: span.id,
                                    });
                                }

                                // Tooltip.
                                if cfg.show_tooltip {
                                    let s = crate::i18n::timeline::strings(cfg.locale);
                                    let locale = cfg.locale;
                                    crate::utils::themed_tooltip(ui, || {
                                        let dur = span.duration();
                                        let (val, suffix) = format_duration(dur);
                                        ui.text(format!(
                                            "{} \u{2014} {:.2}{}",
                                            span.label, val, suffix
                                        ));
                                        if !span.category.is_empty() && span.category != span.label
                                        {
                                            ui.text(format!(
                                                "{}{}",
                                                s.category_label, span.category
                                            ));
                                        }
                                        if let Some(ref src) = span.source {
                                            ui.text(format!("{}{}", s.source_label, src));
                                        }
                                        ui.text(crate::i18n::timeline::start_end(
                                            locale,
                                            span.start * 1000.0,
                                            span.end * 1000.0,
                                        ));
                                        ui.text(format!("{}{}", s.depth_label, span.depth));
                                    });
                                }
                            }

                            // Selection outline.
                            if self.selected_span == Some(span.id) {
                                draw.add_rect(
                                    [sx - 1.0, sy - 1.0],
                                    [sx + span_w + 1.0, ey + 1.0],
                                    col32(cfg.color_selection),
                                )
                                .build();
                            }
                        }
                    }

                    y += track_h;
                }

                // ── Markers ─────────────────────────────────────────────
                if cfg.show_markers {
                    for (mi, marker) in self.markers.iter().enumerate() {
                        let mx = self.time_to_x(marker.time, content_x);
                        if mx < content_x || mx > win_pos[0] + win_size[0] {
                            continue;
                        }
                        let mc = marker.color.unwrap_or(cfg.color_marker);
                        draw.add_line([mx, win_pos[1]], [mx, win_pos[1] + win_size[1]], col32(mc))
                            .build();

                        draw.add_text([mx + 2.0, win_pos[1] + 2.0], col32(mc), &marker.label);

                        if in_content
                            && (mouse_pos[0] - mx).abs() < 4.0
                            && ui.is_mouse_clicked(MouseButton::Left)
                        {
                            events.push(TimelineEvent::MarkerClicked { index: mi });
                        }
                    }
                }

                // Dummy for scroll extent.
                let total_h = self.total_content_height();
                ui.set_cursor_pos([0.0, total_h + ruler_h]);
                ui.dummy([1.0, 1.0]);
            });

        events
    }
}
