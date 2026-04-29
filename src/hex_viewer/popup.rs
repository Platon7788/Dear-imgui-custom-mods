//! Goto + search popups.
//!
//! Both popups are stateless wrappers over a single ImGui input field;
//! they live separately from `draw.rs` so the row-drawing hot path is
//! easier to read. The data inspector stays in `draw.rs` because it's
//! an inline overlay below the buffer, not a floating popup.

use super::HexViewer;
use super::config::{BytesPerRow, HexSearchMode, StringEncoding};
use super::search::parse_address;
use crate::utils::popup::{success_button, themed_popup_style};

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
/// Compact action-button width (px). Trimmed from `76` → `58` on
/// the project owner's request — the popup felt button-heavy with
/// the wider variant, especially on a narrow-input layout.
const ACTION_BTN_WIDTH: f32 = 58.0;
const ACTION_BTN_HEIGHT: f32 = 0.0; // 0 → ImGui auto-height (= text + frame_padding)
/// Pixel gap from the popup's content edge to the closest button.
/// Same constant on both sides so the action row reads as visually
/// balanced even though the buttons themselves are pinned to
/// opposite ends.
const ACTION_EDGE_GAP: f32 = 2.0;

/// Push a tighter set of StyleVar guards over the body of an
/// already-`themed_popup_style`'d popup. Used by goto / search /
/// settings to render a denser layout (chec klists, multi-line
/// option rows) while still inheriting the popup's base padding /
/// rounding.
///
/// The guards drop when `body` returns — pop order is reverse of
/// push, no leakage to sibling popups.
fn compact_popup_body<F: FnOnce()>(ui: &dear_imgui_rs::Ui, body: F) {
    let _spc = ui.push_style_var(dear_imgui_rs::StyleVar::ItemSpacing([6.0, 4.0]));
    let _fp = ui.push_style_var(dear_imgui_rs::StyleVar::FramePadding([6.0, 3.0]));
    let _ip = ui.push_style_var(dear_imgui_rs::StyleVar::ItemInnerSpacing([4.0, 4.0]));
    body();
}

/// Anchor the next ImGui window (a popup) at `pos` in screen space
/// using the given `pivot` (where `(0, 0)` = top-left of the popup
/// snaps to `pos`, `(0.5, 0.5)` = popup centre snaps to `pos`,
/// `(1, 1)` = bottom-right snaps to `pos`).
///
/// `dear-imgui-rs` 0.11 exposes `set_window_pos` (operates on the
/// current window) but no `set_next_window_pos` builder. The
/// equivalent functionality lives in `igSetNextWindowPos` in the
/// raw bindings — call it directly with `ImGuiCond_Always` so the
/// anchor re-applies every frame (matters because ImGui caches
/// popup positions and the user might trigger the same popup from
/// a different click location during a session).
fn anchor_next_popup_at(pos: [f32; 2], pivot: [f32; 2]) {
    // SAFETY: `igSetNextWindowPos` is a side-effect-only ImGui
    // function that just stores the requested position in the
    // shared context — safe to call from any thread that owns
    // the active ImGui context (i.e. the main render thread, which
    // is where the hex viewer always runs).
    unsafe {
        #[allow(clippy::unnecessary_cast)]
        // `ImGuiCond_Always` is `u32` on Linux, `i32` on Windows.
        dear_imgui_rs::sys::igSetNextWindowPos(
            dear_imgui_rs::sys::ImVec2 {
                x: pos[0],
                y: pos[1],
            },
            dear_imgui_rs::sys::ImGuiCond_Always as i32,
            dear_imgui_rs::sys::ImVec2 {
                x: pivot[0],
                y: pivot[1],
            },
        );
    }
}

/// Anchor the next popup with its top-left at `pos`. Use for
/// click-position-aware popups (right-click context menu).
fn anchor_next_popup(pos: [f32; 2]) {
    anchor_next_popup_at(pos, [0.0, 0.0]);
}

