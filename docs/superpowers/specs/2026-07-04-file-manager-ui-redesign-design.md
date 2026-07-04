# FileManager UI Redesign — Design

**Source of truth:** `fldr.svg` (740×630 mockup, decoded from exact element geometry + 14 rasterised labels).

**Goal:** Reorganise the file-manager dialog chrome to match the mockup — split the toolbar into a left *action* cluster and a right-aligned *navigation* cluster, add drive-bar dividers, push the footer buttons to the outer edges, and raise the minimum window size.

## Target layout (from mockup)

```
 Select folder                                              ← title (host popup title)
 ──────────────────────────────────────────────────────────
 🖴 C:\   │   🖴 D:\   │   🖴 E:\                            ← drive bar + vertical dividers
 ──────────────────────────────────────────────────────────
 [New Folder] [New File] [Hidden]          ↑ 🔄 ◀ ▶         ← actions (left) · nav (right-aligned)
 ──────────────────────────────────────────────────────────
 📁  E:\ › …                                                ← breadcrumb (unchanged; already has folder icon)
 ──────────────────────────────────────────────────────────
 ┌─ ⭐ Favorites ─┐  ┌──── Name / Size / Date / Type ──────┐
 │   (150 wide)   │  │                                      │
 └────────────────┘  └──────────────────────────────────────┘
 41 items                                                   ← status (unchanged)
 ──────────────────────────────────────────────────────────
 [Cancel]                                   [Select folder] ← footer: flush-left + flush-right
```

## Decisions

| Topic | Decision | Rationale |
|-------|----------|-----------|
| **Min size** | `min_size` 500×350 → **650×500**; `initial_size` 750×520 → **760×600** | Caption in mockup states "мин. ширина 650 / высота 500". Bump initial so the default opens comfortably above the taller minimum. |
| **Toolbar split** | Left: `New Folder`, `New File`, `Hidden` (icon+text). Right (right-aligned, icon-only + tooltip): `Up`, `Refresh`, `Back`, `Forward`. | Matches mockup's action/nav split. Icon-only nav = compact, matches mockup glyph cluster; tooltips preserve discoverability via existing i18n strings. |
| **Up button** | **Kept**, relocated into the right nav cluster. | Mockup omitted it, but removing a nav affordance without confirmation is riskier; reversible. Also avoids dead-code cascade on `has_parent()`. |
| **Disabled nav** | Render as dimmed *buttons* (Alpha 0.4) instead of `text_disabled`. | Stable widths for right-alignment; reads as buttons (matches mockup). |
| **Inline new-folder/file inputs** | Render on their own line **below** the toolbar row. | The right-aligned nav cluster occupies the row's right edge; inline inputs would collide. |
| **Drive dividers** | 1px vertical line (window draw list) between drive buttons, muted `theme::BORDER`. | Matches mockup `│` separators. |
| **Footer** | Cancel flush-left, primary (Select/Open/Save) flush-right; for OpenFile the filter dropdown / SaveFile the filename input sit centered between them. | Matches mockup edge buttons; keeps filter/filename accessible in their modes. |

## Files touched

| File | Change |
|------|--------|
| `crate/src/file_manager/config.ron` | `min_size` → (650,500); `initial_size` → (760,600) |
| `crate/src/file_manager/render/drive_bar.rs` | vertical divider between drives |
| `crate/src/file_manager/render/toolbar.rs` | actions-left / nav-right split; icon-only nav w/ tooltips; disabled-as-dimmed-button; inline inputs on own line |
| `crate/src/file_manager/render/footer.rs` | edge-aligned Cancel/primary; centered filter/filename middle |
| `docs/file_manager.md` | update the `initial_size` / `min_size` values in the config example |

Not changed: `breadcrumb.rs` (already leads with folder icon), favorites panel, table, status bar, `view.rs` render order.

## APIs used (established in-repo idioms)

- Right-align / edge placement: `ui.set_cursor_pos_x(x)` (see `utils/popup.rs::action_row_labeled`).
- Width measurement: `crate::utils::text::calc_text_size(s)[0]` + `ui.clone_style().frame_padding()[0]*2` + `item_spacing`.
- Vertical divider: `ui.get_window_draw_list().add_line([x,y0],[x,y1], crate::utils::col32(theme::BORDER)).build()` (see `property_inspector/render.rs`).
- Tooltips: `crate::utils::themed_tooltip(ui, || ui.text(strings.back))`.

## Verification

- `cargo clippy --all-targets -- -D warnings` (0 warnings), `cargo test`, `cargo build -p examples-app --example demo_file_manager --release`.
- Launch the demo and capture a screenshot (PowerShell + .NET `System.Drawing`) to visually confirm the new layout against the mockup before hand-off.

## Open (flagged for user, reversible)

1. **Up button** — kept in nav cluster; say the word to drop it for a 1:1 mockup match.
2. **Footer filter/filename placement** — centered between edge buttons; alternative is a separate row above the buttons.
