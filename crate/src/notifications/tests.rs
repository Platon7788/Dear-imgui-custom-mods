//! Unit tests for the notification center.
//!
//! Covers the public API (push / dismiss / counts / builders), the pure
//! lifecycle helpers from `logic` (animation ramps, timer ticking, reaping,
//! and the event-driven frame-demand decision), the easing curves, the DDD
//! ron config defaults + round-trip, and enum invariants — none of which
//! require a live ImGui `Ui`. Split out of `mod.rs` so every file in the
//! module stays under the 500-line cap.

use super::*;

#[test]
fn push_assigns_unique_ids() {
    let mut c = NotificationCenter::new();
    let a = c.push(Notification::info("a"));
    let b = c.push(Notification::info("b"));
    assert_ne!(a, b);
    assert_eq!(c.count(), 2);
}

#[test]
fn dismiss_sets_flag() {
    let mut c = NotificationCenter::new();
    let id = c.push(Notification::info("x"));
    c.dismiss(id);
    assert!(c.queue.iter().find(|n| n.id == id).unwrap().dismissing);
}

#[test]
fn dismiss_all_sets_every_flag() {
    let mut c = NotificationCenter::new();
    c.push(Notification::info("a"));
    c.push(Notification::error("b"));
    c.dismiss_all();
    assert!(c.queue.iter().all(|n| n.dismissing));
}

#[test]
fn severity_labels() {
    assert_eq!(Severity::Info.label(), "Info");
    assert_eq!(Severity::Error.label(), "Error");
}

#[test]
fn placement_grows_up() {
    assert!(Placement::BottomRight.grows_up());
    assert!(Placement::BottomLeft.grows_up());
    assert!(Placement::BottomCenter.grows_up());
    assert!(!Placement::TopRight.grows_up());
}

#[test]
fn builder_sticky_sets_duration() {
    let n = Notification::warning("w").sticky();
    assert_eq!(n.duration, Duration::Sticky);
}

#[test]
fn builder_custom_color_override() {
    let n = Notification::info("x").with_custom_color([1.0, 0.5, 0.0, 1.0]);
    assert_eq!(n.custom_color, Some([1.0, 0.5, 0.0, 1.0]));
}

#[test]
fn builder_actions_accumulate() {
    let n = Notification::info("x")
        .with_action(1, "One")
        .with_action(2, "Two");
    assert_eq!(n.actions.len(), 2);
    assert_eq!(n.actions[0].id, 1);
    assert_eq!(n.actions[1].label, "Two");
}

// ── Pure animation / timer helpers (no Ui needed) ─────────────────────────

#[test]
fn dismiss_all_returns_count_and_skips_already_dismissing() {
    let mut c = NotificationCenter::new();
    c.push(Notification::info("a"));
    c.push(Notification::info("b"));
    let id = c.push(Notification::info("c"));
    c.dismiss(id); // c already dismissing
    assert_eq!(c.dismiss_all(), 2);
    assert_eq!(c.active_count(), 0);
}

#[test]
fn active_count_excludes_dismissing() {
    let mut c = NotificationCenter::new();
    c.push(Notification::info("a"));
    let id = c.push(Notification::info("b"));
    c.dismiss(id);
    assert_eq!(c.count(), 2, "count includes fading toasts");
    assert_eq!(c.active_count(), 1, "active excludes them");
}

#[test]
fn push_assigns_win_id() {
    let mut c = NotificationCenter::new();
    let id = c.push(Notification::info("a"));
    let n = c.queue.iter().find(|n| n.id == id).unwrap();
    assert_eq!(n.win_id, format!("##toast_{id}"));
}

#[test]
fn advance_animations_ramps_enter_then_exit() {
    let mut q = vec![Notification::info("x")];
    q[0].id = 1;
    // Half the entry — enter_t goes to ~0.5.
    advance_animations(&mut q, 0.125, 0.25);
    assert!((q[0].enter_t - 0.5).abs() < 1e-3);
    assert_eq!(q[0].exit_t, 0.0, "exit doesn't move while not dismissing");
    // Finish entry, then start dismissing.
    advance_animations(&mut q, 0.125, 0.25);
    assert_eq!(q[0].enter_t, 1.0);
    q[0].dismissing = true;
    advance_animations(&mut q, 0.125, 0.25);
    assert!((q[0].exit_t - 0.5).abs() < 1e-3);
}

#[test]
fn tick_timers_dismisses_on_budget_exceed() {
    let mut q = vec![Notification::info("x").with_duration_secs(2.0)];
    q[0].id = 1;
    tick_timers(&mut q, 1.0, &[], false);
    assert!(!q[0].dismissing, "1s of 2s — still alive");
    tick_timers(&mut q, 1.5, &[], false); // total 2.5s
    assert!(q[0].dismissing, "2.5s ≥ 2s budget — should dismiss");
}

