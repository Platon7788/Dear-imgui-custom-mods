//! Render pipeline: row drawing, byte color overrides, data inspector,
//! goto/search popups.
//!
//! `render` is the only entry point. It caches font metrics, computes
//! the visible row window from `scroll_y`, and dispatches to the
//! per-row, per-byte drawing helpers. All of these are split off as
//! `&self` / `&mut self` methods on `HexViewer` (defined here) so the
//! state lookup paths stay direct.

use super::HexViewer;
use super::config::Endianness;
use super::input::EditColumn;
use super::provider::{ByteCategory, HexDataProvider};
use crate::utils::color::rgba_f32;
use crate::utils::hex::byte_hex;
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

    /// Draggable horizontal splitter between the hex child-window and
    /// the inspector subview. Draws a 1 px line centred in a
    /// `SPLITTER_THICKNESS`-tall hit zone; the cursor turns into
    /// `ResizeNS` on hover, and dragging adjusts `self.inspector_h`
    /// (clamped against `min_h` / `max_h`).
    fn render_splitter(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        width: f32,
        current_h: f32,
        min_h: f32,
        max_h: f32,
    ) {
        let [start_x, start_y] = ui.cursor_screen_pos();
        // `splitter_id` pre-built in `HexViewer::new` — same
        // motivation as `child_id`: zero per-frame allocation.
        ui.invisible_button(&self.splitter_id, [width, SPLITTER_THICKNESS]);
        let hovered = ui.is_item_hovered();
        let active = ui.is_item_active();
        if hovered || active {
            ui.set_mouse_cursor(Some(dear_imgui_rs::MouseCursor::ResizeNS));
        }
        if active {
            // Drag down → shrink the inspector (push hex/splitter down).
            // The clamp uses `current_h - dy` as the baseline rather than
            // `self.inspector_h` so the very first active frame works
            // even when no manual height was stored yet (`inspector_h`
            // is still `0.0` = auto). Mere hover does NOT prime the
            // value — only an actual drag.
            let dy = ui.io().mouse_delta()[1];
            if dy != 0.0 {
                let next = (current_h - dy).clamp(min_h, max_h);
                if next != self.inspector_h {
                    self.inspector_h = next;
                }
            } else if self.inspector_h == 0.0 {
                // Drag started without movement yet — pin the baseline
                // so a sub-pixel jiggle doesn't snap back to auto.
                self.inspector_h = current_h;
            }
        }

        // Visual: 1 px line centred in the hit zone, brighter when the
        // user is actively grabbing it for clear feedback.
        let line_y = start_y + (SPLITTER_THICKNESS * 0.5).floor();
        let alpha = if active {
            0.85
        } else if hovered {
            0.55
        } else {
            0.30
        };
        let c = self.config.color_header;
        let line_col = col32([c[0], c[1], c[2], c[3] * alpha]);
        let draw_list = ui.get_window_draw_list();
        draw_list
            .add_line([start_x, line_y], [start_x + width, line_y], line_col)
            .thickness(1.0)
            .build();
    }
}

// ── HexViewer impl: layout helpers ───────────────────────────────────────────

impl HexViewer {
    pub(super) fn offset_col_width(&self) -> f32 {
        if self.config.show_offsets {
            // address digits + 1 trailing space (no colon — the
            // column divider already separates this gutter from the
            // hex content; the `:` was redundant signal). The 1-ca
            // trailing space stays so the address text doesn't graze
            // the divider line (centred at column-edge − 0.5 ca).
            //
            // Width history:
            //   pre 2026-04-29 → `digits + 3` (over-padded gutter)
            //   2026-04-29     → `digits + 2` (tightened, kept `:`)
            //   2026-04-30     → `digits + 1` (dropped `:`)
            // Use the cached `effective_data_len` rather than
            // `self.data.len()`. Phase 2: provider-driven renders set
            // this to the provider's clamped `len()` so a streaming
            // host gets a 16-digit gutter when its window spans a
            // 64-bit address range, even though `self.data` may be
            // small or empty. Legacy `set_data*` callers also write
            // the field, so non-provider hosts behave exactly as
            // before.
            let digits = self
                .config
                .address_width
                .hex_digits(self.config.base_address, self.effective_data_len);
            self.char_advance * (digits + 1) as f32
        } else {
            0.0
        }
    }

    pub(super) fn hex_col_width(&self) -> f32 {
        let bpr = self.config.bytes_per_row.value();
        let group = self.config.grouping.value();
        let groups = if group > 0 { bpr.div_ceil(group) } else { 1 };
        let extra_spaces = groups.saturating_sub(1);
        (bpr * 3 + extra_spaces) as f32 * self.char_advance
    }

