# VirtualTable / VirtualTree Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix every correctness bug, behavioural inconsistency, and DDD/quality gap found in the `virtual_table` and `virtual_tree` audit, and factor the duplicated inline-editor logic into one shared, tested seam.

**Architecture:** Prefer *testable pure seams*. Every UI-coupled fix is split into a pure helper (unit-tested without a live `Ui`) plus a thin wiring change in the render path (verified by `cargo build`/`clippy` + a final manual `/verify` pass). Shared helpers live in `virtual_table` (the one-way dependency root that `virtual_tree` already imports). Config *values* move to `.ron` per the project DDD pattern; schema stays in `.rs`.

**Tech Stack:** Rust (edition 2024, let-chains), `dear-imgui-rs` 0.14, `ron` for config values, `foldhash`, `cargo test` / `cargo clippy --all-targets -- -D warnings` / `cargo fmt`.

---

## Findings → Task traceability

| # | Severity | Finding | Task |
|---|----------|---------|------|
| F1 | 🔴 High | `TreeArena::remove` uses `swap_remove` → reorders siblings/roots on delete, breaks FIFO `evict_on_overflow` | Task 1 |
| F2 | 🟠 Med | `VirtualTree::scroll_to_node` silently fails for off-screen targets (SetScrollHereY only inside clipper loop) | Task 2 |
| F3 | 🟠 Med | Tree ignores `config.table.selection_color` / `selection_text_color` | Task 3 |
| F4 | 🟠 Med | `1u64 << d` in tree-line render panics (debug) / corrupts (release) at depth ≥ 65 | Task 4 |
| F5 | 🟡 Low | Table eviction bookkeeping doesn't shift `context_row` | Task 5 |
| F6 | 🟡 Low | Unreachable `_ =>` arms use `return` + spurious `deactivate()` | Task 6 |
| F7 | 🟡 Low | `ops.rs` comment says "BFS" but code is DFS | Task 6 |
| F8 | 🟡 Low | Hardcoded theme values (`striped`, arrow, badge colors) not in `.ron` (DDD violation) | Task 7 |
| F9 | 🟢 Info | Duplicated `EditState` + inline-editor render (~150 lines) across the two modules | Task 8 |
| F10 | 🟢 Info | Filter is O(n)/keystroke; lazy_load+filter only sees loaded nodes | Task 9 (docs) |
| F11 | 🟢 Defer | No per-column float precision (`%.2f` hardcoded) | Deferred — see "Out of scope" |

**Phasing:** Phase 1 (Tasks 1–5) = correctness. Phase 2 (Tasks 6–7) = quality/DDD. Phase 3 (Task 8) = the larger DRY refactor (higher risk, ships last). Phase 4 (Task 9) = docs. Task 10 = final verification.

---

## Task 0: Branch + baseline

**Files:** none (git only)

- [ ] **Step 1: Create a work branch off master**

Run:
```bash
git checkout -b fix/vtable-vtree-hardening
```

- [ ] **Step 2: Confirm a green baseline before changing anything**

Run: `cargo test`
Expected: PASS (all existing `virtual_table` / `virtual_tree` unit tests green). If anything fails here, stop and report — the baseline must be green.

- [ ] **Step 3: Confirm clippy is clean**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings.

---

## Task 1: Stable-order removal in `TreeArena` (F1)

`swap_remove` moves the *last* element into the deleted slot, reordering siblings/roots and breaking the FIFO contract `evict_oldest_root` relies on. `move_node` already uses order-preserving `remove`; make deletion match.

**Files:**
- Modify: `crate/src/virtual_tree/arena/mod.rs:216-227` (the `remove` detach block)
- Test: `crate/src/virtual_tree/arena/tests.rs`

- [ ] **Step 1: Write the failing tests**

Append to `crate/src/virtual_tree/arena/tests.rs`:
```rust
#[test]
fn remove_child_preserves_sibling_order() {
    let mut arena = TreeArena::new();
    let r = arena.insert_root(0).unwrap();
    let a = arena.insert_child(r, 1).unwrap();
    let b = arena.insert_child(r, 2).unwrap();
    let c = arena.insert_child(r, 3).unwrap();
    let d = arena.insert_child(r, 4).unwrap();
    arena.remove(b); // remove the 2nd of 4 children
    let vals: Vec<_> = arena
        .children(r)
        .iter()
        .filter_map(|&id| arena.get_data(id).copied())
        .collect();
    assert_eq!(vals, vec![1, 3, 4], "siblings keep order after a middle removal");
    let _ = (a, c, d);
}

#[test]
fn remove_root_preserves_order() {
    let mut arena = TreeArena::new();
    let r1 = arena.insert_root(1).unwrap();
    let r2 = arena.insert_root(2).unwrap();
    let r3 = arena.insert_root(3).unwrap();
    let r4 = arena.insert_root(4).unwrap();
    arena.remove(r2);
    let vals: Vec<_> = arena
        .roots()
        .iter()
        .filter_map(|&id| arena.get_data(id).copied())
        .collect();
    assert_eq!(vals, vec![1, 3, 4], "roots keep order after a middle removal");
    let _ = (r1, r3, r4);
}

#[test]
fn evict_preserves_fifo_after_multiple() {
    let mut arena = TreeArena::with_capacity(3);
    arena.set_evict_on_overflow(true);
    let r1 = arena.insert_root(1).unwrap();
    let r2 = arena.insert_root(2).unwrap();
    let r3 = arena.insert_root(3).unwrap();
    let _r4 = arena.insert_root(4).unwrap(); // evict oldest (r1) → roots [2,3,4]
    let _r5 = arena.insert_root(5).unwrap(); // evict next-oldest (r2) → roots [3,4,5]
    assert!(arena.get_data(r1).is_none(), "r1 evicted first");
    assert!(arena.get_data(r2).is_none(), "r2 evicted second (FIFO), not the newest");
    let vals: Vec<_> = arena
        .roots()
        .iter()
        .filter_map(|&id| arena.get_data(id).copied())
        .collect();
    assert_eq!(vals, vec![3, 4, 5]);
    let _ = r3;
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p dear-imgui-custom-mod remove_child_preserves_sibling_order remove_root_preserves_order evict_preserves_fifo_after_multiple`
Expected: `remove_child_preserves_sibling_order` and `evict_preserves_fifo_after_multiple` FAIL (order/eviction wrong under `swap_remove`).

- [ ] **Step 3: Switch both detaches to order-preserving `remove`**

