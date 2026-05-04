//! Drawing routines for [`super::DisasmView`] — header, instruction rows
//! (with selection / hover / breakpoint / tooltip), syntax-colored operands,
//! and L-shaped branch arrows.

use super::arrows::MAX_ARROW_DEPTH;
use super::config::DisasmColors;
use super::provider::{FlowKind, Instruction};
use super::tokens::{OperandTokenizer, TokenKind};
use super::{DisasmView, EditColumn};
use crate::utils::color::col32;
use crate::utils::hex::byte_hex;

/// Pixel offset added to the X position of the comment column so
/// it doesn't sit flush against the operands column. User
/// requested ~5 px on 2026-04-29 — gives the comment text breathing
/// room without widening the comment column itself.
const COMMENT_LEFT_PAD: f32 = 5.0;

/// Inner left padding for the Bytes and Instruction columns —
/// keeps their data text from sitting flush against the column
/// divider on the left. ~1 char advance worth of breathing room.
/// Right edge has its own slack via the column-width slack
/// (typical bytes string < 180 px column, mnemonic < 70 px column).
pub(super) const COL_INNER_PAD: f32 = 6.0;

/// Cushion the dynamic comment X keeps from the longest visible
/// instruction text — when an overflowing operand string pushes the
/// comment column right, the divider lands `COMMENT_GAP` pixels past
/// the rightmost glyph so the divider never hugs the text.
pub(super) const COMMENT_GAP: f32 = 10.0;

/// Background tint of the inline edit cell (Bytes / Comment) — warm
/// brown so the user sees instantly which cell is being edited.
/// Theme-independent on purpose: this is a transient, modal-ish UI
/// affordance that needs to read consistently across all themes.
const EDIT_CELL_BG: [f32; 4] = [0.20, 0.15, 0.08, 0.95];

/// Border colour of the inline edit cell — same warm-amber accent
/// the cursor / focus highlight uses.
const EDIT_CELL_BORDER: [f32; 4] = [1.0, 0.7, 0.3, 0.80];

impl DisasmView {
    pub(super) fn draw_header(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        y: f32,
        comment_x: f32,
    ) {
        let cols = &self.config.columns;
        let hdr_col = col32(self.config.colors.header);
        let cw = self.char_advance;
        let mut x = origin_x;

        if self.config.show_breakpoints {
            x += cols.margin;
        }
        if self.config.show_arrows {
            x += cols.arrows;
        }

        // Address — left-aligned (matches the addresses below it).
        draw_list.add_text([x, y], hdr_col, "Address");
        x += cols.address;

        // Bytes — centred within the column for visual balance with
        // the now-spaced data. `centred_x` clamps to the inner-pad
        // boundary so the header never overlaps the left divider.
        if self.config.show_bytes {
            let label = "Bytes";
            let text_w = label.len() as f32 * cw;
            let cx = x + ((cols.bytes - text_w) * 0.5).max(COL_INNER_PAD);
            draw_list.add_text([cx, y], hdr_col, label);
            x += cols.bytes;
        }

        // Instruction — centred across the combined mnemonic + operands
        // span. Stops at `comment_x` (which may have slid right beyond
        // the default span) so the centring still tracks the visible
        // column width.
        let instr_span = (comment_x - x).max(cols.mnemonic + cols.operands);
        let label = "Instruction";
        let text_w = label.len() as f32 * cw;
        let cx = x + ((instr_span - text_w) * 0.5).max(COL_INNER_PAD);
        draw_list.add_text([cx, y], hdr_col, label);

        if self.config.show_comments {
            draw_list.add_text([comment_x + COMMENT_LEFT_PAD, y], hdr_col, "Comment");
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)] // tooltip needs prev/next neighbours for idiom detection
    pub(super) fn draw_instruction_row(
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
                    col32([bg[0], bg[1], bg[2], bg[3] * super::ORIGIN_BG_ALPHA_FACTOR]),
                )
                .filled(true)
                .build();
            // 2. Left-edge stripe.
            draw_list
                .add_rect(
                    [origin_x, y],
                    [origin_x + super::ORIGIN_STRIPE_WIDTH, y + lh],
                    col32([bg[0], bg[1], bg[2], super::ORIGIN_STRIPE_ALPHA]),
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
                    label_owned = format!("{}", bp_num);
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
                    let cx =
                        x + GUTTER_EDGE_PAD + half + GUTTER_CENTRE_GAP + half * 0.5;
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
        let addr = instr.address();
        let addr_str = if cfg.address_width_64 {
            if cfg.uppercase {
                format!("{:016X}", addr)
            } else {
                format!("{:016x}", addr)
            }
        } else if cfg.uppercase {
            format!("{:08X}", addr)
        } else {
            format!("{:08x}", addr)
        };

        // "Just copied" address-gutter flash — translucent
        // accent-coloured pill behind the address text whenever the
        // user has just double-clicked the gutter. Fades over its
        // dwell so it reads as "happened just now, about to
        // disappear". Same idiom as `hex_viewer::draw_row`.
        if let Some((flash_row, frames)) = self.address_flash
            && flash_row == idx
            && frames > 0
        {
            let fade = (frames as f32 / super::input::ADDRESS_FLASH_FRAMES as f32)
                .clamp(0.0, 1.0);
            let c = colors.selection_bg;
            let pad_x = self.char_advance * 0.5;
            let bg_left = x - pad_x;
            // Pill spans the visible address glyphs (`addr_str` is
            // monospace) plus a half-glyph cushion on each side so
            // the highlight doesn't graze the divider.
            let bg_right = x + addr_str.len() as f32 * self.char_advance + pad_x;
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
                .map(|e| e.idx == idx && e.column == EditColumn::Bytes)
                .unwrap_or(false);

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
        let bytes_end_x = if cfg.show_bytes {
            x + cols.bytes
        } else {
            x
        };
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
                .map(|e| e.idx == idx && e.column == EditColumn::Comment)
                .unwrap_or(false);

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
                draw_list.add_text(
                    [prefix_x, y],
                    col32(colors.comment),
                    "; ",
                );
                draw_list.add_text(
                    [prefix_x + 2.0 * self.char_advance, y],
                    col32(colors.comment),
                    comment,
                );
            }
        }