    /// X-coordinate of the ASCII column's left edge, in screen space.
    ///
    /// Right-anchors the ASCII content to the child window's inner
    /// right edge (with a 1-char trailing margin so it doesn't graze
    /// the scrollbar) when there's room — keeps the ASCII column on
    /// the page boundary the way HxD / 010 Editor / Ghidra render
    /// hex dumps. When the window is too narrow, falls back to the
    /// "natural" position one char advance after the hex column so
    /// the layout never overlaps.
    pub(super) fn ascii_col_x(&self, win_x: f32) -> f32 {
        let bpr = self.config.bytes_per_row.value();
        let ascii_w = bpr as f32 * self.char_advance;
        let origin_x = win_x + self.char_advance;

        // "Natural" position — one char advance after the hex column.
        let natural = origin_x + self.offset_col_width() + self.hex_col_width() + self.char_advance;

        // "Anchored" position — right-aligned with one char of
        // breathing room so it doesn't sit under the scrollbar.
        let anchored = win_x + self.inner_content_w - ascii_w - self.char_advance;

        anchored.max(natural)
    }

    fn draw_column_header(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
    ) {
        let bpr = self.config.bytes_per_row.value();
        let group = self.config.grouping.value();
        let hdr_col = col32(self.config.color_header);

        // Helper: paint `label` centred inside `[col_x, col_x + col_w)`.
        // `label.len()` works as a proxy for visual width here because
        // the hex viewer's font is monospaced (`char_advance == width
        // of any glyph`). Falls back to left-aligned when the label is
        // wider than the column (no negative padding).
        let centred_label = |label: &str, col_x: f32, col_w: f32| {
            let label_w = label.len() as f32 * self.char_advance;
            let pad = ((col_w - label_w) * 0.5).max(0.0);
            draw_list.add_text([col_x + pad, y], hdr_col, label);
        };

        if self.config.show_offsets {
            // Header reads as "Address" (not "Offset") — even when the
            // viewer is bound to a `base_address == 0` file dump, the
            // gutter content is still a *virtual address* in the user's
            // mental model (the offset of the byte from the start of
            // the buffer is also its address). Centred over the gutter
            // — left-aligned looked uneven against the centred ASCII
            // label and the right-edge divider.
            centred_label("Address", origin_x, self.offset_col_width());
        }

        let hex_x = origin_x + self.offset_col_width();
        let mut x = hex_x;
        for i in 0..bpr {
            // Column index 0..63 always fits in u8 — safe cast.
            draw_list.add_text([x, y], hdr_col, byte_hex(i as u8, self.config.uppercase));
            x += self.char_advance * 3.0;
            if group > 0 && (i + 1) % group == 0 && i + 1 < bpr {
                x += self.char_advance;
            }
        }

        // Right-edge of the painted content — used to size the
        // separator line so it tracks header / hex / ASCII columns
        // exactly instead of running off into empty padding. Uses the
        // hex column's right edge as the baseline; gets bumped to the
        // ASCII column's right edge below if the ASCII column is
        // visible.
        let win_x = origin_x - self.char_advance;
        let mut right_edge = hex_x + self.hex_col_width();

        if self.config.show_ascii {
            let ascii_x = self.ascii_col_x(win_x);
            let ascii_w = bpr as f32 * self.char_advance;
            centred_label("ASCII", ascii_x, ascii_w);
            right_edge = ascii_x + ascii_w;
        }

        // ── Separator line under the header row ─────────────────────────
        // Visual cue that the top row is column labels, not data. Uses
        // the header color with a 0.55 alpha multiplier so it reads as
        // a gentle divider rather than competing with the labels above.
        let c = self.config.color_header;
        let sep_col = col32([c[0], c[1], c[2], c[3] * 0.55]);
        let sep_y = y + self.line_height - 1.0;
        draw_list
            .add_line([origin_x, sep_y], [right_edge, sep_y], sep_col)
            .thickness(1.0)
            .build();
    }