In `crate/src/virtual_tree/arena/mod.rs`, inside `pub fn remove`, replace the detach block:
```rust
        // Detach from parent first — use position + swap_remove for O(1).
        if let Some(parent_id) = self.get(id)?.parent {
            if let Some(parent_slot) = self.slot_mut(parent_id)
                && let Some(pos) = parent_slot.children.iter().position(|&c| c == id)
            {
                parent_slot.children.swap_remove(pos);
            }
        } else {
            // It's a root — swap_remove is OK since root order may change.
            if let Some(pos) = self.roots.iter().position(|&r| r == id) {
                self.roots.swap_remove(pos);
            }
        }
```
with:
```rust
        // Detach first, preserving sibling / root order. `Vec::remove` is O(n)
        // in the sibling count, but ordering matters: it keeps stable display
        // order on delete and preserves the FIFO contract `evict_oldest_root`
        // depends on. This mirrors `move_node`, which already uses `remove`.
        if let Some(parent_id) = self.get(id)?.parent {
            if let Some(parent_slot) = self.slot_mut(parent_id)
                && let Some(pos) = parent_slot.children.iter().position(|&c| c == id)
            {
                parent_slot.children.remove(pos);
            }
        } else if let Some(pos) = self.roots.iter().position(|&r| r == id) {
            self.roots.remove(pos);
        }
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p dear-imgui-custom-mod remove_child_preserves_sibling_order remove_root_preserves_order evict_preserves_fifo_after_multiple`
Expected: PASS. Also run `cargo test -p dear-imgui-custom-mod arena` — existing arena tests still green.

- [ ] **Step 5: Commit**

```bash
git add crate/src/virtual_tree/arena/mod.rs crate/src/virtual_tree/arena/tests.rs
git commit -m "fix(virtual_tree/arena): preserve sibling/root order on remove (FIFO eviction)

Co-Authored-By: claude-flow <ruv@ruv.net>"
```

---

## Task 2: Off-screen-safe `scroll_to_node` (F2)

Extract the table's scroll fraction math into one shared, tested helper and use it in the tree so scroll works for targets outside the clipper window.

**Files:**
- Modify: `crate/src/virtual_table/helpers.rs` (add `scroll_fraction` + test)
- Modify: `crate/src/virtual_table/mod.rs:166` area (re-export)
- Modify: `crate/src/virtual_table/input.rs:48-54` (use shared helper — behaviour-neutral)
- Modify: `crate/src/virtual_tree/render.rs:169-182` (replace SetScrollHereY loop)

- [ ] **Step 1: Write the failing test for the pure helper**

In `crate/src/virtual_table/helpers.rs`, inside `mod layout_tests`, add:
```rust
    #[test]
    fn scroll_fraction_maps_row_to_unit_interval() {
        assert_eq!(scroll_fraction(0, 100), 0.0);
        assert_eq!(scroll_fraction(99, 100), 1.0);
        assert!((scroll_fraction(50, 101) - 0.5).abs() < f32::EPSILON);
        // Degenerate: single row (or none) never divides by zero.
        assert_eq!(scroll_fraction(0, 1), 0.0);
        assert_eq!(scroll_fraction(0, 0), 0.0);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p dear-imgui-custom-mod scroll_fraction_maps_row_to_unit_interval`
Expected: FAIL to compile — `scroll_fraction` not defined.

- [ ] **Step 3: Add the helper**

In `crate/src/virtual_table/helpers.rs`, after `snap_outer_height`, add:
```rust
/// Fraction in `[0, 1]` to pass to `set_scroll_y(frac * scroll_max_y())` so a
/// target row is brought into view. Computed from the row index (not the live
/// cursor), so it works even when the target is outside the current
/// `ListClipper` window. Shared by `VirtualTable` and `VirtualTree`.
#[inline]
pub(crate) fn scroll_fraction(target: usize, row_count: usize) -> f32 {
    if row_count <= 1 {
        0.0
    } else {
        target as f32 / (row_count - 1) as f32
    }
}
```

- [ ] **Step 4: Re-export at the module root**

In `crate/src/virtual_table/mod.rs`, next to the existing `pub(crate) use helpers::row_height_to_stride;` line, add:
```rust
pub(crate) use helpers::scroll_fraction;
```

- [ ] **Step 5: Run to verify the helper test passes**

Run: `cargo test -p dear-imgui-custom-mod scroll_fraction_maps_row_to_unit_interval`
Expected: PASS.

- [ ] **Step 6: Use the helper in the table (behaviour-neutral refactor)**

In `crate/src/virtual_table/input.rs`, in `handle_scroll`, replace:
```rust
            let frac = target as f32 / (row_count - 1).max(1) as f32;
            ui.set_scroll_y(frac * ui.scroll_max_y());
```
with:
```rust
            let frac = scroll_fraction(target, row_count);
            ui.set_scroll_y(frac * ui.scroll_max_y());
```

- [ ] **Step 7: Fix the tree render path**

In `crate/src/virtual_tree/render.rs`, replace the scroll block:
```rust
        let scroll_target = self
            .scroll_to_node
            .take()
            .and_then(|id| self.flat_view.index_of(id));

        for flat_idx in tok.iter() {
            let idx = flat_idx;
            self.render_row(ui, idx);

            // Scroll to target node
            if scroll_target == Some(idx) {
                unsafe { dear_imgui_rs::sys::igSetScrollHereY(0.5) };
            }
        }
```
with:
```rust
        let scroll_target = self
            .scroll_to_node
            .take()
            .and_then(|id| self.flat_view.index_of(id));

        for flat_idx in tok.iter() {
            self.render_row(ui, flat_idx);
        }

        // Scroll to the target row from its index (not SetScrollHereY inside the
        // clipper loop) so it works even when the target is outside the current
        // clipper window. Mirrors VirtualTable's scroll math.
        if let Some(target) = scroll_target {
            let frac = crate::virtual_table::scroll_fraction(target, row_count);
            ui.set_scroll_y(frac * ui.scroll_max_y());
        }
```

- [ ] **Step 8: Build + clippy**

Run: `cargo build -p dear-imgui-custom-mod && cargo clippy --all-targets -- -D warnings`
Expected: builds clean, no warnings. (Runtime scroll-to-node behaviour is confirmed in the Task 10 `/verify` pass.)

- [ ] **Step 9: Commit**

```bash
git add crate/src/virtual_table/helpers.rs crate/src/virtual_table/mod.rs crate/src/virtual_table/input.rs crate/src/virtual_tree/render.rs
git commit -m "fix(virtual_tree): scroll_to_node works for off-screen targets

Extract shared scroll_fraction helper; tree now scrolls by computed
row offset instead of SetScrollHereY inside the clipper loop.

Co-Authored-By: claude-flow <ruv@ruv.net>"
```

---

## Task 3: Honour table-wide selection colors in the tree (F3)

Introduce two pure resolvers in `virtual_table::row`, unit-test them, then use them in both widgets so selection background/text colour resolve identically.

**Files:**
- Modify: `crate/src/virtual_table/row.rs` (add resolvers + `#[cfg(test)]` module)
- Modify: `crate/src/virtual_table/row_render.rs` (4 call sites → helpers)
- Modify: `crate/src/virtual_tree/row.rs:38-51` and `:165-172` (use helpers)

- [ ] **Step 1: Write the failing resolver tests**

