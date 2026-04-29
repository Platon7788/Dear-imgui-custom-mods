//! Goto-address popup and right-click context menu for [`super::DisasmView`].

use super::config::DisasmDataProvider;
use super::{DisasmView, parse_address};
use crate::utils::clipboard::set_clipboard;

impl DisasmView {
    pub(super) fn render_goto_popup(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        // Same fix as `hex_viewer::render_goto_popup` — `BeginPopup`
        // has to run every frame (not just the open-trigger frame),
        // otherwise the popup flashes for one frame and disappears.
        if self.show_goto {
            ui.open_popup(&self.goto_popup_id);
            self.show_goto = false;
        }

        if let Some(_popup) = ui.begin_popup(&self.goto_popup_id) {
            ui.text("Goto address (hex):");
            ui.input_text("##dv_goto_input", &mut self.goto_buf).build();

            if ui.button("Go") || ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                if let Some(addr) = parse_address(&self.goto_buf) {
                    self.goto_address(addr, provider);
                }
                ui.close_current_popup();
            }
            ui.same_line();
            if ui.button("Cancel") || ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                ui.close_current_popup();
            }
        }
    }

    pub(super) fn render_context_menu(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        if self.show_context_menu {
            ui.open_popup(&self.ctx_popup_id);
            self.show_context_menu = false;
        }

        if let Some(_popup) = ui.begin_popup(&self.ctx_popup_id) {
            let idx = self.context_idx.unwrap_or(0);
            let instr_addr = provider.instruction(idx).map(|i| i.address());
            let has_target = provider
                .instruction(idx)
                .and_then(|i| i.branch_target())
                .is_some();

            if ui.selectable("Copy Address") {
                if let Some(addr) = instr_addr {
                    let s = format!("0x{:X}", addr);
                    set_clipboard(&s);
                }
                ui.close_current_popup();
            }

            let sel_count = self.selection.len();
            let copy_label = if sel_count > 1 {
                format!("Copy {} Instructions", sel_count)
            } else {
                "Copy Instruction".to_string()
            };
            if ui.selectable(&copy_label) {
                self.copy_selected(provider);
                ui.close_current_popup();
            }

            ui.separator();

            if has_target && ui.selectable("Follow Branch") {
                if let Some(target) = provider.instruction(idx).and_then(|i| i.branch_target()) {
                    self.goto_address(target, provider);
                }
                ui.close_current_popup();
            }

            if ui.selectable("Toggle Breakpoint") {
                if let Some(addr) = instr_addr {
                    provider.toggle_breakpoint(addr);
                }
                ui.close_current_popup();
            }

            ui.separator();

            if ui.selectable("Goto Address...") {
                self.show_goto = true;
                self.goto_buf.clear();
                ui.close_current_popup();
            }
        }
    }
}