/// Anchor the next popup with its **centre** at `pos`. Use for
/// modal-style popups (Goto / Search / Settings) that should sit
/// at the visual middle of the host viewer.
fn anchor_next_popup_centred(pos: [f32; 2]) {
    anchor_next_popup_at(pos, [0.5, 0.5]);
}

/// Render a small button styled as a "pseudo-radio" — if `selected`
/// is true, the button background is forced to a brighter accent
/// shade so the active option visually stands out among siblings;
/// otherwise renders as a normal button. Returns `true` on click.
///
/// Used by the search popup's mode + encoding rows to give the user
/// a clear "this is selected" cue without spending a full Combo
/// widget on a 2-3 option set.
fn mode_pill(ui: &dear_imgui_rs::Ui, label: &str, selected: bool) -> bool {
    if selected {
        let _c = ui.push_style_color(dear_imgui_rs::StyleColor::Button, [0.30, 0.50, 0.90, 1.0]);
        let _h = ui.push_style_color(
            dear_imgui_rs::StyleColor::ButtonHovered,
            [0.36, 0.56, 0.96, 1.0],
        );
        let _a = ui.push_style_color(
            dear_imgui_rs::StyleColor::ButtonActive,
            [0.24, 0.44, 0.84, 1.0],
        );
        ui.button(label)
    } else {
        ui.button(label)
    }
}

