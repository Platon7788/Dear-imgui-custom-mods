//! Keyboard navigation for [`super::DisasmView`] — cursor moves,
//! page-up/down, Home/End, Enter/Space to follow, `G` goto, `Ctrl+A`
//! select-all, `F9` breakpoint, `Ctrl+B` bookmark, `Alt+arrow` nav
//! history, `Esc` clear-breadcrumb, plus the inline-edit Esc path.

use super::super::DisasmView;
use super::super::provider::DisasmDataProvider;
use super::rows_in_window;

/// Modifier snapshot for a frame's keyboard pass — lets the pure
/// key→action mapper be tested without an ImGui `Io`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Cursor-relative navigation resolved from a `(key, mods)` pair.
/// Only the *vertical movement* family is modelled here — the
/// branchy "side-effect" keys (follow / goto / search / breakpoint /
/// nav-history) stay inline in [`DisasmView::handle_keyboard`] because
/// they mutate the provider or open popups and aren't pure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MoveAction {
    /// Move one row up (Up, no Alt/Ctrl).
    Up,
    /// Move one row down (Down, no Alt/Ctrl).
    Down,
    /// Move up by `visible_rows` (PageUp).
    PageUp,
    /// Move down by `visible_rows` (PageDown).
    PageDown,
    /// Jump to the first instruction (Home, no Ctrl).
    Home,
    /// Jump to the last instruction (End, no Ctrl).
    End,
}

impl MoveAction {
    /// Resolve the destination row index for this movement given the
    /// current cursor (`None` ⇒ treated as 0), the visible-row count
    /// (for paging) and the last valid index. Returns `None` only for
    /// `Up` when already at row 0 (no movement) so the caller can skip
    /// the redundant selection churn; every other action always
    /// resolves to a concrete (clamped) target.
    #[must_use]
    pub(super) fn destination(
        self,
        cursor: Option<usize>,
        visible_rows: usize,
        last_idx: usize,
    ) -> Option<usize> {
        let cur = cursor.unwrap_or(0);
        match self {
            MoveAction::Up => (cur > 0).then(|| cur - 1),
            MoveAction::Down => Some((cur + 1).min(last_idx)),
            MoveAction::PageUp => Some(cur.saturating_sub(visible_rows)),
            MoveAction::PageDown => Some((cur + visible_rows).min(last_idx)),
            MoveAction::Home => Some(0),
            MoveAction::End => Some(last_idx),
        }
    }
}

/// Pure mapping from a pressed navigation key + modifier state to a
/// [`MoveAction`]. Returns `None` for keys that aren't plain vertical
/// navigation (those are handled imperatively in `handle_keyboard`).
///
/// The `!ctrl` gating on the arrows / Home / End mirrors the inline
/// handler: `Ctrl+Up/Down` are function-scope jumps and `Ctrl+Home/End`
/// are reserved, so they must NOT also fire single-row movement.
#[must_use]
pub(super) fn nav_move_for_key(key: dear_imgui_rs::Key, mods: Mods) -> Option<MoveAction> {
    use dear_imgui_rs::Key;
    match key {
        Key::UpArrow if !mods.alt && !mods.ctrl => Some(MoveAction::Up),
        Key::DownArrow if !mods.alt && !mods.ctrl => Some(MoveAction::Down),
        Key::PageUp => Some(MoveAction::PageUp),
        Key::PageDown => Some(MoveAction::PageDown),
        Key::Home if !mods.ctrl => Some(MoveAction::Home),
        Key::End if !mods.ctrl => Some(MoveAction::End),
        _ => None,
    }
}

