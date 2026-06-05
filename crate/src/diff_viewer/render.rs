//! Per-frame rendering for [`DiffViewer`] — header toolbar, the two
//! side-by-side panels (with synchronized scroll), and the unified
//! single-pane view. Split out of `mod.rs` to keep that file focused
//! on state and the public API. All draw work goes through the
//! window draw-list and is viewport-culled (see
//! [`visible_range`](super::visible_range)).

use super::*;

impl DiffViewer {
    // ── Render ──────────────────────────────────────────────────────────────

    /// Render the diff viewer.
    pub fn render(&mut self, ui: &Ui) -> Vec<DiffViewerEvent> {
        let mut events = Vec::new();

        // Cache metrics. The row height comes straight from the font
        // size via `text_line_height()` (a context read) instead of a
        // per-frame `igCalcTextSize` glyph walk; only the monospace
        // char advance still needs a measurement.
        self.char_advance = calc_text_size("M")[0];
        self.line_height = ui.text_line_height() + 2.0;

        let _id_tok = ui.push_id(&self.id);
        let cfg = self.config.clone();

        // Header
        self.render_header(ui, &cfg, &mut events);

        let avail = ui.content_region_avail();

        match cfg.mode {
            DiffMode::SideBySide => {
                let panel_w = (avail[0] - 2.0) * 0.5;

                // Borrow lines as plain `&[DisplayLine]` slices —
                // disjoint immutable borrows of `self.left_lines`
                // / `self.right_lines`, both alive for the duration
                // of the two `child_window().build(...)` closures
                // (which only *read* the slices and never touch
                // `self`). Replaces a historic `from_raw_parts`
                // unsafe block whose SAFETY note ("not mutated
                // during render") was correct but unenforceable
                // — any future refactor that mutates the lines
                // mid-render would silently invoke UB.
                let left_slice: &[DisplayLine] = &self.left_lines;
                let right_slice: &[DisplayLine] = &self.right_lines;

                let char_advance = self.char_advance;
                let line_height = self.line_height;

                let sync = cfg.sync_scroll;
                let mut left_scroll_y = 0.0f32;
                let mut right_scroll_y = 0.0f32;
                let saved_sync_y = self.sync_scroll_y;

                // Left panel
                ui.child_window("##diff_left")
                    .size([panel_w, avail[1]])
                    .build(ui, || {
                        if sync {
                            let current = ui.scroll_y();
                            if (current - saved_sync_y).abs() > 0.5 {
                                // User scrolled the left panel — it's the source.
                                left_scroll_y = current;
                            } else {
                                ui.set_scroll_y(saved_sync_y);
                                left_scroll_y = saved_sync_y;
                            }
                        }
                        Self::render_panel_static(
                            ui,
                            &cfg,
                            left_slice,
                            true,
                            char_advance,
                            line_height,
                        );
                    });

                ui.same_line_with_spacing(0.0, 0.0);

                // Separator
                {
                    let cursor = ui.cursor_screen_pos();
                    let draw = ui.get_window_draw_list();
                    draw.add_line(
                        cursor,
                        [cursor[0], cursor[1] + avail[1]],
                        col32(cfg.color_separator),
                    )
                    .build();
                }

                ui.same_line();

                // Right panel
                ui.child_window("##diff_right")
                    .size([panel_w, avail[1]])
                    .build(ui, || {
                        if sync {
                            let current = ui.scroll_y();
                            if (current - saved_sync_y).abs() > 0.5 {
                                // User scrolled the right panel.
                                right_scroll_y = current;
                            } else {
                                ui.set_scroll_y(saved_sync_y);
                                right_scroll_y = saved_sync_y;
                            }
                        }
                        Self::render_panel_static(
                            ui,
                            &cfg,
                            right_slice,
                            false,
                            char_advance,
                            line_height,
                        );
                    });

                // Update sync scroll position: whichever panel the user scrolled.
                if sync {
                    if (left_scroll_y - saved_sync_y).abs() > 0.5 {
                        self.sync_scroll_y = left_scroll_y;
                    } else if (right_scroll_y - saved_sync_y).abs() > 0.5 {
                        self.sync_scroll_y = right_scroll_y;
                    }
                }
            }
            DiffMode::Unified => {
                ui.child_window("##diff_unified").size(avail).build(ui, || {
                    self.render_unified(ui, &cfg);
                });
            }
        }

        events
    }