At the end of `crate/src/virtual_table/row.rs`, add:
```rust
#[cfg(test)]
mod selection_tests {
    use super::*;

    fn style(sel: Option<[f32; 4]>, sel_text: Option<[f32; 4]>, text: Option<[f32; 4]>) -> RowStyle {
        RowStyle {
            selection_color: sel,
            selection_text_color: sel_text,
            text_color: text,
            ..Default::default()
        }
    }

    #[test]
    fn bg_prefers_row_override_then_table_default() {
        let dflt = [0.2, 0.45, 0.85, 0.75];
        // No row override → table default.
        assert_eq!(resolve_selection_bg(None, dflt), Some(dflt));
        // Row override wins.
        let s = style(Some([1.0, 0.0, 0.0, 1.0]), None, None);
        assert_eq!(resolve_selection_bg(Some(&s), dflt), Some([1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn bg_transparent_default_suppresses_paint() {
        assert_eq!(resolve_selection_bg(None, [0.2, 0.45, 0.85, 0.0]), None);
    }

    #[test]
    fn text_color_priority_chain() {
        let dflt = Some([1.0, 1.0, 1.0, 1.0]);
        // per-row selection_text_color wins outright.
        let s = style(None, Some([0.1, 0.2, 0.3, 1.0]), Some([9.0, 9.0, 9.0, 1.0]));
        assert_eq!(resolve_selection_text_color(Some(&s), dflt), Some([0.1, 0.2, 0.3, 1.0]));
        // else table default.
        assert_eq!(resolve_selection_text_color(None, dflt), dflt);
        // else per-row text_color fallback.
        let s2 = style(None, None, Some([0.4, 0.4, 0.4, 1.0]));
        assert_eq!(resolve_selection_text_color(Some(&s2), None), Some([0.4, 0.4, 0.4, 1.0]));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p dear-imgui-custom-mod selection_tests`
Expected: FAIL to compile — resolvers not defined.

- [ ] **Step 3: Add the resolvers**

In `crate/src/virtual_table/row.rs`, after the `RowStyle` struct definition, add:
```rust
// ─── Shared selection-colour resolution ─────────────────────────────────────
// Used by both VirtualTable and VirtualTree so the two widgets resolve the
// effective selection background / text colour identically.

/// Effective selection background: per-row `selection_color` override, else the
/// table-wide default. Returns `None` when the result is fully transparent
/// (`alpha == 0`), signalling "don't paint — let the built-in highlight show".
#[inline]
pub(crate) fn resolve_selection_bg(
    row_style: Option<&RowStyle>,
    table_default: [f32; 4],
) -> Option<[f32; 4]> {
    let bg = row_style
        .and_then(|s| s.selection_color)
        .unwrap_or(table_default);
    (bg[3] > 0.0).then_some(bg)
}

/// Effective text colour for a selected row: per-row `selection_text_color`
/// → table-wide default → per-row `text_color` fallback.
#[inline]
pub(crate) fn resolve_selection_text_color(
    row_style: Option<&RowStyle>,
    table_default: Option<[f32; 4]>,
) -> Option<[f32; 4]> {
    row_style
        .and_then(|s| s.selection_text_color)
        .or(table_default)
        .or_else(|| row_style.and_then(|s| s.text_color))
}
```

- [ ] **Step 4: Run to verify the resolver tests pass**

Run: `cargo test -p dear-imgui-custom-mod selection_tests`
Expected: PASS.

- [ ] **Step 5: Route the table's 4 call sites through the resolvers (parity + DRY)**

In `crate/src/virtual_table/row_render.rs`, in `render_row`, replace the selected-bg block:
```rust
        if is_selected {
            let sel_bg = row_style
                .as_ref()
                .and_then(|s| s.selection_color)
                .unwrap_or(self.config.selection_color);
            if sel_bg[3] > 0.0 {
                ui.table_set_row_bg1_color(sel_bg);
            }
        }
```
with:
```rust
        if is_selected
            && let Some(sel_bg) =
                row::resolve_selection_bg(row_style.as_ref(), self.config.selection_color)
        {
            ui.table_set_row_bg1_color(sel_bg);
        }
```
and replace the `row_text_color` computation:
```rust
        let row_text_color = if is_selected {
            row_style
                .as_ref()
                .and_then(|s| s.selection_text_color)
                .or(self.config.selection_text_color)
                .or_else(|| row_style.as_ref().and_then(|s| s.text_color))
        } else {
            row_style.as_ref().and_then(|s| s.text_color)
        };
```
with:
```rust
        let row_text_color = if is_selected {
            row::resolve_selection_text_color(row_style.as_ref(), self.config.selection_text_color)
        } else {
            row_style.as_ref().and_then(|s| s.text_color)
        };
```
Then apply the **identical two replacements** in `render_row_readonly` in `crate/src/virtual_table/render.rs` (same two blocks: the `if is_selected { let sel_bg = … }` around line 347, and the `row_text_color` around line 405). Use `row::resolve_selection_bg(...)` and `row::resolve_selection_text_color(...)` exactly as above.

- [ ] **Step 6: Fix the tree (the actual bug)**

In `crate/src/virtual_tree/row.rs`, replace the row-background block:
```rust
        if is_selected {
            if let Some(ref style) = row_style
                && let Some(sel_bg) = style.selection_color
                && sel_bg[3] > 0.0
            {
                ui.table_set_row_bg1_color(sel_bg);
            }
        } else if let Some(ref style) = row_style
            && let Some(bg) = style.bg_color
        {
            ui.table_set_row_bg1_color(bg);
        } else if self.config.striped && flat_idx % 2 == 1 {
            ui.table_set_row_bg1_color([1.0, 1.0, 1.0, 0.02]);
        }
```
with:
```rust
        if is_selected {
            // Parity with VirtualTable: per-row override → table-wide
            // `config.table.selection_color`. A fully-transparent default
            // suppresses the paint, letting Selectable's Header tint show.
            if let Some(sel_bg) = crate::virtual_table::row::resolve_selection_bg(
                row_style.as_ref(),
                self.config.table.selection_color,
            ) {
                ui.table_set_row_bg1_color(sel_bg);
            }
        } else if let Some(ref style) = row_style
            && let Some(bg) = style.bg_color
        {
            ui.table_set_row_bg1_color(bg);
        } else if self.config.striped && flat_idx % 2 == 1 {
            ui.table_set_row_bg1_color([1.0, 1.0, 1.0, 0.02]);
        }
```
and replace the `row_text_color` computation:
```rust
        let row_text_color = if is_selected {
            row_style
                .as_ref()
                .and_then(|s| s.selection_text_color)
                .or_else(|| row_style.as_ref().and_then(|s| s.text_color))
        } else {
            row_style.as_ref().and_then(|s| s.text_color)
        };
```
with:
```rust
        let row_text_color = if is_selected {
            crate::virtual_table::row::resolve_selection_text_color(
                row_style.as_ref(),
                self.config.table.selection_text_color,
            )
        } else {
            row_style.as_ref().and_then(|s| s.text_color)
        };
```

- [ ] **Step 7: Build, clippy, full test**

Run: `cargo test -p dear-imgui-custom-mod && cargo clippy --all-targets -- -D warnings`
Expected: all pass, no warnings. (`row::` is already in scope in `row_render.rs`/`render.rs` via `use super::*`.)

- [ ] **Step 8: Commit**

```bash
git add crate/src/virtual_table/row.rs crate/src/virtual_table/row_render.rs crate/src/virtual_table/render.rs crate/src/virtual_tree/row.rs
git commit -m "fix(virtual_tree): honour config selection_color/selection_text_color

Add shared resolve_selection_bg/text helpers; route both widgets through
them so tree selection matches VirtualTable.

Co-Authored-By: claude-flow <ruv@ruv.net>"
```

---

