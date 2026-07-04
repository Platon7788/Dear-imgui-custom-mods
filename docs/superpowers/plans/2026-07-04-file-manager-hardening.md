# FileManager Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring `crate::file_manager` up to the hardening bar already met by `virtual_table`/`virtual_tree` — full doc-lint compliance, `// SAFETY:` comments, sub-500-line files, no argument-count lint suppressions, and the i18n catalogue relocated into `crate::i18n`.

**Architecture:** Five independent, individually-committable changes to an existing, fully-tested module. Tasks 1–4 are behaviour-preserving (the existing test suite + `clippy -D warnings` + build are the regression gate). Task 5 relocates the string catalogue to `crate::i18n::file_manager` while keeping the historic public paths (`FmStrings`, `STRINGS_EN`, `STRINGS_RU`, `strings_for_locale`) alive as backward-compatible re-exports, and adds the canonical i18n tests.

**Tech Stack:** Rust 2024, `dear-imgui-rs`, `ron`/`serde`, `windows-sys` (drive enumeration).

**Verification gate (run after every task unless noted):**
```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings   # MUST be 0 warnings
cargo test
```
Final task additionally runs `cargo build --workspace` to prove the demos (examples-app) still compile against the preserved public API.

---

## File Structure

| File | Change | Responsibility after change |
|------|--------|------------------------------|
| `crate/src/file_manager/mod.rs` | Modify | Drop `#![allow(missing_docs)]` (Task 1) |
| `crate/src/file_manager/view.rs` | Modify | `// SAFETY:` on the sizing FFI block (Task 2); build `ToolbarCtx`/`FooterCtx` at call sites (Task 4) |
| `crate/src/file_manager/util.rs` | Modify | `// SAFETY:` on the two FFI blocks (Task 2) |
| `crate/src/file_manager/render/table.rs` | Modify | Loses keyboard-nav block + `PAGE_SIZE_FALLBACK`; now < 500 lines (Task 3) |
| `crate/src/file_manager/render/table_keyboard.rs` | **Create** | `handle_table_keyboard` — arrows/PageUp-Down/Home/End/Enter/Backspace (Task 3) |
| `crate/src/file_manager/render/toolbar.rs` | Modify | `ToolbarCtx` bundle; drop `too_many_arguments` allow (Task 4) |
| `crate/src/file_manager/render/footer.rs` | Modify | `FooterCtx` bundle; drop `too_many_arguments` allow (Task 4) |
| `crate/src/file_manager/render/mod.rs` | Modify | Register `table_keyboard`; re-export `ToolbarCtx`/`FooterCtx` (Tasks 3, 4) |
| `crate/src/i18n/file_manager.rs` | **Create** | `Strings` + `EN`/`RU` + `strings()` catalogue (Task 5) |
| `crate/src/i18n/mod.rs` | Modify | `pub mod file_manager;` (Task 5) |
| `crate/src/i18n/tests.rs` | Modify | `file_manager_strings_resolve` + parity test (Task 5) |
| `crate/src/file_manager/config.rs` | Modify | Replace catalogue defs with re-export shims; trim migrated tests (Task 5) |

---

## Task 1: Drop the stale `missing_docs` suppression

The whole public surface (`FileManager` + pub fields, `config::*`, `FavoritesPanel`/`FavoriteEntry` reached via `favorites_mut()`) is already documented, so the module-level opt-out is dead weight — exactly the suppression `virtual_table`/`virtual_tree` removed during their hardening pass.

**Files:**
- Modify: `crate/src/file_manager/mod.rs:87`

- [ ] **Step 1: Remove the allow attribute**

Delete this line (currently line 87, directly above `mod actions;`):

```rust
#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
```

- [ ] **Step 2: Verify no `missing_docs` warnings surface**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS with 0 warnings. If clippy reports `missing documentation for ...` on any item, add a one-line `///` doc to that exact item and re-run. (None are expected — the public surface was audited as fully documented.)

- [ ] **Step 3: Run the test suite**

Run: `cargo test`
Expected: PASS (no behaviour change).

- [ ] **Step 4: Commit**

```bash
git add crate/src/file_manager/mod.rs
git commit -m "$(cat <<'EOF'
docs(file_manager): drop stale missing_docs suppression

The public surface is already fully documented; the module-level
opt-out was dead weight. Matches the virtual_table/virtual_tree
hardening pass.

Co-Authored-By: claude-flow <ruv@ruv.net>
EOF
)"
```

---

## Task 2: Add `// SAFETY:` comments to the three `unsafe` FFI blocks

