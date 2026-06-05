//! Branch arrow computation for disasm_view.

use super::provider::{DisasmDataProvider, FlowKind, Instruction};

// ── Branch Arrow ────────────────────────────────────────────────────────────

/// Visual branch arrow connecting two visible instructions.
///
/// Indices (`from_idx` / `to_idx`) are **visible-local** (0-based
/// within the current render window): 0 = first visible row, the
/// renderer multiplies by line-height to position the arrow.
///
/// `clipped_from` / `clipped_to` are raised when the corresponding
/// endpoint is *outside* the visible window — index is then clamped
/// to `0` (above) or `visible_count - 1` (below) so the arrow draws
/// at the window edge. The renderer suppresses the arrowhead at any
/// clipped end so the arrow visually reads as "continues offscreen"
/// rather than "lands here". See [`compute_arrows_clipped`] for the
/// scanner that produces clipped arrows.
#[derive(Debug, Clone)]
pub struct BranchArrow {
    /// Source instruction index (visible-local, clamped when clipped).
    pub from_idx: usize,
    /// Target instruction index (visible-local, clamped when clipped).
    pub to_idx: usize,
    /// Nesting depth (0 = closest to text, higher = further left).
    pub depth: usize,
    /// Flow kind of the branch (for coloring).
    pub flow_kind: FlowKind,
    /// Source is outside the visible window — arrowhead at the
    /// `from_idx` end MUST be suppressed by the renderer.
    pub clipped_from: bool,
    /// Target is outside the visible window — arrowhead at the
    /// `to_idx` end MUST be suppressed by the renderer.
    pub clipped_to: bool,
}

/// Maximum nesting depth for branch arrows.
pub const MAX_ARROW_DEPTH: usize = 6;

/// Compute branch arrows for a set of visible instructions.
///
/// **Visible-only**: only arrows whose source AND target both fall
/// inside the `instructions` slice are produced. Arrows leaving or
/// entering the visible range are silently dropped. Use
/// [`compute_arrows_clipped`] when arrow continuity past the
/// visible-window edge matters (the renderer pipeline does).
///
/// Returns arrows sorted by span size (smallest first, drawn closest to text).
pub fn compute_arrows(
    instructions: &[&dyn Instruction],
    visible_start_idx: usize,
    visible_count: usize,
) -> Vec<BranchArrow> {
    let mut arrows = Vec::new();
    let end_idx = visible_start_idx + visible_count;

    for (vis_i, instr) in instructions.iter().enumerate() {
        if let Some(target) = instr.branch_target() {
            // Find target in visible range.
            for (vis_j, other) in instructions.iter().enumerate() {
                let global_j = visible_start_idx + vis_j;
                if other.address() == target && global_j < end_idx {
                    arrows.push(BranchArrow {
                        from_idx: vis_i,
                        to_idx: vis_j,
                        depth: 0, // assigned below
                        flow_kind: instr.flow_kind(),
                        clipped_from: false,
                        clipped_to: false,
                    });
                    break;
                }
            }
        }
    }

    assign_arrow_depths(&mut arrows);
    arrows
}