## Task 4: Overflow-safe tree-line depths (F4)

**Files:**
- Modify: `crate/src/virtual_tree/flat_view.rs` (add `continuation_line_depths` + test)
- Modify: `crate/src/virtual_tree/tree_cell.rs:43-54` (use it)

- [ ] **Step 1: Write the failing test**

In `crate/src/virtual_tree/flat_view.rs`, inside `mod tests`, add:
```rust
    #[test]
    fn continuation_depths_no_shift_overflow_beyond_64() {
        // Must not panic for depth > 64 (guards the `1u64 << d` shift).
        let depths: Vec<u16> = continuation_line_depths(70, u64::MAX).collect();
        assert!(depths.iter().all(|&d| d < 64), "clamped to < 64 levels");
        assert_eq!(depths.len(), 63, "d in 1..64 with all mask bits set");
    }

    #[test]
    fn continuation_depths_respects_mask() {
        // Only bits 1 and 3 set → depths 1 and 3 drawn.
        let mask = (1u64 << 1) | (1u64 << 3);
        let depths: Vec<u16> = continuation_line_depths(5, mask).collect();
        assert_eq!(depths, vec![1, 3]);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p dear-imgui-custom-mod continuation_depths`
Expected: FAIL to compile — function not defined.

- [ ] **Step 3: Add the helper**

In `crate/src/virtual_tree/flat_view.rs`, after the `FlatRow` struct, add:
```rust
/// Ancestor depths in `1..depth` at which a vertical tree-line continuation is
/// drawn, per `continuation_mask`. Clamped to 64 levels: `continuation_mask` is
/// a `u64` (see `rebuild`), and `1u64 << d` for `d >= 64` would panic in debug
/// / silently wrap in release. Deeper trees simply stop drawing lines past 64.
#[inline]
pub(crate) fn continuation_line_depths(depth: u16, mask: u64) -> impl Iterator<Item = u16> {
    (1..depth.min(64)).filter(move |&d| mask & (1u64 << d) != 0)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p dear-imgui-custom-mod continuation_depths`
Expected: PASS.

- [ ] **Step 5: Use it in the renderer**

In `crate/src/virtual_tree/tree_cell.rs`, replace the vertical-lines loop:
```rust
            // Vertical continuation lines at ancestor depths
            for d in 1..flat_row.depth {
                if flat_row.continuation_mask & (1u64 << d) != 0 {
                    let x = cursor_screen[0] + (d as f32) * indent_w + indent_w * 0.5;
                    draw_list
                        .add_line(
                            [x, cursor_screen[1]],
                            [x, cursor_screen[1] + row_h],
                            line_color,
                        )
                        .build();
                }
            }
```
with:
```rust
            // Vertical continuation lines at ancestor depths (overflow-safe).
            for d in
                flat_view::continuation_line_depths(flat_row.depth, flat_row.continuation_mask)
            {
                let x = cursor_screen[0] + (d as f32) * indent_w + indent_w * 0.5;
                draw_list
                    .add_line(
                        [x, cursor_screen[1]],
                        [x, cursor_screen[1] + row_h],
                        line_color,
                    )
                    .build();
            }
```

- [ ] **Step 6: Build + clippy**

Run: `cargo build -p dear-imgui-custom-mod && cargo clippy --all-targets -- -D warnings`
Expected: clean. (`flat_view::` is in scope in `tree_cell.rs` via `use super::*`.)

- [ ] **Step 7: Commit**

```bash
git add crate/src/virtual_tree/flat_view.rs crate/src/virtual_tree/tree_cell.rs
git commit -m "fix(virtual_tree): guard tree-line depth shift against u64 overflow (>64 deep)

Co-Authored-By: claude-flow <ruv@ruv.net>"
```

---

## Task 5: Shift `context_row` on eviction (F5)

**Files:**
- Modify: `crate/src/virtual_table/mod.rs` (`push` guard + `shift_indices_for_eviction`)
- Test: `crate/src/virtual_table/mod.rs` (`mod table_tests`)

- [ ] **Step 1: Write the failing tests**

In `crate/src/virtual_table/mod.rs`, inside `mod table_tests`, add:
```rust
    #[test]
    fn push_eviction_shifts_context_row() {
        let mut t = table(3);
        for v in 0..3 {
            t.push(R(v));
        }
        t.context_row = Some(2);
        t.push(R(3)); // evict logical row 0 → context 2 slides to 1
        assert_eq!(t.context_row, Some(1));
    }

    #[test]
    fn push_eviction_drops_context_row_zero() {
        let mut t = table(3);
        for v in 0..3 {
            t.push(R(v));
        }
        t.context_row = Some(0);
        t.push(R(3)); // the row context pointed at is gone
        assert_eq!(t.context_row, None);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p dear-imgui-custom-mod push_eviction_shifts_context_row push_eviction_drops_context_row_zero`
Expected: FAIL — `context_row` unchanged because eviction bookkeeping is skipped and doesn't touch it.

- [ ] **Step 3: Add `context_row` to the eviction guard**

In `crate/src/virtual_table/mod.rs`, in `push`, extend the condition:
```rust
        if self.data.len() == self.data.capacity()
            && (!self.selected_rows.is_empty()
                || self.selection_anchor.is_some()
                || self.pending_scroll_to.is_some()
                || self.edit_state.active)
        {
            self.shift_indices_for_eviction();
        }
```
to:
```rust
        if self.data.len() == self.data.capacity()
            && (!self.selected_rows.is_empty()
                || self.selection_anchor.is_some()
                || self.pending_scroll_to.is_some()
                || self.context_row.is_some()
                || self.edit_state.active)
        {
            self.shift_indices_for_eviction();
        }
```

- [ ] **Step 4: Shift `context_row` inside the bookkeeping**

In `shift_indices_for_eviction`, after the `pending_scroll_to` match block, add:
```rust
        self.context_row = match self.context_row {
            Some(0) | None => None,
            Some(a) => Some(a - 1),
        };
```

- [ ] **Step 5: Run to verify they pass**

Run: `cargo test -p dear-imgui-custom-mod push_eviction`
Expected: PASS (all four `push_eviction*` tests).

- [ ] **Step 6: Commit**

```bash
git add crate/src/virtual_table/mod.rs
git commit -m "fix(virtual_table): shift context_row on FIFO eviction

Co-Authored-By: claude-flow <ruv@ruv.net>"
```

---

## Task 6: Kill unreachable `return` arms + fix stale comment (F6, F7)

Pure refactor — verified by build/clippy (the `_ =>` arms are unreachable by construction: `editor_kind` is derived from the same `editor` value being re-matched).

**Files:**
- Modify: `crate/src/virtual_table/row_render.rs` (ComboBox + Button arms)
- Modify: `crate/src/virtual_tree/row.rs` (ComboBox + Button arms in `render_data_cell`)
- Modify: `crate/src/virtual_tree/arena/ops.rs:80` (comment)

- [ ] **Step 1: Table — ComboBox arm**