impl DisasmView {
    pub(in crate::disasm_view) fn handle_keyboard(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn DisasmDataProvider,
    ) {
        use dear_imgui_rs::Key;

        let count = provider.instruction_count();
        if count == 0 {
            return;
        }

        let mods = Mods {
            ctrl: ui.io().key_ctrl(),
            alt: ui.io().key_alt(),
            shift: ui.io().key_shift(),
        };

        // Inline edit active → skip navigation.
        if self.edit.is_some() {
            self.handle_edit_keyboard(ui, provider);
            return;
        }

        // `count - 1` via `saturating_sub` removes long-range coupling
        // on the `count == 0` guard above so future refactors can't
        // underflow. `visible_rows` is guarded against zero
        // `line_height` (degenerate font load) and floored at 1 so the
        // keystroke stays responsive on a window briefly reporting 0
        // height.
        let last_idx = count.saturating_sub(1);
        let visible_rows = rows_in_window(ui.window_size()[1], self.line_height).max(1);

        // ── Vertical navigation (pure mapping) ───────────────────
        // Arrows, PageUp/Down, Home, End collapse to a single
        // table-driven path. Each resolves to a destination row, then
        // shared `move_to` applies shift-selection + ensure-visible.
        for key in [
            Key::UpArrow,
            Key::DownArrow,
            Key::PageUp,
            Key::PageDown,
            Key::Home,
            Key::End,
        ] {
            if ui.is_key_pressed(key)
                && let Some(action) = nav_move_for_key(key, mods)
                && let Some(dst) = action.destination(self.cursor_idx, visible_rows, last_idx)
            {
                self.move_to(dst, mods.shift);
                // Up/Down keep the row just visible (ensure_visible);
                // Page/Home/End re-centre the viewport (scroll_to).
                match action {
                    MoveAction::Up | MoveAction::Down => self.ensure_visible(dst, ui),
                    _ => self.scroll_to = Some(dst),
                }
            }
        }

        // ── Function-scope navigation (Ctrl+Up / Ctrl+Down / Ctrl+L) ──
        // Mirrors common reverse-engineering convention (IDA, x64dbg
        // style "go to func top / bottom / select procedure").
        if mods.ctrl && ui.is_key_pressed(Key::UpArrow) {
            self.jump_to_function_start(provider);
        }
        if mods.ctrl && ui.is_key_pressed(Key::DownArrow) {
            self.jump_to_function_end(provider);
        }
        if mods.ctrl && ui.is_key_pressed(Key::L) {
            self.select_function(provider);
        }

        // Ctrl+A — select all. Layout-independence is provided by
        // `crate::input::keyboard::try_inject_ctrl_alt_shortcut` at the
        // host level (see chrome and demo_disasm_view).
        if mods.ctrl && ui.is_key_pressed(Key::A) {
            for i in 0..count {
                self.selection.insert(i);
            }
        }

        // Enter / Space → "Cheat-Engine-style" follow at cursor.
        // Both keys land here so the user can pick whichever is
        // comfortable (Enter for keyboard-only flow, Space when one
        // hand sits on the arrows).
        if ui.is_key_pressed(Key::Enter) || ui.is_key_pressed(Key::Space) {
            self.follow_at_cursor(provider);
        }

        // G → goto address popup.
        if ui.is_key_pressed(Key::G) && !mods.ctrl {
            self.show_goto = true;
            self.goto_buf.clear();
        }

        // Ctrl+F → search-bytes popup. Mirrors `hex_viewer`'s Ctrl+F.
        // Doesn't clear `search_buf` so the user can re-open the popup
        // and tweak the previous query without retyping.
        if mods.ctrl && ui.is_key_pressed(Key::F) && !self.show_search {
            self.show_search = true;
            self.search_focus_pending = true;
        }

        // F3 / Shift+F3 → step through search matches (no-op when no
        // active search). Wraps around at both ends.
        if ui.is_key_pressed(Key::F3) && !self.search_match_starts.is_empty() {
            if mods.shift {
                self.search_prev();
            } else {
                self.search_next();
            }
        }

        // Ctrl+C → copy selected instruction.
        if mods.ctrl && ui.is_key_pressed(Key::C) {
            self.copy_selected(provider);
        }

        // F9 → toggle breakpoint.
        if ui.is_key_pressed(Key::F9)
            && let Some(idx) = self.cursor_idx
            && let Some(instr) = provider.instruction(idx)
        {
            provider.toggle_breakpoint(instr.address());
        }

        // Ctrl+B → toggle bookmark on the cursor row. Editor-style
        // shortcut (VS Code / JetBrains / Sublime). Silently no-ops at
        // the 64-bookmark cap when adding; removal always works.
        if mods.ctrl
            && ui.is_key_pressed(Key::B)
            && let Some(idx) = self.cursor_idx
            && let Some(instr) = provider.instruction(idx)
        {
            self.toggle_bookmark(instr.address());
        }

        // Alt+Left → nav back.
        if mods.alt && ui.is_key_pressed(Key::LeftArrow) {
            self.nav_back(provider);
        }
        // Alt+Right → nav forward.
        if mods.alt && ui.is_key_pressed(Key::RightArrow) {
            self.nav_forward(provider);
        }

        // Esc → clear navigation breadcrumb (origin highlight).
        // Decisive "I'm done with the trail" gesture. Edit-mode Esc is
        // handled separately inside `handle_edit_keyboard` (cancels the
        // edit instead) — this branch only runs when no edit is active.
        if ui.is_key_pressed(Key::Escape) {
            self.origin_addr = None;
        }
    }

    /// Move the cursor to `new_idx`, extending the selection when
    /// `shift` is held (anchored range) or replacing it otherwise.
    /// Shared by every [`MoveAction`] branch above.
    fn move_to(&mut self, new_idx: usize, shift: bool) {
        if shift {
            let anchor = self.sel_anchor.unwrap_or(self.cursor_idx.unwrap_or(0));
            self.select_range(anchor, new_idx);
        } else {
            self.selection.clear();
            self.selection.insert(new_idx);
            self.sel_anchor = Some(new_idx);
        }
        self.cursor_idx = Some(new_idx);
    }

