//! Catalogue-tier authoring guards: idiom + boundary recognisers.

use super::assert_tiers_authored;

#[test]
fn idiom_compact_and_educational_authored_for_every_entry() {
    // Catalogue-completeness guard: every IDIOM_ in the
    // catalogue must have authored Compact + Educational tiers.
    // Each probe is a representative `InstructionContext` for
    // ONE catalogue constant — adding an idiom requires adding
    // a probe here. The guard fires (panic) if any tier slot
    // is still empty.
    use crate::disasm_view::idiom;
    let probes: Vec<(&str, idiom::Idiom)> = vec![
        // IDIOM_PROLOGUE
        (
            "prologue",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("push", "rbp"),
                next: Some(("mov", "rbp, rsp")),
            })
            .unwrap(),
        ),
        // IDIOM_EPILOGUE_LEAVE_RET
        (
            "epilogue-leave-ret",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("leave", ""),
                next: Some(("ret", "")),
            })
            .unwrap(),
        ),
        // IDIOM_EPILOGUE_POP_RET
        (
            "epilogue-pop-ret",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("pop", "rbp"),
                next: Some(("ret", "")),
            })
            .unwrap(),
        ),
        // IDIOM_STACK_ALLOC
        (
            "stack-alloc",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("sub", "rsp, 0x20"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_STACK_FREE
        (
            "stack-free",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("add", "rsp, 0x20"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_ZERO_REG
        (
            "zero-reg",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("xor", "eax, eax"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_NULL_CHECK
        (
            "null-check",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("test", "rax, rax"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_CMP_BRANCH
        (
            "cmp-branch",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("cmp", "rax, 0"),
                next: Some(("je", "0x401000")),
            })
            .unwrap(),
        ),
        // IDIOM_GET_IP
        (
            "get-ip",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("call", "$+5"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_SBB_MASK
        (
            "sbb-mask",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("sbb", "eax, eax"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_ROP_GADGET
        (
            "rop-gadget",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("pop", "rax"),
                next: Some(("ret", "")),
            })
            .unwrap(),
        ),
        // IDIOM_PUSH_ARG_CALL
        (
            "push-arg-call",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("push", "0x1234"),
                next: Some(("call", "foo")),
            })
            .unwrap(),
        ),
        // IDIOM_REG_ARG_CALL — mov rcx, ...; call ...
        (
            "reg-arg-call",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("mov", "rcx, 0x1234"),
                next: Some(("call", "foo")),
            })
            .unwrap(),
        ),
        // IDIOM_RDTSC_PAIR
        (
            "rdtsc-pair",
            idiom::detect(&idiom::InstructionContext {
                prev: Some(("rdtsc", "")),
                current: ("rdtsc", ""),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_INT3_BP
        (
            "int3",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("int", "3"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_INT2D
        (
            "int2d",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("int", "2D"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_LEA_SELF_NOP
        (
            "lea-self-nop",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("lea", "rax, [rax+0]"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_MOV_SELF_NOP
        (
            "mov-self-nop",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("mov", "rax, rax"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_ROTATE_BY_ZERO_NOP
        (
            "rotate-zero-nop",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("rol", "eax, 0"),
                next: None,
            })
            .unwrap(),
        ),
        // IDIOM_XCHG_EAX_NOP
        (
            "xchg-eax-nop",
            idiom::detect(&idiom::InstructionContext {
                prev: None,
                current: ("xchg", "eax, eax"),
                next: None,
            })
            .unwrap(),
        ),
    ];
    let pairs: Vec<(&str, idiom::Idiom)> = probes;
    let refs: Vec<(&str, idiom::Idiom)> = pairs;
    assert_tiers_authored("idiom", &refs, |i| &i.tiers);
}

#[test]
fn boundary_tiers_authored_for_every_entry() {
    use crate::disasm_view::boundary;
    let probes: Vec<(&str, boundary::Boundary)> = vec![
        // BOUNDARY_FUNCTION_START_FRAMED
        (
            "framed",
            boundary::detect(&boundary::BoundaryContext {
                prev: None,
                current: ("push", "rbp"),
                next: Some(("mov", "rbp, rsp")),
            })
            .unwrap(),
        ),
        // BOUNDARY_FUNCTION_START_CET
        (
            "cet",
            boundary::detect(&boundary::BoundaryContext {
                prev: None,
                current: ("endbr64", ""),
                next: None,
            })
            .unwrap(),
        ),
        // BOUNDARY_EPILOGUE_LEAVE_RET
        (
            "leave-ret",
            boundary::detect(&boundary::BoundaryContext {
                prev: None,
                current: ("leave", ""),
                next: Some(("ret", "")),
            })
            .unwrap(),
        ),
        // BOUNDARY_EPILOGUE_POP_RET
        (
            "pop-ret",
            boundary::detect(&boundary::BoundaryContext {
                prev: Some(("pop", "rbp")),
                current: ("ret", ""),
                next: None,
            })
            .unwrap(),
        ),
        // BOUNDARY_EPILOGUE_LEAF_RET
        (
            "leaf-ret",
            boundary::detect(&boundary::BoundaryContext {
                prev: Some(("add", "rsp, 0x28")),
                current: ("ret", ""),
                next: None,
            })
            .unwrap(),
        ),
        // BOUNDARY_FUNCTION_END (bare ret w/o recognised prelude)
        (
            "function-end",
            boundary::detect(&boundary::BoundaryContext {
                prev: None,
                current: ("ret", ""),
                next: None,
            })
            .unwrap(),
        ),
        // BOUNDARY_BLOCK_END_JMP
        (
            "block-end",
            boundary::detect(&boundary::BoundaryContext {
                prev: None,
                current: ("jmp", "0x401000"),
                next: None,
            })
            .unwrap(),
        ),
        // BOUNDARY_BLOCK_FORK_JCC
        (
            "block-fork",
            boundary::detect(&boundary::BoundaryContext {
                prev: None,
                current: ("je", "0x401000"),
                next: None,
            })
            .unwrap(),
        ),
    ];
    assert_tiers_authored("boundary", &probes, |b| &b.tiers);
}