#[test]
fn tick_timers_pauses_while_hovered() {
    let mut q = vec![Notification::info("x").with_duration_secs(2.0)];
    q[0].id = 7;
    // Hovered + pause_on_hover = true → elapsed shouldn't tick.
    tick_timers(&mut q, 5.0, &[(7, true)], true);
    assert_eq!(q[0].elapsed, 0.0);
    assert!(!q[0].dismissing);
    // pause_on_hover = false ignores hover flag.
    tick_timers(&mut q, 5.0, &[(7, true)], false);
    assert!(q[0].dismissing);
}

#[test]
fn reap_dismissed_emits_event_and_drops() {
    let mut q = vec![Notification::info("a"), Notification::info("b")];
    q[0].id = 1;
    q[0].dismissing = true;
    q[0].exit_t = 1.0;
    q[1].id = 2;
    // q[1] not dismissing → kept.
    let mut events = Vec::new();
    reap_dismissed(&mut q, AnimationKind::Fade, &mut events);
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].id, 2);
    assert_eq!(events, vec![NotificationEvent::Dismissed(1)]);
}

#[test]
fn reap_dismissed_keeps_mid_animation() {
    // exit_t < 1.0 with Fade animation → still mid-fade, must not reap.
    let mut q = vec![Notification::info("x")];
    q[0].id = 9;
    q[0].dismissing = true;
    q[0].exit_t = 0.5;
    let mut events = Vec::new();
    reap_dismissed(&mut q, AnimationKind::Fade, &mut events);
    assert_eq!(q.len(), 1, "0.5 < 1.0, should keep");
    assert!(events.is_empty());
}

#[test]
fn reap_dismissed_drops_immediately_when_no_animation() {
    // AnimationKind::None: drop on first reap regardless of exit_t.
    let mut q = vec![Notification::info("x")];
    q[0].id = 11;
    q[0].dismissing = true;
    q[0].exit_t = 0.0;
    let mut events = Vec::new();
    reap_dismissed(&mut q, AnimationKind::None, &mut events);
    assert!(q.is_empty());
    assert_eq!(events, vec![NotificationEvent::Dismissed(11)]);
}

// ── Frame-demand decision (regression for the sticky-pins-host bug) ───────

#[test]
fn needs_frame_false_for_settled_sticky_toast() {
    // Regression: a fully-entered Sticky toast must NOT demand a frame —
    // nothing about it changes between input events, so an event-driven
    // host is free to sleep. The previous `!n.dismissing` condition kept
    // the host pinned at full refresh forever while any sticky toast lived.
    let mut q = vec![Notification::info("x").sticky()];
    q[0].id = 1;
    q[0].enter_t = 1.0; // entry finished
    assert!(
        !needs_frame(&q),
        "settled sticky toast should let the host sleep"
    );
}

#[test]
fn needs_frame_true_while_entering() {
    // Mid-enter animation always demands a frame, even when sticky.
    let mut q = vec![Notification::info("x").sticky()];
    q[0].id = 1;
    q[0].enter_t = 0.5;
    assert!(needs_frame(&q));
}

#[test]
fn needs_frame_true_for_live_timed_toast() {
    // A live (non-dismissing) Timed toast keeps demanding frames so its
    // countdown / progress bar keeps advancing.
    let mut q = vec![Notification::info("x").with_duration_secs(4.0)];
    q[0].id = 1;
    q[0].enter_t = 1.0;
    assert!(needs_frame(&q));
}

#[test]
fn needs_frame_true_while_exiting_then_false_once_idle() {
    // Mid-exit fade demands frames; a dismissing toast whose exit has fully
    // completed (about to be reaped) does not.
    let mut q = vec![Notification::info("x").sticky()];
    q[0].id = 1;
    q[0].enter_t = 1.0;
    q[0].dismissing = true;
    q[0].exit_t = 0.4;
    assert!(needs_frame(&q), "mid-exit fade needs frames");
    q[0].exit_t = 1.0;
    assert!(!needs_frame(&q), "finished exit no longer animates");
}

#[test]
fn needs_frame_false_for_empty_queue() {
    assert!(!needs_frame(&[]));
}

// ── Easing curve invariants ───────────────────────────────────────────────

