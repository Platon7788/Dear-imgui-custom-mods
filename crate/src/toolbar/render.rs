//! Per-frame toolbar rendering — two-pass layout (measure → draw),
//! draw-list painting, hover hit-testing, and event emission.

use dear_imgui_rs::{MouseButton, Ui};

use super::item::{display_text, display_text_ref};
use super::layout::{item_advance_width, separator_advance_width, spacer_width};
use super::{Toolbar, ToolbarEvent, ToolbarItemKind, col32};
use crate::utils::text::calc_text_size;

impl Toolbar {
    /// Render the toolbar. Returns events for this frame.
    pub fn render(&mut self, ui: &Ui) -> Vec<ToolbarEvent> {
        let mut events = Vec::new();
        let cfg = self.config;

        let _id_tok = ui.push_id(&self.id);

        let avail_w = ui.content_region_avail()[0];
        let bar_h = cfg.height;
        let cursor = ui.cursor_screen_pos();
        let draw = ui.get_window_draw_list();

        // Background
        draw.add_rect(
            cursor,
            [cursor[0] + avail_w, cursor[1] + bar_h],
            col32(cfg.color_bg),
        )
        .filled(true)
        .build();

        // Bottom border
        draw.add_line(
            [cursor[0], cursor[1] + bar_h - 1.0],
            [cursor[0] + avail_w, cursor[1] + bar_h - 1.0],
            col32(cfg.color_border),
        )
        .build();

        let mouse_pos = ui.io().mouse_pos();
        let window_hovered = ui.is_window_hovered();
        let btn_h = bar_h - 6.0;
        let btn_y = cursor[1] + 3.0;

        // First pass: tally fixed (non-spacer) width so the spacers can
        // absorb the slack. Seed with the leading `item_spacing` the
        // second pass emits before the first item — otherwise the spacer
        // distribution overshoots by one gap and spacer-anchored items
        // run past the right edge.
        let mut fixed_w = cfg.item_spacing;
        let mut spacer_count = 0;
        for item in &self.items {
            match &item.kind {
                ToolbarItemKind::Spacer => spacer_count += 1,
                ToolbarItemKind::Separator => {
                    fixed_w += separator_advance_width(&cfg);
                }
                ToolbarItemKind::Dropdown { options, selected } => {
                    let base = display_text(item);
                    let label = if *selected < options.len() {
                        format!("{} [{}]", base, options[*selected])
                    } else {
                        base.into_owned()
                    };
                    fixed_w += item_advance_width(calc_text_size(&label)[0], &cfg);
                }
                _ => {
                    let text = display_text(item);
                    fixed_w += item_advance_width(calc_text_size(&text)[0], &cfg);
                }
            }
        }

        let spacer_w = spacer_width(avail_w, fixed_w, spacer_count);

        // Second pass: render
        let mut x = cursor[0] + cfg.item_spacing;

        for (idx, item) in self.items.iter_mut().enumerate() {
            // Separator and Spacer have no display text — handle them first.
            match &mut item.kind {
                ToolbarItemKind::Separator => {
                    x += cfg.separator_margin;
                    draw.add_line(
                        [x, btn_y + 2.0],
                        [x, btn_y + btn_h - 2.0],
                        col32(cfg.color_separator),
                    )
                    .build();
                    x += cfg.separator_width + cfg.separator_margin;
                    continue;
                }
                ToolbarItemKind::Spacer => {
                    x += spacer_w;
                    continue;
                }
                _ => {}
            }

            // Shared pre-computation for Button / Toggle / Dropdown
            let base_display = display_text_ref(&item.icon, &item.label);
            let full_display: std::borrow::Cow<'_, str> = match &item.kind {
                ToolbarItemKind::Dropdown { options, selected } => {
                    if *selected < options.len() {
                        std::borrow::Cow::Owned(format!(
                            "{} [{}]",
                            base_display, options[*selected]
                        ))
                    } else {
                        base_display.clone()
                    }
                }
                _ => base_display.clone(),
            };
            let text_sz = calc_text_size(&full_display);
            let text_w = text_sz[0];
            let btn_w = text_w + cfg.button_padding * 2.0;

            let hovered = item.enabled
                && window_hovered
                && mouse_pos[0] >= x
                && mouse_pos[0] < x + btn_w
                && mouse_pos[1] >= btn_y
                && mouse_pos[1] < btn_y + btn_h;

            let text_color = if item.enabled {
                cfg.color_text
            } else {
                cfg.color_disabled
            };

            match &mut item.kind {
                ToolbarItemKind::Button if hovered => {
                    let bg = if ui.is_mouse_down(MouseButton::Left) {
                        cfg.color_active
                    } else {
                        cfg.color_hover
                    };
                    draw.add_rect([x, btn_y], [x + btn_w, btn_y + btn_h], col32(bg))
                        .rounding(cfg.button_rounding)
                        .filled(true)
                        .build();

                    // Hover underline
                    let uy = btn_y + btn_h - 1.0;
                    draw.add_line(
                        [x + 2.0, uy],
                        [x + btn_w - 2.0, uy],
                        col32(cfg.color_hover_underline),
                    )
                    .thickness(cfg.hover_underline_thickness)
                    .build();

                    if ui.is_mouse_clicked(MouseButton::Left) {
                        events.push(ToolbarEvent::ButtonClicked {
                            index: idx,
                            label: item.label.clone(), // clone only on event (not per-frame)
                        });
                    }

                    if !item.tooltip.is_empty() {
                        crate::utils::themed_tooltip(ui, || ui.text(&item.tooltip));
                    }
                }
                ToolbarItemKind::Button => {}

                ToolbarItemKind::Toggle { on } => {
                    // Toggle background
                    if *on {
                        draw.add_rect(
                            [x, btn_y],
                            [x + btn_w, btn_y + btn_h],
                            col32(cfg.color_toggled),
                        )
                        .rounding(cfg.button_rounding)
                        .filled(true)
                        .build();
                    }

                    if hovered {
                        let bg = if ui.is_mouse_down(MouseButton::Left) {
                            cfg.color_active
                        } else {
                            cfg.color_hover
                        };
                        draw.add_rect([x, btn_y], [x + btn_w, btn_y + btn_h], col32(bg))
                            .rounding(cfg.button_rounding)
                            .filled(true)
                            .build();

                        // Hover underline
                        let uy = btn_y + btn_h - 1.0;
                        draw.add_line(
                            [x + 2.0, uy],
                            [x + btn_w - 2.0, uy],
                            col32(cfg.color_hover_underline),
                        )
                        .thickness(cfg.hover_underline_thickness)
                        .build();

                        if ui.is_mouse_clicked(MouseButton::Left) {
                            *on = !*on;
                            events.push(ToolbarEvent::Toggled {
                                index: idx,
                                label: item.label.clone(), // clone only on event (not per-frame)
                                on: *on,
                            });
                        }

                        if !item.tooltip.is_empty() {
                            crate::utils::themed_tooltip(ui, || ui.text(&item.tooltip));
                        }
                    }
                }

                ToolbarItemKind::Dropdown { options, selected } if hovered => {
                    let bg = if ui.is_mouse_down(MouseButton::Left) {
                        cfg.color_active
                    } else {
                        cfg.color_hover
                    };
                    draw.add_rect([x, btn_y], [x + btn_w, btn_y + btn_h], col32(bg))
                        .rounding(cfg.button_rounding)
                        .filled(true)
                        .build();

                    // Hover underline
                    let uy = btn_y + btn_h - 1.0;
                    draw.add_line(
                        [x + 2.0, uy],
                        [x + btn_w - 2.0, uy],
                        col32(cfg.color_hover_underline),
                    )
                    .thickness(cfg.hover_underline_thickness)
                    .build();

                    if ui.is_mouse_clicked(MouseButton::Left) && !options.is_empty() {
                        *selected = (*selected + 1) % options.len();
                        events.push(ToolbarEvent::DropdownChanged {
                            index: idx,
                            label: item.label.clone(), // clone only on event (not per-frame)
                            selected: *selected,
                        });
                    }

                    if !item.tooltip.is_empty() {
                        crate::utils::themed_tooltip(ui, || ui.text(&item.tooltip));
                    }
                }
                ToolbarItemKind::Dropdown { .. } => {}

                // Separator/Spacer already handled above via `continue`.
                _ => {}
            }

            // Draw the display text (shared across all interactive item types).
            let tx = x + (btn_w - text_sz[0]) * 0.5;
            let ty = btn_y + (btn_h - text_sz[1]) * 0.5;
            draw.add_text([tx, ty], col32(text_color), &full_display);

            x += btn_w + cfg.item_spacing;
        }

        // Advance cursor past the toolbar
        ui.set_cursor_pos([ui.cursor_pos()[0], ui.cursor_pos()[1] + bar_h]);
        ui.dummy([0.0, 0.0]);

        events
    }
}