    /// Check if offset is within a search match.
    ///
    /// Hot — runs once per visible byte per frame. `search_results` is
    /// sorted by construction (`find_pattern_masked` walks the buffer
    /// left-to-right), so we binary-search for the candidate start
    /// instead of scanning all matches. Brings overall hex-render cost
    /// from O(visible_bytes × matches) down to O(visible_bytes × log
    /// matches), which matters when a permissive wildcard pattern
    /// produces thousands of hits.
    pub(super) fn is_search_match(&self, offset: usize) -> bool {
        if self.search_results.is_empty() || self.search_pattern.is_empty() {
            return false;
        }
        let plen = self.search_pattern.len();
        // We want the smallest match-start `s` with `s >= offset - (plen - 1)`,
        // then verify `s <= offset`. A match covers `[s, s + plen)`.
        let lower = offset.saturating_sub(plen.saturating_sub(1));
        let pos = self.search_results.partition_point(|&s| s < lower);
        self.search_results.get(pos).is_some_and(|&s| s <= offset)
    }

    /// Get foreground color with diff/region overrides.
    ///
    /// `byte` is the live byte value at `offset` (read by the caller
    /// from the active provider / row buffer). Using the caller-supplied
    /// `byte` for the reference comparison — rather than re-indexing
    /// `self.data[offset]` — keeps the diff highlight aligned with what
    /// the row drawer actually painted. For provider-driven hosts the
    /// internal `self.data` may not match the live byte at all (streaming
    /// memory pane), so the parameter is the only correct source.
    pub(super) fn byte_fg_with_overrides(&self, offset: usize, byte: u8) -> u32 {
        let cfg = &self.config;

        // Changed byte (diff).
        if cfg.highlight_changes
            && !self.reference.is_empty()
            && offset < self.reference.len()
            && byte != self.reference[offset]
        {
            return col32(cfg.color_changed);
        }

        // Color region.
        for region in &self.regions {
            if offset >= region.offset && offset < region.offset + region.len {
                return col32(region.color);
            }
        }

        // Category / default — uses pre-computed palette (M2).
        // Equivalent to `col32(cfg.byte_fg_color(byte))` but the lookup
        // tables are built once per frame in `render()`.
        self.byte_palette[byte as usize]
    }
}

// ── HexViewer impl: row drawing ──────────────────────────────────────────────

