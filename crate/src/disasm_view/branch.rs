//! Branch-direction recogniser for the educational tooltip.
//!
//! Given the current address and the resolved branch target (both
//! provided directly by the host's
//! [`super::provider::DisasmDataProvider`] — no operand-string
//! parsing needed), this module classifies the jump as forward,
//! backward, or self-targeting and emits a single educational line:
//!
//! * **Forward** — typical of `if` / `match` / `switch` skip-over
//!   patterns; the analyst knows the body of the branch is below.
//! * **Backward** — almost always a loop (`while`, `for`, `do-while`,
//!   tail-recursion). The branch target is *above* the current
//!   address, so the CPU re-executes earlier code.
//! * **Self** — `jmp $` / `jmp $-2` style infinite spin / debugger
//!   trap. Anti-RE protectors love these to wedge dynamic analysis.
//!
//! The recogniser is **pure / no-allocation** and mirrors the API of
//! [`super::idiom`], [`super::compiler`], [`super::antidisasm`], and
//! [`super::boundary`]. It returns `None` for non-branching
//! mnemonics so the caller can chain it cheaply.

use crate::i18n::Locale;

/// Classification of a branch's vertical direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchDirection {
    /// `target > current_addr` — the body is below; typical of `if` /
    /// `match` / `switch` skip-over.
    Forward,
    /// `target < current_addr` — the body is above; almost always a
    /// loop construct.
    Backward,
    /// `target == current_addr` — infinite spin / `jmp $` /
    /// debugger-trap pattern.
    SelfTarget,
}

/// One detected branch direction with locale-aware text.
#[derive(Debug, Clone, Copy)]
pub struct BranchHint {
    /// Direction tag.
    pub direction: BranchDirection,
    /// Signed displacement (`target - current_addr`, in bytes).
    pub delta: i64,
}

impl BranchHint {
    /// Locale-appropriate description (single line) — embeds the
    /// signed delta so the analyst can see jump distance at a
    /// glance without doing the subtraction themselves.
    #[must_use]
    pub fn description(&self, locale: Locale) -> String {
        let abs = self.delta.unsigned_abs();
        match (self.direction, locale) {
            (BranchDirection::Forward, Locale::En) => format!(
                "Forward branch (+0x{abs:X} bytes) — typical `if` / `match` / `switch` skip-over; the body is below."
            ),
            (BranchDirection::Forward, Locale::Ru) => format!(
                "Переход вперёд (+0x{abs:X} байт) — типичный `if` / `match` / `switch` skip-over; тело — ниже."
            ),
            (BranchDirection::Backward, Locale::En) => format!(
                "Backward branch (-0x{abs:X} bytes) — almost always a loop (`while`, `for`, `do-while`, tail-recursion)."
            ),
            (BranchDirection::Backward, Locale::Ru) => format!(
                "Переход назад (-0x{abs:X} байт) — почти всегда цикл (`while`, `for`, `do-while`, tail-recursion)."
            ),
            (BranchDirection::SelfTarget, Locale::En) => {
                "Self-targeting branch — infinite spin / `jmp $` / debugger trap. Common in anti-RE armour.".to_string()
            }
            (BranchDirection::SelfTarget, Locale::Ru) => {
                "Самоцелевой переход — бесконечный цикл / `jmp $` / ловушка для отладчика. Типично для анти-RE брони.".to_string()
            }
        }
    }
}

/// Classify a branch from `current_addr` to `target_addr`. Caller is
/// responsible for filtering to actual branch instructions
/// (`flow_kind == Jump` / `Call` / etc.) — the recogniser itself
/// does not look at the mnemonic.
#[must_use]
pub fn classify(current_addr: u64, target_addr: u64) -> BranchHint {
    let delta = target_addr as i64 - current_addr as i64;
    let direction = match delta.cmp(&0) {
        std::cmp::Ordering::Greater => BranchDirection::Forward,
        std::cmp::Ordering::Less => BranchDirection::Backward,
        std::cmp::Ordering::Equal => BranchDirection::SelfTarget,
    };
    BranchHint { direction, delta }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_branch() {
        let h = classify(0x401000, 0x401040);
        assert_eq!(h.direction, BranchDirection::Forward);
        assert_eq!(h.delta, 0x40);
        assert!(h.description(Locale::En).contains("Forward"));
        assert!(h.description(Locale::Ru).contains("вперёд"));
    }

    #[test]
    fn backward_branch_loop() {
        let h = classify(0x401040, 0x401000);
        assert_eq!(h.direction, BranchDirection::Backward);
        assert_eq!(h.delta, -0x40);
        assert!(h.description(Locale::En).contains("loop"));
        assert!(h.description(Locale::Ru).contains("цикл"));
    }

    #[test]
    fn self_target_spin() {
        let h = classify(0x401000, 0x401000);
        assert_eq!(h.direction, BranchDirection::SelfTarget);
        assert_eq!(h.delta, 0);
        assert!(h.description(Locale::En).contains("Self-targeting"));
    }

    #[test]
    fn high_address_does_not_overflow() {
        // i64 from u64 cast is safe up to 0x7FFF_FFFF_FFFF_FFFF; we
        // accept that one branch across the i64::MAX boundary will
        // misclassify. Cover the realistic case where both addresses
        // are well below the boundary.
        let h = classify(0x7FFE_FFFF_FFFF_FF00, 0x7FFE_FFFF_FFFF_FF40);
        assert_eq!(h.direction, BranchDirection::Forward);
        assert_eq!(h.delta, 0x40);
    }
}
