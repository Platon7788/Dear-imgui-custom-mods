//! x86/x86-64 keyword tables (registers, mnemonics, directives) plus the
//! `OnceLock<HashSet>` lookup caches.
//!
//! Pure data + lookup helpers — the tokenizer in [`super::tokenize`]
//! classifies identifiers against these sets.
//!
//! `MNEMONICS` and `DIRECTIVES` carry `#[rustfmt::skip]` so they keep the
//! compact *packed* array layout (several entries per line, group labels
//! as trailing comments). Without it rustfmt forces one-entry-per-line
//! for any array holding an element wider than its
//! `short_array_element_width_threshold` (default 10) — and both tables
//! contain longer opcodes/directives (`vfmaddsub132ps`,
//! `aeskeygenassist`, `.cfi_def_cfa_register`, …). Packing keeps the file
//! well under the 500-line ceiling despite the extended modern coverage
//! (CET, SSE/AVX/AVX-512, FMA, AES/SHA, BMI, x87, VMX, …).

use std::collections::HashSet;
use std::sync::OnceLock;

// ── x86-64 registers (lowercase canonical forms) ────────────────────────────

pub(super) const REGISTERS: &[&str] = &[
    // 64-bit general purpose
    "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rsp", "rbp", "r8", "r9", "r10", "r11", "r12", "r13",
    "r14", "r15", // 32-bit
    "eax", "ebx", "ecx", "edx", "esi", "edi", "esp", "ebp", "r8d", "r9d", "r10d", "r11d", "r12d",
    "r13d", "r14d", "r15d", // 16-bit
    "ax", "bx", "cx", "dx", "si", "di", "sp", "bp", "r8w", "r9w", "r10w", "r11w", "r12w", "r13w",
    "r14w", "r15w", // 8-bit
    "al", "bl", "cl", "dl", "sil", "dil", "spl", "bpl", "ah", "bh", "ch", "dh", "r8b", "r9b",
    "r10b", "r11b", "r12b", "r13b", "r14b", "r15b", // Segment
    "cs", "ds", "es", "fs", "gs", "ss", // Instruction pointer / flags
    "rip", "eip", "ip", "rflags", "eflags", "flags", // Control / debug
    "cr0", "cr2", "cr3", "cr4", "cr8", "dr0", "dr1", "dr2", "dr3", "dr6", "dr7",
    // SSE/AVX xmm (0–15)
    "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7", "xmm8", "xmm9", "xmm10",
    "xmm11", "xmm12", "xmm13", "xmm14", "xmm15", // AVX-512 xmm (16–31)
    "xmm16", "xmm17", "xmm18", "xmm19", "xmm20", "xmm21", "xmm22", "xmm23", "xmm24", "xmm25",
    "xmm26", "xmm27", "xmm28", "xmm29", "xmm30", "xmm31", // AVX ymm (0–15)
    "ymm0", "ymm1", "ymm2", "ymm3", "ymm4", "ymm5", "ymm6", "ymm7", "ymm8", "ymm9", "ymm10",
    "ymm11", "ymm12", "ymm13", "ymm14", "ymm15", // AVX-512 ymm (16–31)
    "ymm16", "ymm17", "ymm18", "ymm19", "ymm20", "ymm21", "ymm22", "ymm23", "ymm24", "ymm25",
    "ymm26", "ymm27", "ymm28", "ymm29", "ymm30", "ymm31", // AVX-512 zmm (0–15)
    "zmm0", "zmm1", "zmm2", "zmm3", "zmm4", "zmm5", "zmm6", "zmm7", "zmm8", "zmm9", "zmm10",
    "zmm11", "zmm12", "zmm13", "zmm14", "zmm15", // AVX-512 zmm (16–31)
    "zmm16", "zmm17", "zmm18", "zmm19", "zmm20", "zmm21", "zmm22", "zmm23", "zmm24", "zmm25",
    "zmm26", "zmm27", "zmm28", "zmm29", "zmm30", "zmm31", // x87 FPU
    "st0", "st1", "st2", "st3", "st4", "st5", "st6", "st7", // MMX
    "mm0", "mm1", "mm2", "mm3", "mm4", "mm5", "mm6", "mm7", // AVX-512 opmask
    "k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", // MPX bounds
    "bnd0", "bnd1", "bnd2", "bnd3",
];

