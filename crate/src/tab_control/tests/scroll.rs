//! Regression coverage for [`scroll_into_view`](super::super::render::scroll_into_view).
//!
//! The scroll math reads only `tab_widths_cache` + `tabs` (never
//! `calc_text_size`), so it is deterministic and safe to exercise outside an
//! ImGui context once the cache is hand-populated. These tests pin two
//! contracts:
//!
//! 1. Pinned tabs never participate in scroll offsets — they live in the fixed
//!    left strip. Previously `scroll_into_view` summed *all* preceding widths
//!    (pinned included), over-counting `scroll_target` by `pinned_total_w` and
//!    mis-targeting the regular scroll; the fix sums only regular widths and
//!    bails out for pinned indices.
//! 2. The leftward / rightward clamp math matches `fill_hit_scratch`'s regular
//!    coordinate space (origin at the first regular tab = 0).

use super::super::render::scroll_into_view;
use super::super::*;
use super::{Spy, push_raw};

/// Build a control with hand-set tab widths so the scroll math is deterministic
/// without any ImGui text measurement. `specs` is `(name, pinned, width)`.
fn control_with_widths(specs: &[(&str, bool, f32)]) -> TabControl<Spy> {
    let mut pc: TabControl<Spy> = TabControl::new("##scroll");
    for (i, (name, pinned, _w)) in specs.iter().enumerate() {
        let mut s = Spy::new(name);
        s.pinned = *pinned;
        push_raw(&mut pc, (i + 1) as u64, s);
    }
    // Populate the width cache directly, matching the order of `tabs`.
    pc.tab_widths_cache = specs.iter().map(|(_, _, w)| *w).collect();
    // Default `tab_gap` from config.ron is 2.0; pin it explicitly so the
    // arithmetic below is independent of future ron edits.
    pc.config.tab_gap = 2.0;
    pc
}

#[test]
fn scroll_into_view_pinned_index_is_a_noop() {
    // idx 0 is pinned → scroll_target must stay untouched.
    let mut pc = control_with_widths(&[("p1", true, 36.0), ("r1", false, 100.0)]);
    pc.scroll_target = 12.5;
    scroll_into_view(&mut pc, 0, 200.0);
    assert_eq!(pc.scroll_target, 12.5, "pinned tab must not move scroll");
}

#[test]
fn scroll_into_view_regular_offset_excludes_pinned_widths() {
    // Layout: [p1(36, pinned), r1(100), r2(100), r3(100)] with gap 2.
    // The regular scroll space starts at r1 = 0. r3 is at index 3, but its
    // regular-space x must exclude the pinned p1 width entirely:
    //   tx(r3) = r1(100)+gap(2) + r2(100)+gap(2) = 204.
    // With a 150-wide scroll area, scrolling r3 into view (it sits past the
    // right edge) yields scroll_target = tx + tw - area = 204 + 100 - 150 = 154.
    let mut pc = control_with_widths(&[
        ("p1", true, 36.0),
        ("r1", false, 100.0),
        ("r2", false, 100.0),
        ("r3", false, 100.0),
    ]);
    pc.scroll_target = 0.0;
    scroll_into_view(&mut pc, 3, 150.0);
    assert!(
        (pc.scroll_target - 154.0).abs() < 0.01,
        "expected 154.0 (pinned width excluded); got {}",
        pc.scroll_target
    );
}

#[test]
fn scroll_into_view_left_clamp_brings_tab_to_origin() {
    // r1 is at regular-space x = 0. If we're scrolled right (target > 0) and
    // ask to reveal r1, the left-edge branch snaps scroll_target down to 0.
    let mut pc = control_with_widths(&[
        ("r1", false, 100.0),
        ("r2", false, 100.0),
        ("r3", false, 100.0),
    ]);
    pc.scroll_target = 90.0;
    scroll_into_view(&mut pc, 0, 150.0);
    assert_eq!(
        pc.scroll_target, 0.0,
        "left clamp should reveal r1 at origin"
    );
}

#[test]
fn scroll_into_view_already_visible_tab_does_not_move() {
    // r1 fully inside the 150-wide area at target 0 → no change.
    let mut pc = control_with_widths(&[("r1", false, 100.0), ("r2", false, 100.0)]);
    pc.scroll_target = 0.0;
    scroll_into_view(&mut pc, 0, 150.0);
    assert_eq!(pc.scroll_target, 0.0);
}

#[test]
fn scroll_into_view_out_of_range_index_is_a_noop() {
    let mut pc = control_with_widths(&[("r1", false, 100.0)]);
    pc.scroll_target = 5.0;
    scroll_into_view(&mut pc, 99, 150.0);
    assert_eq!(
        pc.scroll_target, 5.0,
        "out-of-range idx must not panic or move"
    );
}

#[test]
fn scroll_into_view_first_regular_after_pinned_targets_zero() {
    // The first regular tab (index 1, after one pinned) sits at regular-space
    // x = 0. Revealing it from a scrolled position clamps to 0 — proving the
    // pinned prefix contributes nothing to the regular origin.
    let mut pc = control_with_widths(&[
        ("p1", true, 36.0),
        ("p2", true, 36.0),
        ("r1", false, 100.0),
        ("r2", false, 100.0),
    ]);
    pc.scroll_target = 40.0;
    scroll_into_view(&mut pc, 2, 150.0);
    assert_eq!(
        pc.scroll_target, 0.0,
        "first regular tab is at scroll origin 0 regardless of pinned count"
    );
}