/// Cross-window arrow scanner — produces arrows that intersect the
/// visible range `[visible_start, visible_start + visible_count)` in
/// any of three ways:
///
/// - **Both endpoints visible** (anchored arrow, `clipped_* == false`).
/// - **One endpoint visible**, the other above or below the window
///   (clamped to `0` / `visible_count - 1` with the corresponding
///   `clipped_*` flag raised).
/// - **Pass-through**: source above + target below (or vice versa).
///   Both `clipped_*` flags raised; renderer paints a vertical line
///   across the entire window with no arrowhead and no horizontal
///   stubs — visually reads as "long jump traversing this view".
///
/// Arrows where BOTH endpoints sit on the same side of the window
/// (both above OR both below) are silently dropped — they don't
/// cross the visible region, so there's nothing to render.
///
/// Returned arrows are sorted **by priority then span** so that
/// caller-side `truncate(N)` (driven by [`super::config::DisasmViewConfig::max_arrows`])
/// drops the *least informative* arrows first:
///
/// 1. Anchored (both endpoints visible) — preserved first
/// 2. Single-clipped (one endpoint visible)
/// 3. Pass-through (both endpoints clipped) — first to drop on overflow
///
/// Within each priority tier, smaller spans come first so depth
/// assignment puts them at depth 0 (closest to text).
///
/// **Cost**: linear scan of all provider instructions per frame —
/// `O(N)` for `N` total instructions, plus the cost of each
/// `provider.index_of_address(target)` call (typically `O(log N)`
/// or `O(1)` in well-built providers; `VecDisasmProvider`'s
/// linear-scan impl is `O(N)` so the effective total is `O(N²)` —
/// fine for typical few-thousand-instruction disassemblies, switch
/// to an indexed provider for 100k+).
pub fn compute_arrows_clipped(
    provider: &dyn DisasmDataProvider,
    visible_start_idx: usize,
    visible_count: usize,
) -> Vec<BranchArrow> {
    if visible_count == 0 {
        return Vec::new();
    }
    let total = provider.instruction_count();
    if total == 0 {
        return Vec::new();
    }
    let visible_end = (visible_start_idx + visible_count).min(total);
    let last_local = visible_count.saturating_sub(1);

    let mut arrows = Vec::new();

    for src in 0..total {
        let Some(instr) = provider.instruction(src) else {
            continue;
        };
        let Some(target_addr) = instr.branch_target() else {
            continue;
        };
        let Some(dst) = provider.index_of_address(target_addr) else {
            continue;
        };

        let src_above = src < visible_start_idx;
        let src_below = src >= visible_end;
        let dst_above = dst < visible_start_idx;
        let dst_below = dst >= visible_end;
        let src_in = !src_above && !src_below;
        let dst_in = !dst_above && !dst_below;

        // Drop arrows that don't cross the window — both above or
        // both below. Pass-through (one above + one below) is
        // explicitly KEPT so long jumps remain visible while
        // scrolling through their middle.
        let both_above = src_above && dst_above;
        let both_below = src_below && dst_below;
        if both_above || both_below {
            continue;
        }

        // Map global → visible-local with clamping.
        let from_idx = if src_in {
            src - visible_start_idx
        } else if src_above {
            0
        } else {
            last_local
        };
        let to_idx = if dst_in {
            dst - visible_start_idx
        } else if dst_above {
            0
        } else {
            last_local
        };

        arrows.push(BranchArrow {
            from_idx,
            to_idx,
            depth: 0, // assigned below
            flow_kind: instr.flow_kind(),
            clipped_from: !src_in,
            clipped_to: !dst_in,
        });
    }

    // Sort by (priority, span) — anchored first, single-clipped
    // next, pass-through last. Within each tier, smaller spans come
    // first so depth assignment gives them depth 0 (visually
    // closest to the text column). The renderer's `truncate` then
    // chops the *tail* of this list, dropping low-priority arrows
    // before high-priority ones.
    arrows.sort_by_key(|a| {
        let priority: u8 = match (a.clipped_from, a.clipped_to) {
            (false, false) => 0,                // anchored
            (false, true) | (true, false) => 1, // half-clipped
            (true, true) => 2,                  // pass-through
        };
        let lo = a.from_idx.min(a.to_idx);
        let hi = a.from_idx.max(a.to_idx);
        (priority, hi - lo)
    });

    // In-order depth assignment — must NOT re-sort, otherwise the
    // priority order above would be lost and pass-through arrows
    // (which are big-span by definition) would migrate to depth 0,
    // crowding out anchored small arrows.
    assign_arrow_depths_in_order(&mut arrows);
    arrows
}

/// Sort arrows by span (smallest first → drawn closest to text)
/// and pack them into [`MAX_ARROW_DEPTH`] horizontal lanes so
/// overlapping arrows nest visually instead of stacking on top
/// of each other.
///
/// Used by the legacy [`compute_arrows`] path — that scanner
/// has no priority concept (everything is anchored), so plain
/// span-ascending sort is correct. [`compute_arrows_clipped`]
/// uses [`assign_arrow_depths_in_order`] instead so its
/// priority ordering survives.
fn assign_arrow_depths(arrows: &mut [BranchArrow]) {
    arrows.sort_by_key(|a| {
        let lo = a.from_idx.min(a.to_idx);
        let hi = a.from_idx.max(a.to_idx);
        hi - lo
    });
    assign_arrow_depths_in_order(arrows);
}

