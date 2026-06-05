//! Type-to-search: incremental filename matching for
//! [`FileManager`](super::FileManager).
//!
//! Split out of `view.rs` to keep both files < 500 lines (CLAUDE.md).
//! `handle_type_to_search` is `pub(super)` because the per-frame driver
//! in [`view`](super::view) calls it once per frame.

use dear_imgui_rs::{Key, Ui};

use super::FileManager;

impl FileManager {
    // ─── Type-to-search ─────────────────────────────────────────────

    /// Handle incremental filename search: accumulate typed characters,
    /// find the first matching entry, and select it. Resets after 0.5s of no input.
    ///
    /// P1-5: skips entirely when an ImGui input is active (so typing in the
    /// rename / new-folder / breadcrumb / filename fields doesn't double-fire
    /// type-to-search) and when the dialog isn't focused. The previous
    /// implementation ran 26 `is_key_pressed` checks every frame regardless.
    ///
    /// P1-6: matches with `starts_with` rather than `contains` — the user
    /// expectation is "jump to the file whose name *begins* with what I typed".
    pub(super) fn handle_type_to_search(&mut self, ui: &Ui) {
        if !self.config.enable_type_to_search {
            return;
        }
        // Skip when any ImGui input is active (rename / new-folder / filename / path).
        if ui.is_any_item_active() {
            return;
        }

        let dt = ui.io().delta_time();
        let timeout = self.config.search_timeout;
        self.search_timer = (self.search_timer + dt).min(timeout + 1.0);

        // P3-5: also accept digits 0-9 — searching for "2024" in a folder of
        // yearly archives should work. Cyrillic / accented filenames are still
        // not matched; full Unicode support requires reading
        // `ui.io().input_queue_characters` which is not yet wired through the
        // current `dear_imgui_rs` binding.
        let mut typed_char = None;
        const ALPHA_KEYS: [(Key, char); 26] = [
            (Key::A, 'a'),
            (Key::B, 'b'),
            (Key::C, 'c'),
            (Key::D, 'd'),
            (Key::E, 'e'),
            (Key::F, 'f'),
            (Key::G, 'g'),
            (Key::H, 'h'),
            (Key::I, 'i'),
            (Key::J, 'j'),
            (Key::K, 'k'),
            (Key::L, 'l'),
            (Key::M, 'm'),
            (Key::N, 'n'),
            (Key::O, 'o'),
            (Key::P, 'p'),
            (Key::Q, 'q'),
            (Key::R, 'r'),
            (Key::S, 's'),
            (Key::T, 't'),
            (Key::U, 'u'),
            (Key::V, 'v'),
            (Key::W, 'w'),
            (Key::X, 'x'),
            (Key::Y, 'y'),
            (Key::Z, 'z'),
        ];
        const DIGIT_KEYS: [(Key, char); 10] = [
            (Key::Key0, '0'),
            (Key::Key1, '1'),
            (Key::Key2, '2'),
            (Key::Key3, '3'),
            (Key::Key4, '4'),
            (Key::Key5, '5'),
            (Key::Key6, '6'),
            (Key::Key7, '7'),
            (Key::Key8, '8'),
            (Key::Key9, '9'),
        ];
        for (k, ch) in ALPHA_KEYS.iter().chain(DIGIT_KEYS.iter()) {
            if ui.is_key_pressed(*k) {
                typed_char = Some(*ch);
                break;
            }
        }

        if let Some(ch) = typed_char {
            if self.search_timer > self.config.search_timeout {
                self.search_buf.clear();
            }
            self.search_timer = 0.0;
            self.search_buf.push(ch);

            // Find first matching entry (P1-6: prefix match, not substring).
            let search = self.search_buf.as_str();
            for (i, e) in self.entries.iter().enumerate() {
                if e.name_lower.starts_with(search) {
                    self.selected_indices.clear();
                    self.selected_indices.push(i);
                    self.scroll_to_index = Some(i);
                    break;
                }
            }
        }
    }
}