impl HexViewer {
    /// Render a single row of bytes.
    ///
    /// `row_bytes` is the already-read slice of bytes for this row,
    /// length ≤ `bpr` (the partial last row has `< bpr` bytes). The
    /// caller (`render_impl`) reads these once per row via the active
    /// provider — `draw_row` itself never touches `self.data` for the
    /// byte values, only for downstream queries like
    /// `byte_fg_with_overrides` (which now also takes the live byte).
    ///
    /// `data_len` is the provider's `usize`-projected length, used for
    /// the `AddressWidth` digit selection so 8 vs 16 digit gutters
    /// match the underlying buffer (legacy behaviour: digits depend on
    /// the buffer's last address).
    #[allow(clippy::too_many_arguments)]
    fn draw_row(
        &self,
        ui: &dear_imgui_rs::Ui,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
        offset: usize,
        row_bytes: &[u8],
        bpr: usize,
        mouse_pos: [f32; 2],
        _win_pos: [f32; 2],
        _win_w: f32,
        data_len: usize,
    ) {
        let cfg = &self.config;
        let group = cfg.grouping.value();
        let row_end = offset + row_bytes.len();

        // ── Offset column ─────────────────────────────────────
        if cfg.show_offsets {
            let row = offset / bpr;
            let addr = cfg.base_address + offset as u64;
            let digits = cfg.address_width.hex_digits(cfg.base_address, data_len);

            // "Just copied" flash — paint a translucent accent-coloured
            // pill behind the address text whenever the user has just
            // clicked the gutter. Tracks the row index, not the byte
            // offset, so the pill stays put even if the cursor wanders
            // off into the data area mid-flash.
            if let Some((flash_row, frames)) = self.address_flash
                && flash_row == row
                && frames > 0
            {
                // Fade out as the counter ticks down — clearer "this
                // happened just now, but it's about to disappear".
                let fade =
                    (frames as f32 / super::input::ADDRESS_FLASH_FRAMES as f32).clamp(0.0, 1.0);
                let c = cfg.color_cursor_bg;
                // Span = 0.5 ca left padding + the visible address
                // glyphs (`{:0NX}`) + 0.5 ca right padding — leaves
                // the trailing format-space unhighlighted so the
                // pill doesn't visually fuse into the column
                // divider. Was `digits + 1` while the format string
                // appended `:`; dropped to `digits` on 2026-04-30
                // when the colon was removed.
                let pad_x = self.char_advance * 0.5;
                let bg_left = origin_x - pad_x;
                let bg_right = origin_x + self.char_advance * digits as f32 + pad_x;
                draw_list
                    .add_rect(
                        [bg_left, y],
                        [bg_right, y + self.line_height],
                        col32([c[0], c[1], c[2], c[3] * 0.65 * fade]),
                    )
                    .filled(true)
                    .rounding(2.0)
                    .build();
            }

            // 16-digit path covers x86_64 / kernel-space dumps; 8-digit
            // is the compact default for files / 32-bit memory.
            // No trailing `:` — the column divider already separates
            // the address from the hex content (user request,
            // 2026-04-30). The single trailing space keeps a 1-ca
            // gap before the divider so the address text never
            // grazes the line.
            //
            // Host can override the displayed address via the
            // `address_formatter` closure (see `set_address_formatter`)
            // — typically used to show "module+offset" strings for
            // mapped memory. The widget itself stays domain-agnostic.
            // When the host returns `Some(s)` we pay one allocation
            // per row for the host's string; the default `None` path
            // routes through the thread-local zero-alloc buffer below.
            let host_str = self.address_formatter.as_ref().and_then(|f| f(addr));
            if let Some(s) = host_str {
                draw_list.add_text([origin_x, y], col32(cfg.color_offset), &s);
            } else {
                // Per-row formatting reuses a thread-local scratch
                // `String` so the address gutter pays zero allocations
                // per frame (the previous `format!("{:016X} ", addr)`
                // built a fresh `String` ~50 rows × 60 fps = ~3000
                // alloc/sec). `draw_row` runs on `&self`, so the
                // mutable scratch buffer can't live on the struct —
                // thread-local is the right scope: each render thread
                // gets its own reused buffer.
                use std::cell::RefCell;
                use std::fmt::Write as _;
                thread_local! {
                    static ADDR_BUF: RefCell<String> = RefCell::new(String::with_capacity(18));
                }
                ADDR_BUF.with(|cell| {
                    let mut buf = cell.borrow_mut();
                    buf.clear();
                    let _ = match (cfg.uppercase, digits) {
                        (true, 16) => write!(&mut *buf, "{:016X} ", addr),
                        (false, 16) => write!(&mut *buf, "{:016x} ", addr),
                        (true, _) => write!(&mut *buf, "{:08X} ", addr),
                        (false, _) => write!(&mut *buf, "{:08x} ", addr),
                    };
                    draw_list.add_text([origin_x, y], col32(cfg.color_offset), buf.as_str());
                });
            }
        }

        let hex_x = origin_x + self.offset_col_width();

        // ── Hex bytes ─────────────────────────────────────────
        //
        // ADR-111 (Phase 7.5): render the row's hex glyphs via
        // **per-colour-run** `add_text` calls instead of one
        // `add_text` per byte. Typical visible row has 2-4 distinct
        // colour categories → 2-4 `add_text` calls instead of 20.
        // Cuts ~80–90 % of the per-row draw-list ops the previous
        // implementation paid. Backgrounds (cursor / selection /
        // search) and hover overlays remain per-byte because they're
        // bounded (≤ 1 cursor, sparse selection / search hits) and
        // need pixel-exact bounds — concatenating them gains nothing.
        //
        // Three passes per row:
        //   1. Per-byte backgrounds + editing-byte identification.
        //   2. Per-colour-run hex text spans (the optimisation).
        //   3. Per-byte hover hit-test + tooltip.
        //
        // The editing byte (at most one per row) is drawn as a
        // dedicated `add_text` mid-pass-2: nibble glyphs / blink
        // underline live in their own per-byte branch unchanged.

        // Pre-compute x position of each byte column. Mirrors the
        // grouping-aware advance in the old single-pass loop so
        // visual layout is pixel-identical.
        let mut x_at = [0.0f32; 64]; // bpr ≤ 64 per `BytesPerRow`.
        {
            let mut xv = hex_x;
            for (col_idx, slot) in x_at.iter_mut().take(bpr).enumerate() {
                *slot = xv;
                xv += self.char_advance * 3.0;
                if group > 0 && (col_idx + 1).is_multiple_of(group) && col_idx + 1 < bpr {
                    xv += self.char_advance;
                }
            }
        }

        // PASS 1 — per-byte backgrounds + locate editing byte (if any).
        let mut editing_col: Option<usize> = None;
        for (col_idx, &xb) in x_at.iter().take(bpr).enumerate() {
            let i = offset + col_idx;
            if i >= row_end {
                break;
            }
            let is_cursor = i == self.cursor;
            let is_selected = self.selection.contains(i);
            let is_editing = is_cursor && self.edit_column == Some(EditColumn::Hex);
            if is_editing {
                editing_col = Some(col_idx);
            }

            // Background: cursor > selection > search.
            let bg = if is_editing {
                Some(col32([0.50, 0.30, 0.10, 0.85]))
            } else if is_cursor {
                Some(col32(cfg.color_cursor_bg))
            } else if is_selected {
                Some(col32(cfg.color_selection_bg))
            } else if self.is_search_match(i) {
                Some(col32(cfg.color_search_match))
            } else {
                None
            };
            if let Some(bg_col) = bg {
                draw_list
                    .add_rect(
                        [xb - 1.0, y],
                        [xb + self.char_advance * 2.0 + 1.0, y + self.line_height],
                        bg_col,
                    )
                    .filled(true)
                    .build();
            }
        }

        // PASS 2 — per-byte hex text rendering.
        //
        // Each byte is emitted as its own `add_text` call at the
        // exact x-coordinate pre-computed in `x_at`. This is the
        // hex-dump invariant the user expects: bytes anchored to a
        // pixel-perfect grid, independent of font metrics or font
        // changes.
        //
        // History (regression fix): an earlier per-colour-run
        // optimisation (ADR-111 Phase 7.5) concatenated runs of
        // same-colour bytes into one string with embedded spaces and
        // emitted the whole run with a single `add_text`. That
        // accumulates `glyph.AdvanceX` across the string in float
        // math — for monospace fonts the per-glyph advance is
        // uniform on paper, but cumulative float addition over
        // ~30+ chars per row produces sub-pixel drift that imgui's
        // glyph-quad rounding then turns into visible 1-px column
        // jitter. The drift is invisible on short rows (~8 bytes)
        // and screams once `bpr` reaches 16-32. Reverting to per-
        // byte rendering is pixel-identical to the column-header
        // path above (`draw_column_header`) which has always been
        // per-byte, so header labels and data columns stay in lock-
        // step regardless of font metrics.
        //
        // Performance: ~16-32 `add_text` calls per row instead of
        // ~2-4. Profiling on a 32-bpr 64-row visible window
        // (~1000 ops/frame) is dominated by GPU vertex generation,
        // not the dispatch — the optimisation traded ~0.05 ms of
        // CPU for a visible glitch.
        let last_col = row_bytes.len(); // exclusive; ≤ bpr
        for col_idx in 0..last_col {
            let i = offset + col_idx;
            let byte = row_bytes[col_idx];
            let xb = x_at[col_idx];

            if Some(col_idx) == editing_col {
                if let Some(hi_nibble) = self.edit_nibble {
                    let mut buf = [b'_'; 2];
                    buf[0] = if cfg.uppercase {
                        b"0123456789ABCDEF"[hi_nibble as usize & 0xF]
                    } else {
                        b"0123456789abcdef"[hi_nibble as usize & 0xF]
                    };
                    // SAFETY: `buf` is two ASCII bytes from the
                    // hex alphabet (or `_`), valid UTF-8 by
                    // construction.
                    let txt = unsafe { std::str::from_utf8_unchecked(&buf) };
                    draw_list.add_text([xb, y], col32([1.0, 1.0, 0.5, 1.0]), txt);
                } else {
                    let txt = byte_hex(byte, cfg.uppercase);
                    draw_list.add_text([xb, y], col32([1.0, 1.0, 0.5, 1.0]), txt);
                    draw_list
                        .add_line(
                            [xb, y + self.line_height - 1.0],
                            [xb + self.char_advance * 2.0, y + self.line_height - 1.0],
                            col32([1.0, 0.8, 0.3, 1.0]),
                        )
                        .thickness(1.5)
                        .build();
                }
                continue;
            }

            let fg = self.byte_fg_with_overrides(i, byte);
            draw_list.add_text([xb, y], fg, byte_hex(byte, cfg.uppercase));
        }

        // PASS 3 — per-byte hover hit-test + tooltip. Unchanged
        // semantics from the pre-7.5 path; kept per-byte because
        // tooltip is rare (≤ 1 hit per frame).
        for col_idx in 0..bpr {
            let i = offset + col_idx;
            if i >= row_end {
                break;
            }
            let xb = x_at[col_idx];
            let is_editing = Some(col_idx) == editing_col;
            let byte_hovered = mouse_pos[0] >= xb
                && mouse_pos[0] < xb + self.char_advance * 2.5
                && mouse_pos[1] >= y
                && mouse_pos[1] < y + self.line_height;
            if byte_hovered && !is_editing {
                let byte = row_bytes[col_idx];
                draw_list
                    .add_rect(
                        [xb - 1.0, y],
                        [xb + self.char_advance * 2.0 + 1.0, y + self.line_height],
                        col32([0.4, 0.63, 0.88, 0.18]),
                    )
                    .filled(true)
                    .build();
                crate::utils::themed_tooltip(ui, || {
                    let addr = cfg.base_address + i as u64;
                    let digits = cfg.address_width.hex_digits(cfg.base_address, data_len);
                    let addr_str = if digits == 16 {
                        format!("Address: 0x{:016X} ({})", addr, i)
                    } else {
                        format!("Address: 0x{:08X} ({})", addr, i)
                    };
                    ui.text(addr_str);
                    ui.text(format!(
                        "Hex: 0x{:02X}  Dec: {}  Oct: 0o{:03o}",
                        byte, byte, byte
                    ));
                    ui.text(format!("Bin: {:08b}", byte));
                    ui.text(format!("Category: {:?}", ByteCategory::of(byte)));
                    if byte.is_ascii_graphic() || byte == b' ' {
                        ui.text(format!("Char: '{}'", byte as char));
                    }
                });
            }
        }

        // ── ASCII column ──────────────────────────────────────
        //
        // ADR-111 (Phase 7.5): same per-colour-run treatment as the
        // hex column. ASCII has only two normal categories
        // (printable vs. dot for non-printable), so most rows fold
        // into 1–2 `add_text` calls.
        if cfg.show_ascii {
            let win_x = origin_x - self.char_advance;
            let ascii_x = self.ascii_col_x(win_x);
            let printable_col = col32(cfg.color_ascii);
            let dot_col = col32(cfg.color_ascii_dot);
            let editing_col_rgb = col32([1.0, 1.0, 0.5, 1.0]);

            // PASS 1 — per-byte backgrounds + identify editing byte.
            let mut ascii_editing_col: Option<usize> = None;
            for col_idx in 0..row_bytes.len() {
                let i = offset + col_idx;
                let is_cursor = i == self.cursor;
                let is_selected = self.selection.contains(i);
                let is_ascii_editing = is_cursor && self.edit_column == Some(EditColumn::Ascii);
                let ax = ascii_x + col_idx as f32 * self.char_advance;

                if is_ascii_editing {
                    ascii_editing_col = Some(col_idx);
                    draw_list
                        .add_rect(
                            [ax, y],
                            [ax + self.char_advance, y + self.line_height],
                            col32([0.50, 0.30, 0.10, 0.85]),
                        )
                        .filled(true)
                        .build();
                } else if is_cursor {
                    draw_list
                        .add_rect(
                            [ax, y],
                            [ax + self.char_advance, y + self.line_height],
                            col32(cfg.color_cursor_bg),
                        )
                        .filled(true)
                        .build();
                } else if is_selected {
                    draw_list
                        .add_rect(
                            [ax, y],
                            [ax + self.char_advance, y + self.line_height],
                            col32(cfg.color_selection_bg),
                        )
                        .filled(true)
                        .build();
                }
            }

            // PASS 2 — per-colour-run text spans.
            use std::cell::RefCell;
            thread_local! {
                static ASCII_SPAN_BUF: RefCell<String> =
                    RefCell::new(String::with_capacity(96));
            }
            ASCII_SPAN_BUF.with(|cell| {
                let mut span_buf = cell.borrow_mut();
                span_buf.clear();
                let mut span_x = ascii_x;
                let mut span_color: u32 = 0;
                let mut span_open = false;
                for (col_idx, &byte) in row_bytes.iter().enumerate() {
                    let ax = ascii_x + col_idx as f32 * self.char_advance;
                    // Editing byte — flush, then draw + underline
                    // individually so the blink underline lines up.
                    if Some(col_idx) == ascii_editing_col {
                        if span_open && !span_buf.is_empty() {
                            draw_list.add_text([span_x, y], span_color, span_buf.as_str());
                        }
                        span_open = false;
                        span_buf.clear();

                        let ch = if (0x20..0x7F).contains(&byte) {
                            byte as char
                        } else {
                            '.'
                        };
                        let mut buf = [0u8; 4];
                        let s = ch.encode_utf8(&mut buf);
                        draw_list.add_text([ax, y], editing_col_rgb, s);
                        draw_list
                            .add_line(
                                [ax, y + self.line_height - 1.0],
                                [ax + self.char_advance, y + self.line_height - 1.0],
                                col32([1.0, 0.8, 0.3, 1.0]),
                            )
                            .thickness(1.5)
                            .build();
                        continue;
                    }

                    let (ch, fg) = if (0x20..0x7F).contains(&byte) {
                        (byte as char, printable_col)
                    } else {
                        ('.', dot_col)
                    };

                    if !span_open || fg != span_color {
                        if span_open && !span_buf.is_empty() {
                            draw_list.add_text([span_x, y], span_color, span_buf.as_str());
                        }
                        span_buf.clear();
                        span_x = ax;
                        span_color = fg;
                        span_open = true;
                    }
                    span_buf.push(ch);
                }
                if span_open && !span_buf.is_empty() {
                    draw_list.add_text([span_x, y], span_color, span_buf.as_str());
                }
            });
        }
    }
}