`lib.rs:84-87` states every `unsafe` block carries a `// SAFETY:` comment. `file_manager` has three FFI blocks with none. All are trivial C calls; document why each is sound.

**Files:**
- Modify: `crate/src/file_manager/view.rs:39`
- Modify: `crate/src/file_manager/util.rs:161`
- Modify: `crate/src/file_manager/util.rs:174`

- [ ] **Step 1: Annotate the window-sizing block in `view.rs`**

Replace (starting at the current line 38 comment `// Set window size before opening popup`):

```rust
        // Set window size before opening popup
        unsafe {
```

with:

```rust
        // Set window size before opening popup.
        // SAFETY: FFI into Dear ImGui. Both calls take POD `ImVec2` values by
        // copy; `igSetNextWindowSizeConstraints` gets `None`/null for its
        // optional custom-callback + user-data. No Rust reference is handed to
        // C, and the calls run inside a live ImGui frame — so this is sound.
        unsafe {
```

- [ ] **Step 2: Annotate the clipboard block in `util.rs`**

Replace:

```rust
    if let Ok(c_str) = std::ffi::CString::new(sanitized) {
        unsafe {
            dear_imgui_rs::sys::igSetClipboardText(c_str.as_ptr());
        }
    }
```

with:

```rust
    if let Ok(c_str) = std::ffi::CString::new(sanitized) {
        // SAFETY: `c_str` is a valid NUL-terminated C string that outlives the
        // call; ImGui copies the text into its own buffer and keeps no pointer.
        unsafe {
            dear_imgui_rs::sys::igSetClipboardText(c_str.as_ptr());
        }
    }
```

- [ ] **Step 3: Annotate the drive-enumeration block in `util.rs`**

Replace:

```rust
    let mut drives = Vec::new();
    unsafe {
        let mask = GetLogicalDrives();
```

with:

```rust
    let mut drives = Vec::new();
    // SAFETY: `GetLogicalDrives` is a parameterless Win32 call returning a
    // bitmask of present drive letters; it touches no caller-owned memory.
    unsafe {
        let mask = GetLogicalDrives();
```

- [ ] **Step 4: Verify**

Run: `cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS, 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add crate/src/file_manager/view.rs crate/src/file_manager/util.rs
git commit -m "$(cat <<'EOF'
docs(file_manager): add SAFETY comments to unsafe FFI blocks

Window-sizing, clipboard, and GetLogicalDrives calls now carry the
crate-standard // SAFETY: rationale (lib.rs convention).

Co-Authored-By: claude-flow <ruv@ruv.net>
EOF
)"
```

---

## Task 3: Split table keyboard navigation into `table_keyboard.rs`

`table.rs` is 504 lines (> 500 budget). The keyboard-navigation block (current lines 432-501) plus its `PAGE_SIZE_FALLBACK` constant (lines 16-19) are a self-contained ~74-line unit. Extracting them to a sibling file drops `table.rs` to ~439 lines and gives the keyboard handling its own home. Behaviour is preserved exactly: the helper returns `Option<Action>`, and the caller overwrites `result.action` only when the helper produced one — identical to the current in-place assignment.

**Files:**
- Create: `crate/src/file_manager/render/table_keyboard.rs`
- Modify: `crate/src/file_manager/render/table.rs` (remove const 16-19 + block 432-501)
- Modify: `crate/src/file_manager/render/mod.rs` (register module)

- [ ] **Step 1: Create `table_keyboard.rs`**

Create `crate/src/file_manager/render/table_keyboard.rs` with exactly:

