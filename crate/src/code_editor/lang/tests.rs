use super::*;

#[test]
fn test_empty_line_all_langs() {
    for lang in [
        Language::None,
        Language::Rust,
        Language::Ron,
        Language::Rhai,
        Language::Toml,
        Language::Json,
        Language::Yaml,
        Language::Xml,
        Language::Hex,
        Language::Asm,
        Language::Sql,
        Language::Diff,
        Language::Ini,
        Language::Dockerfile,
        Language::Markdown,
    ] {
        let (toks, _) = tokenize_line("", &lang, LineState::Code);
        assert!(
            toks.is_empty(),
            "non-empty tokens for {:?} on empty line",
            lang
        );
    }
}

#[test]
fn test_plain_text() {
    let (toks, bc) = tokenize_line("hello world", &Language::None, LineState::Code);
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].kind, TokenKind::Identifier);
    assert_eq!(bc, LineState::Code);
}

#[test]
fn test_definition_names() {
    assert_eq!(definition(&Language::None).name(), "Plain Text");
    assert_eq!(definition(&Language::Rust).name(), "Rust");
    assert_eq!(definition(&Language::Ron).name(), "RON");
    assert_eq!(definition(&Language::Rhai).name(), "Rhai");
    assert_eq!(definition(&Language::Toml).name(), "TOML");
    assert_eq!(definition(&Language::Json).name(), "JSON");
    assert_eq!(definition(&Language::Yaml).name(), "YAML");
    assert_eq!(definition(&Language::Xml).name(), "XML");
    assert_eq!(definition(&Language::Hex).name(), "Hex");
    assert_eq!(definition(&Language::Asm).name(), "Assembly");
    assert_eq!(definition(&Language::Sql).name(), "SQL");
    assert_eq!(definition(&Language::Diff).name(), "Diff");
    assert_eq!(definition(&Language::Ini).name(), "INI");
    assert_eq!(definition(&Language::Dockerfile).name(), "Dockerfile");
    assert_eq!(definition(&Language::Markdown).name(), "Markdown");
}

/// The two `name()` accessors (`Language::name` and the definition's own
/// `name`) must return the identical string for every built-in — a host
/// that shows one in a tab and the other in a status bar must not see two
/// different names for the same language. (ASM used to disagree.)
#[test]
fn language_and_definition_names_agree() {
    for lang in Language::builtins() {
        assert_eq!(
            lang.name(),
            definition(lang).name(),
            "name() mismatch for {lang:?}"
        );
    }
}

/// Drift guard: every menu/coverage list must stay in sync. `ALL_LANGS`
/// (used by the tiling invariant) and `builtins()` (used by the language
/// menu) must contain exactly the same set, so a newly added `Language`
/// can't silently escape either the adversarial test or the user menu.
#[test]
fn all_langs_matches_builtins() {
    for lang in Language::builtins() {
        assert!(
            ALL_LANGS.contains(lang),
            "{lang:?} is in builtins() but missing from ALL_LANGS (tiling coverage hole)"
        );
    }
    for lang in ALL_LANGS {
        assert!(
            Language::builtins().contains(lang),
            "{lang:?} is in ALL_LANGS but missing from builtins() (menu gap)"
        );
    }
}

#[test]
fn language_name_helper() {
    assert_eq!(Language::Rust.name(), "Rust");
    assert_eq!(Language::Asm.name(), "Assembly");
    assert_eq!(Language::Sql.name(), "SQL");
    assert_eq!(Language::Diff.name(), "Diff");
    assert_eq!(Language::Ini.name(), "INI");
    assert_eq!(Language::Dockerfile.name(), "Dockerfile");
    assert_eq!(Language::None.name(), "Plain Text");
}

#[test]
fn builtins_include_new_languages() {
    let all = Language::builtins();
    assert!(all.contains(&Language::Sql));
    assert!(all.contains(&Language::Diff));
    assert!(all.contains(&Language::Ini));
    assert!(all.contains(&Language::Dockerfile));
    // No Custom variant in the built-in list.
    assert!(!all.iter().any(|l| matches!(l, Language::Custom(_))));
}

