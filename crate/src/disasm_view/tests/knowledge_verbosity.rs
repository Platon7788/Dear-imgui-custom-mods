//! Verbosity-tier dispatch guards (`description_for` / branch hints).

#[test]
fn branch_description_for_returns_distinct_text_per_verbosity() {
    // BranchHint::description_for is format!()-based, not
    // catalogue-driven — confirm each of the three verbosity
    // tiers produces distinct text for each direction.
    use crate::disasm_view::branch;
    for direction_probe in [
        branch::classify(0x401000, 0x401040), // forward
        branch::classify(0x401040, 0x401000), // backward
        branch::classify(0x401000, 0x401000), // self
    ] {
        let compact = direction_probe.description_for(
            crate::i18n::Locale::En.into(),
            super::HintVerbosity::Compact,
        );
        let standard = direction_probe.description_for(
            crate::i18n::Locale::En.into(),
            super::HintVerbosity::Standard,
        );
        let educational = direction_probe.description_for(
            crate::i18n::Locale::En.into(),
            super::HintVerbosity::Educational,
        );
        assert_ne!(
            compact, standard,
            "{direction_probe:?}: compact == standard"
        );
        assert!(
            educational.len() >= standard.len(),
            "{direction_probe:?}: educational shorter than standard"
        );
        // RU side same shape.
        let compact_ru = direction_probe.description_for(
            crate::i18n::Locale::Ru.into(),
            super::HintVerbosity::Compact,
        );
        let standard_ru = direction_probe.description_for(
            crate::i18n::Locale::Ru.into(),
            super::HintVerbosity::Standard,
        );
        assert_ne!(
            compact_ru, standard_ru,
            "RU {direction_probe:?}: compact == standard"
        );
    }
}

#[test]
fn description_for_returns_tier_strings_when_authored() {
    use crate::disasm_view::idiom;
    let ctx = idiom::InstructionContext {
        prev: None,
        current: ("xor", "eax, eax"),
        next: None,
    };
    let hit = idiom::detect(&ctx).expect("zero-reg idiom");
    let compact_en = hit.description_for(
        crate::i18n::Locale::En.into(),
        super::HintVerbosity::Compact,
    );
    let standard_en = hit.description_for(
        crate::i18n::Locale::En.into(),
        super::HintVerbosity::Standard,
    );
    let educational_en = hit.description_for(
        crate::i18n::Locale::En.into(),
        super::HintVerbosity::Educational,
    );
    // Standard always returns the en field directly.
    assert_eq!(standard_en, hit.en);
    // Compact must differ from Standard for an authored entry
    // (shorter, tightly-worded telegram).
    assert_ne!(
        compact_en, standard_en,
        "compact tier should be distinct from standard when authored"
    );
    // Educational must be at least as long as Standard for an
    // authored entry (3-5 sentences vs 1-2).
    assert!(
        educational_en.len() >= standard_en.len(),
        "educational tier should not be shorter than standard"
    );
}

#[test]
fn description_for_falls_back_when_tier_unauthored() {
    // Construct a manual Idiom with EMPTY tiers — fallback
    // must return the Standard `en` / `ru`.
    let test_idiom = crate::disasm_view::idiom::Idiom {
        en: "Standard EN text",
        ru: "Standard RU text",
        tiers: crate::disasm_view::HintTiers::EMPTY,
    };
    // All three tiers must return the standard strings.
    for v in [
        super::HintVerbosity::Compact,
        super::HintVerbosity::Standard,
        super::HintVerbosity::Educational,
    ] {
        assert_eq!(
            test_idiom.description_for(crate::i18n::Locale::En.into(), v),
            "Standard EN text"
        );
        assert_eq!(
            test_idiom.description_for(crate::i18n::Locale::Ru.into(), v),
            "Standard RU text"
        );
    }
}

#[test]
fn backwards_compat_old_description_method_unchanged() {
    // Guard: the pre-verbosity `.description(locale)` method
    // must always return the Standard tier (`en`/`ru`) so
    // downstream callers that haven't adopted
    // `description_for` see no behaviour change.
    use crate::disasm_view::idiom;
    let ctx = idiom::InstructionContext {
        prev: None,
        current: ("xor", "eax, eax"),
        next: None,
    };
    let hit = idiom::detect(&ctx).unwrap();
    assert_eq!(hit.description(crate::i18n::Locale::En.into()), hit.en);
    assert_eq!(hit.description(crate::i18n::Locale::Ru.into()), hit.ru);
}