```rust
//! Keyboard navigation for the file table — arrow keys, Page Up/Down, Home/End,
//! Enter (open dir / confirm file), Backspace (parent directory).
//!
//! Split out of [`table`](super::table) to keep that file within the 500-line
//! budget (CLAUDE.md). Operates only on the selection/scroll state and the
//! entry slice, returning at most one deferred [`Action`].

use dear_imgui_rs::{Key, Ui};

use crate::file_manager::actions::Action;
use crate::file_manager::entry::FsEntry;

/// Approximate visible row count for PageUp/PageDown when the window height
/// isn't reliably available. `render_file_table` normally derives the page
/// size from `window_size().y / line_height`; this is the fallback.
const PAGE_SIZE_FALLBACK: usize = 20;

/// Handle keyboard navigation for the file table.
///
/// No-ops while any text input is active or the table window (and its
/// children) is unfocused. Mutates `selected_indices` and `scroll_to_index`
/// in place; returns a deferred [`Action`] for Enter (open dir / confirm file)
/// or Backspace (go to parent), otherwise `None`.
pub(super) fn handle_table_keyboard(
    ui: &Ui,
    entries: &[FsEntry],
    selected_indices: &mut Vec<usize>,
    scroll_to_index: &mut Option<usize>,
) -> Option<Action> {
    let mut action = None;

    if !ui.is_any_item_active()
        && ui.is_window_focused_with_flags(
            dear_imgui_rs::FocusedFlags::ROOT_WINDOW | dear_imgui_rs::FocusedFlags::CHILD_WINDOWS,
        )
    {
        if ui.is_key_pressed(Key::UpArrow) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = current.saturating_sub(1);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        if ui.is_key_pressed(Key::DownArrow) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = (current + 1).min(entries.len() - 1);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        if ui.is_key_pressed(Key::Enter)
            && let Some(&idx) = selected_indices.first()
            && let Some(e) = entries.get(idx)
        {
            if e.is_dir {
                action = Some(Action::NavigateTo(e.path.clone()));
            } else {
                action = Some(Action::ConfirmSelection);
            }
        }
        if ui.is_key_pressed(Key::Backspace) {
            action = Some(Action::GoParent);
        }

        // Page Up / Page Down — derive page_size from the actual visible row
        // count. Falls back to the constant when window height is invalid.
        let row_h = ui.text_line_height_with_spacing().max(1.0);
        let win_h = ui.window_size()[1].max(0.0);
        let page_size = if win_h > 0.0 && row_h > 0.0 {
            ((win_h / row_h) as usize).max(1)
        } else {
            PAGE_SIZE_FALLBACK
        };
        if ui.is_key_pressed(Key::PageUp) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = current.saturating_sub(page_size);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        if ui.is_key_pressed(Key::PageDown) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = (current + page_size).min(entries.len() - 1);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        // Home / End
        if ui.is_key_pressed(Key::Home) && !entries.is_empty() {
            selected_indices.clear();
            selected_indices.push(0);
            *scroll_to_index = Some(0);
        }
        if ui.is_key_pressed(Key::End) && !entries.is_empty() {
            let last = entries.len() - 1;
            selected_indices.clear();
            selected_indices.push(last);
            *scroll_to_index = Some(last);
        }
    }

    action
}
```

- [ ] **Step 2: Remove `PAGE_SIZE_FALLBACK` from `table.rs`**

Delete these lines from `crate/src/file_manager/render/table.rs` (currently 16-19):

```rust
/// Default approximate visible row count for PageUp/PageDown when window
/// height isn't reliably available (P3-1: render normally derives this from
/// `window_size().y / line_height`, this is the fallback).
pub(super) const PAGE_SIZE_FALLBACK: usize = 20;
```

- [ ] **Step 3: Replace the keyboard block in `table.rs`**

Replace the entire block (currently lines 432-501), which starts with `// Keyboard navigation (disabled when any text input is active)` and ends with the closing `}` of the `Home / End` section just before `    result\n}`, with:

```rust
    // Keyboard navigation (extracted to `table_keyboard` to keep this file
    // within the 500-line budget). Overwrites any row-loop action only when a
    // navigation key actually fired — identical to the previous in-place logic.
    if let Some(a) =
        super::table_keyboard::handle_table_keyboard(ui, entries, selected_indices, scroll_to_index)
    {
        result.action = Some(a);
    }

    result
```

- [ ] **Step 4: Register the module in `render/mod.rs`**

In `crate/src/file_manager/render/mod.rs`, add `mod table_keyboard;` to the module declaration list, keeping alphabetical order (after `mod table;`):

```rust
mod table;
mod table_keyboard;
mod toolbar;
```

No re-export is needed — `table.rs` reaches it via `super::table_keyboard`.

- [ ] **Step 5: Verify file is under budget**

Run: `wc -l crate/src/file_manager/render/table.rs`
Expected: a number < 500 (≈ 439).

- [ ] **Step 6: Verify build/lint/tests**

Run: `cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS, 0 warnings.

- [ ] **Step 7: Commit**

```bash
git add crate/src/file_manager/render/table.rs crate/src/file_manager/render/table_keyboard.rs crate/src/file_manager/render/mod.rs
git commit -m "$(cat <<'EOF'
refactor(file_manager): split table keyboard nav into table_keyboard.rs

Moves the arrow/PageUp-Down/Home/End/Enter/Backspace handling and
PAGE_SIZE_FALLBACK out of table.rs (504 -> ~439 lines, back under the
500-line budget). Behaviour is unchanged: the helper returns an Option
and the caller overwrites result.action only when nav fired.