        // ── Tooltip on hover (comprehensive) ─────────────────
        if row_hovered {
            // Honour `cfg.address_width_64` AND `cfg.uppercase` — fixes
            // the long-standing bug where the gutter respected both
            // flags but the tooltip hard-coded uppercase 16-digit
            // formatting regardless of widget config. The 32-bit
            // shadow is only emitted in 64-bit mode (in 32-bit mode
            // the primary line already shows 8 digits — a duplicate
            // would just clutter the popup).
            let strings = self.strings();
            let upper = cfg.uppercase;
            let is_64 = cfg.address_width_64;
            crate::utils::themed_tooltip(ui, || {
                let primary = match (upper, is_64) {
                    (true, true) => format!("{}0x{:016X}", strings.tooltip_address_prefix, addr),
                    (false, true) => format!("{}0x{:016x}", strings.tooltip_address_prefix, addr),
                    (true, false) => format!("{}0x{:08X}", strings.tooltip_address_prefix, addr),
                    (false, false) => format!("{}0x{:08x}", strings.tooltip_address_prefix, addr),
                };
                ui.text(primary);
                if is_64 && addr <= 0xFFFF_FFFF {
                    let shadow = if upper {
                        format!("{}0x{:08X}", strings.tooltip_address32_prefix, addr as u32)
                    } else {
                        format!("{}0x{:08x}", strings.tooltip_address32_prefix, addr as u32)
                    };
                    ui.text(shadow);
                }

                let bytes = instr.bytes();
                ui.text(format!(
                    "{}{} {}",
                    strings.tooltip_size_label,
                    bytes.len(),
                    strings.tooltip_unit_bytes,
                ));
                let mut hex_str = String::with_capacity(bytes.len() * 3);
                for (i, b) in bytes.iter().enumerate() {
                    if i > 0 {
                        hex_str.push(' ');
                    }
                    hex_str.push_str(byte_hex(*b, upper));
                }
                ui.text(format!("{}{}", strings.tooltip_bytes_label, hex_str));

                ui.text(format!(
                    "{}{} {}",
                    strings.tooltip_instr_label,
                    instr.mnemonic(),
                    instr.operands(),
                ));

                let flow_desc = match instr.flow_kind() {
                    FlowKind::Normal => strings.flow_normal,
                    FlowKind::Jump => strings.flow_jump,
                    FlowKind::Call => strings.flow_call,
                    FlowKind::Return => strings.flow_return,
                    FlowKind::Nop => strings.flow_nop,
                    FlowKind::Stack => strings.flow_stack,
                    FlowKind::System => strings.flow_system,
                    FlowKind::Invalid => strings.flow_invalid,
                };
                ui.text(format!("{}{}", strings.tooltip_flow_label, flow_desc));

                if let Some(target) = instr.branch_target() {
                    let target_str = match (upper, is_64) {
                        (true, true) => format!("{}0x{:016X}", strings.tooltip_target_label, target),
                        (false, true) => format!("{}0x{:016x}", strings.tooltip_target_label, target),
                        (true, false) => format!("{}0x{:08X}", strings.tooltip_target_label, target),
                        (false, false) => format!("{}0x{:08x}", strings.tooltip_target_label, target),
                    };
                    ui.text(target_str);
                    let off = target as i64 - addr as i64;
                    let (sign, mag) = if off >= 0 { ('+', off) } else { ('-', -off) };
                    let off_hex = if upper {
                        format!("0x{mag:X}")
                    } else {
                        format!("0x{mag:x}")
                    };
                    ui.text(format!(
                        "Offset: {sign}{off_hex} ({mag} {})",
                        strings.tooltip_unit_bytes,
                    ));
                }

                ui.text(format!(
                    "{}{}",
                    strings.tooltip_block_label,
                    instr.block_index(),
                ));

                if instr.has_breakpoint() {
                    let bp_num = instr.breakpoint_number();
                    if bp_num > 0 {
                        ui.text(format!("{}#{}", strings.tooltip_breakpoint_label, bp_num));
                    } else {
                        ui.text(strings.tooltip_breakpoint_yes);
                    }
                }

                if instr.is_current() {
                    ui.text(strings.tooltip_current_ip);
                }

                if let Some(comment) = instr.comment() {
                    ui.text(format!("{}{}", strings.tooltip_comment_label, comment));
                }

                // ── Educational block (opt-out per cfg flag) ────────
                //
                // 1. Mnemonic explainer (`cfg.show_explanation`) —
                //    plain-language description of the current opcode.
                // 2. Idiom detector (`cfg.show_idiom`) — recognises
                //    multi-instruction patterns from prev/current/next
                //    (prologue / cmp+Jcc / get-IP / NULL-check / ...).
                // 3. Gotcha (`cfg.show_gotcha`) — anti-debug /
                //    anti-disasm / obfuscation warnings tied to
                //    specific mnemonics (rdtsc timing, int 2D probe,
                //    push/ret ROP gadgets, ...).
                //
                // Each block has its own toggle so senior REs can
                // strip the tooltip down to the raw fields.
                let info = super::mnemonic::lookup(instr.mnemonic());

                if cfg.show_explanation
                    && let Some(info) = info
                {
                    ui.separator();
                    ui.text(format!(
                        "{}{}",
                        strings.tooltip_explanation_label,
                        info.description(cfg.locale),
                    ));
                }

                if cfg.show_idiom {
                    let prev_pair = prev_instr.map(|p| (p.mnemonic(), p.operands()));
                    let next_pair = next_instr.map(|n| (n.mnemonic(), n.operands()));
                    let ctx = super::idiom::InstructionContext {
                        prev: prev_pair,
                        current: (instr.mnemonic(), instr.operands()),
                        next: next_pair,
                    };
                    if let Some(idiom) = super::idiom::detect(&ctx) {
                        // Reuse the explanation separator if it was
                        // already drawn; if not, add one now so the
                        // educational lines visually break away from
                        // the technical fields above.
                        if !cfg.show_explanation || info.is_none() {
                            ui.separator();
                        }
                        ui.text(format!(
                            "{}{}",
                            strings.tooltip_idiom_label,
                            idiom.description(cfg.locale),
                        ));
                    }
                }

                if cfg.show_gotcha
                    && let Some(info) = info
                    && let Some(gotcha) = info.gotcha(cfg.locale)
                {
                    if !(cfg.show_explanation || cfg.show_idiom) {
                        ui.separator();
                    }
                    ui.text(format!("{}{}", strings.tooltip_gotcha_label, gotcha));
                }
            });
        }
    }

    /// Draw operand string with basic syntax coloring.
    pub(super) fn draw_colored_operands(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        x: f32,
        y: f32,
        operands: &str,
        colors: &DisasmColors,
    ) {
        let cw = self.char_advance;
        let mut cx = x;

        for token in OperandTokenizer::new(operands) {
            let color = match token.kind {
                TokenKind::Register => colors.operand_register,
                TokenKind::Number => colors.operand_number,
                TokenKind::Memory => colors.operand_memory,
                TokenKind::String => colors.operand_string,
                TokenKind::Plain => colors.operand_default,
            };
            draw_list.add_text([cx, y], col32(color), token.text);
            cx += token.text.len() as f32 * cw;
        }
    }

    /// Draw branch arrows between instructions.
    pub(super) fn draw_arrows(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        origin_y: f32,
    ) {
        let cols = &self.config.columns;
        let colors = &self.config.colors;
        let lh = self.line_height;

        // Arrow area starts after margin.
        let arrow_base_x = origin_x
            + if self.config.show_breakpoints {
                cols.margin
            } else {
                0.0
            }
            + cols.arrows;
        let depth_spacing = cols.arrows / (MAX_ARROW_DEPTH as f32 + 1.0);

        for arrow in &self.cached_arrows {
            let from_y = origin_y + (arrow.from_idx) as f32 * lh + lh * 0.5;
            let to_y = origin_y + (arrow.to_idx) as f32 * lh + lh * 0.5;
            let x = arrow_base_x - (arrow.depth as f32 + 1.0) * depth_spacing;

            let color = col32(colors.arrow_color(arrow.flow_kind));
            let thickness = if arrow.depth == 0 { 1.5 } else { 1.0 };

            // Horizontal stub at the source end — suppressed when
            // the source is offscreen (clipped) so the arrow visually
            // reads as "vertical line entering from above/below"
            // rather than "wraps into the gutter at this row".
            if !arrow.clipped_from {
                draw_list
                    .add_line([arrow_base_x, from_y], [x, from_y], color)
                    .thickness(thickness)
                    .build();
            }
            // Vertical line.
            draw_list
                .add_line([x, from_y], [x, to_y], color)
                .thickness(thickness)
                .build();
            // Horizontal stub at the target end — same suppression
            // for clipped target as for clipped source.
            if !arrow.clipped_to {
                draw_list
                    .add_line([x, to_y], [arrow_base_x, to_y], color)
                    .thickness(thickness)
                    .build();
            }

            // Arrowhead at target — only when the target is in
            // window; suppressing the head for clipped targets is
            // the visual cue "continues offscreen".
            if !arrow.clipped_to {
                let dir = if to_y > from_y { 1.0 } else { -1.0 };
                let head_size = 4.0;
                draw_list
                    .add_triangle(
                        [arrow_base_x, to_y],
                        [arrow_base_x - head_size, to_y - head_size * dir],
                        [arrow_base_x - head_size, to_y + head_size * dir],
                        color,
                    )
                    .filled(true)
                    .build();
            }
        }
    }

    /// Draw thin vertical dividers between the address / bytes /
    /// instruction / comment columns. Same `colors.separator` with
    /// alpha 0.40 treatment as `hex_viewer`'s column dividers — the
    /// lines read as a gentle visual cue, not heavy borders.
    ///
    /// Skipped for the leftmost (margin / arrows) gutters because
    /// those areas already have their own visual identity (numbered
    /// breakpoint cells, branch arrows). The first divider sits
    /// between the address column and the bytes column.
    pub(super) fn draw_column_dividers(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        div_top: f32,
        div_bot: f32,
        comment_x: f32,
    ) {
        let cols = &self.config.columns;
        let c = self.config.colors.separator;
        let div_col = col32([c[0], c[1], c[2], c[3] * 0.40]);

        let mut x = origin_x;
        if self.config.show_breakpoints {
            x += cols.margin;
        }
        if self.config.show_arrows {
            x += cols.arrows;
        }
        x += cols.address;

        let emit = |dx: f32| {
            draw_list
                .add_line([dx, div_top], [dx, div_bot], div_col)
                .thickness(1.0)
                .build();
        };

        // Divider 1 — between address and bytes (only when bytes
        // column is visible; otherwise the next visible column starts
        // right after address).
        if self.config.show_bytes {
            emit(x);
            x += cols.bytes;
        }

        // Divider 2 — between bytes and instruction (mnemonic).
        emit(x);

        // Divider 3 — between instruction and comment, only when
        // comment column is visible. Sits at the per-frame dynamic
        // `comment_x` so the divider follows whenever the
        // instruction text would have collided with the default
        // column boundary.
        if self.config.show_comments {
            emit(comment_x);
        }
    }
}