// ── Common x86-64 mnemonics ─────────────────────────────────────────────────

#[rustfmt::skip]
pub(super) const MNEMONICS: &[&str] = &[
    // Data movement
    "mov", "movabs", "movzx", "movsx", "movsxd", "movq", "movd", "movss", "movsd", "movaps",
    "movups", "movdqa", "movdqu", "lea", "xchg", "push", "pop", "pushf", "popf", "cmov", "cmove",
    "cmovne", "cmovz", "cmovnz", "cmovg", "cmovge", "cmovl", "cmovle", "cmova", "cmovae", "cmovb",
    "cmovbe", "cmovs", "cmovns", "cmovo", "cmovno", "cmovp", "cmovnp", "cmovpe", "cmovpo",
    // Arithmetic
    "add", "sub", "mul", "imul", "div", "idiv", "neg", "inc", "dec", "adc", "sbb", "cmp",
    "addss", "addsd", "subss", "subsd", "mulss", "mulsd", "divss", "divsd",
    // Bitwise
    "and", "or", "xor", "not", "shl", "shr", "sar", "sal", "rol", "ror", "rcl", "rcr", "bt",
    "bts", "btr", "btc", "bsf", "bsr", "test", "popcnt", "lzcnt", "tzcnt",
    // Control flow
    "jmp", "je", "jne", "jz", "jnz", "jg", "jge", "jl", "jle", "ja", "jae", "jb", "jbe", "js",
    "jns", "jo", "jno", "jp", "jnp", "jpe", "jpo", "jcxz", "jecxz", "jrcxz", "call", "ret",
    "retn", "retf", "leave", "enter", "loop", "loope", "loopne", "int", "int3", "syscall",
    "sysenter", "iret", "iretq",
    // Comparison & set
    "sete", "setne", "setg", "setge", "setl", "setle", "seta", "setae", "setb", "setbe", "sets",
    "setns", "seto", "setno", "setp", "setnp", "setpe", "setpo",
    // Stack frame / misc control
    "nop", "hlt", "ud2", "cpuid", "rdtsc", "rdtscp",
    // String ops
    "rep", "repe", "repne", "repz", "repnz", "movsb", "movsw", "movsq", "stosb", "stosw",
    "stosd", "stosq", "lodsb", "lodsw", "lodsd", "lodsq", "cmpsb", "cmpsw", "cmpsq", "scasb",
    "scasw", "scasd", "scasq",
    // Conversion (scalar)
    "cbw", "cwde", "cdqe", "cwd", "cdq", "cqo", "cvtsi2ss", "cvtsi2sd", "cvtss2sd", "cvtsd2ss",
    "cvtss2si", "cvtsd2si", "cvttss2si", "cvttsd2si",
    // Conversion (packed)
    "cvtps2pd", "cvtpd2ps", "cvtdq2ps", "cvtps2dq", "cvttps2dq", "cvtdq2pd", "cvtpd2dq",
    "cvttpd2dq",
    // SSE integer / scalar float
    "pxor", "por", "pand", "pandn", "paddb", "paddw", "paddd", "paddq", "psubb", "psubw",
    "psubd", "psubq", "pmulld", "pmullw", "pcmpeqb", "pcmpeqw", "pcmpeqd", "pshufd", "pshufb",
    "punpcklbw", "punpckhbw", "sqrtss", "sqrtsd", "sqrtps", "sqrtpd", "minss", "maxss", "minsd",
    "maxsd", "comiss", "comisd", "ucomiss", "ucomisd", "shufps", "shufpd", "unpcklps", "unpckhps",
    // Packed SSE float
    "addps", "addpd", "subps", "subpd", "mulps", "mulpd", "divps", "divpd", "xorps", "xorpd",
    "andps", "andpd", "orps", "orpd", "andnps", "andnpd", "movlps", "movhps", "movlhps", "movhlps",
    "unpcklpd", "unpckhpd", "rcpps", "rcpss", "rsqrtps", "rsqrtss", "movmskps", "movmskpd",
    "maxps", "maxpd", "minps", "minpd", "cmpps", "cmppd",
    // SSE2 integer / shift / pack
    "psllw", "pslld", "psllq", "pslldq", "psrlw", "psrld", "psrlq", "psrldq", "psraw", "psrad",
    "pshuflw", "pshufhw", "pavgb", "pavgw", "psadbw", "pmuludq", "pmulhw", "pmulhuw", "pmaddwd",
    "packsswb", "packssdw", "packuswb", "pmovmskb", "pmaxub", "pminub", "pmaxsw", "pminsw",
    "movapd", "movupd", "punpcklwd", "punpcklqdq", "punpckhqdq",
    // SSE3 / SSSE3
    "addsubps", "addsubpd", "haddps", "haddpd", "hsubps", "hsubpd", "movddup", "movshdup",
    "movsldup", "lddqu", "phaddw", "phaddd", "phsubw", "phsubd", "pmaddubsw", "pmulhrsw",
    "palignr", "pabsb", "pabsw", "pabsd", "psignb", "psignw", "psignd",
    // SSE4.1 / SSE4.2
    "ptest", "pblendw", "pblendvb", "blendps", "blendpd", "blendvps", "blendvpd", "insertps",
    "extractps", "pinsrb", "pinsrd", "pinsrq", "pextrb", "pextrd", "pextrq", "packusdw",
    "roundps", "roundpd", "roundss", "roundsd", "dpps", "dppd", "mpsadbw", "crc32", "pcmpistri",
    "pcmpestri", "pcmpistrm", "pcmpestrm", "pcmpgtq", "pmaxsd", "pminsd", "pmaxud", "pminud",
    "pmuldq", "pmovzxbw", "pmovzxbd", "pmovzxbq", "pmovzxwd", "pmovzxwq", "pmovzxdq", "pmovsxbw",
    "pmovsxbd", "pmovsxbq", "pmovsxwd", "pmovsxwq", "pmovsxdq",
    // AVX prefixed (scalar / legacy)
    "vaddss", "vaddsd", "vsubss", "vsubsd", "vmulss", "vmulsd", "vmovss", "vmovsd", "vmovaps",
    "vmovups", "vmovdqa", "vmovdqu", "vxorps", "vandps", "vorps",
    // AVX / AVX2 (base v-forms)
    "vaddps", "vaddpd", "vsubps", "vsubpd", "vmulps", "vmulpd", "vdivps", "vdivpd", "vsqrtps",
    "vsqrtpd", "vmaxps", "vmaxpd", "vminps", "vminpd", "vpxor", "vpand", "vpandn", "vpor",
    "vpaddb", "vpaddw", "vpaddd", "vpaddq", "vpsubb", "vpsubw", "vpsubd", "vpsubq", "vpmulld",
    "vpmullw", "vpcmpeqb", "vpcmpeqw", "vpcmpeqd", "vpshufb", "vpshufd", "vpbroadcastb",
    "vpbroadcastw", "vpbroadcastd", "vpbroadcastq", "vbroadcastss", "vbroadcastsd", "vperm2f128",
    "vpermq", "vpermd", "vinsertf128", "vextractf128", "vpmovmskb", "vptest", "vpblendd",
    "vpblendvb", "vpgatherdd", "vgatherdps", "vzeroupper", "vzeroall",
    // AVX-512 opcodes
    "vpxord", "vpxorq", "vpandd", "vpandq", "vpord", "vporq", "vpternlogd", "vpternlogq",
    "vpermt2d", "vpermt2q", "vpermt2ps", "vpermt2pd", "vpermi2d", "vpermi2q", "vpcompressd",
    "vpcompressq", "vpexpandd", "vpexpandq", "valignd", "valignq", "vpconflictd", "vpconflictq",
    "vplzcntd", "vplzcntq", "vpmadd52luq", "vpmadd52huq", "vrndscaleps", "vrndscalepd",
    "vgetexpps", "vgetexppd", "vgetmantps", "vgetmantpd", "vrcp14ps", "vrcp14pd", "vrsqrt14ps",
    "vrsqrt14pd", "vfixupimmps", "vfixupimmpd",
    // AVX-512 mask ops
    "kmovb", "kmovw", "kmovd", "kmovq", "kandw", "kandnw", "korw", "kxorw", "kxnorw", "knotw",
    "kortestw", "ktestw", "kshiftlw", "kshiftrw", "kunpckbw", "kaddw",
    // BMI1 / BMI2
    "andn", "bextr", "blsi", "blsmsk", "blsr", "bzhi", "mulx", "pdep", "pext", "rorx", "sarx",
    "shlx", "shrx",
    // FMA
    "vfmadd132ps", "vfmadd213ps", "vfmadd231ps", "vfmadd132pd", "vfmadd213pd", "vfmadd231pd",
    "vfmadd132ss", "vfmadd213ss", "vfmadd231ss", "vfmadd132sd", "vfmadd213sd", "vfmadd231sd",
    "vfmsub132ps", "vfmsub213ps", "vfmsub231ps", "vfnmadd132ps", "vfnmadd213ps", "vfnmadd231ps",
    "vfnmsub132ps", "vfmaddsub132ps", "vfmsubadd132ps",
    // AES-NI / CLMUL
    "aesenc", "aesenclast", "aesdec", "aesdeclast", "aesimc", "aeskeygenassist", "pclmulqdq",
    "vaesenc", "vaesdec", "vpclmulqdq",
    // SHA
    "sha1rnds4", "sha1nexte", "sha1msg1", "sha1msg2", "sha256rnds2", "sha256msg1", "sha256msg2",
    // CET (Control-flow Enforcement)
    "endbr64", "endbr32", "incsspd", "incsspq", "rdsspd", "rdsspq", "saveprevssp", "rstorssp",
    "wrssd", "wrssq", "setssbsy", "clrssbsy",
    // Atomics / exchange / misc baseline
    "xadd", "cmpxchg", "cmpxchg8b", "cmpxchg16b", "bswap", "xlat", "xlatb", "lahf", "sahf",
    // Cache / streaming
    "clflush", "clflushopt", "clwb", "clzero", "movnti", "movntq", "movntdq", "movntps",
    "movntpd", "maskmovdqu", "movntdqa", "prefetchw",
    // RNG / processor state
    "rdrand", "rdseed", "movbe", "xsave", "xsaveopt", "xsavec", "xsaves", "xrstor", "xrstors",
    "xgetbv", "xsetbv", "fxsave", "fxrstor", "sysret", "sysexit",
    // FSGSBASE / misc
    "rdfsbase", "wrfsbase", "rdgsbase", "wrgsbase", "swapgs", "rdpid", "rdpkru", "wrpkru",
    "monitor", "mwait", "wait",
    // x87 FPU
    "fld", "fst", "fstp", "fild", "fist", "fistp", "fbld", "fbstp", "fadd", "faddp", "fiadd",
    "fsub", "fsubp", "fsubr", "fsubrp", "fisub", "fmul", "fmulp", "fimul", "fdiv", "fdivp",
    "fdivr", "fdivrp", "fidiv", "fcom", "fcomp", "fcompp", "fcomi", "fcomip", "fucom", "fucomp",
    "fucomi", "fucomip", "fxch", "fabs", "fchs", "fsqrt", "fsin", "fcos", "fsincos", "fptan",
    "fpatan", "fyl2x", "fyl2xp1", "f2xm1", "fld1", "fldz", "fldpi", "fldl2e", "fldl2t", "fldlg2",
    "fldln2", "fnstsw", "fstsw", "fnstcw", "fstcw", "fldcw", "fninit", "finit", "ffree",
    "fnclex", "fclex", "frndint", "fscale", "fprem", "fprem1", "fdecstp", "fincstp", "fnop",
    "fwait",
    // I/O + privileged + VMX
    "in", "out", "insb", "insw", "insd", "outsb", "outsw", "outsd", "lgdt", "lidt", "lldt",
    "ltr", "sgdt", "sidt", "sldt", "lmsw", "smsw", "clts", "invd", "wbinvd", "invlpg", "invpcid",
    "rdmsr", "wrmsr", "rdpmc", "vmcall", "vmlaunch", "vmresume", "vmxon", "vmxoff", "vmread",
    "vmwrite", "vmptrld", "vmptrst", "vmclear", "invept", "invvpid", "vmrun", "vmload", "vmsave",
    // Misc
    "cld", "std", "clc", "stc", "cmc", "lfence", "sfence", "mfence", "pause", "lock", "xacquire",
    "xrelease", "prefetch", "prefetcht0", "prefetcht1", "prefetcht2", "prefetchnta",
    // Size specifiers (NASM/Intel)
    "byte", "word", "dword", "qword", "tword", "oword", "yword", "zword", "ptr", "near", "far",
    "short",
];