Co-Authored-By: claude-flow <ruv@ruv.net>
EOF
)"
```

---

## Task 4: Bundle `toolbar`/`footer` arguments into Ctx structs

`render_toolbar` (11 args) and `render_footer` (10 args) carry `#[allow(clippy::too_many_arguments)]`. `render_file_table` already solved this with a `TableCtx` borrow-bundle; apply the same pattern so both allows can go. Each body is unchanged — a destructure at the top rebinds the exact same names/types the parameters used.

**Files:**
- Modify: `crate/src/file_manager/render/toolbar.rs`
- Modify: `crate/src/file_manager/render/footer.rs`
- Modify: `crate/src/file_manager/render/mod.rs`
- Modify: `crate/src/file_manager/view.rs`

- [ ] **Step 1: Convert `render_toolbar` to take `ToolbarCtx`**

In `crate/src/file_manager/render/toolbar.rs`, replace the doc comment + `#[allow(clippy::too_many_arguments)]` + full parameter list (current lines 15-34) with the struct definition and the new signature + destructure:

```rust
/// Borrow-bundle for [`render_toolbar`] — replaces the 11-argument signature
/// with a single context struct (mirrors [`TableCtx`](super::table::TableCtx)).
pub(crate) struct ToolbarCtx<'a> {
    pub strings: &'a FmStrings,
    pub has_parent: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub show_new_folder: &'a mut bool,
    pub new_folder_buf: &'a mut String,
    pub show_new_file: &'a mut bool,
    pub new_file_buf: &'a mut String,
    pub show_hidden: bool,
    pub config: &'a FileManagerConfig,
    pub buf: &'a mut String,
}

/// Render the navigation toolbar.
///
/// Disabled buttons are shown as grayed-out text. The "New Folder" / "New File"
/// buttons toggle inline input fields with Create/Cancel buttons.
/// Only one inline input can be open at a time.
pub(crate) fn render_toolbar(ui: &Ui, ctx: ToolbarCtx<'_>) -> Option<Action> {
    let ToolbarCtx {
        strings,
        has_parent,
        can_back,
        can_forward,
        show_new_folder,
        new_folder_buf,
        show_new_file,
        new_file_buf,
        show_hidden,
        config,
        buf,
    } = ctx;

    let mut action = None;
```

Everything below `let mut action = None;` stays as-is. (The destructured locals have the same names and types — `&mut bool`, `&mut String`, `&FmStrings`, `&FileManagerConfig` — as the old parameters, so the body compiles unchanged.)

- [ ] **Step 2: Convert `render_footer` to take `FooterCtx`**

In `crate/src/file_manager/render/footer.rs`, replace the doc comment + `#[allow(clippy::too_many_arguments)]` + parameter list (current lines 15-30, ending at `) -> (bool, bool, Option<Action>) {`) with:

```rust
/// Borrow-bundle for [`render_footer`] — replaces the 10-argument signature
/// with a single context struct (mirrors [`TableCtx`](super::table::TableCtx)).
pub(crate) struct FooterCtx<'a> {
    pub strings: &'a FmStrings,
    pub mode: DialogMode,
    pub entries: &'a [FsEntry],
    pub selected_indices: &'a [usize],
    pub filename_buf: &'a mut String,
    pub filters: &'a [FileFilter],
    pub active_filter: usize,
    pub config: &'a FileManagerConfig,
    pub buf: &'a mut String,
}

/// Render the footer: filter dropdown, filename input (SaveFile), and the
/// Confirm/Cancel button pair.
///
/// Returns `(confirmed, cancelled, filter_action)`.
pub(crate) fn render_footer(ui: &Ui, ctx: FooterCtx<'_>) -> (bool, bool, Option<Action>) {
    let FooterCtx {
        strings,
        mode,
        entries,
        selected_indices,
        filename_buf,
        filters,
        active_filter,
        config,
        buf,
    } = ctx;

    let mut confirmed = false;
```

Everything below `let mut confirmed = false;` stays as-is.

- [ ] **Step 3: Update `render/mod.rs` re-exports**

In `crate/src/file_manager/render/mod.rs`, change the two re-export lines:

```rust
pub(crate) use footer::render_footer;
```
to
```rust
pub(crate) use footer::{FooterCtx, render_footer};
```

and
```rust
pub(crate) use toolbar::render_toolbar;
```
to
```rust
pub(crate) use toolbar::{ToolbarCtx, render_toolbar};
```

- [ ] **Step 4: Update the toolbar call site in `view.rs`**

In `crate/src/file_manager/view.rs`, replace the toolbar call (current lines 94-111) with:

