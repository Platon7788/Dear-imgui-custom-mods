//! Per-row painting for [`super::super::DisasmView`] — block tints,
//! the current-execution / search / origin / selection / cursor / hover
//! highlight stack, the margin gutter (bookmark + breakpoint tag), the
//! address / bytes / mnemonic / operands / comment columns (with inline
//! edit cells), and the comprehensive hover tooltip.

use super::*;
use crate::disasm_view::EditColumn;
use crate::disasm_view::provider::{DisasmDataProvider, Instruction};
use crate::utils::hex::byte_hex;

impl DisasmView {
    #[allow(clippy::too_many_arguments)] // tooltip needs prev/next neighbours for idiom detection
    pub(in crate::disasm_view) fn draw_instruction_row(
        &self,
        ui: &dear_imgui_rs::Ui,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
        idx: usize,
        instr: &dyn Instruction,
        prev_instr: Option<&dyn Instruction>,
        next_instr: Option<&dyn Instruction>,
        mouse_pos: [f32; 2],
        win_w: f32,
        comment_x: f32,
        provider: &dyn DisasmDataProvider,
    ) {
        let cfg = &self.config;
        let colors = &cfg.colors;
        let cols = &cfg.columns;
        let lh = self.line_height;

        // ── Block tint background ─────────────────────────────
        if cfg.show_block_tints {
            let tint = colors.block_tint(instr.block_index());
            if tint[3] > 0.0 {
                draw_list
                    .add_rect([origin_x, y], [origin_x + win_w, y + lh], col32(tint))
                    .filled(true)
                    .build();
            }
        }

        // ── Current execution highlight ───────────────────────
        // Two-pass: translucent fill (warning hue, alpha 0.18) so
        // the per-token mnemonic / operand colours still read
        // through, plus a crisp 1-px outline (danger hue, alpha
        // 0.90) for the unmistakable "stopped here" marker. User
        // feedback 2026-04-30: solid amber fill drowned the row;
        // halve the fill, add the red border.
        if instr.is_current() {
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32(colors.current_line_bg),
                )
                .filled(true)
                .build();
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32(colors.current_line_border),
                )
                .filled(false)
                .thickness(1.0)
                .build();
        }

        // ── Search-match highlight ────────────────────────────
        // Painted before selection so user-selection still wins
        // visually when both apply (selection is the more transient
        // / interactive state). Set lookup is O(log n) on BTreeSet
        // so the per-row cost stays bounded as match counts grow.
        if self.search_match_set.contains(&idx) {
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32(colors.search_match_bg),
                )
                .filled(true)
                .build();
        }

        // ── Origin breadcrumb (two-part: bg fill + left stripe) ──
        // - Faint background fill: ambient "you came from here"
        //   awareness while scrolling; same hue as cursor (visual
        //   grouping) but `ORIGIN_BG_ALPHA_FACTOR=0.30` so it
        //   doesn't compete.
        // - Crisp 3-px left-edge stripe at `ORIGIN_STRIPE_ALPHA=0.90`:
        //   the unmistakable marker that survives stacking with
        //   block tints / search-match / hover backgrounds.
        // Compared by address — survives provider mutations like
        // inserting a new instruction below the breadcrumb. The
        // cursor row's full-alpha highlight overdraws the bg fill;
        // the stripe peeks through at the very left edge so the
        // user can still see "you're back at the breadcrumb".
        if let Some(origin) = self.origin_addr
            && instr.address() == origin
        {
            let bg = colors.selection_bg;
            // 1. Ambient background fill.
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32([
                        bg[0],
                        bg[1],
                        bg[2],
                        bg[3] * super::super::ORIGIN_BG_ALPHA_FACTOR,
                    ]),
                )
                .filled(true)
                .build();
            // 2. Left-edge stripe.
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + super::super::ORIGIN_STRIPE_WIDTH, y + lh],
                    col32([bg[0], bg[1], bg[2], super::super::ORIGIN_STRIPE_ALPHA]),
                )
                .filled(true)
                .build();
        }

        // ── Selection highlight ───────────────────────────────
        let is_selected = self.selection.contains(&idx);
        let is_cursor = self.cursor_idx == Some(idx);
        if is_selected {
            // Brighter for cursor row, dimmer for other selected rows.
            let alpha = if is_cursor { 0.55 } else { 0.35 };
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32([
                        colors.selection_bg[0],
                        colors.selection_bg[1],
                        colors.selection_bg[2],
                        alpha,
                    ]),
                )
                .filled(true)
                .build();
        } else if is_cursor {
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32(colors.selection_bg),
                )
                .filled(true)
                .build();
        }

        // ── Row hover ─────────────────────────────────────────
        let row_hovered = mouse_pos[1] >= y
            && mouse_pos[1] < y + lh
            && mouse_pos[0] >= origin_x
            && mouse_pos[0] < origin_x + win_w;
        if row_hovered && !is_selected && !is_cursor {
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + win_w, y + lh],
                    col32(colors.hover_bg),
                )
                .filled(true)
                .build();
        }

        let mut x = origin_x;

        // ── Margin gutter (bookmark | breakpoint number) ───────
        // Layout inside the single `cols.margin` column:
        //
        //   |  bookmark  |  bp number  |
        //   | left half  |  right half |
        //
        // - LEFT half: bookmark marker (icon glyph or ring fallback).
        // - RIGHT half: breakpoint number — coloured digit only,
        //   NO background fill (user feedback 2026-04-30: the dark
        //   tint read as a heavy "highlighted band" rather than a
        //   small numeric tag).
        //
        // Margin reserved when EITHER feature is on; both flags
        // gate independently so disabling one doesn't hide the
        // other.
        if cfg.show_breakpoints || cfg.show_bookmarks {
            // Reserve a 3-px inset on each side of the gutter +
            // a 2-px gap at the centre between the two halves so
            // the bookmark and bp digit never touch each other or
            // the column borders. User feedback 2026-04-30: prior
            // layout slammed both markers flush against the edges.
            const GUTTER_EDGE_PAD: f32 = 3.0;
            const GUTTER_CENTRE_GAP: f32 = 2.0;
            let usable = (cols.margin - 2.0 * GUTTER_EDGE_PAD - GUTTER_CENTRE_GAP).max(0.0);
            let half = usable * 0.5;
            // Left half — bookmark marker, centred between the
            // left edge pad and the centre gap.
            if cfg.show_bookmarks && self.is_bookmarked(instr.address()) {
                let cx = x + GUTTER_EDGE_PAD + half * 0.5;
                let cy = y + lh * 0.5;
                if cfg.icons_available {
                    let glyph = crate::icons::BOOKMARK_CHECK_OUTLINE;
                    let sz = crate::utils::text::calc_text_size(glyph);
                    draw_list.add_text(
                        [cx - sz[0] * 0.5, cy - sz[1] * 0.5],
                        col32(colors.bookmark),
                        glyph,
                    );
                } else {
                    let radius = (lh * 0.28).min(half * 0.45);
                    draw_list
                        .add_circle([cx, cy], radius, col32(colors.bookmark))
                        .filled(false)
                        .thickness(1.4)
                        .num_segments(20)
                        .build();
                }
            }
            // Right half — watchpoint / breakpoint label. Priority:
            //   - watchpoint set        -> "RW"
            //   - execution breakpoint  -> "<bp_number>"
            //
            // The viewer surfaces a single watchpoint kind; hosts
            // that distinguish read-only vs write-only data
            // breakpoints handle that on the engine side and report
            // the union back through `Instruction::has_watchpoint`.
            // Watchpoints share the breakpoint visual class
            // ("things that pause the running process") — the `RW`
            // glyph differentiates from the numeric bp tag. Digit
            // / letters only — no background fill.
            if cfg.show_breakpoints {
                let has_wp = instr.has_watchpoint();
                let bp_num = instr.breakpoint_number();
                let label_owned: String;
                let label: Option<&str> = if has_wp {
                    Some("RW")
                } else if bp_num > 0 {
                    label_owned = format!("{bp_num}");
                    Some(label_owned.as_str())
                } else {
                    None
                };
                if let Some(label) = label {
                    // Watchpoint glyph tinted with `operand_memory`
                    // (orange) to match the popup colour-coding —
                    // the same hue the menu's "Toggle Watchpoint"
                    // entry uses. Execution breakpoints keep the
                    // per-bp_number hue from `bp_color`.
                    let label_color = if has_wp {
                        colors.operand_memory
                    } else {
                        colors.bp_color(bp_num.max(1))
                    };
                    let text_w = label.len() as f32 * self.char_advance;
                    let cx = x + GUTTER_EDGE_PAD + half + GUTTER_CENTRE_GAP + half * 0.5;
                    let tx = cx - text_w * 0.5;
                    let text_h = self.line_height - 4.0;
                    let ty = y + (lh - text_h) * 0.5;
                    draw_list.add_text([tx, ty], col32(label_color), label);
                }
            }
            x += cols.margin;
        }

        // ── Arrow area (drawn separately in draw_arrows) ──────
        if cfg.show_arrows {
            x += cols.arrows;
        }

        // ── Address column ────────────────────────────────────
        // Host can override the displayed address via
        // `DisasmDataProvider::symbol_name(addr)` — typically used to
        // show "module+offset" (e.g. `kernel32+0x1234`) for mapped
        // memory regions, or a known export / symbol name when one is
        // resolvable. Returning `None` falls back to the raw absolute
        // hex format. The widget itself stays domain-agnostic — it has
        // no knowledge of modules, PE files, or PDB symbols.
        let addr = instr.address();
        let addr_str = provider.symbol_name(addr).unwrap_or_else(|| {
            match (cfg.address_width_64, cfg.uppercase) {
                (true, true) => format!("{addr:016X}"),
                (true, false) => format!("{addr:016x}"),
                (false, true) => format!("{addr:08X}"),
                (false, false) => format!("{addr:08x}"),
            }
        });

        // "Just copied" address-gutter flash — translucent
        // accent-coloured pill behind the address text whenever the
        // user has just double-clicked the gutter. Fades over its
        // dwell so it reads as "happened just now, about to
        // disappear". Same idiom as `hex_viewer::draw_row`.
        if let Some((flash_row, frames)) = self.address_flash
            && flash_row == idx
            && frames > 0
        {
            let fade =
                (frames as f32 / super::super::input::ADDRESS_FLASH_FRAMES as f32).clamp(0.0, 1.0);
            let c = colors.selection_bg;
            let pad_x = self.char_advance * 0.5;
            let bg_left = x - pad_x;
            // Pill spans the visible address glyphs plus a half-glyph
            // cushion on each side so the highlight doesn't graze the
            // divider. `chars().count()` (not `len()`) so a non-ASCII
            // `symbol_name` (e.g. a Cyrillic module name) sizes the
            // pill to its glyph count, not its UTF-8 byte length.
            let addr_glyphs = addr_str.chars().count() as f32;
            let bg_right = x + addr_glyphs * self.char_advance + pad_x;
            draw_list
                .add_rect(
                    [bg_left, y],
                    [bg_right, y + lh],
                    col32([c[0], c[1], c[2], c[3] * 0.65 * fade]),
                )
                .filled(true)
                .rounding(2.0)
                .build();
        }

        draw_list.add_text([x, y], col32(colors.address), &addr_str);
        x += cols.address;

        // ── Bytes column (with inline InputText edit) ──────────
        if cfg.show_bytes {
            let is_editing_bytes = self
                .edit
                .as_ref()
                .is_some_and(|e| e.idx == idx && e.column == EditColumn::Bytes);

            // `data_x` shifts the bytes content right by COL_INNER_PAD
            // so it doesn't sit flush against the left divider — see
            // module-level constant for rationale.
            let data_x = x + COL_INNER_PAD;

            if is_editing_bytes {
                self.edit_render_pos.set(Some([data_x, y]));
                self.edit_render_width
                    .set((cols.bytes - COL_INNER_PAD).max(40.0));

                // Draw placeholder background so it's visible.
                draw_list
                    .add_rect(
                        [data_x - 2.0, y],
                        [x + cols.bytes, y + lh],
                        col32(EDIT_CELL_BG),
                    )
                    .filled(true)
                    .build();
                draw_list
                    .add_rect(
                        [data_x - 2.0, y],
                        [x + cols.bytes, y + lh],
                        col32(EDIT_CELL_BORDER),
                    )
                    .build();
            } else {
                let bytes = instr.bytes();
                if cfg.byte_category_colors {
                    // Per-byte category tint — same 5-tier scheme
                    // `hex_viewer` uses (zero / control / printable
                    // / high / 0xFF). Each byte gets its own
                    // `add_text` call because draw_list lacks a
                    // multi-colour run primitive — but the cost is
                    // bounded (instructions are 1..=15 bytes on x86)
                    // and the visual parity with hex_viewer is the
                    // whole point of the feature. Uses the
                    // `&'static str` returned by `byte_hex` directly
                    // — no copy, no `unsafe`, no allocation.
                    let cw = self.char_advance;
                    let mut bx = data_x;
                    for b in bytes.iter() {
                        let s = byte_hex(*b, cfg.uppercase);
                        let fg = colors.byte_fg_color(*b);
                        draw_list.add_text([bx, y], col32(fg), s);
                        bx += cw * 3.0;
                    }
                } else {
                    // Flat colour fallback — single buffer alloc
                    // (3 chars/byte) so we don't pay the per-byte
                    // `add_text` cost when category tinting is off.
                    let mut bytes_str = String::with_capacity(bytes.len() * 3);
                    for (i, b) in bytes.iter().enumerate() {
                        if i > 0 {
                            bytes_str.push(' ');
                        }
                        bytes_str.push_str(byte_hex(*b, cfg.uppercase));
                    }
                    draw_list.add_text([data_x, y], col32(colors.bytes), &bytes_str);
                }
            }
            // (No `x +=` here — `x` is read once below for `instr_data_x`
            //  and never again. Dropping the dead post-increment.)
        }

        // ── Mnemonic ──────────────────────────────────────────
        // Same COL_INNER_PAD treatment as Bytes: keep mnemonic/operand
        // text off the left divider.
        let bytes_end_x = if cfg.show_bytes { x + cols.bytes } else { x };
        let instr_data_x = bytes_end_x + COL_INNER_PAD;
        let mnemonic = instr.mnemonic();
        let mnemonic_color = colors.mnemonic_color(instr.flow_kind());
        draw_list.add_text([instr_data_x, y], col32(mnemonic_color), mnemonic);

        // ── Operands (with syntax coloring) ───────────────────
        // Operand text starts right after the mnemonic — they share
        // the conceptual "Instruction" span; only the leading edge
        // (mnemonic) gets the inner pad.
        let operands_x = instr_data_x + cols.mnemonic;
        let operands = instr.operands();
        self.draw_colored_operands(draw_list, operands_x, y, operands, colors);

        // ── Comment ───────────────────────────────────────────
        // `comment_x` is the per-frame dynamic value computed in
        // `render()` — typically the default operand-end X, but slid
        // right when any visible instruction text would otherwise
        // collide with the comment column.
        if cfg.show_comments {
            let is_editing_comment = self
                .edit
                .as_ref()
                .is_some_and(|e| e.idx == idx && e.column == EditColumn::Comment);

            if is_editing_comment {
                // Hand the inline-input slot off to render() —
                // same pattern as the Bytes edit path. Use the
                // **dynamic** comment column width so the input
                // field stretches to the right edge of the host
                // window (per the per-frame `frame_comment_w`).
                let edit_x = comment_x + COMMENT_LEFT_PAD;
                // `frame_comment_w` is `None` only on frame 0 — the
                // edit cell can't be activated that early (requires a
                // double-click that follows a render), so unwrapping
                // to `cols.comment` is a safe theoretical fallback.
                let edit_w = (self.frame_comment_w.get().unwrap_or(cols.comment)
                    - COMMENT_LEFT_PAD)
                    .max(cols.comment.max(120.0));
                self.edit_render_pos.set(Some([edit_x, y]));
                self.edit_render_width.set(edit_w);

                // Highlight the comment cell in the same warm
                // accent the bytes-edit path uses, so the user
                // sees instantly which cell is being edited.
                draw_list
                    .add_rect(
                        [edit_x - 2.0, y],
                        [edit_x + edit_w, y + lh],
                        col32(EDIT_CELL_BG),
                    )
                    .filled(true)
                    .build();
                draw_list
                    .add_rect(
                        [edit_x - 2.0, y],
                        [edit_x + edit_w, y + lh],
                        col32(EDIT_CELL_BORDER),
                    )
                    .build();
            } else if let Some(comment) = instr.comment() {
                // Two `add_text` calls instead of `format!("; {}", ..)`
                // — saves a per-row per-frame `String` allocation
                // that the renderer was paying for on every visible
                // commented instruction. The "; " prefix advances
                // the cursor by `2 * char_advance` (monospace).
                let prefix_x = comment_x + COMMENT_LEFT_PAD;
                draw_list.add_text([prefix_x, y], col32(colors.comment), "; ");
                draw_list.add_text(
                    [prefix_x + 2.0 * self.char_advance, y],
                    col32(colors.comment),
                    comment,
                );
            }
        }

        // ── Tooltip on hover (comprehensive) ─────────────────
        // The tooltip body lives in `draw::tooltip` to keep this
        // per-row paint file under the file-size ceiling.
        if row_hovered {
            self.draw_row_tooltip(ui, instr, prev_instr, next_instr, addr);
        }
    }
}
