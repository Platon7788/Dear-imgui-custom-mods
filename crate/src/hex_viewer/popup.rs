//! Goto + search popups.
//!
//! Both popups are stateless wrappers over a single ImGui input field;
//! they live separately from `draw.rs` so the row-drawing hot path is
//! easier to read. The data inspector stays in `draw.rs` because it's
//! an inline overlay below the buffer, not a floating popup.

use super::HexViewer;
use super::config::{BytesPerRow, HexSearchMode, StringEncoding};
use super::search::parse_address;
use crate::i18n;
use crate::utils::popup::{
    action_row_labeled, anchor_next_popup_centred, anchor_next_popup_topleft, compact_popup_body,
    selected_button, themed_popup_style,
};

// Approximate body width used for centring / right-anchoring the
// goto / search popup buttons. Real popup width is auto-sized by
// ImGui based on content, but the input field reserves
// `INPUT_WIDTH + window_padding` so we use that as a good-enough
// reference for laying out the action row below it.
//
// Bumped from `240` → `360` on 2026-04-29 (~+50 %) — the encoding
// row in the search popup (Hex / String / ASCII / UTF-8 / UTF-16LE)
// pushed past the previous width and forced ImGui to auto-grow
// asymmetrically. Wider also gives UTF-16LE / Cyrillic search
// queries more horizontal breathing room.
const POPUP_INPUT_WIDTH: f32 = 360.0;

// `anchor_next_popup_centred` / `anchor_next_popup_topleft` moved to
// `crate::utils::popup` on 2026-05-04 so both `hex_viewer` and
// `disasm_view` share the same implementation. Imported above.

/// Pseudo-radio mode pill — auto-fit-width wrapper around the
/// crate-wide [`crate::utils::popup::selected_button`] helper. Kept
/// as a local alias so the search-popup body reads
/// `mode_pill(ui, "ASCII", is_ascii)` instead of the longer
/// `selected_button(ui, "ASCII", [0.0, 0.0], is_ascii)`. All actual
/// styling (BLUE accent, ±0.06 hover/active lift) lives in
/// `utils::popup` — single source of truth shared with
/// `success_button` / `danger_button`.
fn mode_pill(ui: &dear_imgui_rs::Ui, label: &str, selected: bool) -> bool {
    selected_button(ui, label, [0.0, 0.0], selected)
}

// `render_action_row` and `compact_popup_body` graduated to
// `crate::utils::popup` on 2026-04-29 so disasm_view (and any
// future popup-host widget) can reuse the same geometry without
// drift. See the import block above.