In `crate/src/virtual_table/row_render.rs`, `EditorKind::ComboBox` branch, change:
```rust
                            let items = match &self.columns[col_idx].editor {
                                CellEditor::ComboBox { items } => items,
                                _ => {
                                    self.edit_state.deactivate();
                                    return;
                                }
                            };
```
to:
```rust
                            let items = match &self.columns[col_idx].editor {
                                CellEditor::ComboBox { items } => items,
                                // Unreachable: editor_kind already classified this
                                // column as ComboBox. Skip the cell rather than
                                // aborting the whole row if that ever changes.
                                _ => continue,
                            };
```

- [ ] **Step 2: Table — Button arm**

In the `EditorKind::Button` branch, change:
```rust
                        let label = match &self.columns[col_idx].editor {
                            CellEditor::Button { label } => label.as_str(),
                            _ => {
                                self.edit_state.deactivate();
                                return;
                            }
                        };
```
to:
```rust
                        let label = match &self.columns[col_idx].editor {
                            CellEditor::Button { label } => label.as_str(),
                            _ => continue, // unreachable; skip cell, don't abort row
                        };
```

- [ ] **Step 3: Tree — ComboBox + Button arms**

In `crate/src/virtual_tree/row.rs`, `render_data_cell`, change both `_ => { self.edit_state.deactivate(); return; }` arms (ComboBox and Button) to:
```rust
                        _ => return, // unreachable; editor_kind already matched
```
(`render_data_cell` renders one cell, so `return` skips just this cell — no spurious `deactivate`.)

- [ ] **Step 4: Fix the stale comment**

In `crate/src/virtual_tree/arena/ops.rs`, in `update_subtree_depth`, change:
```rust
    /// Update depth of a node's entire subtree after reparenting.
    /// Iterative BFS to avoid stack overflow on deep trees.
```
to:
```rust
    /// Update depth of a node's entire subtree after reparenting.
    /// Iterative DFS (LIFO stack) to avoid stack overflow on deep trees.
```

- [ ] **Step 5: Build, clippy, full test**

Run: `cargo test -p dear-imgui-custom-mod && cargo clippy --all-targets -- -D warnings`
Expected: all pass, no warnings.

- [ ] **Step 6: Commit**

```bash
git add crate/src/virtual_table/row_render.rs crate/src/virtual_tree/row.rs crate/src/virtual_tree/arena/ops.rs
git commit -m "refactor(virtual_table,virtual_tree): drop unreachable return arms; fix DFS comment

Co-Authored-By: claude-flow <ruv@ruv.net>"
```

---

## Task 7: Move tree theme values into `config.ron` (F8, DDD compliance)

Add `striped_color`, `arrow_color`, `badge_color` to `TreeConfig` (schema in `.rs`, values in `.ron`, `#[serde(default)]` so older ron still parses).

**Files:**
- Modify: `crate/src/virtual_tree/config.rs` (3 fields + 3 default fns)
- Modify: `crate/src/virtual_tree/config.ron` (3 values)
- Modify: `crate/src/virtual_tree/row.rs` (striped color)
- Modify: `crate/src/virtual_tree/tree_cell.rs` (arrow + badge colors)
- Test: `crate/src/virtual_tree/mod.rs` (`mod tests`)

- [ ] **Step 1: Write the failing test**

In `crate/src/virtual_tree/mod.rs`, inside `mod tests`, add:
```rust
    #[test]
    fn tree_config_theme_values_from_ron() {
        let cfg = TreeConfig::default();
        assert_eq!(cfg.striped_color, [1.0, 1.0, 1.0, 0.02]);
        assert_eq!(cfg.arrow_color, [0.65, 0.68, 0.72, 1.0]);
        assert_eq!(cfg.badge_color, [0.50, 0.55, 0.62, 1.0]);
    }

    #[test]
    fn tree_config_round_trips_through_ron() {
        let cfg = TreeConfig::default();
        let s = ron::to_string(&cfg).expect("serialize");
        let back: TreeConfig = ron::from_str(&s).expect("deserialize");
        assert_eq!(back.striped_color, cfg.striped_color);
        assert_eq!(back.arrow_color, cfg.arrow_color);
        assert_eq!(back.badge_color, cfg.badge_color);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p dear-imgui-custom-mod tree_config_theme_values_from_ron tree_config_round_trips_through_ron`
Expected: FAIL to compile — fields don't exist.

- [ ] **Step 3: Add the schema fields + defaults**

In `crate/src/virtual_tree/config.rs`, add these fields to `TreeConfig` (just before `card_border`):
```rust
    /// Zebra-stripe background tint for odd rows (used when `striped`).
    #[serde(default = "default_striped_color")]
    pub striped_color: [f32; 4],

    /// Fill colour of the `ExpandStyle::Arrow` triangle.
    #[serde(default = "default_arrow_color")]
    pub arrow_color: [f32; 4],

    /// Text colour of the per-node badge (`VirtualTreeNode::badge`).
    #[serde(default = "default_badge_color")]
    pub badge_color: [f32; 4],
```
and add the default fns next to `default_card_border_true`:
```rust
fn default_striped_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 0.02]
}

fn default_arrow_color() -> [f32; 4] {
    [0.65, 0.68, 0.72, 1.0]
}

fn default_badge_color() -> [f32; 4] {
    [0.50, 0.55, 0.62, 1.0]
}
```

- [ ] **Step 4: Add the values to `config.ron`**

In `crate/src/virtual_tree/config.ron`, before the closing `)` (after `evict_on_overflow: false,` / `card_border: true,`), add:
```
    striped_color: (1.0, 1.0, 1.0, 0.02),
    arrow_color: (0.65, 0.68, 0.72, 1.0),
    badge_color: (0.50, 0.55, 0.62, 1.0),
```

- [ ] **Step 5: Run to verify the config tests pass**

Run: `cargo test -p dear-imgui-custom-mod tree_config_theme_values_from_ron tree_config_round_trips_through_ron`
Expected: PASS.

- [ ] **Step 6: Wire the values into the renderer**

In `crate/src/virtual_tree/row.rs`, change:
```rust
        } else if self.config.striped && flat_idx % 2 == 1 {
            ui.table_set_row_bg1_color([1.0, 1.0, 1.0, 0.02]);
        }
```
to:
```rust
        } else if self.config.striped && flat_idx % 2 == 1 {
            ui.table_set_row_bg1_color(self.config.striped_color);
        }
```
In `crate/src/virtual_tree/tree_cell.rs`, in the `ExpandStyle::Arrow` arm, change:
```rust
                    let arrow_color = crate::utils::color::rgba_f32(0.65, 0.68, 0.72, 1.0);
```
to:
```rust
                    let ac = self.config.arrow_color;
                    let arrow_color = crate::utils::color::rgba_f32(ac[0], ac[1], ac[2], ac[3]);
```
and in the badge block, change:
```rust
                ui.text_colored([0.50, 0.55, 0.62, 1.0], badge);
```
to:
```rust
                ui.text_colored(self.config.badge_color, badge);
```

- [ ] **Step 7: Build, clippy, full test**

Run: `cargo test -p dear-imgui-custom-mod && cargo clippy --all-targets -- -D warnings`
Expected: all pass, no warnings.

- [ ] **Step 8: Commit**

```bash
git add crate/src/virtual_tree/config.rs crate/src/virtual_tree/config.ron crate/src/virtual_tree/row.rs crate/src/virtual_tree/tree_cell.rs
git commit -m "refactor(virtual_tree): move striped/arrow/badge colours into config.ron (DDD)

Co-Authored-By: claude-flow <ruv@ruv.net>"
```