/// Pack arrows into [`MAX_ARROW_DEPTH`] horizontal lanes WITHOUT
/// re-sorting — assumes the caller has already arranged the slice
/// in render-priority order (smaller / more important arrows
/// first). Each arrow gets the lowest depth slot whose existing
/// entries don't overlap its `[lo, hi]` span; arrows that
/// would exceed `MAX_ARROW_DEPTH - 1` get clamped to the
/// outermost lane.
fn assign_arrow_depths_in_order(arrows: &mut [BranchArrow]) {
    // Fixed-size array of Vecs — avoids the 6 heap allocations that
    // `vec![Vec::new(); MAX_ARROW_DEPTH]` paid every call.
    let mut depth_slots: [Vec<(usize, usize)>; MAX_ARROW_DEPTH] =
        std::array::from_fn(|_| Vec::new());
    for arrow in arrows {
        let lo = arrow.from_idx.min(arrow.to_idx);
        let hi = arrow.from_idx.max(arrow.to_idx);
        // `depth_slots` is a fixed `[_; MAX_ARROW_DEPTH]` array, so the
        // iterator is already bounded — no `.take()` needed. Find the
        // lowest lane whose existing spans don't overlap `[lo, hi]`
        // (half-open overlap test `lo < shi && hi > slo`); if every
        // lane up to the last overlaps, `found_depth` lands one past
        // the array and the `.min` below clamps it to the outer lane.
        let mut found_depth = MAX_ARROW_DEPTH;
        for (d, slot) in depth_slots.iter().enumerate() {
            if slot.iter().all(|&(slo, shi)| lo >= shi || hi <= slo) {
                found_depth = d;
                break;
            }
        }
        let depth = found_depth.min(MAX_ARROW_DEPTH - 1);
        arrow.depth = depth;
        depth_slots[depth].push((lo, hi));
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disasm_view::provider::{InstructionEntry, VecDisasmProvider};

    fn arrow(from: usize, to: usize, clipped_from: bool, clipped_to: bool) -> BranchArrow {
        BranchArrow {
            from_idx: from,
            to_idx: to,
            depth: 0,
            flow_kind: FlowKind::Jump,
            clipped_from,
            clipped_to,
        }
    }

    /// Disjoint spans all pack into lane 0.
    #[test]
    fn depth_disjoint_spans_share_lane_zero() {
        let mut arrows = [arrow(0, 1, false, false), arrow(4, 5, false, false)];
        assign_arrow_depths_in_order(&mut arrows);
        assert_eq!(arrows[0].depth, 0);
        assert_eq!(arrows[1].depth, 0);
    }

    /// Overlapping spans must land on different lanes.
    #[test]
    fn depth_overlapping_spans_nest() {
        // [0,5] overlaps [2,3] — the later-processed one moves to lane 1.
        let mut arrows = [arrow(0, 5, false, false), arrow(2, 3, false, false)];
        assign_arrow_depths_in_order(&mut arrows);
        assert_ne!(arrows[0].depth, arrows[1].depth);
    }

    /// Reversed endpoints (`from > to`) still pack by absolute span.
    #[test]
    fn depth_uses_absolute_span_when_reversed() {
        let mut arrows = [arrow(5, 0, false, false), arrow(3, 2, false, false)];
        assign_arrow_depths_in_order(&mut arrows);
        assert_ne!(arrows[0].depth, arrows[1].depth);
    }

    /// More overlapping arrows than `MAX_ARROW_DEPTH` clamp to the
    /// outermost lane instead of panicking on an out-of-range index.
    #[test]
    fn depth_clamps_at_max_depth() {
        // MAX_ARROW_DEPTH + 2 fully-overlapping spans.
        let mut arrows: Vec<BranchArrow> = (0..MAX_ARROW_DEPTH + 2)
            .map(|_| arrow(0, 9, false, false))
            .collect();
        assign_arrow_depths_in_order(&mut arrows);
        assert!(arrows.iter().all(|a| a.depth < MAX_ARROW_DEPTH));
        // The last spans pile into the outermost lane.
        assert_eq!(arrows.last().unwrap().depth, MAX_ARROW_DEPTH - 1);
    }

    fn jump_provider() -> VecDisasmProvider {
        // 20 nops at 0x1000, 0x1001, ... with a few branches wired.
        let mut p = VecDisasmProvider::new();
        for i in 0..20u64 {
            p.push(InstructionEntry::new(0x1000 + i, vec![0x90], "nop", ""));
        }
        p
    }

    /// Both endpoints inside the window → anchored arrow, no clip flags.
    #[test]
    fn clipped_both_visible_is_anchored() {
        let mut p = jump_provider();
        // src idx 2 → dst idx 5 (both inside window [0,10)).
        p.instructions_mut()[2] = InstructionEntry::new(0x1002, vec![0xEB], "jmp", "0x1005")
            .with_flow(FlowKind::Jump)
            .with_target(0x1005);
        let arrows = compute_arrows_clipped(&p, 0, 10);
        let a = arrows.iter().find(|a| a.from_idx == 2).unwrap();
        assert_eq!(a.to_idx, 5);
        assert!(!a.clipped_from && !a.clipped_to);
    }

    /// Target below the window → `to_idx` clamps to the last visible
    /// row and `clipped_to` is raised.
    #[test]
    fn clipped_target_below_window_clamps() {
        let mut p = jump_provider();
        // src idx 1 (visible) → dst idx 15 (below window [0,10)).
        p.instructions_mut()[1] = InstructionEntry::new(0x1001, vec![0xEB], "jmp", "0x100F")
            .with_flow(FlowKind::Jump)
            .with_target(0x100F);
        let arrows = compute_arrows_clipped(&p, 0, 10);
        let a = arrows.iter().find(|a| a.from_idx == 1).unwrap();
        assert!(a.clipped_to, "off-window target must be flagged clipped");
        assert!(!a.clipped_from);
        assert_eq!(a.to_idx, 9, "clamped to last visible local row");
    }

    /// Source above + target below → pass-through, both clipped, both
    /// endpoints clamped to the window edges.
    #[test]
    fn clipped_pass_through_keeps_both_flags() {
        let mut p = jump_provider();
        // Window [5,10). src idx 1 (above) → dst idx 15 (below).
        p.instructions_mut()[1] = InstructionEntry::new(0x1001, vec![0xEB], "jmp", "0x100F")
            .with_flow(FlowKind::Jump)
            .with_target(0x100F);
        let arrows = compute_arrows_clipped(&p, 5, 5);
        assert_eq!(arrows.len(), 1);
        let a = &arrows[0];
        assert!(a.clipped_from && a.clipped_to);
        assert_eq!(a.from_idx, 0); // clamped to top edge
        assert_eq!(a.to_idx, 4); // clamped to bottom edge (visible_count-1)
    }

    /// Both endpoints on the same side of the window are dropped.
    #[test]
    fn clipped_both_below_is_dropped() {
        let mut p = jump_provider();
        // Window [0,5). src idx 12 → dst idx 15, both below.
        p.instructions_mut()[12] = InstructionEntry::new(0x100C, vec![0xEB], "jmp", "0x100F")
            .with_flow(FlowKind::Jump)
            .with_target(0x100F);
        let arrows = compute_arrows_clipped(&p, 0, 5);
        assert!(arrows.is_empty());
    }

    /// Zero visible_count and empty provider both yield no arrows
    /// (no panic on the `last_local`/`saturating_sub` path).
    #[test]
    fn clipped_degenerate_inputs_yield_empty() {
        let p = jump_provider();
        assert!(compute_arrows_clipped(&p, 0, 0).is_empty());
        let empty = VecDisasmProvider::new();
        assert!(compute_arrows_clipped(&empty, 0, 10).is_empty());
    }

    /// Priority sort: anchored arrows sort before pass-through, so a
    /// caller `truncate(1)` keeps the anchored one.
    #[test]
    fn clipped_priority_anchored_before_pass_through() {
        let mut p = jump_provider();
        // Window [5,10). Anchored: idx 6 → idx 8 (both visible).
        p.instructions_mut()[6] = InstructionEntry::new(0x1006, vec![0xEB], "jmp", "0x1008")
            .with_flow(FlowKind::Jump)
            .with_target(0x1008);
        // Pass-through: idx 1 (above) → idx 15 (below).
        p.instructions_mut()[1] = InstructionEntry::new(0x1001, vec![0xEB], "jmp", "0x100F")
            .with_flow(FlowKind::Jump)
            .with_target(0x100F);
        let arrows = compute_arrows_clipped(&p, 5, 5);
        assert_eq!(arrows.len(), 2);
        // Anchored (no clip flags) must come first.
        assert!(!arrows[0].clipped_from && !arrows[0].clipped_to);
        assert!(arrows[1].clipped_from && arrows[1].clipped_to);
    }
}