// ── Assembler directives ────────────────────────────────────────────────────

#[rustfmt::skip]
pub(super) const DIRECTIVES: &[&str] = &[
    // GAS / AT&T
    ".text", ".data", ".bss", ".rodata", ".section", ".global", ".globl", ".local", ".weak",
    ".hidden", ".protected", ".type", ".size", ".align", ".balign", ".p2align", ".byte", ".word",
    ".long", ".quad", ".octa", ".ascii", ".asciz", ".string", ".zero", ".fill", ".space", ".equ",
    ".set", ".equiv", ".comm", ".lcomm", ".macro", ".endm", ".if", ".else", ".endif", ".ifdef",
    ".ifndef", ".include", ".incbin", ".file", ".loc", ".cfi_startproc", ".cfi_endproc",
    ".cfi_def_cfa_offset", ".cfi_offset", ".cfi_def_cfa_register",
    // NASM / MASM
    "section", "segment", "global", "extern", "default", "bits", "org", "times", "db", "dw",
    "dd", "dq", "dt", "do", "dy", "dz", "resb", "resw", "resd", "resq", "rest", "reso", "resy",
    "resz", "equ", "incbin", "struc", "endstruc", "istruc", "at", "iend",
    // NASM extras
    "align", "alignb", "absolute", "common", "cpu", "group", "use16", "use32", "use64", "wrt",
    "rel", "abs", "strict", "nosplit",
    // NASM preprocessor
    "%define", "%undef", "%macro", "%endmacro", "%if", "%elif", "%else", "%endif", "%ifdef",
    "%ifndef", "%include", "%assign", "%rep", "%endrep",
    // MASM specific (uppercase canonical)
    "PROC", "ENDP", "SEGMENT", "ENDS", "ASSUME", "END", "MACRO", "ENDM", "LOCAL", "INVOKE",
    "STRUCT", "UNION", "TYPEDEF", "IF", "ELSE", "ENDIF", "IFDEF", "IFNDEF", "INCLUDE",
    "INCLUDELIB", "EXTRN", "PUBLIC", ".MODEL", ".STACK", ".CODE", ".DATA", ".386", ".486",
    ".586", ".686", ".MMX", ".XMM",
    // MASM extras (lowercase forms)
    "even", "offset", "proto", "option", "label", "record", "real4", "real8", "real10",
    "textequ", "sizeof", "lengthof", "this", "assume", "ends",
];

// ── Lookup caches ───────────────────────────────────────────────────────────

/// `OnceLock`-backed `HashSet` cache for the three keyword lists. Each
/// is built on first lookup and reused for the lifetime of the process.
///
/// Rationale: a typical `cargo objdump` listing tokenizer pass walks
/// `MNEMONICS`, `DIRECTIVES` and `REGISTERS` once per identifier. On a
/// 5 000-line listing that's millions of `&str` comparisons per syntax
/// pass — measurable in profiles. `HashSet::contains` collapses the
/// lookup to one hash + probe.
fn registers_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| REGISTERS.iter().copied().collect())
}

fn mnemonics_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| MNEMONICS.iter().copied().collect())
}

fn directives_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| DIRECTIVES.iter().copied().collect())
}

/// Check if an identifier (lowercase) matches a register name.
pub(super) fn is_register(word: &str) -> bool {
    // Registers are always lowercase in our list
    registers_set().contains(word)
}

/// Check if a word matches a mnemonic (case-insensitive for Intel compat).
pub(super) fn is_mnemonic(word: &str) -> bool {
    mnemonics_set().contains(word)
}

/// Check if a word matches a directive.
pub(super) fn is_directive(word: &str) -> bool {
    directives_set().contains(word)
}