```rust
            if deferred.is_none()
                && let Some(a) = render::render_toolbar(
                    ui,
                    render::ToolbarCtx {
                        strings,
                        has_parent: self.has_parent(),
                        can_back: self.history.can_go_back(),
                        can_forward: self.history.can_go_forward(),
                        show_new_folder: &mut self.show_new_folder,
                        new_folder_buf: &mut self.new_folder_buf,
                        show_new_file: &mut self.show_new_file,
                        new_file_buf: &mut self.new_file_buf,
                        show_hidden: self.show_hidden,
                        config: &self.config,
                        buf: &mut self.fmt_buf,
                    },
                )
            {
                deferred = Some(a);
            }
```

- [ ] **Step 5: Update the footer call site in `view.rs`**

In `crate/src/file_manager/view.rs`, replace the footer call (current lines 263-274) with:

```rust
            let (foot_confirmed, foot_cancelled, foot_action) = render::render_footer(
                ui,
                render::FooterCtx {
                    strings,
                    mode: self.mode,
                    entries: &self.entries,
                    selected_indices: &self.selected_indices,
                    filename_buf: &mut self.filename_buf,
                    filters: &self.filters,
                    active_filter: self.active_filter,
                    config: &self.config,
                    buf: &mut self.fmt_buf,
                },
            );
```

- [ ] **Step 6: Verify the allows are gone and nothing regressed**

Run: `cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS, 0 warnings. (If clippy re-flags `too_many_arguments` anywhere, a call site was missed.)

Run: `grep -rn "too_many_arguments" crate/src/file_manager/`
Expected: no output.

- [ ] **Step 7: Commit**

```bash
git add crate/src/file_manager/render/toolbar.rs crate/src/file_manager/render/footer.rs crate/src/file_manager/render/mod.rs crate/src/file_manager/view.rs
git commit -m "$(cat <<'EOF'
refactor(file_manager): bundle toolbar/footer args into Ctx structs

Introduces ToolbarCtx/FooterCtx borrow-bundles mirroring TableCtx and
drops both clippy::too_many_arguments suppressions. Bodies unchanged
(destructure rebinds the same names); split-borrow rules at the call
sites are identical to the previous argument lists.

Co-Authored-By: claude-flow <ruv@ruv.net>
EOF
)"
```

---

## Task 5: Move the i18n catalogue into `crate::i18n::file_manager`

CLAUDE.md mandates each localized widget own a `crate::i18n::<widget>` sub-module (`Strings` + `EN`/`RU` + `strings()`), covered by the central parity tests in `i18n/tests.rs`. `file_manager` instead self-hosts its catalogue in `config.rs`. Relocate it, keep the historic public paths as re-export shims (no API break), and add the canonical tests. Chosen approach: **full migration with backward-compatible aliases** (user-confirmed).

**Files:**
- Create: `crate/src/i18n/file_manager.rs`
- Modify: `crate/src/i18n/mod.rs`
- Modify: `crate/src/i18n/tests.rs`
- Modify: `crate/src/file_manager/config.rs`

- [ ] **Step 1: Write the two failing i18n tests first**

In `crate/src/i18n/tests.rs`, add these two tests (place `file_manager_strings_resolve` after `code_editor_strings_resolve`, and `file_manager_parity_key_fields_nonempty` in the parity section after `code_editor_parity_key_fields_nonempty`):

```rust
#[test]
fn file_manager_strings_resolve() {
    let en = file_manager::strings(Locale::En);
    let ru = file_manager::strings(Locale::Ru);
    assert_eq!(en.cancel, "Cancel");
    assert_eq!(ru.cancel, "Отмена");
    assert_ne!(en.cancel, ru.cancel);
    assert_ne!(en.save, ru.save);
    assert_ne!(en.col_name, ru.col_name);
}
```

```rust
#[test]
fn file_manager_parity_key_fields_nonempty() {
    let en = file_manager::strings(Locale::En);
    let ru = file_manager::strings(Locale::Ru);
    for s in [en.select_folder, en.open, en.save, en.col_name] {
        assert!(!s.is_empty());
    }
    for s in [ru.select_folder, ru.open, ru.save, ru.col_name] {
        assert!(!s.is_empty());
    }
    assert_ne!(en.select_folder, ru.select_folder);
    assert_ne!(en.col_date, ru.col_date);
}
```

- [ ] **Step 2: Run them to confirm they fail to compile**

Run: `cargo test --lib i18n::tests::file_manager 2>&1 | head -20`
Expected: FAIL — `failed to resolve: ... file_manager` / `could not find 'file_manager' in ...` (the module doesn't exist yet).

- [ ] **Step 3: Create `crate/src/i18n/file_manager.rs`**

Create the file with the `Strings` struct (all 40 fields, doc comments preserved from the old `FmStrings`), the `EN` and `RU` constants (values verbatim from the old `STRINGS_EN`/`STRINGS_RU`), and the resolver. Fully documented — no `#![allow(missing_docs)]` (aligns with the Task 1 hardening spirit; the module is crate-public so `missing_docs` applies):

