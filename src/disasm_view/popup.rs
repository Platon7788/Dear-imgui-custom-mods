//! Goto-address popup and right-click context menu for [`super::DisasmView`].
//!
//! Both popups share the crate-wide `themed_popup_style` look (same
//! padding / rounding as `hex_viewer`'s) and the centred-anchor
//! positioning so the goto popup always lands at the visual middle
//! of the viewer regardless of where the user pressed `G`.

use super::config::DisasmDataProvider;
use super::{DisasmView, parse_address};
use crate::utils::clipboard::set_clipboard;
use crate::utils::popup::{action_row, compact_popup_body, themed_popup_style};

/// Reference width for the goto popup body (input + action row).
/// Matches `hex_viewer::popup::POPUP_INPUT_WIDTH` so both modules
/// host visually-identical goto dialogs.
const POPUP_INPUT_WIDTH: f32 = 360.0;

/// Anchor the next ImGui window (a popup) at `pos` in screen space
/// using the given `pivot`. See `hex_viewer::popup` for full
/// rationale — same `igSetNextWindowPos(.., Cond_Always, pivot)`
/// FFI call (no `set_next_window_pos` builder in dear-imgui-rs 0.11).
fn anchor_next_popup_at(pos: [f32; 2], pivot: [f32; 2]) {
    // SAFETY: side-effect-only ImGui call; just records the
    // requested next-window position in the shared context.
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

/// Top-left anchor — for the right-click context menu (spawns
/// where the click happened).
fn anchor_next_popup_topleft(pos: [f32; 2]) {
    anchor_next_popup_at(pos, [0.0, 0.0]);
}

/// Centred anchor — for modal popups (Goto). Pivot `(0.5, 0.5)`
/// means `pos` is the popup's CENTRE, so the body is centred no
/// matter what its size is.
fn anchor_next_popup_centred(pos: [f32; 2]) {
    anchor_next_popup_at(pos, [0.5, 0.5]);
}

impl DisasmView {
    pub(super) fn render_goto_popup(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        // CRITICAL: `BeginPopup` runs every frame the popup is open,
        // not only on the trigger frame — see `hex_viewer::popup` for
        // the bug history. `OpenPopup` stays gated on the flag.
        if self.show_goto {
            anchor_next_popup_centred(self.component_center);
            ui.open_popup(&self.goto_popup_id);
            self.show_goto = false;
            self.goto_focus_pending = true;
        }

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.goto_popup_id) {
                compact_popup_body(ui, || {
                    ui.text("Goto address (hex):");

                    if self.goto_focus_pending {
                        ui.set_keyboard_focus_here();
                        self.goto_focus_pending = false;
                    }
                    ui.set_next_item_width(POPUP_INPUT_WIDTH);
                    ui.input_text("##dv_goto_input", &mut self.goto_buf)
                        .build();

                    let (go_clicked, cancel_clicked) =
                        action_row(ui, POPUP_INPUT_WIDTH, "Go");
                    if cancel_clicked {
                        ui.close_current_popup();
                    }
                    if go_clicked {
                        if let Some(addr) = parse_address(&self.goto_buf) {
                            self.goto_address(addr, provider);
                        }
                        ui.close_current_popup();
                    }
                });
            }
        });
    }

    /// Right-click context menu — Copy / Follow / Toggle bp / Goto /
    /// Search-style actions. Icons follow the same atlas-safe
    /// glyph rules as `hex_viewer` (Latin-1 + Arrows + General
    /// Punctuation): no gear, no target, no magnifier — those code
    /// points render as `?` in the default ImGui font atlas.
    pub(super) fn render_context_menu(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        if self.show_context_menu {
            anchor_next_popup_topleft(self.popup_open_pos);
            ui.open_popup(&self.ctx_popup_id);
            self.show_context_menu = false;
        }

        themed_popup_style(ui, || {
            if let Some(_popup) = ui.begin_popup(&self.ctx_popup_id) {
                let idx = self.context_idx.unwrap_or(0);
                let instr_addr = provider.instruction(idx).map(|i| i.address());
                let has_target = provider
                    .instruction(idx)
                    .and_then(|i| i.branch_target())
                    .is_some();

                // ── Copy Address (`»` like hex_viewer's "navigate to" verb) ──
                if ui.menu_item("\u{00BB}  Copy Address") {
                    if let Some(addr) = instr_addr {
                        set_clipboard(&format!("0x{:X}", addr));
                    }
                    ui.close_current_popup();
                }

                let sel_count = self.selection.len();
                let copy_label = if sel_count > 1 {
                    format!("\u{00BB}  Copy {} Instructions\tCtrl+C", sel_count)
                } else {
                    "\u{00BB}  Copy Instruction\tCtrl+C".to_string()
                };
                if ui.menu_item(&copy_label) {
                    self.copy_selected(provider);
                    ui.close_current_popup();
                }

                ui.separator();

                // ── Follow Branch (`→` arrow — visible action) ──
                // Greyed-out when no branch target on this row so the
                // user gets the "nothing to follow" cue.
                let _follow_dim = if !has_target {
                    Some(ui.push_style_var(dear_imgui_rs::StyleVar::Alpha(0.40)))
                } else {
                    None
                };
                if ui.menu_item("\u{2192}  Follow Branch\tEnter") && has_target {
                    if let Some(target) =
                        provider.instruction(idx).and_then(|i| i.branch_target())
                    {
                        self.goto_address(target, provider);
                    }
                    ui.close_current_popup();
                }
                drop(_follow_dim);

                // ── Toggle Breakpoint (`●` filled circle — bp visual) ──
                if ui.menu_item("\u{25CF}  Toggle Breakpoint\tF9") {
                    if let Some(addr) = instr_addr {
                        provider.toggle_breakpoint(addr);
                    }
                    ui.close_current_popup();
                }

                ui.separator();

                // ── Goto Address (`»`, mirrors G key) ──
                if ui.menu_item("\u{00BB}  Goto Address...\tG") {
                    self.show_goto = true;
                    self.goto_buf.clear();
                    self.goto_focus_pending = true;
                }
            }
        });
    }
}
