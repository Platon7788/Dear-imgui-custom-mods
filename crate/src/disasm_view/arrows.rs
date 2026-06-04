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
        let mut found_depth = 0;
        'depth: for (d, slot) in depth_slots.iter().enumerate().take(MAX_ARROW_DEPTH) {
            for &(slo, shi) in slot {
                if lo < shi && hi > slo {
                    found_depth = d + 1;
                    continue 'depth;
                }
            }
            found_depth = d;
            break;
        }
        let depth = found_depth.min(MAX_ARROW_DEPTH - 1);
        arrow.depth = depth;
        depth_slots[depth].push((lo, hi));
    }
}