#[test]
fn from_extension_mapping() {
    assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
    assert_eq!(Language::from_extension(".RS"), Some(Language::Rust));
    assert_eq!(Language::from_extension("toml"), Some(Language::Toml));
    assert_eq!(Language::from_extension("jsonc"), Some(Language::Json));
    assert_eq!(Language::from_extension("json5"), Some(Language::Json));
    assert_eq!(Language::from_extension("nasm"), Some(Language::Asm));
    assert_eq!(Language::from_extension("yml"), Some(Language::Yaml));
    assert_eq!(Language::from_extension("HTML"), Some(Language::Xml));
    assert_eq!(Language::from_extension("asm"), Some(Language::Asm));
    assert_eq!(Language::from_extension("sql"), Some(Language::Sql));
    assert_eq!(Language::from_extension("patch"), Some(Language::Diff));
    assert_eq!(Language::from_extension("diff"), Some(Language::Diff));
    assert_eq!(Language::from_extension("cfg"), Some(Language::Ini));
    assert_eq!(Language::from_extension("conf"), Some(Language::Ini));
    assert_eq!(Language::from_extension("properties"), Some(Language::Ini));
    assert_eq!(
        Language::from_extension("editorconfig"),
        Some(Language::Ini)
    );
    assert_eq!(Language::from_extension("desktop"), Some(Language::Ini));
    assert_eq!(
        Language::from_extension("dockerfile"),
        Some(Language::Dockerfile)
    );
    // Plain text and unknown → None.
    assert_eq!(Language::from_extension("txt"), None);
    assert_eq!(Language::from_extension("xyz"), None);
}

#[test]
fn from_path_detection() {
    assert_eq!(Language::from_path("src/main.rs"), Some(Language::Rust));
    assert_eq!(
        Language::from_path("C:\\proj\\schema.SQL"),
        Some(Language::Sql)
    );
    // Bare `Dockerfile` filename (no extension) is special-cased.
    assert_eq!(
        Language::from_path("Dockerfile"),
        Some(Language::Dockerfile)
    );
    assert_eq!(
        Language::from_path("docker/Dockerfile"),
        Some(Language::Dockerfile)
    );
    // `Dockerfile.<suffix>` variants (dev/prod/…) map to Dockerfile.
    assert_eq!(
        Language::from_path("Dockerfile.dev"),
        Some(Language::Dockerfile)
    );
    assert_eq!(
        Language::from_path("docker/Dockerfile.prod"),
        Some(Language::Dockerfile)
    );
    // `*.dockerfile` still resolves via the extension table.
    assert_eq!(
        Language::from_path("build.dockerfile"),
        Some(Language::Dockerfile)
    );
    // No extension and not `Dockerfile` → None.
    assert_eq!(Language::from_path("README"), None);
    assert_eq!(Language::from_path("notes.txt"), None);
}

#[test]
fn test_definition_bracket_pairs() {
    let pairs = definition(&Language::Rust).bracket_pairs();
    assert!(pairs.contains(&('(', ')')));
    assert!(pairs.contains(&('{', '}')));

    let xml_pairs = definition(&Language::Xml).bracket_pairs();
    assert!(xml_pairs.contains(&('<', '>')));

    let plain_pairs = definition(&Language::None).bracket_pairs();
    assert!(plain_pairs.is_empty());
}

#[test]
fn test_definition_comment_delimiters() {
    assert_eq!(
        definition(&Language::Rust).line_comment_prefix(),
        Some("//")
    );
    assert_eq!(definition(&Language::Toml).line_comment_prefix(), Some("#"));
    assert_eq!(definition(&Language::Yaml).line_comment_prefix(), Some("#"));
    assert_eq!(definition(&Language::Xml).line_comment_prefix(), None);
    assert_eq!(definition(&Language::None).line_comment_prefix(), None);

    assert_eq!(
        definition(&Language::Rust).block_comment_delimiters(),
        Some(("/*", "*/"))
    );
    assert_eq!(
        definition(&Language::Xml).block_comment_delimiters(),
        Some(("<!--", "-->"))
    );
    assert!(
        definition(&Language::Yaml)
            .block_comment_delimiters()
            .is_none()
    );
}

#[test]
fn test_covers_full_line_rust() {
    let line = "pub fn foo(x: i32) -> bool { true }";
    let (toks, _) = tokenize_line(line, &Language::Rust, LineState::Code);
    let total: usize = toks.iter().map(|t| t.len).sum();
    assert_eq!(total, line.len());
}

const ALL_LANGS: &[Language] = &[
    Language::None,
    Language::Rust,
    Language::Ron,
    Language::Rhai,
    Language::Toml,
    Language::Json,
    Language::Yaml,
    Language::Xml,
    Language::Hex,
    Language::Asm,
    Language::Sql,
    Language::Diff,
    Language::Ini,
    Language::Dockerfile,
    Language::Markdown,
];