// ── HexViewer impl: data inspector ───────────────────────────────────────────
//
// Three-column grid laid out by hand on the foreground draw list. Each
// column is "label gap value" with shared `label_off` so the value
// columns line up vertically. Mixing `add_text` with the live
// `cursor_screen_pos` keeps the panel exactly four line-heights tall —
// the most compact layout that still shows every numeric
// reinterpretation simultaneously.

/// Width of the typed-value grid's label cell ("u64 ", "char "), in glyphs.
/// Five fits both 3-letter labels (`u32`, `f64`, `hex`) and `char` with a
/// single trailing space.
const INSPECTOR_LABEL_GLYPHS: f32 = 5.0;

/// Horizontal offset of the second value column (start of `u64`), in glyphs.
/// Sized to clear `u32 4294967295` (label + max u32 = 14 chars) with two
/// glyphs of breathing room.
const INSPECTOR_COL2_GLYPHS: f32 = 16.0;

/// Horizontal offset of the third column (`char` + info row), in glyphs.
/// Sized to clear the longest middle-column value `u64 18446744073709551615`
/// (24 chars) with two glyphs of breathing room.
const INSPECTOR_COL3_GLYPHS: f32 = 42.0;

/// Total inspector height in line-heights — four data rows; the info row
/// piggy-backs on column 3 row 1, so it does not need its own line.
const INSPECTOR_ROWS: f32 = 4.0;

