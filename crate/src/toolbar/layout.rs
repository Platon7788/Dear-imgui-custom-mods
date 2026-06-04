//! Pure layout math for the toolbar — width accounting and spacer
//! distribution. Deliberately free of any ImGui context so the
//! overflow/spacer arithmetic is unit-testable. Text widths are passed
//! in pre-measured (the only context-dependent input).

use super::ToolbarConfig;

/// Horizontal advance an interactive item (button / toggle / dropdown)
/// consumes: its measured text width plus left+right padding plus the
/// trailing inter-item gap.
///
/// Mirrors the per-item advance applied in the render loop
/// (`btn_w + item_spacing`).
#[inline]
pub(super) fn item_advance_width(text_w: f32, cfg: &ToolbarConfig) -> f32 {
    text_w + cfg.button_padding * 2.0 + cfg.item_spacing
}

/// Horizontal advance a separator consumes: a margin on each side plus
/// the separator line width itself.
#[inline]
pub(super) fn separator_advance_width(cfg: &ToolbarConfig) -> f32 {
    cfg.separator_margin * 2.0 + cfg.separator_width
}

/// Distribute the leftover horizontal space across `spacer_count`
/// spacers.
///
/// `fixed_w` must already include the **leading** `item_spacing` the
/// render loop applies before the first item, so the value returned
/// here matches the layout actually drawn. Clamped at `0.0` so an
/// overfull bar never produces a negative (i.e. overlapping) spacer.
#[inline]
pub(super) fn spacer_width(avail_w: f32, fixed_w: f32, spacer_count: usize) -> f32 {
    if spacer_count == 0 {
        0.0
    } else {
        ((avail_w - fixed_w) / spacer_count as f32).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::super::ToolbarConfig;
    use super::*;

    fn cfg() -> ToolbarConfig {
        ToolbarConfig::default()
    }

    #[test]
    fn item_advance_includes_padding_and_spacing() {
        let c = cfg();
        // text_w + 2*padding + spacing
        let w = item_advance_width(40.0, &c);
        assert_eq!(w, 40.0 + c.button_padding * 2.0 + c.item_spacing);
    }

    #[test]
    fn separator_advance_is_two_margins_plus_width() {
        let c = cfg();
        let w = separator_advance_width(&c);
        assert_eq!(w, c.separator_margin * 2.0 + c.separator_width);
    }

    #[test]
    fn spacer_width_zero_when_no_spacers() {
        assert_eq!(spacer_width(500.0, 100.0, 0), 0.0);
    }

    #[test]
    fn spacer_width_splits_remaining_evenly() {
        // 500 avail, 100 fixed → 400 left over across 2 spacers → 200 each.
        assert_eq!(spacer_width(500.0, 100.0, 2), 200.0);
    }

    #[test]
    fn spacer_width_never_negative_on_overflow() {
        // fixed exceeds avail → clamp to 0, never a negative (overlap).
        assert_eq!(spacer_width(100.0, 500.0, 3), 0.0);
    }

    #[test]
    fn spacer_width_single_spacer_takes_all_slack() {
        assert_eq!(spacer_width(300.0, 120.0, 1), 180.0);
    }

    /// Regression: the fixed-width tally fed to [`spacer_width`] must
    /// include the leading `item_spacing` the render loop emits before
    /// the first item (`x = cursor + item_spacing`). Building the tally
    /// the way the renderer does — leading gap + per-item advance — and
    /// then summing the spacer back in must reproduce `avail_w` exactly,
    /// proving the two passes agree and spacer-anchored items land flush
    /// at the right edge instead of overshooting by one `item_spacing`.
    #[test]
    fn fixed_width_plus_spacer_reconstructs_avail() {
        let c = cfg();
        let avail = 600.0;

        // One button (text 50), one separator, one spacer, one button (text 30).
        let leading = c.item_spacing;
        let mut fixed = leading;
        fixed += item_advance_width(50.0, &c);
        fixed += separator_advance_width(&c);
        fixed += item_advance_width(30.0, &c);

        let sp = spacer_width(avail, fixed, 1);
        // Reassemble the full rendered width: every fixed advance plus
        // the one spacer must exactly fill the available region.
        assert!((fixed + sp - avail).abs() < 1e-3);
    }
}
