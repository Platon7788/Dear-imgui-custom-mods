//! Render pipeline: row drawing, byte color overrides, data inspector,
//! goto/search popups.
//!
//! `render` is the only entry point. It caches font metrics, computes
//! the visible row window from `scroll_y`, and dispatches to the
//! per-row, per-byte drawing helpers. All of these are split off as
//! `&self` / `&mut self` methods on `HexViewer` (defined here) so the
//! state lookup paths stay direct.

use super::HexViewer;
use super::provider::HexDataProvider;
use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

/// Maximum row size accepted by [`HexViewer`] — mirrors the `BytesPerRow`
/// enum upper bound. The per-row scratch buffer in the render path is
/// stack-allocated to this size so it never spills to the heap.
pub(super) const MAX_BYTES_PER_ROW: usize = 64;

/// Hard cap on the `usize`-projected provider length used by the render
/// path. Streaming providers can legally return `u64::MAX` from `len()`;
/// blindly casting that to `usize` and feeding it into row-count /
/// scroll-extent math would produce non-finite floats and integer wrap.
///
/// `1 << 56` keeps total_rows × line_height inside finite `f32` for any
/// sane line height while still letting the scrollbar represent a
/// terabyte-scale window (the legacy 4 KB row cap of an unbounded
/// provider would advertise a ~16 PB virtual buffer — well within this
/// envelope). For typical hosts wrapping a small buffer the cap is
/// never hit; `provider.len()` is already well below `usize::MAX`.
pub(super) const PROVIDER_LEN_CAP: u64 = 1u64 << 56;

/// Materialise `provider.len()` as a clamped `usize`. Streaming providers
/// may return `u64::MAX`; the render path treats anything above
/// [`PROVIDER_LEN_CAP`] as "very large" so `total_rows * line_height`
/// stays a finite `f32`.
#[inline]
pub(super) fn provider_len_usize<P: HexDataProvider + ?Sized>(p: &P) -> usize {
    let n = p.len();
    if n >= PROVIDER_LEN_CAP {
        PROVIDER_LEN_CAP as usize
    } else {
        n as usize
    }
}

/// Convert `[r, g, b, a]` to packed u32 color.
pub(super) fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// Pixel thickness of the draggable splitter strip between the hex
/// child-window and the inspector. The visual line is 1 px; the rest
/// is invisible hit-area so the user has a comfortable grab zone.
pub(super) const SPLITTER_THICKNESS: f32 = 5.0;

/// Dash-pattern "on" run length (pixels) for the group-divider guide.
/// 4 / 3 reads as a calm pattern at typical font sizes — short enough
/// to look like a guide, long enough that anti-aliasing doesn't smear
/// the dashes into a continuous line.
const DASH_ON: f32 = 4.0;
/// Dash-pattern "off" run length (pixels).
const DASH_OFF: f32 = 3.0;

/// Render a dashed vertical line from `(x, y0)` to `(x, y1)` with the
/// given on/off run pattern. Emits one `add_line` call per dash —
/// inexpensive at the call counts we use (≤ ~25 dashes per row strip
/// per group boundary). The first dash starts exactly at `y0` so the
/// pattern is stable across scrolls.
fn draw_dashed_v_line(
    draw_list: &dear_imgui_rs::DrawListMut<'_>,
    x: f32,
    y0: f32,
    y1: f32,
    color: u32,
    on: f32,
    off: f32,
) {
    if y1 <= y0 || on <= 0.0 {
        return;
    }
    let step = on + off;
    if step <= 0.0 {
        return;
    }
    let mut y = y0;
    while y < y1 {
        let segment_end = (y + on).min(y1);
        if segment_end > y {
            draw_list
                .add_line([x, y], [x, segment_end], color)
                .thickness(1.0)
                .build();
        }
        y += step;
    }
}

// ── HexViewer impl: render entry + virtualisation ────────────────────────────