#[test]
fn easing_endpoints_and_monotonic() {
    assert!((ease_out_cubic(0.0)).abs() < 1e-6);
    assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-6);
    assert!((ease_in_cubic(0.0)).abs() < 1e-6);
    assert!((ease_in_cubic(1.0) - 1.0).abs() < 1e-6);
    // Monotonic non-decreasing across the unit interval.
    let mut prev_out = -1.0;
    let mut prev_in = -1.0;
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let o = ease_out_cubic(t);
        let n = ease_in_cubic(t);
        assert!(o >= prev_out, "ease_out_cubic not monotonic at t={t}");
        assert!(n >= prev_in, "ease_in_cubic not monotonic at t={t}");
        prev_out = o;
        prev_in = n;
    }
}

// ── DDD config: ron defaults parse + round-trip ───────────────────────────

#[test]
fn config_ron_defaults_parse() {
    let cfg = CenterConfig::default();
    assert_eq!(cfg.placement, Placement::TopRight);
    assert_eq!(cfg.max_visible, 5);
    assert_eq!(cfg.width, 340.0);
    assert_eq!(cfg.animation, AnimationKind::Fade);
    assert!(cfg.pause_on_hover);
    assert!(
        cfg.colors_override.is_none(),
        "runtime palette is skip+default"
    );
}

#[test]
fn config_round_trips_through_ron() {
    let cfg = CenterConfig::new()
        .with_placement(Placement::BottomCenter)
        .with_max_visible(3)
        .with_animation(AnimationKind::SlideIn)
        .with_width(420.0);
    let s = ron::to_string(&cfg).expect("serialize");
    let back: CenterConfig = ron::from_str(&s).expect("deserialize");
    assert_eq!(back.placement, Placement::BottomCenter);
    assert_eq!(back.max_visible, 3);
    assert_eq!(back.animation, AnimationKind::SlideIn);
    assert_eq!(back.width, 420.0);
}

#[test]
fn with_max_visible_clamps_to_one() {
    // Builder guards against a zero/empty visible slice.
    let cfg = CenterConfig::new().with_max_visible(0);
    assert_eq!(cfg.max_visible, 1);
}

// ── Enum defaults / placement orientation helpers ─────────────────────────

#[test]
fn enum_defaults() {
    assert_eq!(Severity::default(), Severity::Info);
    assert_eq!(Placement::default(), Placement::TopRight);
    assert_eq!(AnimationKind::default(), AnimationKind::Fade);
    assert_eq!(Duration::default(), Duration::Timed(4.0));
}

#[test]
fn placement_slide_directions() {
    assert!(Placement::TopLeft.slides_from_left());
    assert!(Placement::BottomLeft.slides_from_left());
    assert!(!Placement::TopLeft.slides_from_right());
    assert!(Placement::TopRight.slides_from_right());
    assert!(Placement::BottomRight.slides_from_right());
    // Center placements slide from neither edge.
    assert!(!Placement::TopCenter.slides_from_left());
    assert!(!Placement::TopCenter.slides_from_right());
    assert!(!Placement::BottomCenter.slides_from_left());
    assert!(!Placement::BottomCenter.slides_from_right());
}

#[test]
fn severity_accent_picks_matching_field() {
    let c = NotificationColors::dark();
    assert_eq!(Severity::Info.accent(&c), c.info);
    assert_eq!(Severity::Success.accent(&c), c.success);
    assert_eq!(Severity::Warning.accent(&c), c.warning);
    assert_eq!(Severity::Error.accent(&c), c.error);
    assert_eq!(Severity::Debug.accent(&c), c.debug);
}

// ── push() id assignment: wrap-around collision avoidance ─────────────────

#[test]
fn push_skips_id_already_in_flight_after_wrap() {
    // Simulate counter wrap: a long-lived toast already occupies an id while
    // next_id is rewound to it. push() must skip the live id rather than
    // colliding, so both toasts keep distinct, addressable ids.
    let mut c = NotificationCenter::new();
    let first = c.push(Notification::info("sticky").sticky());
    // Force next_id to collide with the live toast.
    c.next_id = first;
    let second = c.push(Notification::info("fresh"));
    assert_ne!(first, second, "push must not reuse a live id");
    assert_eq!(c.count(), 2);
}

#[test]
fn push_pre_formats_action_ids() {
    let mut c = NotificationCenter::new();
    let id = c.push(
        Notification::info("a")
            .with_action(1, "One")
            .with_action(2, "Two"),
    );
    let n = c.queue.iter().find(|n| n.id == id).unwrap();
    assert_eq!(n.action_ids.len(), 2, "one pre-built id per action");
    assert_eq!(n.action_ids[0], format!("One##act_{id}_0"));
    assert_eq!(n.action_ids[1], format!("Two##act_{id}_1"));
    assert_eq!(n.close_id, format!("##close_{id}"));
}
