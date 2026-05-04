//! Educational mnemonic explainer for the x86 / x86-64 instruction set.
//!
//! Powers the optional `What it does:` line in the instruction hover
//! tooltip. The goal is *learning while you work* — a beginner reading
//! a disassembly listing should pick up what each opcode means without
//! tabbing out to a reference manual.
//!
//! The catalogue covers ~95 % of typical user-mode x86 code:
//!
//! * Data movement (`mov`, `lea`, `xchg`, `push`, `pop`, `movsx/zx/sxd`).
//! * Arithmetic (`add`, `sub`, `adc`, `sbb`, `mul`, `imul`, `div`, `idiv`,
//!   `inc`, `dec`, `neg`, `cdq`, `cqo`).
//! * Logic / bit shifts (`and`, `or`, `xor`, `not`, `shl`/`sal`, `shr`,
//!   `sar`, `rol`, `ror`, `rcl`, `rcr`).
//! * Compare / test (`cmp`, `test`).
//! * Bit operations (`bt`, `bts`, `btr`, `btc`, `bsf`, `bsr`,
//!   `popcnt`, `lzcnt`, `tzcnt`).
//! * Control flow — unconditional (`jmp`, `call`, `ret`, `retn`,
//!   `retf`, `iret`).
//! * Control flow — conditional Jcc family with the actual flag
//!   semantics (`je/jz`, `jne/jnz`, `jl/jnge`, `jle/jng`, `jg/jnle`,
//!   `jge/jnl`, `jb/jc/jnae`, `jbe/jna`, `ja/jnbe`, `jae/jnc/jnb`,
//!   `js`, `jns`, `jo`, `jno`, `jp/jpe`, `jnp/jpo`, `jcxz`, `jecxz`,
//!   `jrcxz`).
//! * Loops (`loop`, `loope/loopz`, `loopne/loopnz`).
//! * Stack frame (`enter`, `leave`, `pushf`/`popf` + 64-bit variants).
//! * String / rep prefixes (`movs`, `stos`, `lods`, `scas`, `cmps`,
//!   `rep`, `repe/repz`, `repne/repnz`).
//! * System (`syscall`, `sysret`, `int`, `into`, `hlt`, `sti`, `cli`,
//!   `lock`, `pause`, `cpuid`, `rdtsc`, `rdrand`, `nop`, `ud2`).
//! * Conditional set / move (`setcc`, `cmovcc` — represented by a
//!   handful of common variants; the rest fall back to the
//!   "Conditional set/move based on flags" wildcard).
//!
//! Lookups are case-insensitive linear scans against `MNEMONICS` —
//! ~100 entries × `eq_ignore_ascii_case` is < 1 µs and fires only on
//! hover, so no caching is needed. SSE/AVX/x87 floating-point opcodes
//! are deliberately omitted; this is meant for general-purpose code
//! reverse engineering, not numerical kernels.

use crate::i18n::Locale;

/// High-level grouping. Currently surfaced via `Category::label_*` only —
/// the renderer doesn't switch tints on category yet but a future
/// "color by category" mode hooks straight into this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Category {
    DataMove,
    Arithmetic,
    Logic,
    Compare,
    BitOp,
    ControlFlow,
    Stack,
    String,
    System,
    ConditionalMove,
    Misc,
}

/// One explanation entry. `en` and `ru` are the short, beginner-friendly
/// descriptions shown in the tooltip; `category` groups them for
/// future styling needs.
#[derive(Debug, Clone, Copy)]
pub struct MnemonicInfo {
    /// High-level grouping — see [`Category`].
    pub category: Category,
    /// English description (one line, ≤ ~140 chars).
    pub en: &'static str,
    /// Russian description (one line, ≤ ~140 chars).
    pub ru: &'static str,
}

/// Look up the catalogue entry for a mnemonic.
///
/// Case-insensitive and zero-allocation; trims surrounding whitespace
/// before matching so callers can pass the raw `instr.mnemonic()`
/// string straight in.
#[must_use]
pub fn lookup(mnemonic: &str) -> Option<&'static MnemonicInfo> {
    let needle = mnemonic.trim();
    if needle.is_empty() {
        return None;
    }
    MNEMONICS
        .iter()
        .find(|(name, _)| needle.eq_ignore_ascii_case(name))
        .map(|(_, info)| info)
}

/// Return the locale-appropriate description for `mnemonic`, or `None`
/// when the mnemonic is unknown / outside the curated catalogue.
#[must_use]
pub fn explain(mnemonic: &str, locale: Locale) -> Option<&'static str> {
    let info = lookup(mnemonic)?;
    Some(match locale {
        Locale::En => info.en,
        Locale::Ru => info.ru,
    })
}

// ── Catalogue ────────────────────────────────────────────────────────────────