```rust
//! `crate::file_manager` localisation strings.
//!
//! Relocated from `file_manager/config.rs` to match the project-wide i18n
//! convention (one `crate::i18n::<widget>` sub-module per localized widget).
//! The historic paths `file_manager::{FmStrings, STRINGS_EN, STRINGS_RU,
//! strings_for_locale}` are preserved as re-export shims in
//! `file_manager/config.rs`.

use super::Locale;

/// All user-facing strings for the file manager dialog.
///
/// Resolve through [`strings`] (or the `FileManager` locale API). Switching to
/// [`RU`] requires the host to bake `GlyphRanges::Cyrillic` (or a superset)
/// into the active font atlas — otherwise non-ASCII characters render as `?`.
#[derive(Debug)]
pub struct Strings {
    // ── Dialog titles ──
    /// Window title for SelectFolder mode.
    pub select_folder: &'static str,
    /// Window title for OpenFile mode.
    pub open_file: &'static str,
    /// Window title for SaveFile mode.
    pub save_file: &'static str,

    // ── Toolbar buttons ──
    /// Tooltip for the "go to parent" button.
    pub up: &'static str,
    /// Tooltip for the "go back" button.
    pub back: &'static str,
    /// Tooltip for the "go forward" button.
    pub forward: &'static str,
    /// Label for "New Folder" toolbar button.
    pub new_folder: &'static str,
    /// Label for "New File" toolbar button.
    pub new_file: &'static str,
    /// Label for the "Create" button in new folder/file inputs.
    pub create: &'static str,
    /// Label for the "Cancel" button.
    pub cancel: &'static str,
    /// Label for the confirm button in SaveFile mode.
    pub save: &'static str,
    /// Label for the confirm button in OpenFile mode.
    pub open: &'static str,

    // ── Footer / inputs ──
    /// Label for the filename text input (SaveFile mode).
    pub filename: &'static str,
    /// Label for the "All Files" filter entry.
    pub all_files: &'static str,
    /// Shown when directory is empty.
    pub empty_parens: &'static str,

    // ── Error messages ──
    /// Prefix for "cannot read directory" errors.
    pub cannot_read_dir: &'static str,
    /// Prefix for "create folder failed" errors.
    pub create_folder_failed: &'static str,
    /// Prefix for "create file failed" errors.
    pub create_file_failed: &'static str,
    /// Prefix for "path not found" errors.
    pub path_not_found: &'static str,

    // ── Overwrite confirmation ──
    /// Title for the overwrite confirmation modal.
    pub overwrite_title: &'static str,
    /// Body text for the overwrite confirmation modal.
    pub overwrite_message: &'static str,
    /// "Yes" button label.
    pub yes: &'static str,
    /// "No" button label.
    pub no: &'static str,

    // ── Sidebar ──
    /// Header label for the favorites sidebar.
    pub favorites: &'static str,

    // ── Table column headers ──
    /// Column header: file name.
    pub col_name: &'static str,
    /// Column header: file size.
    pub col_size: &'static str,
    /// Column header: date modified.
    pub col_date: &'static str,
    /// Column header: file type/extension.
    pub col_type: &'static str,

    // ── Context menu / actions ──
    /// Context menu item: rename entry.
    pub rename: &'static str,
    /// Context menu item: delete entry.
    pub delete: &'static str,
    /// Title for the delete confirmation modal.
    pub confirm_delete_title: &'static str,
    /// Body text prefix for the delete confirmation modal.
    pub confirm_delete_message: &'static str,
    /// Prefix for "rename failed" errors.
    pub rename_failed: &'static str,
    /// Prefix for "delete failed" errors.
    pub delete_failed: &'static str,
    /// Context menu item: copy file path to clipboard.
    pub copy_path: &'static str,
    /// Toolbar toggle: show/hide hidden files.
    pub show_hidden: &'static str,

    // ── Status bar ──
    /// Suffix for item count, e.g. "42 items".
    pub status_items: &'static str,
    /// Suffix for selection count, e.g. "3 selected".
    pub status_selected: &'static str,
    /// Tooltip: keyboard shortcut hint for status bar.
    pub shortcut_hint: &'static str,
    /// "Select All" label (Ctrl+A context).
    pub select_all: &'static str,
}

/// Default English catalogue.
pub const EN: Strings = Strings {
    select_folder: "Select Folder",
    open_file: "Open File",
    save_file: "Save File",
    up: "Up",
    back: "Back",
    forward: "Forward",
    new_folder: "New Folder",
    new_file: "New File",
    create: "Create",
    cancel: "Cancel",
    save: "Save",
    open: "Open",
    filename: "Filename:",
    all_files: "All Files (*.*)",
    empty_parens: "(empty)",
    cannot_read_dir: "Cannot read directory",
    create_folder_failed: "Failed to create folder",
    create_file_failed: "Failed to create file",
    path_not_found: "Path not found",
    overwrite_title: "Confirm Overwrite",
    overwrite_message: "File already exists. Overwrite?",
    yes: "Yes",
    no: "No",
    favorites: "Favorites",
    col_name: "Name",
    col_size: "Size",
    col_date: "Date Modified",
    col_type: "Type",
    rename: "Rename",
    delete: "Delete",
    confirm_delete_title: "Confirm Delete",
    confirm_delete_message: "Are you sure you want to delete",
    rename_failed: "Failed to rename",
    delete_failed: "Failed to delete",
    copy_path: "Copy Path",
    show_hidden: "Hidden",
    status_items: "items",
    status_selected: "selected",
    shortcut_hint: "F2: Rename · Del: Delete · Backspace: Parent · Type to search",
    select_all: "Select All",
};

/// Russian catalogue. Requires the host to bake `GlyphRanges::Cyrillic` (or a
/// superset) into the active font atlas — without that, non-ASCII characters
/// render as `?` placeholders.
pub const RU: Strings = Strings {
    select_folder: "Выбор папки",
    open_file: "Открыть файл",
    save_file: "Сохранить файл",
    up: "Вверх",
    back: "Назад",
    forward: "Вперёд",
    new_folder: "Новая папка",
    new_file: "Новый файл",
    create: "Создать",
    cancel: "Отмена",
    save: "Сохранить",
    open: "Открыть",
    filename: "Имя файла:",
    all_files: "Все файлы (*.*)",
    empty_parens: "(пусто)",
    cannot_read_dir: "Не удаётся прочитать каталог",
    create_folder_failed: "Не удалось создать папку",
    create_file_failed: "Не удалось создать файл",
    path_not_found: "Путь не найден",
    overwrite_title: "Подтверждение перезаписи",
    overwrite_message: "Файл уже существует. Перезаписать?",
    yes: "Да",
    no: "Нет",
    favorites: "Избранное",
    col_name: "Имя",
    col_size: "Размер",
    col_date: "Изменён",
    col_type: "Тип",
    rename: "Переименовать",
    delete: "Удалить",
    confirm_delete_title: "Подтверждение удаления",
    confirm_delete_message: "Вы уверены, что хотите удалить",
    rename_failed: "Не удалось переименовать",
    delete_failed: "Не удалось удалить",
    copy_path: "Копировать путь",
    show_hidden: "Скрытые",
    status_items: "эл.",
    status_selected: "выделено",
    shortcut_hint: "F2: переименовать · Del: удалить · Backspace: вверх · Введите для поиска",
    select_all: "Выделить всё",
};

/// Resolve the static catalogue for `locale`.
pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}
```

- [ ] **Step 4: Register the module in `i18n/mod.rs`**

In `crate/src/i18n/mod.rs`, add `pub mod file_manager;` in alphabetical order (between `disasm_view` and `force_graph`):

```rust
pub mod disasm_view;
pub mod file_manager;
pub mod force_graph;
```

- [ ] **Step 5: Run the new i18n tests — expect PASS**

Run: `cargo test --lib i18n::tests::file_manager`
Expected: PASS (`file_manager_strings_resolve`, `file_manager_parity_key_fields_nonempty`).

- [ ] **Step 6: Replace the catalogue in `config.rs` with re-export shims**

In `crate/src/file_manager/config.rs`, delete the entire `// ─── Strings (localizable) ───` section — the `pub struct FmStrings { … }` definition, `pub static STRINGS_EN = …`, `pub static STRINGS_RU = …`, and `pub fn strings_for_locale(…) { match … }` (current lines 96-314) — and replace with:

```rust
// ─── Strings (localizable) ──────────────────────────────────────────────────
//
// The catalogue now lives in `crate::i18n::file_manager` (project-wide i18n
// convention). These shims preserve the historic public paths
// (`file_manager::FmStrings`, `STRINGS_EN`, `STRINGS_RU`,
// `strings_for_locale`) so existing callers keep compiling unchanged.

/// All user-facing strings for the file manager dialog.
///
/// Alias of [`crate::i18n::file_manager::Strings`], kept for backward
/// compatibility with the historic `file_manager::FmStrings` path.
pub type FmStrings = crate::i18n::file_manager::Strings;

/// Default English catalogue — re-export of [`crate::i18n::file_manager::EN`].
pub use crate::i18n::file_manager::EN as STRINGS_EN;

/// Russian catalogue — re-export of [`crate::i18n::file_manager::RU`].
pub use crate::i18n::file_manager::RU as STRINGS_RU;

/// Resolve the static catalogue for `locale`.
///
/// ```rust,no_run
/// # use dear_imgui_custom_mod::i18n::Locale;
/// # use dear_imgui_custom_mod::file_manager::strings_for_locale;
/// let s = strings_for_locale(Locale::Ru);
/// assert_eq!(s.cancel, "Отмена");
/// ```
#[must_use]
pub fn strings_for_locale(locale: crate::i18n::Locale) -> &'static FmStrings {
    crate::i18n::file_manager::strings(locale)
}
```

Leave the rest of `config.rs` untouched — `default_strings()` (`&STRINGS_EN` still promotes to `&'static`), the `#[serde(skip, default = "default_strings")] pub strings: &'static FmStrings` field, and `FileManagerConfig` all keep working through the alias.

- [ ] **Step 7: Trim the migrated tests from `config.rs`**

In the `#[cfg(test)] mod tests` block of `config.rs`, delete the two catalogue tests now owned by `i18n/tests.rs`:
- `strings_en_and_ru_diverge_on_translatable_keys`
- `strings_for_locale_resolves`