    fn render_header(
        &mut self,
        ui: &Ui,
        cfg: &DiffViewerConfig,
        events: &mut Vec<DiffViewerEvent>,
    ) {
        // Navigation and stats
        let s = &self.stats;
        ui.text_colored(
            cfg.color_header,
            format!(
                "{} vs {}  |  +{} -{} ~{}  |  {} hunks",
                self.old_label,
                self.new_label,
                s.added,
                s.removed,
                s.modified,
                self.hunks.len(),
            ),
        );

        ui.same_line();
        let s = crate::i18n::diff_viewer::strings(self.config.locale);
        if ui.button(s.prev_button) {
            self.prev_hunk();
            events.push(DiffViewerEvent::HunkSelected {
                index: self.current_hunk,
            });
        }
        ui.same_line();
        if ui.button(s.next_button) {
            self.next_hunk();
            events.push(DiffViewerEvent::HunkSelected {
                index: self.current_hunk,
            });
        }

        if !self.hunks.is_empty() {
            ui.same_line();
            ui.text_colored(
                cfg.color_line_number,
                format!("  Hunk {}/{}", self.current_hunk + 1, self.hunks.len()),
            );
        }

        ui.separator();
    }

    fn render_panel_static(
        ui: &Ui,
        cfg: &DiffViewerConfig,
        lines: &[DisplayLine],
        is_left: bool,
        char_advance: f32,
        line_height: f32,
    ) {
        let draw = ui.get_window_draw_list();
        let win_pos = ui.cursor_screen_pos();
        let avail = ui.content_region_avail();
        let win_w = avail[0];

        let gutter_w = if cfg.show_line_numbers {
            char_advance * 5.0
        } else {
            0.0
        };

        // Viewport culling: only iterate the rows that intersect the
        // visible scroll region (plus one row of slack each side).
        // Without this every line — even far off-screen ones — paid a
        // `draw.add_text` + `format!` per frame.
        let mouse_pos = ui.io().mouse_pos();
        let scroll_y = ui.scroll_y();
        let (first, last) = visible_range(scroll_y, avail[1], line_height, lines.len());

        for (offset, line) in lines[first..last].iter().enumerate() {
            let vi = first + offset;
            let y = win_pos[1] + vi as f32 * line_height;

            // Background
            let bg = match line.kind {
                LineKind::Added => Some(cfg.color_added_bg),
                LineKind::Removed => Some(cfg.color_removed_bg),
                LineKind::FoldMarker => Some(cfg.color_fold),
                LineKind::Equal => None,
            };
            if let Some(bg_color) = bg {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + win_w, y + line_height],
                    col32(bg_color),
                )
                .filled(true)
                .build();
            }