---

## Task 8: De-duplicate the inline editor (F9) — higher-risk, ships last

Both modules carry a near-identical `EditState` value-buffer + a ~90-line editor-widget match. Extract the *value buffers* and the *widget render* into `virtual_table::edit_common`; keep each widget's key (`row: usize` vs `node: Option<NodeId>`) in a thin wrapper. This removes ~150 duplicated lines with one authoritative implementation.

> **Risk note:** the editor match encodes subtle focus/commit semantics (`just_activated`, `is_item_deactivated_after_edit`, `commit_on_focus_loss`). Copy the widget bodies *verbatim* from the existing code — do not "improve" them here. Runtime behaviour is confirmed in the Task 10 `/verify` pass.

**Files:**
- Create: `crate/src/virtual_table/edit_common.rs`
- Modify: `crate/src/virtual_table/mod.rs` (declare module + re-export)
- Modify: `crate/src/virtual_table/edit.rs` (delegate `EditState`)
- Modify: `crate/src/virtual_table/editor.rs` (call shared widget render)
- Modify: `crate/src/virtual_tree/mod.rs` (delegate `EditState`)
- Modify: `crate/src/virtual_tree/edit.rs` (call shared widget render)

- [ ] **Step 1: Write the failing tests for the shared buffers**

Create `crate/src/virtual_table/edit_common.rs` with only the test module first (so the test drives the API):
```rust
//! Shared inline-editor value buffers + widget render, used by both
//! `VirtualTable` and `VirtualTree` so the editor semantics live in one place.

use super::column::CellEditor;
use super::row::CellValue;
use dear_imgui_rs::{Key, Ui};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_copy_from_value_and_take_round_trip() {
        let mut b = EditBuffers::default();
        b.copy_from_value(&CellValue::Text("hi".into()));
        assert!(b.just_activated);
        match b.take_cell_value(&CellEditor::TextInput) {
            CellValue::Text(s) => assert_eq!(s, "hi"),
            _ => panic!("expected Text"),
        }
        // text_buf is replaced with a fresh pre-allocated buffer.
        assert!(b.text_buf.is_empty());
        assert!(b.text_buf.capacity() >= 256);
    }

    #[test]
    fn buffers_clamp_int_to_i32() {
        let mut b = EditBuffers::default();
        b.copy_from_value(&CellValue::Int(i64::MAX));
        assert_eq!(b.int_val, i32::MAX);
        match b.take_cell_value(&CellEditor::SpinInt { step: 1, step_fast: 10 }) {
            CellValue::Int(v) => assert_eq!(v, i32::MAX as i64),
            _ => panic!("expected Int"),
        }
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p dear-imgui-custom-mod buffers_copy_from_value_and_take_round_trip`
Expected: FAIL to compile — `EditBuffers` not defined / module not declared.

- [ ] **Step 3: Declare the module**

In `crate/src/virtual_table/mod.rs`, next to `mod edit;`, add:
```rust
pub(crate) mod edit_common;
```

- [ ] **Step 4: Implement `EditBuffers` + `EditOutcome` + `render_editor_widget`**

At the top of `crate/src/virtual_table/edit_common.rs` (above the `#[cfg(test)]` module), add:
```rust
/// The per-type value buffers behind an active inline editor. Widget-agnostic:
/// the owning widget adds its own key (`row: usize` / `node: NodeId`).
#[derive(Clone, Debug)]
pub(crate) struct EditBuffers {
    /// True on the very first frame after activation (drives focus grab).
    pub just_activated: bool,
    pub text_buf: String,
    pub bool_val: bool,
    pub int_val: i32,
    pub float_val: f32,
    pub choice_idx: usize,
    pub color_val: [f32; 4],
}

impl Default for EditBuffers {
    fn default() -> Self {
        Self {
            just_activated: false,
            text_buf: String::with_capacity(256),
            bool_val: false,
            int_val: 0,
            float_val: 0.0,
            choice_idx: 0,
            color_val: [1.0; 4],
        }
    }
}

impl EditBuffers {
    /// Copy a cell value into the buffers and arm `just_activated`.
    pub fn copy_from_value(&mut self, value: &CellValue) {
        self.just_activated = true;
        match value {
            CellValue::Text(s) => {
                self.text_buf.clear();
                self.text_buf.push_str(s);
            }
            CellValue::Bool(b) => self.bool_val = *b,
            CellValue::Int(v) => {
                self.int_val = (*v).clamp(i32::MIN as i64, i32::MAX as i64) as i32
            }
            CellValue::Float(v) => self.float_val = (*v as f32).clamp(f32::MIN, f32::MAX),
            CellValue::Choice(idx) => self.choice_idx = *idx,
            CellValue::Color(c) => self.color_val = *c,
            CellValue::Progress(_) | CellValue::Custom => {}
        }
    }

    /// Build a `CellValue` from the buffers matching `editor`. For text, moves
    /// the string out (zero-copy) and leaves a fresh pre-allocated buffer.
    pub fn take_cell_value(&mut self, editor: &CellEditor) -> CellValue {
        match editor {
            CellEditor::None | CellEditor::TextInput => {
                let text = std::mem::replace(&mut self.text_buf, String::with_capacity(256));
                CellValue::Text(text)
            }
            CellEditor::Checkbox => CellValue::Bool(self.bool_val),
            CellEditor::ComboBox { .. } => CellValue::Choice(self.choice_idx),
            CellEditor::SliderInt { .. } | CellEditor::SpinInt { .. } => {
                CellValue::Int(self.int_val as i64)
            }
            CellEditor::SliderFloat { .. } | CellEditor::SpinFloat { .. } => {
                CellValue::Float(self.float_val as f64)
            }
            CellEditor::ColorEdit => CellValue::Color(self.color_val),
            CellEditor::ProgressBar => CellValue::Progress(self.float_val),
            CellEditor::Button { .. } | CellEditor::Custom => CellValue::Custom,
        }
    }
}

/// Result of rendering the editor widget for one frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EditOutcome {
    /// Still editing — keep the editor open.
    Continue,
    /// Commit the buffered value back to the cell.
    Commit,
    /// Discard the edit.
    Cancel,
    /// `CellEditor::Custom` — the caller must render via the node/row trait.
    Custom,
}

/// Render the built-in editor widget for `editor` into `buf`, returning the
/// frame's outcome. Focus/commit semantics are identical to the pre-refactor
/// per-module code; `Custom` is delegated back to the caller.
pub(crate) fn render_editor_widget(
    ui: &Ui,
    editor: &CellEditor,
    buf: &mut EditBuffers,
    first_frame: bool,
    commit_on_focus_loss: bool,
) -> EditOutcome {
    let mut outcome = EditOutcome::Continue;
    match editor {
        CellEditor::TextInput => {
            if first_frame {
                unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
            }
            let entered = ui
                .input_text("##edit", &mut buf.text_buf)
                .enter_returns_true(true)
                .build();
            if entered {
                outcome = EditOutcome::Commit;
            } else if !first_frame {
                if ui.is_item_deactivated_after_edit() {
                    outcome = if commit_on_focus_loss {
                        EditOutcome::Commit
                    } else {
                        EditOutcome::Cancel
                    };
                } else if ui.is_item_deactivated() {
                    outcome = EditOutcome::Cancel;
                }
            }
        }
        CellEditor::SliderInt { min, max } => {
            ui.slider_config("##edit", *min, *max).build(&mut buf.int_val);
            if !first_frame && ui.is_item_deactivated_after_edit() {
                outcome = EditOutcome::Commit;
            }
        }
        CellEditor::SliderFloat { min, max } => {
            ui.slider_config("##edit", *min, *max)
                .build(&mut buf.float_val);
            if !first_frame && ui.is_item_deactivated_after_edit() {
                outcome = EditOutcome::Commit;
            }
        }
        CellEditor::SpinInt { step, step_fast } => {
            if first_frame {
                unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
            }
            unsafe {
                dear_imgui_rs::sys::igInputInt(
                    c"##edit".as_ptr(),
                    &mut buf.int_val,
                    *step,
                    *step_fast,
                    0,
                );
            }
            if !first_frame {
                if ui.is_item_deactivated_after_edit() {
                    outcome = if commit_on_focus_loss {
                        EditOutcome::Commit
                    } else {
                        EditOutcome::Cancel
                    };
                } else if ui.is_item_deactivated() {
                    outcome = EditOutcome::Cancel;
                }
            }
        }
        CellEditor::SpinFloat { step, step_fast } => {
            if first_frame {
                unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
            }
            unsafe {
                dear_imgui_rs::sys::igInputFloat(
                    c"##edit".as_ptr(),
                    &mut buf.float_val,
                    *step,
                    *step_fast,
                    c"%.2f".as_ptr(),
                    0,
                );
            }
            if !first_frame {
                if ui.is_item_deactivated_after_edit() {
                    outcome = if commit_on_focus_loss {
                        EditOutcome::Commit
                    } else {
                        EditOutcome::Cancel
                    };
                } else if ui.is_item_deactivated() {
                    outcome = EditOutcome::Cancel;
                }
            }
        }
        CellEditor::Custom => {
            outcome = EditOutcome::Custom;
        }
        _ => {
            outcome = EditOutcome::Cancel;
        }
    }

    if ui.is_key_pressed(Key::Escape) {
        outcome = EditOutcome::Cancel;
    }
    outcome
}
```

