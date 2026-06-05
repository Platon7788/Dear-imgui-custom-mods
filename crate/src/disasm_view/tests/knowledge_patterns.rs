//! Catalogue-tier authoring guards: antidisasm + compiler + mnemonic.

use super::assert_tiers_authored;

#[test]
fn antidisasm_tiers_authored_for_every_entry() {
    use crate::disasm_view::antidisasm;
    let probes: Vec<(&str, antidisasm::AntiDisasmTrick)> = vec![
        // TRICK_PUSH_RET_CFG
        (
            "push-ret",
            antidisasm::detect(&antidisasm::AntiDisasmContext {
                prev: None,
                current: ("push", "0x401000"),
                next: Some(("ret", "")),
            })
            .unwrap(),
        ),
        // TRICK_OPAQUE_AFTER_MOV
        (
            "opaque-mov",
            antidisasm::detect(&antidisasm::AntiDisasmContext {
                prev: Some(("mov", "eax, 5")),
                current: ("cmp", "eax, 5"),
                next: Some(("jne", "0x401000")),
            })
            .unwrap(),
        ),
        // TRICK_SMC_RIP_WRITE
        (
            "smc-rip",
            antidisasm::detect(&antidisasm::AntiDisasmContext {
                prev: None,
                current: ("xor", "[rip+0x100], eax"),
                next: None,
            })
            .unwrap(),
        ),
        // TRICK_HYPERVISOR_BIT
        (
            "hv-bit",
            antidisasm::detect(&antidisasm::AntiDisasmContext {
                prev: Some(("cpuid", "")),
                current: ("bt", "ecx, 31"),
                next: None,
            })
            .unwrap(),
        ),
        // TRICK_HYPERVISOR_VENDOR
        (
            "hv-vendor",
            antidisasm::detect(&antidisasm::AntiDisasmContext {
                prev: Some(("mov", "eax, 0x40000000")),
                current: ("cpuid", ""),
                next: None,
            })
            .unwrap(),
        ),
        // TRICK_TRAP_FLAG_ARM
        (
            "trap-flag",
            antidisasm::detect(&antidisasm::AntiDisasmContext {
                prev: Some(("or", "[rsp], 0x100")),
                current: ("popf", ""),
                next: None,
            })
            .unwrap(),
        ),
        // TRICK_JMP_INTO_INSTRUCTION
        (
            "jmp-into-next",
            antidisasm::detect(&antidisasm::AntiDisasmContext {
                prev: None,
                current: ("jmp", "short $+2"),
                next: None,
            })
            .unwrap(),
        ),
        // TRICK_RDTSC_DELTA
        (
            "rdtsc-delta",
            antidisasm::detect(&antidisasm::AntiDisasmContext {
                prev: Some(("rdtsc", "")),
                current: ("sub", "eax, [rsp+8]"),
                next: None,
            })
            .unwrap(),
        ),
    ];
    assert_tiers_authored("antidisasm", &probes, |t| &t.tiers);
}

#[test]
fn compiler_tiers_authored_for_every_entry() {
    use crate::disasm_view::abi::Abi;
    use crate::disasm_view::compiler;
    let probes: Vec<(&str, compiler::CompilerPattern)> = vec![
        // PATTERN_PEB_WIN64
        (
            "peb-win64",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("mov", "rax, gs:[0x60]"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_PEB_WIN32
        (
            "peb-win32",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("mov", "eax, fs:[0x30]"),
                next: None,
                abi: Abi::Cdecl,
            })
            .unwrap(),
        ),
        // PATTERN_TEB_SELF_WIN64
        (
            "teb-win64",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("mov", "rax, gs:[0x30]"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_TIB_SELF_WIN32
        (
            "tib-win32",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("mov", "eax, fs:[0x18]"),
                next: None,
                abi: Abi::Cdecl,
            })
            .unwrap(),
        ),
        // PATTERN_CHKSTK
        (
            "chkstk",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("call", "__chkstk"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_WIN64_LEAF_FRAME
        (
            "leaf-frame",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("sub", "rsp, 0x28"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_VTABLE_CALL
        (
            "vtable-call",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("call", "qword ptr [rax+0x10]"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_VTABLE_CALL_SLOT0
        (
            "vtable-slot0",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("call", "qword ptr [rcx]"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_SEH_INSTALL_WIN32
        (
            "seh-install",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("mov", "dword ptr fs:[0], esp"),
                next: None,
                abi: Abi::Cdecl,
            })
            .unwrap(),
        ),
        // PATTERN_SECURITY_COOKIE
        (
            "security-cookie",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("mov", "rax, [__security_cookie]"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_ATOMIC_CAS
        (
            "atomic-cas",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("cmpxchg", "[rax], rdx"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_ATOMIC_RMW
        (
            "atomic-rmw",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("lock", ""),
                next: Some(("xadd", "[rax], rdx")),
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_CPUID_DETECT
        (
            "cpuid-detect",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("cpuid", ""),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_INDIRECT_TAIL_JMP
        (
            "indirect-tail",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("jmp", "rax"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
        // PATTERN_IAT_THUNK
        (
            "iat-thunk",
            compiler::detect(&compiler::CompilerContext {
                prev: None,
                current: ("jmp", "qword ptr [rip+0x1234]"),
                next: None,
                abi: Abi::Win64,
            })
            .unwrap(),
        ),
    ];
    assert_tiers_authored("compiler", &probes, |p| &p.tiers);
}

#[test]
fn mnemonic_top_30_tiers_authored() {
    // Top-30 most-common mnemonics were upgraded to entry_t /
    // entry_gt — assert each of those has Compact + Educational
    // description tiers populated.
    use crate::disasm_view::mnemonic;
    let top_30 = [
        "mov", "lea", "xchg", "push", "pop", "call", "ret", "jmp", "je", "jne", "jz", "jnz", "jg",
        "jl", "ja", "jb", "jae", "jbe", "cmp", "test", "add", "sub", "inc", "dec", "xor", "and",
        "or", "mul", "imul", "div",
    ];
    for mn in top_30 {
        let info = mnemonic::lookup(mn)
            .unwrap_or_else(|| panic!("top-30 mnemonic {mn:?} missing from catalogue"));
        assert!(
            !info.description_tiers.compact_en.is_empty(),
            "mnemonic::{mn}: description compact_en empty"
        );
        assert!(
            !info.description_tiers.compact_ru.is_empty(),
            "mnemonic::{mn}: description compact_ru empty"
        );
        assert!(
            !info.description_tiers.educational_en.is_empty(),
            "mnemonic::{mn}: description educational_en empty"
        );
        assert!(
            !info.description_tiers.educational_ru.is_empty(),
            "mnemonic::{mn}: description educational_ru empty"
        );
    }
}