impl HexViewer {
    pub(super) fn render_goto_popup(&mut self, ui: &dear_imgui_rs::Ui) {
        // CRITICAL: `BeginPopup` must run **every frame** the popup
        // is open — calling it only on the open-trigger frame (the
        // bug we had until 2026-04-29) made the popup flash for
        // exactly one frame and disappear. The `OpenPopup` call
        // stays gated on the trigger flag because that's the
        // edge-only "please open now" signal; `BeginPopup` is the
        // always-on "render-the-body-if-open" check.
        if self.show_goto {
            // Centre on the hex viewer so the popup always lands in
            // the visual middle, no matter where the user clicked or
            // pressed Ctrl+G from.
            anchor_next_popup_centred(self.component_center);
            ui.open_popup(&self.goto_popup_id);
            self.show_goto = false;
            self.goto_focus_pending = true;
        }

        let s = self.strings();
        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.goto_popup_id) {
                compact_popup_body(ui, || {
                    ui.text(s.goto_title);

                    if self.goto_focus_pending {
                        ui.set_keyboard_focus_here();
                        self.goto_focus_pending = false;
                    }
                    ui.set_next_item_width(POPUP_INPUT_WIDTH);
                    ui.input_text("##goto_input", &mut self.goto_buf).build();

                    let (go_clicked, cancel_clicked) =
                        action_row_labeled(ui, POPUP_INPUT_WIDTH, s.action_go, s.action_cancel);
                    if cancel_clicked {
                        ui.close_current_popup();
                    }
                    if go_clicked {
                        if let Some(addr) = parse_address(&self.goto_buf) {
                            // BUG-127 (2026-05-15): VA-aware goto.
                            //
                            // If the parsed address lies inside the buffer,
                            // jump there directly (legacy in-buffer goto).
                            // Otherwise — and only if the host installed a
                            // `va_goto_callback` — defer to the host so it
                            // can re-anchor the window. Without a callback
                            // we fall back to the legacy clamp behaviour
                            // for binary-editor-style hosts.
                            // Use `effective_data_len` so streaming
                            // providers (whose `self.data` may be
                            // empty / lagging) still resolve "is the
                            // typed VA inside the current visible
                            // window?" correctly.
                            let base = self.config.base_address;
                            let buf_end = base.saturating_add(self.effective_data_len as u64);
                            let in_buffer = addr >= base && addr < buf_end;
                            if in_buffer {
                                let offset = (addr - base) as usize;
                                self.goto(offset);
                            } else if let Some(cb) = self.va_goto_callback.as_mut() {
                                cb(addr);
                            } else {
                                // No callback: clamp into buffer (legacy).
                                let offset = addr.saturating_sub(base) as usize;
                                self.goto(offset);
                            }
                        }
                        ui.close_current_popup();
                    }
                });
            }
        });
    }

    pub(super) fn render_search_popup(&mut self, ui: &dear_imgui_rs::Ui) {
        if self.show_search {
            anchor_next_popup_centred(self.component_center);
            ui.open_popup(&self.search_popup_id);
            self.show_search = false;
            self.search_focus_pending = true;
        }

        let s = self.strings();
        let locale = self.config.locale;
        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.search_popup_id) {
                compact_popup_body(ui, || {
                    let is_hex = matches!(self.config.search_mode, HexSearchMode::Hex);
                    let is_string = self.config.search_mode.is_string();

                    if mode_pill(ui, s.mode_hex, is_hex) {
                        self.config.search_mode = HexSearchMode::Hex;
                    }
                    ui.same_line();
                    if mode_pill(ui, s.mode_string, is_string) {
                        let encoding = match self.config.search_mode {
                            HexSearchMode::String(e) => e,
                            HexSearchMode::Hex => StringEncoding::Ascii,
                        };
                        self.config.search_mode = HexSearchMode::String(encoding);
                    }

                    if let HexSearchMode::String(current) = self.config.search_mode {
                        ui.same_line();
                        ui.dummy([12.0, 0.0]);
                        for &enc in StringEncoding::ALL.iter() {
                            ui.same_line();
                            // Encoding labels (`ASCII` / `UTF-8` /
                            // `UTF-16LE`) are technical abbreviations
                            // — kept untranslated.
                            if mode_pill(ui, enc.display_name(), enc == current) {
                                self.config.search_mode = HexSearchMode::String(enc);
                            }
                        }
                    }

                    let hint = match self.config.search_mode {
                        HexSearchMode::Hex => s.hint_hex,
                        HexSearchMode::String(StringEncoding::Ascii) => s.hint_ascii,
                        HexSearchMode::String(StringEncoding::Utf8) => s.hint_utf8,
                        HexSearchMode::String(StringEncoding::Utf16Le) => s.hint_utf16le,
                    };
                    ui.text(hint);

                    if self.search_focus_pending {
                        ui.set_keyboard_focus_here();
                        self.search_focus_pending = false;
                    }
                    ui.set_next_item_width(POPUP_INPUT_WIDTH);
                    ui.input_text("##search_input", &mut self.search_buf)
                        .build();

                    if !self.search_results.is_empty() {
                        ui.text(i18n::hex_viewer::result_n_of_m(
                            locale,
                            self.search_idx + 1,
                            self.search_results.len(),
                        ));
                    }

                    let (find_clicked, cancel_clicked) =
                        action_row_labeled(ui, POPUP_INPUT_WIDTH, s.action_find, s.action_cancel);
                    if cancel_clicked {
                        ui.close_current_popup();
                    }
                    if find_clicked {
                        self.do_search();
                        if !self.search_results.is_empty() {
                            ui.close_current_popup();
                        }
                    }
                });
            }
        });
    }

    /// Right-click context menu — Go to Address / Back / Forward /
    /// Settings. The Back / Forward entries are greyed out (rendered
    /// at half alpha) when their respective stack is empty, so the
    /// user gets a visible "nothing to navigate to" cue without the
    /// menu re-laying-out.
    pub(super) fn render_context_menu(&mut self, ui: &dear_imgui_rs::Ui) {
        if self.show_context_menu {
            anchor_next_popup_topleft(self.popup_open_pos);
            ui.open_popup(&self.context_popup_id);
            self.show_context_menu = false;
        }

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.context_popup_id) {
                // Colour-code entries by action class — mirrors the
                // `disasm_view` context-menu pattern. Each
                // `menu_item` runs inside a
                // `push_style_color(StyleColor::Text, ...)` block
                // so the icon + label + shortcut all carry the
                // same hue. dear_imgui_rs paints `menu_item` as a
                // single colour, so this is the lightest path to
                // per-entry tinting (no manual `selectable +
                // text_colored` layout).
                //
                // Palette:
                //   Goto / Search / Step back / Step forward
                //                       -> `offset` (address-gutter accent)
                //   Settings            -> default text colour
                //
                // The shared accent for navigation entries makes
                // them read as one visual class (every "go
                // somewhere" verb), while Settings stays neutral
                // as the catch-all "more options" entry.
                let nav_col = self.config.color_offset;

                // ── Go to Address (mirrors Ctrl+G) ─────────────
                // Icon glyphs limited to ranges the default ImGui
                // font atlas reliably ships:
                //  - U+00AB / U+00BB `«` / `»`  (Latin-1 supplement)
                //  - U+2190 / U+2192 `←` / `→`  (Arrows block, verified
                //    rendering in this project's atlas)
                //  - U+2026 `…`                  (General Punctuation)
                let s = self.strings();
                {
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, nav_col);
                    if ui.menu_item(s.menu_goto) {
                        self.show_goto = true;
                        self.goto_buf.clear();
                        self.goto_focus_pending = true;
                        ui.close_current_popup();
                    }
                    if ui.menu_item(s.menu_search) {
                        self.show_search = true;
                        // search_focus_pending is also raised by the popup
                        // body itself when `show_search` is true on the
                        // open-trigger frame — kept here as belt-and-suspenders
                        // so the context-menu path doesn't depend on the popup
                        // body's flag-set order.
                        self.search_focus_pending = true;
                        ui.close_current_popup();
                    }
                }

                ui.separator();

                // ── Step back (Alt+Left) ─────────────────────
                let can_back = self.nav.can_go_back();
                {
                    let _back_dim = if !can_back {
                        Some(ui.push_style_var(dear_imgui_rs::StyleVar::Alpha(0.40)))
                    } else {
                        None
                    };
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, nav_col);
                    if ui.menu_item(s.menu_step_back) && can_back {
                        self.nav_back();
                        ui.close_current_popup();
                    }
                    drop(_back_dim);
                }

                // ── Step forward (Alt+Right) ─────────────────
                let can_fwd = self.nav.can_go_forward();
                {
                    let _fwd_dim = if !can_fwd {
                        Some(ui.push_style_var(dear_imgui_rs::StyleVar::Alpha(0.40)))
                    } else {
                        None
                    };
                    let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Text, nav_col);
                    if ui.menu_item(s.menu_step_forward) && can_fwd {
                        self.nav_forward();
                        ui.close_current_popup();
                    }
                    drop(_fwd_dim);
                }

                ui.separator();

                // ── Settings — default text colour ───────────
                // MDI `wrench-cog` (U+F1B91) when the host has the
                // MDI atlas loaded; ellipsis (`\u{2026}`) fallback
                // otherwise — gated by `config.icons_available` so
                // the entry never renders as a `?` placeholder.
                // Icon glyph + localised "Settings..." label.
                // The icon prefix isn't translatable (it's an icon),
                // so we synthesise the label per-frame instead of
                // baking the prefix into the catalogue.
                let icon = if self.config.icons_available {
                    "\u{F1B91}  "
                } else {
                    "\u{2026}  "
                };
                let settings_label = format!("{icon}{}", s.menu_settings);
                if ui.menu_item(&settings_label) {
                    self.show_settings = true;
                    ui.close_current_popup();
                }
            }
        });
    }

    /// Settings popup — exposes the most-tweaked layout / colour
    /// toggles inline so the user can adjust the viewer without the
    /// host app wiring its own settings panel. This is intentionally
    /// a small subset of [`crate::hex_viewer::HexViewerConfig`] —
    /// fields that change the visible byte grid (BPR, ASCII column,
    /// inspector, dividers, byte categories, uppercase, address
    /// width). Anything more exotic stays on the config struct for
    /// programmatic access.
    pub(super) fn render_settings_popup(&mut self, ui: &dear_imgui_rs::Ui) {
        // `show_settings` is the one-shot open trigger (set when the
        // user picks "Settings..." in the context menu). Once the
        // popup is opened ImGui owns its lifecycle — close-on-
        // outside-click, ESC, and the explicit Close button below
        // all funnel through `close_current_popup`. Resetting
        // `show_settings` on the same frame avoids the popup
        // perpetually re-opening if the user clicks outside (ImGui
        // closes it, but our flag would still be `true`).
        if self.show_settings {
            anchor_next_popup_centred(self.component_center);
            ui.open_popup(&self.settings_popup_id);
            self.show_settings = false;
        }

        let s = self.strings();
        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.settings_popup_id) {
                compact_popup_body(ui, || {
                    let icon = if self.config.icons_available {
                        "\u{F1B91}  "
                    } else {
                        "\u{2026}  "
                    };
                    let header = format!("{icon}{}", s.settings_title);
                    ui.text(&header);
                    ui.separator();

                    ui.text(s.settings_bytes_per_row);
                    let bpr_btn_size = [32.0_f32, 0.0];
                    let current_bpr = self.config.bytes_per_row.value();
                    for (i, preset) in BytesPerRow::ALL.iter().enumerate() {
                        if i > 0 {
                            ui.same_line();
                        }
                        let is_current = preset.value() == current_bpr;
                        if selected_button(ui, preset.display_name(), bpr_btn_size, is_current)
                            && !is_current
                        {
                            self.config.bytes_per_row = *preset;
                        }
                    }

                    ui.separator();
                    ui.text(s.settings_display);
                    ui.checkbox(s.settings_show_ascii, &mut self.config.show_ascii);
                    ui.checkbox(s.settings_show_inspector, &mut self.config.show_inspector);
                    ui.checkbox(s.settings_show_offsets, &mut self.config.show_offsets);
                    ui.checkbox(
                        s.settings_show_column_headers,
                        &mut self.config.show_column_headers,
                    );
                    ui.checkbox(
                        s.settings_show_column_dividers,
                        &mut self.config.show_column_dividers,
                    );
                    ui.checkbox(
                        s.settings_show_group_dividers,
                        &mut self.config.show_group_dividers,
                    );
                    ui.checkbox(s.settings_show_splitter, &mut self.config.show_splitter);

                    ui.separator();
                    ui.text(s.settings_format);
                    ui.checkbox(s.settings_uppercase, &mut self.config.uppercase);
                    ui.checkbox(s.settings_category_colors, &mut self.config.category_colors);
                    ui.checkbox(s.settings_dim_zeros, &mut self.config.dim_zeros);

                    ui.separator();

                    let total_w = ui.content_region_avail()[0];
                    let close_w = 64.0_f32;
                    let close_x = ui.cursor_pos()[0] + (total_w - close_w - 2.0).max(0.0);
                    ui.set_cursor_pos_x(close_x);
                    if ui.button_with_size(s.action_close, [close_w, 0.0])
                        || ui.is_key_pressed(dear_imgui_rs::Key::Escape)
                    {
                        ui.close_current_popup();
                    }
                }); // compact_popup_body
            }
        });
    }
}