- [ ] **Step 5: Run to verify the buffer tests pass**

Run: `cargo test -p dear-imgui-custom-mod buffers_copy_from_value_and_take_round_trip buffers_clamp_int_to_i32`
Expected: PASS.

- [ ] **Step 6: Delegate `VirtualTable::EditState` to `EditBuffers`**

In `crate/src/virtual_table/edit.rs`, replace the struct + `Default` + `activate`/`take_cell_value` with a thin wrapper. New file body (keep the existing `#[cfg(test)] mod tests` unchanged at the bottom):
```rust
//! Inline cell editing state and logic (table). Value buffers + widget render
//! live in `super::edit_common`; this only adds the row/col key.

use super::column::CellEditor;
use super::edit_common::EditBuffers;
use super::row::CellValue;

/// Tracks the currently active inline editor, if any.
#[derive(Clone, Debug, Default)]
pub(crate) struct EditState {
    pub active: bool,
    pub row: usize,
    pub col: usize,
    pub buf: EditBuffers,
}

impl EditState {
    /// True on the first frame after activation.
    #[inline]
    pub(super) fn just_activated(&self) -> bool {
        self.buf.just_activated
    }

    #[inline]
    pub(super) fn set_activated(&mut self, v: bool) {
        self.buf.just_activated = v;
    }

    pub(super) fn activate(&mut self, row: usize, col: usize, value: &CellValue) {
        self.active = true;
        self.row = row;
        self.col = col;
        self.buf.copy_from_value(value);
    }

    pub(super) fn deactivate(&mut self) {
        self.active = false;
    }

    pub(super) fn take_cell_value(&mut self, editor: &CellEditor) -> CellValue {
        self.buf.take_cell_value(editor)
    }

    #[inline]
    pub(super) fn is_editing(&self, row: usize, col: usize) -> bool {
        self.active && self.row == row && self.col == col
    }
}
```
Then update the existing `mod tests` in this file: fields `es.text_buf`, `es.int_val`, etc. become `es.buf.text_buf`, `es.buf.int_val`; `es.just_activated` becomes `es.just_activated()`. (Adjust each assertion accordingly — the test intent is unchanged.)

- [ ] **Step 7: Rewrite `VirtualTable::render_editor_inline` to call the shared widget**

In `crate/src/virtual_table/editor.rs`, replace the whole `match &editor_snapshot { … }` body plus the trailing Escape/commit/cancel logic in `render_editor_inline` with:
```rust
        let first_frame = self.edit_state.just_activated();
        if first_frame {
            self.edit_state.set_activated(false);
        }

        let outcome = match &editor_snapshot {
            CellEditor::Custom => {
                let mut committed = false;
                if let Some(row) = self.data.get_mut(idx)
                    && row.render_editor(ui, col_idx)
                {
                    committed = true;
                }
                if ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                    edit_common::EditOutcome::Cancel
                } else if committed {
                    edit_common::EditOutcome::Commit
                } else {
                    edit_common::EditOutcome::Continue
                }
            }
            other => edit_common::render_editor_widget(
                ui,
                other,
                &mut self.edit_state.buf,
                first_frame,
                self.config.commit_on_focus_loss,
            ),
        };

        match outcome {
            edit_common::EditOutcome::Commit => {
                let value = self.edit_state.take_cell_value(&editor_snapshot);
                if let Some(row) = self.data.get_mut(idx) {
                    row.set_cell_value(col_idx, &value);
                }
                self.edit_state.deactivate();
            }
            edit_common::EditOutcome::Cancel | edit_common::EditOutcome::Custom => {
                // Custom is handled above; an unexpected Custom here means an
                // unsupported editor → cancel.
                self.edit_state.deactivate();
            }
            edit_common::EditOutcome::Continue => {}
        }
```
(Keep the `ui.set_next_item_width(-1.0);` and `let editor_snapshot = self.columns[col_idx].editor.clone();` lines at the top of the function. `edit_common` is in scope via `use super::*`.)

- [ ] **Step 8: Delegate `VirtualTree::EditState` the same way**

In `crate/src/virtual_tree/mod.rs`, replace the `struct EditState { … }` + its `impl` with:
```rust
#[derive(Clone, Debug, Default)]
struct EditState {
    active: bool,
    node: Option<NodeId>,
    col: usize,
    buf: crate::virtual_table::edit_common::EditBuffers,
}

impl EditState {
    fn just_activated(&self) -> bool {
        self.buf.just_activated
    }

    fn set_activated(&mut self, v: bool) {
        self.buf.just_activated = v;
    }

    fn activate(&mut self, node: NodeId, col: usize, value: &CellValue) {
        self.active = true;
        self.node = Some(node);
        self.col = col;
        self.buf.copy_from_value(value);
    }

    fn deactivate(&mut self) {
        self.active = false;
    }

    fn take_cell_value(&mut self, editor: &CellEditor) -> CellValue {
        self.buf.take_cell_value(editor)
    }

    #[inline]
    fn is_editing(&self, node: NodeId, col: usize) -> bool {
        self.active && self.node == Some(node) && self.col == col
    }
}
```
Then update this file's `mod tests` (`edit_state_is_keyed_by_node_not_row`) — it calls `es.activate(...)`/`es.is_editing(...)`/`es.deactivate()`, which are unchanged, so it needs no edits.