impl HexViewer {
    // BUG-125's `render_simple()` fallback path was removed on
    // 2026-05-15 (session 88) once BUG-126's nested-child clip bug was
    // fixed inside `render()` via the `window_pos()` Y reference. The
    // canonical `render` now works in every host layout the project
    // cares about. Hosts that wired up `render_simple` should switch
    // to `render`.

    /// Generic provider-driven render entry. Both the legacy
    /// [`Self::render`] (which wraps `self.data` in an
    /// [`super::provider::ArcVecDataProvider`]) and the new
    /// [`Self::render_with_provider`] funnel through here, so the
    /// virtualisation / hit-test / draw-list code is provider-agnostic.
    ///
    /// All per-frame byte reads issued by the row drawer and the data
    /// inspector go through `provider.read()` into a stack-allocated
    /// scratch buffer (`MAX_BYTES_PER_ROW`), so a host implementing
    /// `HexDataProvider` over a lazy-fetched sliding-window backing store
    /// (debugger memory pane, raw-disk view) sees exactly the bytes
    /// inside the visible viewport requested in one batched read per
    /// row + one 8-byte read for the inspector.
    pub(super) fn render_impl(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn HexDataProvider,
    ) {
        if provider.is_empty() {
            return;
        }

        // Live refresh hook — gives streaming providers a chance to
        // poll their backing store once per frame before the row loop
        // reads bytes through `provider.read()`.
        if self.config.auto_refresh_frames > 0 {
            self.frame_count = self.frame_count.wrapping_add(1);
        }
        provider.refresh();

        // Tick down the address-gutter "just copied" flash. Counter
        // hits zero → state cleared so the rect-pill stops painting.
        if let Some((row, frames)) = self.address_flash {
            if frames > 1 {
                self.address_flash = Some((row, frames - 1));
            } else {
                self.address_flash = None;
            }
        }

        // Cache font metrics. Guard against the rare zero-glyph case (e.g.
        // before the font atlas is fully built or in test stubs) — division
        // by zero in the row math below would produce inf-cast UB.
        let [cw, ch] = calc_text_size("0");
        self.char_advance = cw.max(1.0);
        self.line_height = (ch + 2.0).max(1.0);

        let bpr = self.config.bytes_per_row.value();
        // Provider-projected `usize` length (clamped at `PROVIDER_LEN_CAP`
        // so a streaming provider returning `u64::MAX` doesn't break the
        // `total_rows * line_height` extent math below). Hosts wrapping a
        // small buffer pay no clamp — `provider.len()` is well below the
        // cap and round-trips losslessly.
        let data_len_usize = provider_len_usize(provider);
        // Publish for layout/hit-test helpers (`offset_col_width`,
        // `mouse_to_*`) — they read this instead of `self.data.len()`
        // so a provider-driven render reports the provider's view.
        self.effective_data_len = data_len_usize;
        let total_rows = data_len_usize.div_ceil(bpr);

        // BUG-128 (M2): rebuild the 256-entry byte_palette once at the
        // top of the frame so the per-visible-byte path inside
        // `byte_fg_with_overrides` is a single array lookup rather than
        // a 5-arm match + struct field read. Cost is fixed at O(256),
        // independent of how many bytes are visible.
        for b in 0u32..256 {
            self.byte_palette[b as usize] = col32(self.config.byte_fg_color(b as u8));
        }

        self.render_goto_popup(ui);
        self.render_search_popup(ui);
        self.render_context_menu(ui);
        self.render_settings_popup(ui);

        let avail = ui.content_region_avail();
        // Inspector height — auto by default, overridden by user when
        // they drag the splitter. The clamp is gated on `show_inspector`
        // so toggling the panel off/on later doesn't try to fit a
        // user-sized value into a zero-height envelope (`f32::clamp`
        // panics in debug when `min > max`).
        let auto_h = self.inspector_height();
        let splitter_h = if self.config.show_inspector && self.config.show_splitter {
            SPLITTER_THICKNESS
        } else {
            0.0
        };
        let (inspector_h, min_inspector_h, max_inspector_h) = if self.config.show_inspector {
            let min = auto_h.max(self.line_height * 2.0);
            let max = (avail[1] - splitter_h - self.line_height * 5.0).max(min);
            let h = if self.inspector_h > 0.0 {
                self.inspector_h.clamp(min, max)
            } else {
                auto_h
            };
            // Persist the clamp result so the next frame starts from
            // the already-bounded value (avoids creep when the parent
            // shrinks).
            if self.inspector_h > 0.0 {
                self.inspector_h = h;
            }
            (h, min, max)
        } else {
            (0.0, 0.0, 0.0)
        };
        let child_h = avail[1] - inspector_h - splitter_h;

        // `child_id` pre-built in `HexViewer::new` — clone-once
        // here unblocks the closure's `&mut self` borrow while
        // still saving the formatter overhead that the previous
        // `format!("##hv_child_{}", self.id)` per-frame call paid.
        // The clone is a small byte-copy; `format!` was alloc +
        // Display trait dispatch + write_str dance.
        let child_id = self.child_id.clone();
        ui.child_window(&child_id)
            .size([avail[0], child_h])
            .build(ui, || {
                self.focused = ui.is_window_focused();

                // Cache inner content width — used by `ascii_col_x` to
                // right-anchor the ASCII column. `content_region_avail`
                // here returns the post-scrollbar inner width because
                // the cursor sits at the top-left of the body before
                // any widgets have advanced it.
                self.inner_content_w = ui.content_region_avail()[0];

                // Cache the screen-space centre of the child window —
                // used by the modal popups (Goto / Search / Settings)
                // to anchor at the viewer's visual middle regardless
                // of where the trigger came from. `window_pos` returns
                // the absolute screen position of *this* child, and
                // `window_size` is its outer rect, so `pos + size*0.5`
                // is the centre point we want the popup to land at
                // with a `(0.5, 0.5)` pivot.
                let wp = ui.window_pos();
                let ws = ui.window_size();
                self.component_center = [wp[0] + ws[0] * 0.5, wp[1] + ws[1] * 0.5];

                // Scroll-to target.
                if let Some(row) = self.scroll_to_row.take() {
                    let y = row as f32 * self.line_height;
                    ui.set_scroll_y(y);
                }

                if self.focused {
                    self.handle_keyboard(ui, provider);
                }
                self.handle_mouse(ui, avail[0], provider);

                let draw_list = ui.get_window_draw_list();
                // BUG-126 (2026-05-15, session 88): use `window_pos()` not
                // `cursor_screen_pos()` for the Y reference.
                //
                // Diagnostic confirmed: in a **nested** child_window chain
                // (host's outer child → our inner hex child), the value
                // returned by `cursor_screen_pos()` already includes the
                // scroll offset of *some* ancestor's layout pass, so the
                // formula `y = win_y + offset - scroll_y` effectively
                // subtracts scroll TWICE — rows render outside the
                // effective clip rect and the entire pane appears blank.
                //
                // `window_pos()` returns the FIXED screen-space origin of
                // the current child window (top-left corner of its frame),
                // not the cursor — so the formula reads as the textbook
                // "screen_top + row_offset_in_content - scroll = row_y".
                // Same result in top-level windows (`cursor_screen_pos`
                // and `window_pos` start equal there), but corrects the
                // double-subtraction bug in nested child layouts.
                let [win_x, _win_y_cursor] = ui.cursor_screen_pos();
                let [_wp_x, win_y] = ui.window_pos();
                let scroll_y = ui.scroll_y();
                let visible_h = ui.window_size()[1];

                // Correct virtualization: use scroll_y from ImGui scrollbar.
                let first_row = (scroll_y / self.line_height) as usize;
                let visible_count = (visible_h / self.line_height) as usize + 2;
                let last_row = (first_row + visible_count).min(total_rows);

                // BUG-128 (2026-05-15): publish the visible row range
                // for the VA-native API getters (`viewport_first_va` /
                // `viewport_last_va`). Same source of truth as the loop
                // below — hosts can never see a stale or different
                // viewport than what was actually drawn this frame.
                self.viewport_first_row = first_row;
                self.viewport_last_row = last_row;

                let header_offset = if self.config.show_column_headers {
                    1
                } else {
                    0
                };

                // Origin nudged right by one glyph so the leftmost
                // column doesn't sit flush against the child-window
                // border. Mirrors the right-side scrollbar gap and is
                // applied uniformly to header, rows, and hit-testing
                // (see `mouse_to_offset`).
                let origin_x = win_x + self.char_advance;

                // Column header — draw at fixed position relative to window.
                if self.config.show_column_headers && first_row == 0 {
                    let hdr_y = win_y;
                    self.draw_column_header(&draw_list, origin_x, hdr_y);
                }

                // Vertical dividers between offset / hex / ASCII columns.
                // Drawn first so byte text paints on top of them. Same
                // colour treatment as the horizontal header separator
                // for visual consistency.
                if self.config.show_column_dividers {
                    let c = self.config.color_header;
                    let div_col = col32([c[0], c[1], c[2], c[3] * 0.40]);
                    let div_top = win_y;
                    let div_bot = win_y + visible_h;
                    let hex_x_local = origin_x + self.offset_col_width();
                    // Offset side: visible address ends at
                    // `hex_x_local - ca`, hex content starts at
                    // `hex_x_local` — the visible gap is exactly 1 ca
                    // wide (the trailing space of `"{addr} "` lives
                    // inside `offset_col_width`). Centre the divider in
                    // it at `hex_x_local - 0.5 ca`.
                    if self.config.show_offsets {
                        let dx = hex_x_local - self.char_advance * 0.5;
                        draw_list
                            .add_line([dx, div_top], [dx, div_bot], div_col)
                            .thickness(1.0)
                            .build();
                    }
                    // ASCII side: with right-anchored ASCII the gap
                    // between hex and ASCII is no longer fixed (it
                    // grows with window width). Pinning the divider to
                    // hex's right edge (with a small breathing offset)
                    // keeps it acting as a "this column has ended"
                    // marker rather than floating in dead space.
                    if self.config.show_ascii {
                        let dx = hex_x_local + self.hex_col_width() + self.char_advance * 0.5;
                        draw_list
                            .add_line([dx, div_top], [dx, div_bot], div_col)
                            .thickness(1.0)
                            .build();
                    }
                }

                // Dashed vertical guides between byte groups inside the
                // hex column. Off by default — flip `show_group_dividers`
                // for dense layouts (32 bpr / group=4) where the eye
                // loses track of group boundaries. Lower alpha than the
                // major column dividers so they read as a calm hint,
                // not a hard divider.
                if self.config.show_group_dividers {
                    let bpr_value = self.config.bytes_per_row.value();
                    let group_value = self.config.grouping.value();
                    if group_value > 0 && group_value < bpr_value {
                        let c = self.config.color_header;
                        let dash_col = col32([c[0], c[1], c[2], c[3] * 0.28]);
                        let div_top = win_y;
                        let div_bot = win_y + visible_h;
                        let hex_x_local = origin_x + self.offset_col_width();
                        // X of byte column N inside the hex region:
                        //   x_at[N] = hex_x_local + N*3*ca + groups_so_far*ca
                        // We want the divider in the middle of the
                        // extra-space gap that follows byte (group-1),
                        // (2*group-1), ... so place it half-way through
                        // the trailing space that closes each group —
                        // that's `x_at[N] - 0.5*ca` for N = group, 2*group, ...
                        let ca = self.char_advance;
                        let groups = bpr_value.div_ceil(group_value);
                        for g in 1..groups {
                            let n = g * group_value; // byte index at group start
                            if n >= bpr_value {
                                break;
                            }
                            // Position halfway into the inter-group gap.
                            // x_at[n] = hex_x_local + 3*ca*n + ca*g  (g already-passed groups added one ca each)
                            let x_at_n = hex_x_local + ca * (3.0 * n as f32 + g as f32);
                            let dx = (x_at_n - 0.5 * ca).round();
                            draw_dashed_v_line(
                                &draw_list, dx, div_top, div_bot, dash_col, DASH_ON, DASH_OFF,
                            );
                        }
                    }
                }

                // Rows: position each row at its absolute scroll position.
                // Hover-tooltip is gated on `is_window_hovered` so that
                // moving the cursor down into the inspector subview
                // (or any sibling widget below) immediately suppresses
                // the byte tooltip — without this, the last visible
                // row's hit-rect can still match `mouse_pos.y` and the
                // tooltip ghosts through the inspector.
                let mouse_pos = if ui.is_window_hovered() {
                    ui.io().mouse_pos()
                } else {
                    [f32::NEG_INFINITY, f32::NEG_INFINITY]
                };
                // Stack-allocated per-row scratch buffer. One `provider.read()`
                // call per visible row gives streaming providers a single
                // chance to satisfy the row from their cache / fetch batch.
                // `bpr` ≤ MAX_BYTES_PER_ROW by `BytesPerRow` construction.
                let mut row_buf = [0u8; MAX_BYTES_PER_ROW];
                for row in first_row..last_row {
                    // Absolute Y position within the scrollable area.
                    let y = win_y + (row + header_offset) as f32 * self.line_height - scroll_y;
                    let offset = row * bpr;
                    let row_end = (offset + bpr).min(data_len_usize);
                    let row_len = row_end - offset;
                    let n_read = provider.read(offset as u64, &mut row_buf[..row_len]);
                    // If the provider returned fewer bytes than requested
                    // (live-memory hole, MMIO gap), zero-fill the tail so
                    // the row drawer renders a uniform '00' / '.' run —
                    // matches the legacy in-memory contract where reading
                    // past the buffer returned nothing visible at all but
                    // still drew the partial row.
                    if n_read < row_len {
                        row_buf[n_read..row_len].fill(0);
                    }

                    self.draw_row(
                        ui,
                        &draw_list,
                        origin_x,
                        y,
                        offset,
                        &row_buf[..row_len],
                        bpr,
                        mouse_pos,
                        [win_x, win_y],
                        avail[0],
                        data_len_usize,
                    );
                }

                // Total content height for scrollbar. For unbounded /
                // streaming providers `total_rows` can be huge (clamped
                // by `PROVIDER_LEN_CAP`).
                //
                // Phase 7.18m (2026-05-20): cap raised 1e7 → 5e8 pixels.
                // Reason: 1e7 ≈ 9.4 MB scroll scope was too small for a
                // 256 MB streaming window (user got "stuck in 256 KB"
                // because scrollbar maxed out at 9.4 MB internal scroll
                // position). 5e8 pixels ≈ ~250 MB scope at line_height
                // 17, bytes_per_row 16. f32 precision: ULP at 5e8 is
                // ~30 pixels (~2 rows = 32 bytes) — wheel events
                // (~51 px/tick) still register precisely; scrollbar
                // drag granularity = visible-row width ≈ adequate.
                //
                // For full address-space navigation (terabyte VA scope)
                // hosts should expose Ctrl+G goto / Modules widget click
                // — the scrollbar handles "huge but bounded" scopes, not
                // the entire 64-bit address space.
                let total_h_raw = (total_rows + header_offset) as f32 * self.line_height;
                let total_h = total_h_raw.min(5.0e8);
                ui.dummy([avail[0], total_h]);
            });

        if self.config.show_inspector {
            if self.config.show_splitter {
                self.render_splitter(ui, avail[0], inspector_h, min_inspector_h, max_inspector_h);
            }
            self.render_inspector(ui, provider, data_len_usize);
        }
    }
}
