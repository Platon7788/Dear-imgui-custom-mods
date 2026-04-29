//! Goto + search popups.
//!
//! Both popups are stateless wrappers over a single ImGui input field;
//! they live separately from `draw.rs` so the row-drawing hot path is
//! easier to read. The data inspector stays in `draw.rs` because it's
//! an inline overlay below the buffer, not a floating popup.

use super::HexViewer;
use super::config::HexSearchMode;
use super::search::parse_address;

impl HexViewer {
    pub(super) fn render_goto_popup(&mut self, ui: &dear_imgui_rs::Ui) {
        if !self.show_goto {
            return;
        }

        ui.open_popup(&self.goto_popup_id);
        self.show_goto = false;

        if let Some(_popup) = ui.begin_popup(&self.goto_popup_id) {
            ui.text("Goto offset (hex or decimal):");
            ui.input_text("##goto_input", &mut self.goto_buf).build();

            if ui.button("Go") || ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                if let Some(addr) = parse_address(&self.goto_buf) {
                    let offset = addr.saturating_sub(self.config.base_address) as usize;
                    self.goto(offset);
                }
                ui.close_current_popup();
            }
            ui.same_line();
            if ui.button("Cancel") || ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                ui.close_current_popup();
            }
        }
    }

    pub(super) fn render_search_popup(&mut self, ui: &dear_imgui_rs::Ui) {
        if !self.show_search {
            return;
        }

        ui.open_popup(&self.search_popup_id);
        self.show_search = false;

        if let Some(_popup) = ui.begin_popup(&self.search_popup_id) {
            let mode_name = self.config.search_mode.display_name();
            if ui.button(mode_name) {
                self.config.search_mode = match self.config.search_mode {
                    HexSearchMode::Hex => HexSearchMode::Ascii,
                    HexSearchMode::Ascii => HexSearchMode::Hex,
                };
            }
            ui.same_line();

            let hint = match self.config.search_mode {
                HexSearchMode::Hex => "Hex (e.g. 4D 5A ?? 00):",
                HexSearchMode::Ascii => "ASCII string:",
            };
            ui.text(hint);
            ui.input_text("##search_input", &mut self.search_buf)
                .build();

            if !self.search_results.is_empty() {
                ui.text(format!(
                    "Result {}/{}",
                    self.search_idx + 1,
                    self.search_results.len()
                ));
            }

            if ui.button("Find") || ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                self.do_search();
                if self.search_results.is_empty() {
                    ui.text("No matches found.");
                } else {
                    ui.close_current_popup();
                }
            }
            ui.same_line();
            if ui.button("Cancel") || ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                ui.close_current_popup();
            }
        }
    }
}