impl HexViewer {
    /// Pixel height required for the inspector subview. `0.0` when the
    /// inspector is hidden via config.
    pub(super) fn inspector_height(&self) -> f32 {
        if self.config.show_inspector {
            // INSPECTOR_ROWS lines + 1 px gap below the separator + ~2 px
            // breathing room so the bottom row never grazes the child border.
            self.line_height * INSPECTOR_ROWS + 3.0
        } else {
            0.0
        }
    }

    fn render_inspector(
        &self,
        ui: &dear_imgui_rs::Ui,
        provider: &mut dyn HexDataProvider,
        data_len: usize,
    ) {
        if self.cursor >= data_len {
            return;
        }

        ui.separator();
        let offset = self.cursor;
        // Pull up to 8 bytes once from the active provider — that's
        // the largest reinterpretation the inspector renders (f64 /
        // u64). For a streaming provider this is the single read
        // that backs every numeric row below; if the provider returns
        // fewer than 8 bytes (live-memory hole, edge of mapped region)
        // the `remaining` value drops the unread rows to `—` exactly
        // like the legacy path did for buffers shorter than 8 bytes
        // at `cursor`.
        let mut inspector_buf = [0u8; 8];
        let want = 8usize.min(data_len.saturating_sub(offset));
        let got = provider.read(offset as u64, &mut inspector_buf[..want]);
        if got == 0 {
            // Provider couldn't satisfy the cursor read (live-memory
            // hole, unmapped page). Skip the inspector entirely
            // instead of rendering an all-'—' row — matches the
            // empty-buffer guard above.
            return;
        }
        let remaining = got;
        let bytes = &inspector_buf[..got];
        let le = matches!(self.config.endianness, Endianness::Little);

        let label_col = col32(self.config.color_inspector_label);
        let value_col = col32(self.config.color_inspector_value);

        let draw_list = ui.get_window_draw_list();
        let [x_raw, y_top] = ui.cursor_screen_pos();
        let cw = self.char_advance;
        let lh = self.line_height;

        // Mirror the one-glyph left padding the hex area uses so the
        // inspector's first column lines up visually with the offset
        // column above instead of hugging the window border.
        let x = x_raw + cw;
        // 1 px gap below the separator so the first row doesn't sit
        // flush against the divider line.
        let y0 = y_top + 1.0;

        let label_off = INSPECTOR_LABEL_GLYPHS * cw;
        let col2_x = x + INSPECTOR_COL2_GLYPHS * cw;
        let col3_x = x + INSPECTOR_COL3_GLYPHS * cw;

        // ── Numeric reinterpretations (decoded once, reused) ────────────
        let u16_v = (remaining >= 2).then(|| {
            if le {
                u16::from_le_bytes([bytes[0], bytes[1]])
            } else {
                u16::from_be_bytes([bytes[0], bytes[1]])
            }
        });
        let arr4 = (remaining >= 4).then(|| [bytes[0], bytes[1], bytes[2], bytes[3]]);
        let u32_v = arr4.map(|a| {
            if le {
                u32::from_le_bytes(a)
            } else {
                u32::from_be_bytes(a)
            }
        });
        let f32_v = arr4.map(|a| {
            if le {
                f32::from_le_bytes(a)
            } else {
                f32::from_be_bytes(a)
            }
        });
        let arr8 = (remaining >= 8).then(|| {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes[..8]);
            arr
        });
        let u64_v = arr8.map(|a| {
            if le {
                u64::from_le_bytes(a)
            } else {
                u64::from_be_bytes(a)
            }
        });
        let f64_v = arr8.map(|a| {
            if le {
                f64::from_le_bytes(a)
            } else {
                f64::from_be_bytes(a)
            }
        });
        let char_str = {
            let b = bytes[0];
            if (0x20..0x7F).contains(&b) {
                format!("'{}'", b as char)
            } else {
                format!("\\x{:02X}", b)
            }
        };
        let em_dash = || "\u{2014}".to_string();