/// Adversarial inputs must never panic and must always produce tokens
/// whose byte spans exactly tile the line (contiguous, no gaps, no
/// overlaps, total == line length). This is the single most important
/// invariant: the renderer slices the line by these spans, so any gap
/// or overlap is a UTF-8 panic waiting to happen.
///
/// Coverage spans EVERY `LineState` start-state — not just `Code` /
/// `BlockComment`, but the four rich resume states (`Str`, `Fenced`,
/// `HtmlRaw`, `YamlBlock`), including field variations and quote/fence
/// bytes a given language never itself produces — crossed with a corpus
/// that both terminates markers at EOL and *opens* every multi-line
/// construct. Tiling must hold for all (language × state × input) combos.
#[test]
fn all_langs_no_panic_full_coverage_all_states() {
    let samples = [
        "",
        " ",
        "\t\t",
        "/* unterminated",
        "*/ stray close",
        "\"unterminated string",
        "'unterminated char",
        "r#\"unterminated raw",
        "你好世界 — 多字节",
        "😀😀😀",
        "0x 0b 0o 1_2_3 1.2e+3 .5 1..2 1..=3",
        "#![attr(unclosed",
        "<!-- xml unclosed",
        "<![CDATA[ unclosed",
        // Opening markers that terminate the line exactly — regression
        // guard for the block-comment/CDATA/PI scanners that used to
        // over-run the line by one byte (span past EOL → tiling break).
        "/*",
        "x /*",
        "<!--",
        "<![CDATA[",
        "<?",
        "key: value # comment",
        "DE AD BE EF GG",
        "&amp; <b>x</b>",
        "let x = b'\\x41'; // mix",
        "\\\\\\ stray backslashes",
        ":::,,,...===!!!",
        // Multi-line OPENERS — each produces a non-Code carry; feeding
        // these plus the resume states below exercises every tokenizer's
        // resume path (where the new multi-line work is most fragile).
        "a = \"\"\"triple basic open",
        "b = '''triple literal open",
        "s = r#\"raw open no close",
        "key: |",
        "block: >-",
        "```rust",
        "~~~",
        "<script>var x = 1 < 2;",
        "<style>.a { color: red }",
        "`backtick ${world",
        "text with ]]> stray close",
        "close triple \"\"\" here",
        "end raw \"# here",
        "fence close ```",
    ];
    // Representative start-states: the two plain ones plus every rich
    // resume variant, including quote/fence/field values some languages
    // never emit — tiling must survive an unexpected carry, not just an
    // in-family one.
    let states = [
        LineState::Code,
        LineState::BlockComment(1),
        LineState::BlockComment(3),
        LineState::Str {
            quote: b'"',
            raw: false,
            hashes: 0,
            triple: false,
        },
        LineState::Str {
            quote: b'"',
            raw: false,
            hashes: 0,
            triple: true,
        },
        LineState::Str {
            quote: b'\'',
            raw: false,
            hashes: 0,
            triple: true,
        },
        LineState::Str {
            quote: b'"',
            raw: true,
            hashes: 2,
            triple: false,
        },
        LineState::Str {
            quote: b'`',
            raw: false,
            hashes: 0,
            triple: false,
        },
        LineState::Fenced {
            fence: b'`',
            count: 3,
        },
        LineState::Fenced {
            fence: b'~',
            count: 4,
        },
        LineState::HtmlRaw { is_style: false },
        LineState::HtmlRaw { is_style: true },
        LineState::YamlBlock { indent: 0 },
        LineState::YamlBlock { indent: 4 },
    ];
    for lang in ALL_LANGS {
        for &state in &states {
            for s in &samples {
                let (toks, _) = tokenize_line(s, lang, state);
                let mut pos = 0usize;
                for t in &toks {
                    assert_eq!(
                        t.start, pos,
                        "non-contiguous span in {lang:?} (state={state:?}) on {s:?}: {toks:?}"
                    );
                    // Spans must land on char boundaries so the renderer
                    // can slice without panicking.
                    assert!(
                        s.is_char_boundary(t.start) && s.is_char_boundary(t.start + t.len),
                        "span not on char boundary in {lang:?} (state={state:?}) on {s:?}"
                    );
                    pos += t.len;
                }
                assert_eq!(
                    pos,
                    s.len(),
                    "span total != line len in {lang:?} (state={state:?}) on {s:?}: {toks:?}"
                );
            }
        }
    }
}

/// Block-comment carry state must be idempotent in the sense that a
/// language advertising no block comments never reports "still in a
/// block comment".
#[test]
fn non_block_comment_langs_never_carry_state() {
    for lang in [Language::Toml, Language::Yaml, Language::Asm, Language::Hex] {
        for line in ["/* not a comment here */", "x */", "/* x"] {
            let (_, carry) = tokenize_line(line, &lang, LineState::Code);
            assert_eq!(
                carry,
                LineState::Code,
                "{lang:?} unexpectedly carried block-comment state"
            );
        }
    }
}