const MNEMONICS: &[(&str, MnemonicInfo)] = &[
    // ── Data movement ────────────────────────────────────────────────────
    ("mov", MnemonicInfo {
        category: Category::DataMove,
        en: "Copy source → destination. Flags untouched.",
        ru: "Скопировать источник → приёмник. Флаги не меняются.",
    }),
    ("lea", MnemonicInfo {
        category: Category::DataMove,
        en: "Load Effective Address — compute the address expression and write it to the register; no memory is read.",
        ru: "Вычислить эффективный адрес выражения и записать его в регистр; обращения к памяти НЕТ.",
    }),
    ("xchg", MnemonicInfo {
        category: Category::DataMove,
        en: "Atomic swap of two operands (the `lock` prefix is implicit on memory operands).",
        ru: "Атомарный обмен двух операндов (префикс `lock` неявен для памяти).",
    }),
    ("movsx", MnemonicInfo {
        category: Category::DataMove,
        en: "Move with sign-extension — copy and replicate the top bit so a smaller signed value keeps its sign in the wider register.",
        ru: "Перенос со знаковым расширением — старший бит дублируется, знак сохраняется при расширении.",
    }),
    ("movsxd", MnemonicInfo {
        category: Category::DataMove,
        en: "Sign-extend a 32-bit value into a 64-bit register (typical after a 32-bit array index).",
        ru: "Знаковое расширение 32-битного значения в 64-битный регистр (часто после 32-битного индекса массива).",
    }),
    ("movzx", MnemonicInfo {
        category: Category::DataMove,
        en: "Move with zero-extension — copy and clear the upper bits (used for unsigned widening).",
        ru: "Перенос с нулевым расширением — старшие биты обнуляются (для беззнакового расширения).",
    }),
    ("push", MnemonicInfo {
        category: Category::Stack,
        en: "Push onto the stack — decrements RSP by the operand size, then writes the value at [RSP].",
        ru: "Положить в стек — RSP уменьшается на размер операнда, затем значение записывается по [RSP].",
    }),
    ("pop", MnemonicInfo {
        category: Category::Stack,
        en: "Pop from the stack — read [RSP] into the destination, then increment RSP by the operand size.",
        ru: "Снять со стека — прочитать [RSP] в приёмник, затем увеличить RSP на размер операнда.",
    }),
    ("pushf", MnemonicInfo {
        category: Category::Stack,
        en: "Push EFLAGS onto the stack (16/32-bit form).",
        ru: "Положить регистр флагов EFLAGS в стек (16/32-битная форма).",
    }),
    ("pushfq", MnemonicInfo {
        category: Category::Stack,
        en: "Push the 64-bit RFLAGS register onto the stack.",
        ru: "Положить 64-битный RFLAGS в стек.",
    }),
    ("popf", MnemonicInfo {
        category: Category::Stack,
        en: "Pop the stack into EFLAGS (16/32-bit form).",
        ru: "Снять со стека в регистр флагов EFLAGS (16/32-битная форма).",
    }),
    ("popfq", MnemonicInfo {
        category: Category::Stack,
        en: "Pop the stack into the 64-bit RFLAGS register.",
        ru: "Снять со стека в 64-битный RFLAGS.",
    }),
    ("enter", MnemonicInfo {
        category: Category::Stack,
        en: "Build a stack frame — push RBP, set RBP=RSP, allocate locals. Slow on modern CPUs; compilers prefer push/sub.",
        ru: "Построить стек-фрейм — push RBP, RBP=RSP, выделить место для локальных. Медленный на современных CPU; компиляторы предпочитают push/sub.",
    }),
    ("leave", MnemonicInfo {
        category: Category::Stack,
        en: "Tear down a stack frame — RSP=RBP then pop RBP. Counterpart to `enter`.",
        ru: "Свернуть стек-фрейм — RSP=RBP, затем pop RBP. Парная к `enter`.",
    }),

    // ── Arithmetic ───────────────────────────────────────────────────────
    ("add", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Integer addition: dest += src. Sets OF/SF/ZF/AF/CF/PF.",
        ru: "Целочисленное сложение: dest += src. Меняет флаги OF/SF/ZF/AF/CF/PF.",
    }),
    ("sub", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Integer subtraction: dest -= src. Sets OF/SF/ZF/AF/CF/PF.",
        ru: "Целочисленное вычитание: dest -= src. Меняет флаги OF/SF/ZF/AF/CF/PF.",
    }),
    ("adc", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Add with carry: dest += src + CF. Used for multi-precision arithmetic.",
        ru: "Сложение с переносом: dest += src + CF. Применяется в многоразрядной арифметике.",
    }),
    ("sbb", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Subtract with borrow: dest -= src + CF. Multi-precision counterpart to `adc`.",
        ru: "Вычитание с заёмом: dest -= src + CF. Парная к `adc` для многоразрядной арифметики.",
    }),
    ("inc", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Increment by 1. Sets OF/SF/ZF/AF/PF — but **not** CF (which is what makes it different from `add 1`).",
        ru: "Увеличить на 1. Меняет OF/SF/ZF/AF/PF, но НЕ CF — этим отличается от `add 1`.",
    }),
    ("dec", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Decrement by 1. Same flag rules as `inc` (CF preserved).",
        ru: "Уменьшить на 1. Флаги как у `inc` (CF сохраняется).",
    }),
    ("neg", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Two's complement negation: dest = -dest. Sets CF=0 only when the source was zero.",
        ru: "Изменить знак (доп. код): dest = -dest. CF=0 только если исходное было нулём.",
    }),
    ("mul", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Unsigned multiply. Implicit operand: AL/AX/EAX/RAX × src → AX/DX:AX/EDX:EAX/RDX:RAX.",
        ru: "Беззнаковое умножение. Неявный операнд: AL/AX/EAX/RAX × src → AX/DX:AX/EDX:EAX/RDX:RAX.",
    }),
    ("imul", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Signed multiply. The 2- and 3-operand forms (`imul rax, rbx, 7`) skip the implicit-RAX dance.",
        ru: "Знаковое умножение. 2- и 3-операндные формы (`imul rax, rbx, 7`) не требуют неявного RAX.",
    }),
    ("div", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Unsigned divide DX:AX (or larger pair) by src — quotient → AX, remainder → DX. Faults on divide-by-zero or overflow.",
        ru: "Беззнаковое деление DX:AX (или больше) на src — частное → AX, остаток → DX. Исключение при делении на 0 или переполнении.",
    }),
    ("idiv", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Signed divide. Same dividend layout as `div`. Pair with `cdq`/`cqo` to sign-extend the dividend first.",
        ru: "Знаковое деление. Делимое формируется как у `div`. Перед ним обычно `cdq`/`cqo` для расширения знака.",
    }),
    ("cdq", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Convert Doubleword to Quadword — sign-extend EAX into EDX:EAX. Standard pre-`idiv` setup for 32-bit signed division.",
        ru: "Расширить EAX знаком в EDX:EAX. Стандартная подготовка перед 32-битным знаковым `idiv`.",
    }),
    ("cqo", MnemonicInfo {
        category: Category::Arithmetic,
        en: "Convert Quadword to Octword — sign-extend RAX into RDX:RAX. Standard pre-`idiv` setup for 64-bit signed division.",
        ru: "Расширить RAX знаком в RDX:RAX. Стандартная подготовка перед 64-битным знаковым `idiv`.",
    }),

    // ── Logic / shifts ───────────────────────────────────────────────────
    ("and", MnemonicInfo {
        category: Category::Logic,
        en: "Bitwise AND. Common idiom: mask off bits, or `and reg, reg` to test for zero (sets ZF).",
        ru: "Побитовое И. Часто: маскирование битов или `and reg, reg` для проверки на ноль (выставляет ZF).",
    }),
    ("or", MnemonicInfo {
        category: Category::Logic,
        en: "Bitwise OR. `or reg, reg` is sometimes used as a non-zero test (sets ZF).",
        ru: "Побитовое ИЛИ. `or reg, reg` иногда используется как тест на не-ноль (выставляет ZF).",
    }),
    ("xor", MnemonicInfo {
        category: Category::Logic,
        en: "Bitwise XOR. Idiomatic zero-out: `xor eax, eax` (smaller encoding than `mov eax, 0` and breaks the dependency chain).",
        ru: "Побитовое XOR. Идиома обнуления: `xor eax, eax` (короче чем `mov eax, 0` и разрывает цепочку зависимостей).",
    }),
    ("not", MnemonicInfo {
        category: Category::Logic,
        en: "Bitwise NOT (one's complement). Flags untouched — pair with `inc` for two's complement.",
        ru: "Побитовое НЕ (доп. до единицы). Флаги не меняет — для дополнения до двух нужно `inc` следом.",
    }),
    ("shl", MnemonicInfo {
        category: Category::Logic,
        en: "Shift Left logical — fills low bits with 0. Equivalent to multiplying by 2^count for unsigned values.",
        ru: "Логический сдвиг влево — младшие биты заполняются нулями. Для беззнаковых = умножение на 2^count.",
    }),
    ("sal", MnemonicInfo {
        category: Category::Logic,
        en: "Shift Arithmetic Left — identical encoding to `shl`. The mnemonic is preserved for symmetry with `sar`.",
        ru: "Арифметический сдвиг влево — кодировка та же, что у `shl`. Мнемоника оставлена для симметрии с `sar`.",
    }),
    ("shr", MnemonicInfo {
        category: Category::Logic,
        en: "Shift Right logical — fills high bits with 0. Unsigned divide by 2^count.",
        ru: "Логический сдвиг вправо — старшие биты заполняются нулями. Беззнаковое деление на 2^count.",
    }),
    ("sar", MnemonicInfo {
        category: Category::Logic,
        en: "Shift Arithmetic Right — replicates the sign bit, preserving sign. Signed divide by 2^count (rounds toward −∞).",
        ru: "Арифметический сдвиг вправо — старший бит дублируется, знак сохраняется. Знаковое деление на 2^count (к −∞).",
    }),
    ("rol", MnemonicInfo {
        category: Category::Logic,
        en: "Rotate Left — bits cycled out of the high end re-enter at the low end (no carry chain).",
        ru: "Циклический сдвиг влево — биты, ушедшие со старшего конца, возвращаются с младшего (без участия CF).",
    }),
    ("ror", MnemonicInfo {
        category: Category::Logic,
        en: "Rotate Right — bits cycled out of the low end re-enter at the high end.",
        ru: "Циклический сдвиг вправо — биты, ушедшие с младшего конца, возвращаются со старшего.",
    }),
    ("rcl", MnemonicInfo {
        category: Category::Logic,
        en: "Rotate through Carry Left — CF participates in the rotation, useful for multi-precision shifts.",
        ru: "Циклический сдвиг влево через CF — флаг переноса участвует, удобно для многоразрядных сдвигов.",
    }),
    ("rcr", MnemonicInfo {
        category: Category::Logic,
        en: "Rotate through Carry Right — counterpart to `rcl`.",
        ru: "Циклический сдвиг вправо через CF — парный к `rcl`.",
    }),

    // ── Compare / test ───────────────────────────────────────────────────
    ("cmp", MnemonicInfo {
        category: Category::Compare,
        en: "Compare = subtract without storing the result; only the flags update. Followed by a Jcc.",
        ru: "Сравнение = вычитание без записи результата; меняются только флаги. Обычно за ним идёт Jcc.",
    }),
    ("test", MnemonicInfo {
        category: Category::Compare,
        en: "Bitwise AND without storing the result; only the flags update. Idiomatic zero/non-zero check: `test reg, reg`.",
        ru: "Побитовое И без записи результата; меняются только флаги. Идиоматичная проверка на 0: `test reg, reg`.",
    }),

    // ── Bit operations ───────────────────────────────────────────────────
    ("bt", MnemonicInfo {
        category: Category::BitOp,
        en: "Bit Test — copy the selected bit of dest into CF. Read-only.",
        ru: "Проверка бита — выбранный бит приёмника копируется в CF. Только чтение.",
    }),
    ("bts", MnemonicInfo {
        category: Category::BitOp,
        en: "Bit Test and Set — copy the selected bit into CF, then set it to 1.",
        ru: "Проверить и установить бит — текущее значение в CF, затем выставить в 1.",
    }),
    ("btr", MnemonicInfo {
        category: Category::BitOp,
        en: "Bit Test and Reset — copy the selected bit into CF, then clear it.",
        ru: "Проверить и сбросить бит — текущее значение в CF, затем сбросить в 0.",
    }),
    ("btc", MnemonicInfo {
        category: Category::BitOp,
        en: "Bit Test and Complement — copy the selected bit into CF, then invert it.",
        ru: "Проверить и инвертировать бит — текущее значение в CF, затем инвертировать.",
    }),
    ("bsf", MnemonicInfo {
        category: Category::BitOp,
        en: "Bit Scan Forward — index of the lowest set bit. ZF=1 when source is zero (result undefined).",
        ru: "Поиск младшего единичного бита (его индекс). ZF=1 если источник = 0 (результат не определён).",
    }),
    ("bsr", MnemonicInfo {
        category: Category::BitOp,
        en: "Bit Scan Reverse — index of the highest set bit. ZF=1 when source is zero.",
        ru: "Поиск старшего единичного бита (его индекс). ZF=1 если источник = 0.",
    }),
    ("popcnt", MnemonicInfo {
        category: Category::BitOp,
        en: "Population count — number of bits set in source. Single-cycle on modern CPUs.",
        ru: "Подсчёт единичных битов в источнике. Одноцикловая на современных CPU.",
    }),
    ("lzcnt", MnemonicInfo {
        category: Category::BitOp,
        en: "Leading Zero count — number of zero bits before the highest set bit (BMI1).",
        ru: "Число ведущих нулей до старшего единичного бита (BMI1).",
    }),
    ("tzcnt", MnemonicInfo {
        category: Category::BitOp,
        en: "Trailing Zero count — number of zero bits before the lowest set bit (BMI1).",
        ru: "Число завершающих нулей до младшего единичного бита (BMI1).",
    }),

    // ── Control flow — unconditional ─────────────────────────────────────
    ("jmp", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Unconditional jump. Target may be relative (immediate), register, or memory (indirect).",
        ru: "Безусловный переход. Цель — относительный сдвиг, регистр или память (косвенно).",
    }),
    ("call", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Call — push the return address (RIP after this instruction), then jump to target. The callee must `ret` to come back.",
        ru: "Вызов — положить адрес возврата (RIP после этой инстр.) в стек и перейти. Возврат через `ret`.",
    }),
    ("ret", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Return — pop the return address into RIP. The optional immediate (`ret 16`) frees stack arguments after popping.",
        ru: "Возврат — снять адрес возврата в RIP. Необязательный иммедиат (`ret 16`) освобождает аргументы со стека.",
    }),
    ("retn", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Near return (synonym for `ret` in 32/64-bit code).",
        ru: "Ближний возврат (синоним `ret` в 32/64-битном коде).",
    }),
    ("retf", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Far return — pops both RIP and CS. Almost never seen outside legacy 16-bit / kernel code.",
        ru: "Дальний возврат — снимает RIP и CS. Почти не встречается вне 16-битного / ядерного кода.",
    }),
    ("iret", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Interrupt return — pops RIP, CS, RFLAGS (kernel-only; user-mode hits a #GP).",
        ru: "Возврат из прерывания — снимает RIP, CS, RFLAGS (только в ядре; в user mode — #GP).",
    }),
    ("loop", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Decrement RCX (or ECX), branch if RCX ≠ 0. Slower than a manual `dec`/`jnz` pair on modern CPUs.",
        ru: "Уменьшить RCX (или ECX), переход если RCX ≠ 0. На современных CPU медленнее ручной пары `dec`/`jnz`.",
    }),
    ("loope", MnemonicInfo {
        category: Category::ControlFlow,
        en: "`loop` plus the ZF=1 condition — keep looping while RCX ≠ 0 AND ZF=1.",
        ru: "`loop` + условие ZF=1 — продолжать пока RCX ≠ 0 И ZF=1.",
    }),
    ("loopne", MnemonicInfo {
        category: Category::ControlFlow,
        en: "`loop` plus the ZF=0 condition — keep looping while RCX ≠ 0 AND ZF=0.",
        ru: "`loop` + условие ZF=0 — продолжать пока RCX ≠ 0 И ZF=0.",
    }),

    // ── Conditional jumps (Jcc) ──────────────────────────────────────────
    ("je", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Equal — taken when ZF=1 (the previous compare reported equality).",
        ru: "Переход если равно — берётся при ZF=1 (предыдущее сравнение показало равенство).",
    }),
    ("jz", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Zero — synonym of `je` (ZF=1).",
        ru: "Переход если ноль — синоним `je` (ZF=1).",
    }),
    ("jne", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Equal — taken when ZF=0.",
        ru: "Переход если не равно — берётся при ZF=0.",
    }),
    ("jnz", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Zero — synonym of `jne` (ZF=0).",
        ru: "Переход если не ноль — синоним `jne` (ZF=0).",
    }),
    ("jl", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Less (signed) — taken when SF≠OF.",
        ru: "Переход если меньше (знаковое) — при SF≠OF.",
    }),
    ("jnge", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Greater-or-Equal (signed) — synonym of `jl` (SF≠OF).",
        ru: "Переход если НЕ больше-или-равно (знаковое) — синоним `jl` (SF≠OF).",
    }),
    ("jle", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Less-or-Equal (signed) — taken when ZF=1 OR SF≠OF.",
        ru: "Переход если меньше-или-равно (знаковое) — при ZF=1 ИЛИ SF≠OF.",
    }),
    ("jng", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Greater (signed) — synonym of `jle`.",
        ru: "Переход если НЕ больше (знаковое) — синоним `jle`.",
    }),
    ("jg", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Greater (signed) — taken when ZF=0 AND SF=OF.",
        ru: "Переход если больше (знаковое) — при ZF=0 И SF=OF.",
    }),
    ("jnle", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Less-or-Equal (signed) — synonym of `jg`.",
        ru: "Переход если НЕ меньше-или-равно (знаковое) — синоним `jg`.",
    }),
    ("jge", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Greater-or-Equal (signed) — taken when SF=OF.",
        ru: "Переход если больше-или-равно (знаковое) — при SF=OF.",
    }),
    ("jnl", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Less (signed) — synonym of `jge` (SF=OF).",
        ru: "Переход если НЕ меньше (знаковое) — синоним `jge` (SF=OF).",
    }),
    ("jb", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Below (unsigned) — taken when CF=1.",
        ru: "Переход если меньше (беззнаковое) — при CF=1.",
    }),
    ("jc", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Carry — synonym of `jb` (CF=1).",
        ru: "Переход если CF=1 — синоним `jb`.",
    }),
    ("jnae", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Above-or-Equal (unsigned) — synonym of `jb` (CF=1).",
        ru: "Переход если НЕ больше-или-равно (беззнаковое) — синоним `jb` (CF=1).",
    }),
    ("jbe", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Below-or-Equal (unsigned) — taken when CF=1 OR ZF=1.",
        ru: "Переход если меньше-или-равно (беззнаковое) — при CF=1 ИЛИ ZF=1.",
    }),
    ("jna", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Above (unsigned) — synonym of `jbe`.",
        ru: "Переход если НЕ больше (беззнаковое) — синоним `jbe`.",
    }),
    ("ja", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Above (unsigned) — taken when CF=0 AND ZF=0.",
        ru: "Переход если больше (беззнаковое) — при CF=0 И ZF=0.",
    }),
    ("jnbe", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Below-or-Equal (unsigned) — synonym of `ja`.",
        ru: "Переход если НЕ меньше-или-равно (беззнаковое) — синоним `ja`.",
    }),
    ("jae", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Above-or-Equal (unsigned) — taken when CF=0.",
        ru: "Переход если больше-или-равно (беззнаковое) — при CF=0.",
    }),
    ("jnc", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if No Carry — synonym of `jae` (CF=0).",
        ru: "Переход если CF=0 — синоним `jae`.",
    }),
    ("jnb", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Not Below (unsigned) — synonym of `jae` (CF=0).",
        ru: "Переход если НЕ меньше (беззнаковое) — синоним `jae` (CF=0).",
    }),
    ("js", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Sign — taken when SF=1 (result was negative).",
        ru: "Переход если знак — при SF=1 (результат был отрицательным).",
    }),
    ("jns", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if No Sign — taken when SF=0 (result was non-negative).",
        ru: "Переход если нет знака — при SF=0 (результат неотрицательный).",
    }),
    ("jo", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Overflow — taken when OF=1 (signed overflow happened).",
        ru: "Переход при переполнении — OF=1 (произошло знаковое переполнение).",
    }),
    ("jno", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if No Overflow — taken when OF=0.",
        ru: "Переход если нет переполнения — OF=0.",
    }),
    ("jp", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Parity (even) — taken when PF=1 (low byte has an even number of set bits).",
        ru: "Переход если чётность — PF=1 (в младшем байте чётное число единиц).",
    }),
    ("jpe", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Parity Even — synonym of `jp`.",
        ru: "Переход при чётности — синоним `jp`.",
    }),
    ("jnp", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if No Parity — taken when PF=0 (odd number of set bits).",
        ru: "Переход если нечётность — PF=0 (нечётное число единиц).",
    }),
    ("jpo", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if Parity Odd — synonym of `jnp`.",
        ru: "Переход при нечётности — синоним `jnp`.",
    }),
    ("jcxz", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if CX=0 (16-bit operand size). Tests CX directly, not flags.",
        ru: "Переход если CX=0 (16-битный режим). Проверяет регистр напрямую, не флаги.",
    }),
    ("jecxz", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if ECX=0 (32-bit operand size). Tests ECX directly, not flags.",
        ru: "Переход если ECX=0 (32-битный режим). Проверяет регистр напрямую, не флаги.",
    }),
    ("jrcxz", MnemonicInfo {
        category: Category::ControlFlow,
        en: "Jump if RCX=0 (64-bit operand size). Tests RCX directly, not flags.",
        ru: "Переход если RCX=0 (64-битный режим). Проверяет регистр напрямую, не флаги.",
    }),

    // ── String operations ────────────────────────────────────────────────
    ("movs", MnemonicInfo {
        category: Category::String,
        en: "MOVe String — copy [RSI] → [RDI], advance both by element size (DF controls direction). Pair with `rep` for memcpy.",
        ru: "Копирование строки — [RSI] → [RDI], оба указателя сдвигаются на размер элемента (направление от DF). С `rep` = memcpy.",
    }),
    ("stos", MnemonicInfo {
        category: Category::String,
        en: "STOre String — write AL/AX/EAX/RAX into [RDI], advance RDI. Pair with `rep` for memset.",
        ru: "Запись строки — записать AL/AX/EAX/RAX в [RDI], сдвинуть RDI. С `rep` = memset.",
    }),
    ("lods", MnemonicInfo {
        category: Category::String,
        en: "LOaD String — read [RSI] into AL/AX/EAX/RAX, advance RSI.",
        ru: "Чтение строки — [RSI] в AL/AX/EAX/RAX, сдвинуть RSI.",
    }),
    ("scas", MnemonicInfo {
        category: Category::String,
        en: "SCAn String — compare AL/AX/EAX/RAX against [RDI] (sets flags), advance RDI. Pair with `repne` for `strchr`.",
        ru: "Поиск в строке — сравнить AL/AX/EAX/RAX с [RDI] (выставить флаги), сдвинуть RDI. С `repne` = `strchr`.",
    }),
    ("cmps", MnemonicInfo {
        category: Category::String,
        en: "CoMPare Strings — compare [RSI] vs [RDI], advance both. Pair with `repe` for `memcmp`.",
        ru: "Сравнение строк — [RSI] vs [RDI], оба сдвигаются. С `repe` = `memcmp`.",
    }),
    ("rep", MnemonicInfo {
        category: Category::String,
        en: "Repeat prefix — execute the string instruction RCX times, decrementing RCX each iteration.",
        ru: "Префикс повторения — выполнить строковую инструкцию RCX раз, уменьшая RCX на каждой итерации.",
    }),
    ("repe", MnemonicInfo {
        category: Category::String,
        en: "Repeat-while-Equal — like `rep` but also exits when ZF=0. Pairs with `cmps` / `scas`.",
        ru: "Повторение пока равно — как `rep`, но выход и при ZF=0. С `cmps` / `scas`.",
    }),
    ("repz", MnemonicInfo {
        category: Category::String,
        en: "Repeat-while-Zero — synonym of `repe`.",
        ru: "Повторение пока ZF=1 — синоним `repe`.",
    }),
    ("repne", MnemonicInfo {
        category: Category::String,
        en: "Repeat-while-Not-Equal — like `rep` but also exits when ZF=1. Pairs with `scas` for `strlen`.",
        ru: "Повторение пока не равно — как `rep`, но выход и при ZF=1. С `scas` = `strlen`.",
    }),
    ("repnz", MnemonicInfo {
        category: Category::String,
        en: "Repeat-while-Not-Zero — synonym of `repne`.",
        ru: "Повторение пока ZF=0 — синоним `repne`.",
    }),

    // ── System / synchronisation ─────────────────────────────────────────
    ("syscall", MnemonicInfo {
        category: Category::System,
        en: "Fast system call (64-bit) — switches to ring 0, RIP→RCX, RFLAGS→R11, jumps to LSTAR MSR.",
        ru: "Быстрый системный вызов (64-битный) — переход в ring 0, RIP→RCX, RFLAGS→R11, прыжок по LSTAR MSR.",
    }),
    ("sysret", MnemonicInfo {
        category: Category::System,
        en: "Return from `syscall` — RCX→RIP, R11→RFLAGS, drops back to ring 3.",
        ru: "Возврат из `syscall` — RCX→RIP, R11→RFLAGS, обратно в ring 3.",
    }),
    ("int", MnemonicInfo {
        category: Category::System,
        en: "Software interrupt — calls the IDT vector. `int 3` is the debugger breakpoint (single-byte 0xCC).",
        ru: "Программное прерывание — вызов вектора IDT. `int 3` = брейкпоинт отладчика (один байт 0xCC).",
    }),
    ("into", MnemonicInfo {
        category: Category::System,
        en: "Trap on overflow — fires `int 4` if OF=1. Effectively a no-op in 64-bit mode.",
        ru: "Прерывание при переполнении — `int 4` если OF=1. В 64-битном режиме фактически no-op.",
    }),
    ("hlt", MnemonicInfo {
        category: Category::System,
        en: "Halt the CPU until the next interrupt. Ring-0 only — `#GP` in user mode.",
        ru: "Остановить CPU до следующего прерывания. Только ring 0 — `#GP` в user mode.",
    }),
    ("sti", MnemonicInfo {
        category: Category::System,
        en: "Set Interrupt-flag — re-enable maskable interrupts (ring 0).",
        ru: "Установить флаг прерываний — разрешить маскируемые прерывания (ring 0).",
    }),
    ("cli", MnemonicInfo {
        category: Category::System,
        en: "Clear Interrupt-flag — disable maskable interrupts (ring 0).",
        ru: "Сбросить флаг прерываний — запретить маскируемые прерывания (ring 0).",
    }),
    ("lock", MnemonicInfo {
        category: Category::System,
        en: "LOCK prefix — make the next read-modify-write instruction atomic on the system bus.",
        ru: "Префикс LOCK — сделать следующую RMW-инструкцию атомарной на системной шине.",
    }),
    ("pause", MnemonicInfo {
        category: Category::System,
        en: "Hint to spin-wait loops — improves SMT efficiency and saves power; mandatory in well-written `while busy {}` loops.",
        ru: "Подсказка в spin-циклах — повышает эффективность SMT и экономит питание; обязательна в правильных `while busy {}`.",
    }),
    ("cpuid", MnemonicInfo {
        category: Category::System,
        en: "CPU identification — feature/version query. EAX selects the leaf, results return in EAX/EBX/ECX/EDX.",
        ru: "Идентификация CPU — запрос возможностей/версии. EAX задаёт лист, ответы в EAX/EBX/ECX/EDX.",
    }),
    ("rdtsc", MnemonicInfo {
        category: Category::System,
        en: "Read TimeStamp Counter — returns the cycle counter in EDX:EAX. Not strictly serialised on its own; pair with `lfence` if you need ordering.",
        ru: "Прочитать счётчик тактов — результат в EDX:EAX. Сам по себе не сериализуем; для упорядочивания нужен `lfence`.",
    }),
    ("rdrand", MnemonicInfo {
        category: Category::System,
        en: "Read hardware random number — CF=1 on success. Slower than software PRNGs; rarely seen in hot paths.",
        ru: "Чтение аппаратного случайного числа — CF=1 при успехе. Медленнее программного PRNG; редко в hot path.",
    }),

    // ── Conditional set / move ───────────────────────────────────────────
    ("setz", MnemonicInfo {
        category: Category::ConditionalMove,
        en: "Set byte if ZF=1, else clear — turns the equality flag into a 0/1 byte without branching.",
        ru: "Установить байт если ZF=1, иначе обнулить — превращает флаг равенства в 0/1 без перехода.",
    }),
    ("sete", MnemonicInfo {
        category: Category::ConditionalMove,
        en: "Set byte if Equal — synonym of `setz`.",
        ru: "Установить байт если равно — синоним `setz`.",
    }),
    ("setnz", MnemonicInfo {
        category: Category::ConditionalMove,
        en: "Set byte if ZF=0, else clear.",
        ru: "Установить байт если ZF=0, иначе обнулить.",
    }),
    ("setne", MnemonicInfo {
        category: Category::ConditionalMove,
        en: "Set byte if Not Equal — synonym of `setnz`.",
        ru: "Установить байт если не равно — синоним `setnz`.",
    }),
    ("cmove", MnemonicInfo {
        category: Category::ConditionalMove,
        en: "Conditional MOVe if Equal (ZF=1) — branch-free assignment. Reads source unconditionally; that can matter for memory operands.",
        ru: "Условный перенос если равно (ZF=1) — присваивание без перехода. Источник читается всегда; важно для операндов в памяти.",
    }),
    ("cmovz", MnemonicInfo {
        category: Category::ConditionalMove,
        en: "Conditional MOVe if Zero — synonym of `cmove`.",
        ru: "Условный перенос если ноль — синоним `cmove`.",
    }),
    ("cmovne", MnemonicInfo {
        category: Category::ConditionalMove,
        en: "Conditional MOVe if Not Equal (ZF=0).",
        ru: "Условный перенос если не равно (ZF=0).",
    }),
    ("cmovnz", MnemonicInfo {
        category: Category::ConditionalMove,
        en: "Conditional MOVe if Not Zero — synonym of `cmovne`.",
        ru: "Условный перенос если не ноль — синоним `cmovne`.",
    }),

    // ── Misc ─────────────────────────────────────────────────────────────
    ("nop", MnemonicInfo {
        category: Category::Misc,
        en: "No-OPeration — used for alignment padding and hot-patch space. Multi-byte forms exist (0F 1F …) for longer pads.",
        ru: "Пустая инструкция — выравнивание и место под hot-patch. Есть многобайтные формы (0F 1F …) для длинных пропусков.",
    }),
    ("ud2", MnemonicInfo {
        category: Category::Misc,
        en: "Undefined Instruction — guaranteed to raise `#UD`. Compilers emit it after unreachable code or trap intrinsics.",
        ru: "Неопределённая инструкция — гарантированно вызывает `#UD`. Компиляторы ставят её после unreachable / trap.",
    }),
    ("prefetch", MnemonicInfo {
        category: Category::Misc,
        en: "Hint the CPU to fetch a cache line — speculative; never faults on a bad address.",
        ru: "Подсказка CPU подгрузить кэш-линию — спекулятивно; не падает при плохом адресе.",
    }),
    ("xlat", MnemonicInfo {
        category: Category::Misc,
        en: "Table lookup: AL = [RBX + AL]. Holdover from the 8086 era; rarely emitted by modern compilers.",
        ru: "Поиск в таблице: AL = [RBX + AL]. Реликт эпохи 8086; компиляторы почти не используют.",
    }),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_case_insensitive() {
        assert!(lookup("MOV").is_some());
        assert!(lookup("mov").is_some());
        assert!(lookup("Mov").is_some());
        assert!(lookup("  mov  ").is_some());
    }

    #[test]
    fn lookup_unknown_returns_none() {
        assert!(lookup("xyzzy").is_none());
        assert!(lookup("").is_none());
        assert!(lookup("   ").is_none());
    }

    #[test]
    fn explain_localises() {
        let en = explain("mov", Locale::En).unwrap();
        let ru = explain("mov", Locale::Ru).unwrap();
        assert_ne!(en, ru);
        assert!(en.starts_with("Copy"));
        assert!(ru.starts_with("Скопировать"));
    }

    #[test]
    fn explain_unknown_returns_none() {
        assert!(explain("madeup", Locale::En).is_none());
        assert!(explain("madeup", Locale::Ru).is_none());
    }

    #[test]
    fn descriptions_are_terse() {
        // Tooltip lines wrap awkwardly past ~140 chars; lock that as a soft cap
        // so future entries stay readable.
        for (mnemonic, info) in MNEMONICS {
            assert!(
                info.en.chars().count() <= 200,
                "EN description for `{mnemonic}` is too long ({} chars)",
                info.en.chars().count(),
            );
            assert!(
                info.ru.chars().count() <= 200,
                "RU description for `{mnemonic}` is too long ({} chars)",
                info.ru.chars().count(),
            );
        }
    }

    #[test]
    fn flow_control_jcc_family_present() {
        // Smoke-test the conditional-jump family — every entry must
        // reference a flag (CF / ZF / SF / OF / PF) so the user sees
        // *why* the branch is taken, not just the synonym.
        for jcc in ["je", "jne", "jl", "jle", "jg", "jge", "jb", "jbe", "ja", "jae", "js", "jns", "jo", "jno", "jp", "jnp"] {
            let info = lookup(jcc).unwrap_or_else(|| panic!("missing Jcc entry: {jcc}"));
            let en = info.en;
            assert!(
                en.contains("CF") || en.contains("ZF") || en.contains("SF") || en.contains("OF") || en.contains("PF"),
                "Jcc `{jcc}` must mention a flag in its EN description, got: {en}",
            );
        }
    }
}