        // ── Column 1: u8 / i8 / u16 / u32 ───────────────────────────────
        let col1: [(&str, String); 4] = [
            ("u8", format!("{}", bytes[0])),
            ("i8", format!("{}", bytes[0] as i8)),
            ("u16", u16_v.map_or_else(em_dash, |v| v.to_string())),
            ("u32", u32_v.map_or_else(em_dash, |v| v.to_string())),
        ];
        for (i, (label, value)) in col1.iter().enumerate() {
            let row_y = y0 + i as f32 * lh;
            draw_list.add_text([x, row_y], label_col, *label);
            draw_list.add_text([x + label_off, row_y], value_col, value);
        }

        // ── Column 2: u64 / f32 / f64 / hex ─────────────────────────────
        let col2: [(&str, String); 4] = [
            ("u64", u64_v.map_or_else(em_dash, |v| v.to_string())),
            ("f32", f32_v.map_or_else(em_dash, |v| format!("{:.6e}", v))),
            ("f64", f64_v.map_or_else(em_dash, |v| format!("{:.6e}", v))),
            ("hex", format!("0x{:02X}", bytes[0])),
        ];
        for (i, (label, value)) in col2.iter().enumerate() {
            let row_y = y0 + i as f32 * lh;
            draw_list.add_text([col2_x, row_y], label_col, *label);
            draw_list.add_text([col2_x + label_off, row_y], value_col, value);
        }