            // Hover row highlight
            let row_hovered = mouse_pos[1] >= y
                && mouse_pos[1] < y + line_height
                && mouse_pos[0] >= win_pos[0]
                && mouse_pos[0] < win_pos[0] + win_w;
            if row_hovered {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + win_w, y + line_height],
                    col32([1.0, 1.0, 1.0, 0.04]),
                )
                .filled(true)
                .build();
            }

            // Gutter background
            if cfg.show_line_numbers {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + gutter_w, y + line_height],
                    col32(cfg.color_gutter_bg),
                )
                .filled(true)
                .build();
            }

            // Line number
            if cfg.show_line_numbers {
                let num = if is_left { line.old_num } else { line.new_num };
                if let Some(n) = num {
                    draw.add_text(
                        [win_pos[0] + 2.0, y],
                        col32(cfg.color_line_number),
                        format!("{n:>4}"),
                    );
                }
            }

            // Text
            let text_x = win_pos[0] + gutter_w + 4.0;
            let text_color = match line.kind {
                LineKind::FoldMarker => cfg.color_fold,
                LineKind::Added => cfg.color_added_text,
                LineKind::Removed => cfg.color_removed_text,
                LineKind::Equal => cfg.color_text,
            };
            draw.add_text([text_x, y], col32(text_color), &line.text);
        }

        // Dummy for scroll extent
        let total_h = lines.len() as f32 * line_height;
        ui.set_cursor_pos([0.0, total_h]);
        ui.dummy([1.0, 1.0]);
    }

    fn render_unified(&self, ui: &Ui, cfg: &DiffViewerConfig) {
        let draw = ui.get_window_draw_list();
        let win_pos = ui.cursor_screen_pos();
        let avail = ui.content_region_avail();
        let win_w = avail[0];

        let gutter_w = if cfg.show_line_numbers {
            self.char_advance * 10.0 // old + new numbers
        } else {
            0.0
        };

        // In unified mode, interleave left and right lines.
        // `left_lines` carry `old_num`, `right_lines` carry `new_num`.
        let line_count = self.left_lines.len().min(self.right_lines.len());

        // Hoist per-frame reads out of the row loop and cull to the
        // visible scroll window (see `render_panel_static`).
        let mouse_pos = ui.io().mouse_pos();
        let scroll_y = ui.scroll_y();
        let (first, last) = visible_range(scroll_y, avail[1], self.line_height, line_count);

        for vi in first..last {
            let left = &self.left_lines[vi];
            let right = &self.right_lines[vi];
            let y = win_pos[1] + vi as f32 * self.line_height;

            let (kind, text) = if left.kind == LineKind::FoldMarker {
                (LineKind::FoldMarker, &left.text)
            } else if left.kind == LineKind::Removed {
                (LineKind::Removed, &left.text)
            } else if right.kind == LineKind::Added {
                (LineKind::Added, &right.text)
            } else {
                (LineKind::Equal, &left.text)
            };

            // Background
            let bg = match kind {
                LineKind::Added => Some(cfg.color_added_bg),
                LineKind::Removed => Some(cfg.color_removed_bg),
                LineKind::FoldMarker => Some(cfg.color_fold),
                LineKind::Equal => None,
            };
            if let Some(bg_color) = bg {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + win_w, y + self.line_height],
                    col32(bg_color),
                )
                .filled(true)
                .build();
            }

            // Hover row highlight
            let row_hovered = mouse_pos[1] >= y
                && mouse_pos[1] < y + self.line_height
                && mouse_pos[0] >= win_pos[0]
                && mouse_pos[0] < win_pos[0] + win_w;
            if row_hovered {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + win_w, y + self.line_height],
                    col32([1.0, 1.0, 1.0, 0.04]),
                )
                .filled(true)
                .build();
            }

            // Current hunk accent bar
            if !self.hunks.is_empty() {
                let hunk = &self.hunks[self.current_hunk];
                let in_hunk = match (left.old_num, right.new_num) {
                    (Some(n), _) if n > hunk.old_start && n <= hunk.old_start + hunk.old_count => {
                        true
                    }
                    (_, Some(n)) if n > hunk.new_start && n <= hunk.new_start + hunk.new_count => {
                        true
                    }
                    _ => false,
                };
                if in_hunk {
                    draw.add_rect(
                        [win_pos[0], y],
                        [win_pos[0] + 3.0, y + self.line_height],
                        col32([0.40, 0.63, 0.88, 0.8]),
                    )
                    .filled(true)
                    .build();
                }
            }

            // Line numbers (old | new)
            if cfg.show_line_numbers {
                draw.add_rect(
                    [win_pos[0], y],
                    [win_pos[0] + gutter_w, y + self.line_height],
                    col32(cfg.color_gutter_bg),
                )
                .filled(true)
                .build();

                if let Some(n) = left.old_num {
                    draw.add_text(
                        [win_pos[0] + 2.0, y],
                        col32(cfg.color_line_number),
                        format!("{n:>4}"),
                    );
                }
                if let Some(n) = right.new_num {
                    draw.add_text(
                        [win_pos[0] + self.char_advance * 5.0, y],
                        col32(cfg.color_line_number),
                        format!("{n:>4}"),
                    );
                }
            }

            // Prefix
            let prefix = match kind {
                LineKind::Added => "+ ",
                LineKind::Removed => "- ",
                LineKind::FoldMarker => "  ",
                LineKind::Equal => "  ",
            };
            let text_x = win_pos[0] + gutter_w + 2.0;
            let prefix_w = self.char_advance * 2.0;
            let text_color = match kind {
                LineKind::Added => cfg.color_added_text,
                LineKind::Removed => cfg.color_removed_text,
                LineKind::FoldMarker => cfg.color_fold,
                LineKind::Equal => cfg.color_text,
            };
            draw.add_text([text_x, y], col32(text_color), prefix);
            draw.add_text([text_x + prefix_w, y], col32(text_color), text);
        }

        let total_h = line_count as f32 * self.line_height;
        ui.set_cursor_pos([0.0, total_h]);
        ui.dummy([1.0, 1.0]);
    }
}