Keep the three config-behaviour guard tests (`default_locale_is_english`, `locale_round_trips_through_ron`, `locale_field_optional_in_ron`) and the `use crate::i18n::Locale;` import (still used by them).

- [ ] **Step 8: Full verify**

Run:
```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --workspace
```
Expected: all PASS, 0 warnings. `cargo build --workspace` proves the examples-app demos still compile against `file_manager::{FmStrings, STRINGS_EN, STRINGS_RU, strings_for_locale}`.

- [ ] **Step 9: Confirm docs still reference valid paths**

Run: `grep -rn "STRINGS_EN\|STRINGS_RU\|FmStrings\|strings_for_locale" docs/`
Expected: any hits describe paths that still resolve (via the shims). No edit required unless a line asserts the catalogue *lives in* `config.rs` — if so, update that sentence to point at `crate::i18n::file_manager`.

- [ ] **Step 10: Commit**

```bash
git add crate/src/i18n/file_manager.rs crate/src/i18n/mod.rs crate/src/i18n/tests.rs crate/src/file_manager/config.rs
git commit -m "$(cat <<'EOF'
refactor(i18n): move file_manager catalogue into crate::i18n

Relocates FmStrings/STRINGS_EN/STRINGS_RU into
crate::i18n::file_manager (Strings/EN/RU/strings()), matching the
project-wide i18n convention, and adds the canonical
file_manager_strings_resolve + parity tests to i18n/tests.rs.
config.rs keeps backward-compatible re-export shims so the public
API (FmStrings, STRINGS_EN, STRINGS_RU, strings_for_locale) is
unchanged.

Co-Authored-By: claude-flow <ruv@ruv.net>
EOF
)"
```

---

## Self-Review

**Spec coverage:** Task 1 → missing_docs suppression. Task 2 → SAFETY comments (all 3 blocks). Task 3 → table.rs < 500. Task 4 → both too_many_arguments allows removed. Task 5 → catalogue in crate::i18n + central tests + preserved API. All five audit findings mapped.

**Placeholder scan:** No TBD/TODO/"handle edge cases"/"similar to". Every code step shows complete code. `unsafe` SAFETY text is concrete per-block.

**Type consistency:** `ToolbarCtx`/`FooterCtx` field names/types match both the struct defs and the view.rs literals. `handle_table_keyboard(ui, entries, selected_indices, scroll_to_index) -> Option<Action>` matches its single call site. `crate::i18n::file_manager::{Strings, EN, RU, strings}` names match the shims (`FmStrings` alias, `STRINGS_EN`/`STRINGS_RU` re-exports, `strings_for_locale` delegate) and the tests (`file_manager::strings`). `Strings` field names used in tests (`cancel`, `save`, `col_name`, `select_folder`, `open`, `col_date`) all exist in the struct.

**Ordering note:** Tasks are independent and committed separately; execute 1→5. Tasks 1-4 are behaviour-preserving (existing suite is the gate); Task 5 adds tests first (Steps 1-2) before the implementation.