/// Action-row layout shared by goto / search popups: Cancel pinned
/// `ACTION_EDGE_GAP` from the left, the green primary button pinned
/// the same distance from the right. Returns `(primary_clicked,
/// cancel_clicked)` so each popup body can dispatch its own logic
/// without re-implementing the geometry.
///
/// `primary_label` is the body of the green button (e.g. `"Go"`,
/// `"Find"`). `body_w` is the reference width used for the right
/// anchor; passing `POPUP_INPUT_WIDTH` keeps the action row
/// horizontally aligned with the input field above it.
///
/// Enter triggers `primary`, Escape triggers `cancel` — the
/// keyboard fallthrough happens here so each call site doesn't
/// have to thread `is_key_pressed` checks through its own button
/// expressions.
fn render_action_row(ui: &dear_imgui_rs::Ui, body_w: f32, primary_label: &str) -> (bool, bool) {
    let row_origin_x = ui.cursor_pos()[0];
    let cancel_x = row_origin_x + ACTION_EDGE_GAP;
    let primary_x = row_origin_x + body_w - ACTION_BTN_WIDTH - ACTION_EDGE_GAP;

    ui.set_cursor_pos_x(cancel_x);
    let cancel_clicked = ui.button_with_size("Cancel", [ACTION_BTN_WIDTH, ACTION_BTN_HEIGHT])
        || ui.is_key_pressed(dear_imgui_rs::Key::Escape);

    ui.same_line();
    ui.set_cursor_pos_x(primary_x);
    let primary_clicked = success_button(ui, primary_label, [ACTION_BTN_WIDTH, ACTION_BTN_HEIGHT])
        || ui.is_key_pressed(dear_imgui_rs::Key::Enter);

    (primary_clicked, cancel_clicked)
}

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

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.goto_popup_id) {
                compact_popup_body(ui, || {
                    ui.text("Goto address (hex or decimal):");

                    if self.goto_focus_pending {
                        ui.set_keyboard_focus_here();
                        self.goto_focus_pending = false;
                    }
                    ui.set_next_item_width(POPUP_INPUT_WIDTH);
                    ui.input_text("##goto_input", &mut self.goto_buf).build();

                    let (go_clicked, cancel_clicked) =
                        render_action_row(ui, POPUP_INPUT_WIDTH, "Go");
                    if cancel_clicked {
                        ui.close_current_popup();
                    }
                    if go_clicked {
                        if let Some(addr) = parse_address(&self.goto_buf) {
                            let offset = addr.saturating_sub(self.config.base_address) as usize;
                            self.goto(offset);
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

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.search_popup_id) {
                compact_popup_body(ui, || {
                    // ── Mode row: Hex | String ────────────────────
                    // Two pseudo-radio buttons (highlighted-on-active).
                    // Selecting `String` preserves the previously chosen
                    // encoding (defaults to `Ascii` first time).
                    let is_hex = matches!(self.config.search_mode, HexSearchMode::Hex);
                    let is_string = self.config.search_mode.is_string();

                    if mode_pill(ui, "Hex", is_hex) {
                        self.config.search_mode = HexSearchMode::Hex;
                    }
                    ui.same_line();
                    if mode_pill(ui, "String", is_string) {
                        // Keep prior encoding choice if we were already
                        // in String mode; otherwise default to ASCII.
                        let encoding = match self.config.search_mode {
                            HexSearchMode::String(e) => e,
                            HexSearchMode::Hex => StringEncoding::Ascii,
                        };
                        self.config.search_mode = HexSearchMode::String(encoding);
                    }

                    // ── Encoding row (visible only in String mode) ─
                    if let HexSearchMode::String(current) = self.config.search_mode {
                        ui.text("Encoding:");
                        ui.same_line();
                        for (i, &enc) in StringEncoding::ALL.iter().enumerate() {
                            if i > 0 {
                                ui.same_line();
                            }
                            if mode_pill(ui, enc.display_name(), enc == current) {
                                self.config.search_mode = HexSearchMode::String(enc);
                            }
                        }
                    }

                    let hint = match self.config.search_mode {
                        HexSearchMode::Hex => "Hex pattern (e.g. 4D 5A ?? 00):",
                        HexSearchMode::String(StringEncoding::Ascii) => "ASCII string:",
                        HexSearchMode::String(StringEncoding::Utf8) => "UTF-8 string:",
                        HexSearchMode::String(StringEncoding::Utf16Le) => {
                            "UTF-16LE string (e.g. Windows wchar_t):"
                        }
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
                        ui.text(format!(
                            "Result {}/{}",
                            self.search_idx + 1,
                            self.search_results.len()
                        ));
                    }

                    let (find_clicked, cancel_clicked) =
                        render_action_row(ui, POPUP_INPUT_WIDTH, "Find");
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
            anchor_next_popup(self.popup_open_pos);
            ui.open_popup(&self.context_popup_id);
            self.show_context_menu = false;
        }

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.context_popup_id) {
                // ── Go to Address (mirrors Ctrl+G) ─────────────
                // Icon glyphs limited to ranges the default ImGui
                // font atlas reliably ships:
                //  - U+00AB / U+00BB `«` / `»`  (Latin-1 supplement)
                //  - U+2190 / U+2192 `←` / `→`  (Arrows block, verified
                //    rendering in this project's atlas)
                //  - U+2026 `…`                  (General Punctuation)
                // Earlier we used U+2316 (target) and U+2699 (gear);
                // both showed up as `?` because those code-points
                // are not present in the default font atlas. The
                // replacements pick semantically-close glyphs that
                // do render.
                //
                // `»` doubles as a "navigate forward to" hint for
                // Go to Address — visually distinct from the
                // single `→` used by Step forward so the two don't
                // collide.
                if ui.menu_item("\u{00BB}  Go to Address\tCtrl+G") {
                    self.show_goto = true;
                    self.goto_buf.clear();
                }

                // ── Search (Ctrl+F) ──────────────────────────
                // Same `»` family glyph as Go to Address — both
                // are "find / navigate to" actions; the magnifier
                // (U+1F50D) lives in supplementary plane and would
                // render as `?` in the default atlas. Use search
                // shortcut hint so the user discovers Ctrl+F
                // without leaving the menu.
                if ui.menu_item("\u{00BB}  Search\tCtrl+F") {
                    self.show_search = true;
                    self.search_focus_pending = true;
                }

                ui.separator();

                // ── Step back (Alt+Left) ─────────────────────
                let can_back = self.nav.can_go_back();
                let _back_dim = if !can_back {
                    Some(ui.push_style_var(dear_imgui_rs::StyleVar::Alpha(0.40)))
                } else {
                    None
                };
                if ui.menu_item("\u{2190}  Step back\tAlt+Left") && can_back {
                    self.nav_back();
                }
                drop(_back_dim);

                // ── Step forward (Alt+Right) ─────────────────
                let can_fwd = self.nav.can_go_forward();
                let _fwd_dim = if !can_fwd {
                    Some(ui.push_style_var(dear_imgui_rs::StyleVar::Alpha(0.40)))
                } else {
                    None
                };
                if ui.menu_item("\u{2192}  Step forward\tAlt+Right") && can_fwd {
                    self.nav_forward();
                }
                drop(_fwd_dim);

                ui.separator();

                // ── Settings ─────────────────────────────────
                // `…` (ellipsis) reads as "more / detailed
                // configuration" and is a single Unicode code
                // point that's universally present in fonts. The
                // gear icon (U+2699) we used originally rendered
                // as `?` in the default atlas.
                if ui.menu_item("\u{2026}  Settings...") {
                    self.show_settings = true;
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

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.settings_popup_id) {
                compact_popup_body(ui, || {
                    // Same `…` icon as the menu entry that opens this
                    // popup — keeps the visual breadcrumb consistent.
                    ui.text("\u{2026}  Hex Viewer Settings");
                    ui.separator();

                    // ── Bytes per row ────────────────────────────
                    ui.text("Bytes per row:");
                    // Fixed 32 px square-ish buttons keep the row from
                    // taking the full popup width and add visual rhythm.
                    let bpr_btn_size = [32.0_f32, 0.0];
                    let current_bpr = self.config.bytes_per_row.value();
                    for (i, preset) in BytesPerRow::ALL.iter().enumerate() {
                        if i > 0 {
                            ui.same_line();
                        }
                        let is_current = preset.value() == current_bpr;
                        let label = preset.display_name();
                        if is_current {
                            let _c = ui.push_style_color(
                                dear_imgui_rs::StyleColor::Button,
                                [0.30, 0.50, 0.90, 1.0],
                            );
                            ui.button_with_size(label, bpr_btn_size);
                        } else if ui.button_with_size(label, bpr_btn_size) {
                            self.config.bytes_per_row = *preset;
                        }
                    }

                    ui.separator();
                    ui.text("Display:");
                    ui.checkbox("Show ASCII", &mut self.config.show_ascii);
                    ui.checkbox("Show inspector", &mut self.config.show_inspector);
                    ui.checkbox("Show offsets", &mut self.config.show_offsets);
                    ui.checkbox("Show column headers", &mut self.config.show_column_headers);
                    ui.checkbox(
                        "Show column dividers",
                        &mut self.config.show_column_dividers,
                    );
                    ui.checkbox("Show splitter", &mut self.config.show_splitter);

                    ui.separator();
                    ui.text("Format:");
                    ui.checkbox("Uppercase hex", &mut self.config.uppercase);
                    ui.checkbox("Category colors", &mut self.config.category_colors);
                    ui.checkbox("Dim zero bytes", &mut self.config.dim_zeros);

                    ui.separator();

                    // Close button — compact, right-anchored with the
                    // same 2-px edge gap as goto / search popups.
                    let total_w = ui.content_region_avail()[0];
                    let close_w = 64.0_f32;
                    let close_x = ui.cursor_pos()[0] + (total_w - close_w - 2.0).max(0.0);
                    ui.set_cursor_pos_x(close_x);
                    if ui.button_with_size("Close", [close_w, ACTION_BTN_HEIGHT])
                        || ui.is_key_pressed(dear_imgui_rs::Key::Escape)
                    {
                        ui.close_current_popup();
                    }
                }); // compact_popup_body
            }
        });
    }
}