    fn handle_edit_keyboard(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        _provider: &mut dyn DisasmDataProvider,
    ) {
        // InputText widget handles all input now. Only Escape needs
        // manual handling (ImGui InputText doesn't cancel on Esc by
        // default).
        if ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
            self.edit = None;
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::Key;

    const NONE: Mods = Mods {
        ctrl: false,
        alt: false,
        shift: false,
    };
    const CTRL: Mods = Mods {
        ctrl: true,
        alt: false,
        shift: false,
    };
    const ALT: Mods = Mods {
        ctrl: false,
        alt: true,
        shift: false,
    };

    // ── nav_move_for_key ─────────────────────────────────────────
    #[test]
    fn plain_arrows_map_to_up_down() {
        assert_eq!(nav_move_for_key(Key::UpArrow, NONE), Some(MoveAction::Up));
        assert_eq!(
            nav_move_for_key(Key::DownArrow, NONE),
            Some(MoveAction::Down)
        );
    }

    #[test]
    fn ctrl_arrows_are_not_plain_moves() {
        // Ctrl+Up/Down are function-scope jumps — must NOT map to a
        // plain MoveAction (otherwise the row would also step).
        assert_eq!(nav_move_for_key(Key::UpArrow, CTRL), None);
        assert_eq!(nav_move_for_key(Key::DownArrow, CTRL), None);
    }

    #[test]
    fn alt_arrows_are_not_plain_moves() {
        // Alt+Left/Right are nav-history; Alt+Up/Down do nothing.
        assert_eq!(nav_move_for_key(Key::UpArrow, ALT), None);
        assert_eq!(nav_move_for_key(Key::DownArrow, ALT), None);
    }

    #[test]
    fn page_keys_map_regardless_of_ctrl() {
        assert_eq!(
            nav_move_for_key(Key::PageUp, NONE),
            Some(MoveAction::PageUp)
        );
        assert_eq!(
            nav_move_for_key(Key::PageDown, CTRL),
            Some(MoveAction::PageDown)
        );
    }

    #[test]
    fn home_end_gated_on_ctrl() {
        assert_eq!(nav_move_for_key(Key::Home, NONE), Some(MoveAction::Home));
        assert_eq!(nav_move_for_key(Key::End, NONE), Some(MoveAction::End));
        // Ctrl reserved for future Ctrl+Home/End → no plain move.
        assert_eq!(nav_move_for_key(Key::Home, CTRL), None);
        assert_eq!(nav_move_for_key(Key::End, CTRL), None);
    }

    #[test]
    fn unrelated_key_maps_to_none() {
        assert_eq!(nav_move_for_key(Key::A, NONE), None);
        assert_eq!(nav_move_for_key(Key::Enter, NONE), None);
    }

    // ── MoveAction::destination ──────────────────────────────────
    #[test]
    fn up_at_top_yields_no_move() {
        assert_eq!(MoveAction::Up.destination(Some(0), 10, 99), None);
    }

    #[test]
    fn up_decrements() {
        assert_eq!(MoveAction::Up.destination(Some(5), 10, 99), Some(4));
    }

    #[test]
    fn down_clamps_at_last() {
        assert_eq!(MoveAction::Down.destination(Some(99), 10, 99), Some(99));
        assert_eq!(MoveAction::Down.destination(Some(98), 10, 99), Some(99));
    }

    #[test]
    fn page_up_saturates_at_zero() {
        assert_eq!(MoveAction::PageUp.destination(Some(3), 10, 99), Some(0));
        assert_eq!(MoveAction::PageUp.destination(Some(25), 10, 99), Some(15));
    }

    #[test]
    fn page_down_clamps_at_last() {
        assert_eq!(MoveAction::PageDown.destination(Some(95), 10, 99), Some(99));
        assert_eq!(MoveAction::PageDown.destination(Some(40), 10, 99), Some(50));
    }

    #[test]
    fn home_end_resolve_to_bounds() {
        assert_eq!(MoveAction::Home.destination(Some(42), 10, 99), Some(0));
        assert_eq!(MoveAction::End.destination(Some(42), 10, 99), Some(99));
    }

    #[test]
    fn none_cursor_treated_as_zero() {
        assert_eq!(MoveAction::Up.destination(None, 10, 99), None);
        assert_eq!(MoveAction::Down.destination(None, 10, 99), Some(1));
        assert_eq!(MoveAction::End.destination(None, 10, 99), Some(99));
    }

    #[test]
    fn single_row_buffer_never_panics() {
        // last_idx == 0 (one instruction). Down/PageDown/End all clamp.
        assert_eq!(MoveAction::Down.destination(Some(0), 5, 0), Some(0));
        assert_eq!(MoveAction::PageDown.destination(Some(0), 5, 0), Some(0));
        assert_eq!(MoveAction::End.destination(Some(0), 5, 0), Some(0));
        assert_eq!(MoveAction::Up.destination(Some(0), 5, 0), None);
    }
}