- [ ] **Step 9: Rewrite `VirtualTree::render_editor_inline` to call the shared widget**

In `crate/src/virtual_tree/edit.rs`, replace the `match &editor_snapshot { … }` + trailing logic (keeping the top `ui.set_next_item_width(-1.0);` / `editor_snapshot` clone) with:
```rust
        let first_frame = self.edit_state.just_activated();
        if first_frame {
            self.edit_state.set_activated(false);
        }

        let outcome = match &editor_snapshot {
            CellEditor::Custom => {
                let mut committed = false;
                if let Some(data) = self.arena.get_data_mut(node_id)
                    && data.render_editor(ui, col_idx, node_id)
                {
                    committed = true;
                }
                if ui.is_key_pressed(dear_imgui_rs::Key::Escape) {
                    crate::virtual_table::edit_common::EditOutcome::Cancel
                } else if committed {
                    crate::virtual_table::edit_common::EditOutcome::Commit
                } else {
                    crate::virtual_table::edit_common::EditOutcome::Continue
                }
            }
            other => crate::virtual_table::edit_common::render_editor_widget(
                ui,
                other,
                &mut self.edit_state.buf,
                first_frame,
                self.config.table.commit_on_focus_loss,
            ),
        };

        match outcome {
            crate::virtual_table::edit_common::EditOutcome::Commit => {
                let value = self.edit_state.take_cell_value(&editor_snapshot);
                if let Some(data) = self.arena.get_data_mut(node_id) {
                    data.set_cell_value(col_idx, &value);
                }
                self.edit_state.deactivate();
            }
            crate::virtual_table::edit_common::EditOutcome::Cancel
            | crate::virtual_table::edit_common::EditOutcome::Custom => {
                self.edit_state.deactivate();
            }
            crate::virtual_table::edit_common::EditOutcome::Continue => {}
        }
```

- [ ] **Step 10: Build, clippy, full test**

Run: `cargo test -p dear-imgui-custom-mod && cargo clippy --all-targets -- -D warnings`
Expected: all pass (including the migrated `edit.rs` table tests), no warnings.

- [ ] **Step 11: Commit**

```bash
git add crate/src/virtual_table/edit_common.rs crate/src/virtual_table/mod.rs crate/src/virtual_table/edit.rs crate/src/virtual_table/editor.rs crate/src/virtual_tree/mod.rs crate/src/virtual_tree/edit.rs
git commit -m "refactor(virtual_table,virtual_tree): unify inline editor into edit_common

Single EditBuffers + render_editor_widget shared by both widgets; each
keeps only its own key (row idx / NodeId). Removes ~150 duplicated lines.

Co-Authored-By: claude-flow <ruv@ruv.net>"
```

---

## Task 9: Document known limitations (F10)

Docs-only: make the filter's cost model and the lazy+filter interaction explicit for callers.

**Files:**
- Modify: `crate/src/virtual_tree/api.rs` (doc on `set_filter`)

- [ ] **Step 1: Expand the `set_filter` doc**

In `crate/src/virtual_tree/api.rs`, replace the doc-less `pub fn set_filter` with:
```rust
    /// Apply a search filter. Empty/whitespace clears it.
    ///
    /// **Cost:** O(n) over all *materialized* nodes per call — one
    /// `matches_filter` per node. For live search over very large trees,
    /// debounce on the host side (filter on a short idle, not every keystroke).
    ///
    /// **Lazy trees:** only materialized nodes are scanned. Matches inside
    /// not-yet-loaded branches are not found and do not trigger a load — expand
    /// (or eager-load) the relevant branches first if they must be searchable.
    pub fn set_filter(&mut self, query: &str) {
```

- [ ] **Step 2: Build the docs**

Run: `cargo doc -p dear-imgui-custom-mod --no-deps`
Expected: builds without warnings for these items.

- [ ] **Step 3: Commit**

```bash
git add crate/src/virtual_tree/api.rs
git commit -m "docs(virtual_tree): document filter O(n) cost and lazy+filter limitation

Co-Authored-By: claude-flow <ruv@ruv.net>"
```

---

## Task 10: Final verification

**Files:** none (verification only)

- [ ] **Step 1: Full workspace test**

Run: `cargo test --workspace`
Expected: PASS (library + demos).

- [ ] **Step 2: Full clippy gate**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Format check**

Run: `cargo fmt --all --check`
Expected: no diff. (If it reports diffs, run `cargo fmt --all` and commit the formatting.)

- [ ] **Step 4: Manual runtime `/verify` of the UI-coupled fixes**

Use the `/verify` skill (or run a tree demo: `cargo run -p examples-app --example demo_<tree_demo> --release`) and confirm end-to-end:
1. **F2 scroll:** `scroll_to_node` on a collapsed/off-screen deep node scrolls it into view.
2. **F3 selection:** setting `config.table.selection_color` to a distinctive colour actually tints selected tree rows; `selection_text_color` recolours selected text.
3. **F4 tree lines:** a tree ≥ 65 levels deep with `show_tree_lines = true` renders without panicking (debug build).
4. **F9 editor:** double-click-edit still commits on Enter / focus-loss and cancels on Esc for Text, SpinInt/Float, Slider, and Custom cells in **both** the table and the tree.

Record the observed result for each. If any regresses, fix before finishing.

- [ ] **Step 5: Finish the branch**

Use the `superpowers:finishing-a-development-branch` skill to choose merge / PR / cleanup.

---

## Out of scope (explicit)

- **F11 — per-column float precision.** Formatting flows through `CellValue::format_into` (no column context) and can be overridden by `cell_display_text`. A correct fix needs a new `ColumnDef` field threaded into every text-render arm without bypassing user overrides — a feature, not a bug fix. Deferred to its own spec.
- **Filter debounce** is a host-side concern (documented in Task 9), not a library change.
- **PageUp/Down step (±20) and the `alignment_pad` right `-4.0` fudge** are behavioural layout constants, not theme values; left as constants intentionally. Revisit only if a caller needs them tunable.

---

## Self-review

- **Spec coverage:** F1→T1, F2→T2, F3→T3, F4→T4, F5→T5, F6/F7→T6, F8→T7, F9→T8, F10→T9, F11→Out-of-scope. All findings mapped.
- **Placeholder scan:** every code step contains full code; no "TODO/handle edge cases/similar to".
- **Type consistency:** `scroll_fraction(target, row_count)`, `resolve_selection_bg(Option<&RowStyle>, [f32;4])`, `resolve_selection_text_color(Option<&RowStyle>, Option<[f32;4]>)`, `continuation_line_depths(u16, u64)`, `EditBuffers`/`EditOutcome`/`render_editor_widget` are named identically at definition and every call site. `EditState.buf` field name is consistent across both modules and their delegating methods.