        // ── Column 3: char (row 0) + info (row 1) ───────────────────────
        // Single typed cell on top, single full-info line right under.
        // Rows 2-3 stay empty by design — the panel is anchored to four
        // lines tall so it lines up with the longest left/middle column.
        draw_list.add_text([col3_x, y0], label_col, "char");
        draw_list.add_text([col3_x + label_off, y0], value_col, &char_str);

        let undo_info = if self.undo.can_undo() || self.undo.can_redo() {
            format!(
                "   Undo {} / Redo {}",
                self.undo.undo_count(),
                self.undo.redo_count()
            )
        } else {
            String::new()
        };
        let edit_info = match self.edit_column {
            Some(EditColumn::Hex) => "   [EDITING HEX]",
            Some(EditColumn::Ascii) => "   [EDITING ASCII]",
            None => "",
        };
        // Address column width tracks the same `AddressWidth` policy as
        // the offset gutter — keeps file-dump panels compact while
        // letting 64-bit memory dumps breathe.
        let digits = self
            .config
            .address_width
            .hex_digits(self.config.base_address, data_len);
        let addr_str = if digits == 16 {
            format!(
                "0x{:016X} ({})",
                self.config.base_address + offset as u64,
                offset
            )
        } else {
            format!(
                "0x{:08X} ({})",
                self.config.base_address + offset as u64,
                offset
            )
        };
        let info = format!(
            "{}   {}-endian   {} bytes{}{}",
            addr_str,
            self.config.endianness.display_name().to_lowercase(),
            data_len,
            undo_info,
            edit_info,
        );
        draw_list.add_text([col3_x, y0 + lh], label_col, &info);

        // Reserve exactly the painted area so the host child-window
        // scrollbar (if any) computes the right thumb size and no
        // dead space appears at the bottom.
        ui.dummy([0.0, INSPECTOR_ROWS * lh + 1.0]);
    }
}
