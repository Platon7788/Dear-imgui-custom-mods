//! Educational mnemonic explainer for the x86 / x86-64 instruction set.
//!
//! Powers the optional `What it does:` and `Watch out:` lines in the
//! instruction hover tooltip. The goal is *learning while you work* —
//! a beginner reading a disassembly listing should pick up what each
//! opcode means without tabbing out to a reference manual, AND get
//! warned about the common anti-debug / anti-disasm / obfuscation
//! tricks built around that opcode.
//!
//! Coverage targets ~95 % of typical user-mode x86, with explicit
//! dual-mode descriptions: every entry that touches a register or a
//! flag spells out **both** the 32-bit form (ESP / EIP / EFLAGS) and
//! the 64-bit form (RSP / RIP / RFLAGS). Lookups are
//! case-insensitive zero-allocation linear scans against `MNEMONICS`
//! (~110 entries × `eq_ignore_ascii_case`) — under a microsecond, fires
//! only on hover, no caching needed.
//!
//! Pairs with [`crate::disasm_view::idiom`] for multi-instruction
//! pattern detection (prologue / epilogue / NULL-check / call-with-args /
//! get-IP idiom / etc.) and [`crate::disasm_view::abi`] for calling-
//! convention argument hints.
//!
//! SSE/AVX/x87 floating-point opcodes are deliberately omitted; this
//! is meant for general-purpose code reverse engineering, not
//! numerical kernels.

use crate::i18n::Locale;

/// High-level grouping. Surfaced via `Category` only — the renderer
/// doesn't switch tints on category yet but a future
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

/// One catalogue entry. `en`/`ru` are the short, beginner-friendly
/// descriptions shown in the tooltip; `gotcha_en`/`gotcha_ru` are
/// optional warnings about common anti-RE / anti-debug / obfuscation
/// patterns that abuse this mnemonic. `category` groups them for
/// future styling.
#[derive(Debug, Clone, Copy)]
pub struct MnemonicInfo {
    /// High-level grouping — see [`Category`].
    pub category: Category,
    /// English description (one line, ≤ ~240 chars).
    pub en: &'static str,
    /// Russian description (one line, ≤ ~240 chars).
    pub ru: &'static str,
    /// Optional anti-RE / anti-debug / obfuscation warning, English.
    pub gotcha_en: Option<&'static str>,
    /// Optional anti-RE / anti-debug / obfuscation warning, Russian.
    pub gotcha_ru: Option<&'static str>,
}

impl MnemonicInfo {
    /// Helper: locale-appropriate description.
    #[inline]
    #[must_use]
    pub fn description(&self, locale: Locale) -> &'static str {
        match locale {
            Locale::En => self.en,
            Locale::Ru => self.ru,
        }
    }

    /// Helper: locale-appropriate gotcha (or `None`).
    #[inline]
    #[must_use]
    pub fn gotcha(&self, locale: Locale) -> Option<&'static str> {
        match locale {
            Locale::En => self.gotcha_en,
            Locale::Ru => self.gotcha_ru,
        }
    }
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
    lookup(mnemonic).map(|info| info.description(locale))
}

/// Return the locale-appropriate gotcha for `mnemonic`, or `None`
/// when the mnemonic is unknown OR has no documented gotcha.
#[must_use]
pub fn gotcha(mnemonic: &str, locale: Locale) -> Option<&'static str> {
    lookup(mnemonic).and_then(|info| info.gotcha(locale))
}

// Builder helpers keep the catalogue table compact + readable.
const fn entry(
    category: Category,
    en: &'static str,
    ru: &'static str,
) -> MnemonicInfo {
    MnemonicInfo {
        category,
        en,
        ru,
        gotcha_en: None,
        gotcha_ru: None,
    }
}

const fn entry_g(
    category: Category,
    en: &'static str,
    ru: &'static str,
    gotcha_en: &'static str,
    gotcha_ru: &'static str,
) -> MnemonicInfo {
    MnemonicInfo {
        category,
        en,
        ru,
        gotcha_en: Some(gotcha_en),
        gotcha_ru: Some(gotcha_ru),
    }
}

// ── Catalogue ────────────────────────────────────────────────────────────────

const MNEMONICS: &[(&str, MnemonicInfo)] = &[
    // ── Data movement ────────────────────────────────────────────────────
    ("mov", entry_g(
        Category::DataMove,
        "Copy source → destination. Flags untouched. 32/64-bit forms differ only in operand size (E.. vs R.. registers, dword vs qword memory).",
        "Скопировать источник → приёмник. Флаги не меняются. 32/64-битные формы отличаются только размером операнда (E.. vs R.. регистры, dword vs qword память).",
        "VMProtect / Themida heavily abuse `mov` for register juggling — long chains `mov r1,r2; mov r2,r3; …` often boil down to a single original move under the obfuscation.",
        "VMProtect / Themida активно используют `mov` для перетасовки регистров — длинные цепочки `mov r1,r2; mov r2,r3; …` часто сводятся к одному реальному переносу.",
    )),
    ("lea", entry_g(
        Category::DataMove,
        "Load Effective Address — compute the address expression and write it to the register; no memory is read. 32-bit form uses E.., 64-bit uses R.. or RIP-relative.",
        "Вычислить эффективный адрес и записать его в регистр; память НЕ читается. 32-битная форма — E.., 64-битная — R.. или RIP-relative.",
        "Common obfuscation: `lea reg, [reg+const]` is a flag-free `add reg, const`. Look for `lea eax, [eax+1]` instead of `inc eax`, `lea reg, [reg*2+0]` for shifts, etc.",
        "Типичная обфускация: `lea reg, [reg+const]` — это `add reg, const` без изменения флагов. Ищите `lea eax, [eax+1]` вместо `inc eax`, `lea reg, [reg*2+0]` вместо сдвигов.",
    )),
    ("xchg", entry_g(
        Category::DataMove,
        "Atomic swap of two operands (the `lock` prefix is implicit on memory operands). Works identically in x32 and x64 modulo register width.",
        "Атомарный обмен двух операндов (префикс `lock` неявен для памяти). В x32 и x64 работает одинаково с поправкой на ширину регистров.",
        "`xchg reg, reg` to itself is a 1-byte `nop` (0x90 = `xchg eax/rax, eax/rax`). Mutation engines love filler `xchg` chains that cancel out.",
        "`xchg reg, reg` сам с собой — это 1-байтный `nop` (0x90 = `xchg eax/rax, eax/rax`). Мутационные движки любят заполнять код парными `xchg`, которые гасят друг друга.",
    )),
    ("movsx", entry(
        Category::DataMove,
        "Move with sign-extension — copy and replicate the top bit so a smaller signed value keeps its sign in the wider register. Works for byte→word/dword and word→dword in x32; adds byte/word/dword→qword in x64.",
        "Перенос со знаковым расширением — старший бит дублируется, знак сохраняется. byte→word/dword и word→dword в x32; в x64 добавляется byte/word/dword→qword.",
    )),
    ("movsxd", entry(
        Category::DataMove,
        "Sign-extend a 32-bit value into a 64-bit register. **x64-only** — the 32-bit instruction set doesn't have it. Common after a 32-bit array index in 64-bit code.",
        "Знаковое расширение 32-битного значения в 64-битный регистр. **Только x64** — в 32-битном наборе её нет. Часто после 32-битного индекса массива в x64-коде.",
    )),
    ("movzx", entry(
        Category::DataMove,
        "Move with zero-extension — copy and clear the upper bits. Used for unsigned widening; works the same in x32 and x64.",
        "Перенос с нулевым расширением — старшие биты обнуляются. Беззнаковое расширение, одинаково в x32 и x64.",
    )),
    ("push", entry_g(
        Category::Stack,
        "Push onto the stack. **x32**: ESP -= 4, [ESP] = src. **x64**: RSP -= 8, [RSP] = src. Operand size locked to the mode (no `push eax` in x64 — only `push rax`).",
        "Положить в стек. **x32**: ESP -= 4, [ESP] = src. **x64**: RSP -= 8, [RSP] = src. Размер операнда привязан к режиму (в x64 нет `push eax` — только `push rax`).",
        "ROP gadget building block. `push reg; ret` is a manual jump that bypasses CFG / RFG mitigations — see it in exploits and rare in normal code.",
        "Кирпичик для ROP-цепочек. `push reg; ret` — ручной переход, обходит CFG/RFG-защиту — встречается в эксплоитах, в обычном коде почти не бывает.",
    )),
    ("pop", entry(
        Category::Stack,
        "Pop from the stack. **x32**: dst = [ESP], ESP += 4. **x64**: dst = [RSP], RSP += 8. Pair to `push`.",
        "Снять со стека. **x32**: dst = [ESP], ESP += 4. **x64**: dst = [RSP], RSP += 8. Парная к `push`.",
    )),
    ("pushf", entry_g(
        Category::Stack,
        "Push EFLAGS (16 or 32 bits). The 64-bit form is `pushfq` — the assembler picks based on operand-size prefix.",
        "Положить EFLAGS в стек (16 или 32 бита). 64-битная форма — `pushfq`; ассемблер выбирает по префиксу размера операнда.",
        "Anti-debug: `pushf; pop reg; test reg, 0x100` reads the Trap Flag (TF). If TF=1 a single-step debugger is attached.",
        "Анти-debug: `pushf; pop reg; test reg, 0x100` читает Trap Flag (TF). TF=1 — подключён single-step отладчик.",
    )),
    ("pushfq", entry_g(
        Category::Stack,
        "Push the full 64-bit RFLAGS register onto the stack (x64-only).",
        "Положить весь 64-битный RFLAGS в стек (только x64).",
        "Same TF-reading anti-debug as `pushf` but with the full 64-bit flags word.",
        "Та же анти-debug проверка TF, что у `pushf`, но с полным 64-битным словом флагов.",
    )),
    ("popf", entry_g(
        Category::Stack,
        "Pop the stack into EFLAGS (16 or 32 bits).",
        "Снять со стека в регистр флагов EFLAGS (16/32-битная форма).",
        "Anti-debug: `or [esp], 0x100; popf` arms the Trap Flag manually — the next instruction raises a single-step exception that the protector handles in its own SEH chain to detect debuggers.",
        "Анти-debug: `or [esp], 0x100; popf` вручную выставляет Trap Flag — следующая инструкция вызовет single-step исключение, которое протектор перехватит в своём SEH, чтобы обнаружить отладчик.",
    )),
    ("popfq", entry_g(
        Category::Stack,
        "Pop the stack into the full 64-bit RFLAGS register (x64-only).",
        "Снять со стека в полный 64-битный RFLAGS (только x64).",
        "Same TF-arming anti-debug as `popf`.",
        "Та же анти-debug установка TF, что у `popf`.",
    )),
    ("enter", entry_g(
        Category::Stack,
        "Build a stack frame. **x32**: pushes EBP, sets EBP=ESP, allocates locals. **x64**: same with RBP/RSP. Slow on modern CPUs; compilers prefer `push rbp; mov rbp, rsp; sub rsp, N`.",
        "Построить стек-фрейм. **x32**: push EBP, EBP=ESP, выделение под локальные. **x64**: то же с RBP/RSP. На современных CPU медленнее ручной пары `push rbp; mov rbp, rsp; sub rsp, N` — компиляторы её и предпочитают.",
        "Seeing `enter` in modern code = handwritten asm or a deliberate signature. MSVC / GCC / Clang never emit it.",
        "`enter` в современном коде — рукописный asm или преднамеренная сигнатура. MSVC / GCC / Clang её не генерируют.",
    )),
    ("leave", entry(
        Category::Stack,
        "Tear down a stack frame. **x32**: ESP=EBP, pop EBP. **x64**: RSP=RBP, pop RBP. Counterpart to `enter`; pairs with `ret` for the standard function epilogue.",
        "Свернуть стек-фрейм. **x32**: ESP=EBP, pop EBP. **x64**: RSP=RBP, pop RBP. Парная к `enter`; вместе с `ret` — стандартный эпилог функции.",
    )),

    // ── Arithmetic ───────────────────────────────────────────────────────
    ("add", entry(
        Category::Arithmetic,
        "Integer addition: dest += src. Sets OF/SF/ZF/AF/CF/PF. Operand width follows the mnemonic suffix or register size in x32; same in x64 with R.. registers.",
        "Целочисленное сложение: dest += src. Меняет OF/SF/ZF/AF/CF/PF. Ширина операнда по суффиксу мнемоники или размеру регистра — в x32 и x64 одинаково.",
    )),
    ("sub", entry(
        Category::Arithmetic,
        "Integer subtraction: dest -= src. Sets OF/SF/ZF/AF/CF/PF. Used both for math and to allocate stack space (`sub esp/rsp, N`).",
        "Целочисленное вычитание: dest -= src. Меняет OF/SF/ZF/AF/CF/PF. Используется и для арифметики, и для выделения места на стеке (`sub esp/rsp, N`).",
    )),
    ("adc", entry(
        Category::Arithmetic,
        "Add with carry: dest += src + CF. Multi-precision addition primitive (e.g. 128-bit add on x64 = `add lo,lo; adc hi,hi`).",
        "Сложение с переносом: dest += src + CF. Базовая операция для многоразрядной арифметики (128-битное сложение в x64 = `add lo,lo; adc hi,hi`).",
    )),
    ("sbb", entry_g(
        Category::Arithmetic,
        "Subtract with borrow: dest -= src + CF. Multi-precision counterpart to `adc`.",
        "Вычитание с заёмом: dest -= src + CF. Парная к `adc` для многоразрядной арифметики.",
        "Branch-free idiom: `sbb eax, eax` produces -1 if CF=1, else 0 — turns a flag into a -1/0 mask without a jump.",
        "Безветвевая идиома: `sbb eax, eax` даёт -1 если CF=1, иначе 0 — превращает флаг в маску -1/0 без перехода.",
    )),
    ("inc", entry_g(
        Category::Arithmetic,
        "Increment by 1. Sets OF/SF/ZF/AF/PF — but **not** CF (which is what makes it different from `add reg, 1`).",
        "Увеличить на 1. Меняет OF/SF/ZF/AF/PF, но НЕ CF — этим отличается от `add reg, 1`.",
        "x32 has 1-byte `inc reg` opcodes (0x40..0x47) — in x64 those are repurposed as REX prefixes, so `inc eax` becomes a 2-byte form. Tools that misclassify modes show garbage here.",
        "В x32 есть 1-байтные `inc reg` (0x40..0x47) — в x64 эти байты стали REX-префиксами, и `inc eax` кодируется в 2 байта. Инструменты, путающие режимы, показывают мусор.",
    )),
    ("dec", entry_g(
        Category::Arithmetic,
        "Decrement by 1. Same flag rules as `inc` (CF preserved).",
        "Уменьшить на 1. Флаги как у `inc` (CF сохраняется).",
        "Same 1-byte (x32) vs REX-prefixed (x64) split as `inc`.",
        "То же раздвоение 1-байт (x32) / REX-префикс (x64), что у `inc`.",
    )),
    ("neg", entry(
        Category::Arithmetic,
        "Two's complement negation: dest = -dest. CF=0 only when source was zero — so `neg reg; sbb reg, reg` is the classic \"is non-zero?\" → -1/0 mask trick.",
        "Изменить знак (доп. код): dest = -dest. CF=0 только если источник был 0 — так `neg reg; sbb reg, reg` даёт -1/0 маску \"не ноль?\".",
    )),
    ("mul", entry(
        Category::Arithmetic,
        "Unsigned multiply, implicit operand. **x32**: AL/AX/EAX × src → AX/DX:AX/EDX:EAX. **x64**: adds RAX × src → RDX:RAX. Sets only CF/OF.",
        "Беззнаковое умножение, неявный операнд. **x32**: AL/AX/EAX × src → AX/DX:AX/EDX:EAX. **x64**: добавляет RAX × src → RDX:RAX. Меняет только CF/OF.",
    )),
    ("imul", entry(
        Category::Arithmetic,
        "Signed multiply. The 2- and 3-operand forms (`imul rax, rbx, 7`) skip the implicit-RAX dance; available in x32 (with E.. registers) and x64.",
        "Знаковое умножение. 2- и 3-операндные формы (`imul rax, rbx, 7`) обходят неявный RAX; есть в x32 (с E..) и в x64.",
    )),
    ("div", entry_g(
        Category::Arithmetic,
        "Unsigned divide. **x32**: DX:AX / src or EDX:EAX / src. **x64**: adds RDX:RAX / src. Quotient → AX/EAX/RAX, remainder → DX/EDX/RDX. Faults #DE on divide-by-zero or quotient overflow.",
        "Беззнаковое деление. **x32**: DX:AX / src или EDX:EAX / src. **x64**: добавляется RDX:RAX / src. Частное → AX/EAX/RAX, остаток → DX/EDX/RDX. #DE при делении на 0 или переполнении частного.",
        "Anti-debug pattern: deliberate `div` by 0 to raise #DE — the protector handles the SEH itself; if a debugger swallows it instead, the protector knows it's being analysed.",
        "Анти-debug: умышленный `div` на 0 для #DE — протектор сам обрабатывает SEH; если отладчик перехватит исключение, протектор поймёт что его анализируют.",
    )),
    ("idiv", entry(
        Category::Arithmetic,
        "Signed divide. Same dividend layout as `div`. Pair with `cdq`/`cqo` to sign-extend the dividend first.",
        "Знаковое деление. Делимое формируется как у `div`. Перед ним обычно `cdq`/`cqo` для расширения знака.",
    )),
    ("cdq", entry(
        Category::Arithmetic,
        "Convert Doubleword to Quadword — sign-extend EAX into EDX:EAX. Standard pre-`idiv` setup for 32-bit signed division. Available in both x32 and x64.",
        "Расширить EAX знаком в EDX:EAX. Стандартная подготовка перед 32-битным знаковым `idiv`. Есть в x32 и x64.",
    )),
    ("cqo", entry(
        Category::Arithmetic,
        "Convert Quadword to Octword — sign-extend RAX into RDX:RAX. **x64-only**.",
        "Расширить RAX знаком в RDX:RAX. **Только x64**.",
    )),

    // ── Logic / shifts ───────────────────────────────────────────────────
    ("and", entry(
        Category::Logic,
        "Bitwise AND. Common idiom: mask off bits, or `and reg, reg` to test for zero (sets ZF). x32/x64 share the encoding.",
        "Побитовое И. Часто: маскирование битов или `and reg, reg` для проверки на ноль (выставляет ZF). x32/x64 кодируется одинаково.",
    )),
    ("or", entry(
        Category::Logic,
        "Bitwise OR. `or reg, reg` is sometimes used as a non-zero test (sets ZF). `or reg, -1` produces an all-ones value cheaply.",
        "Побитовое ИЛИ. `or reg, reg` иногда используется как тест на не-ноль (выставляет ZF). `or reg, -1` быстро даёт значение со всеми единицами.",
    )),
    ("xor", entry_g(
        Category::Logic,
        "Bitwise XOR. Idiomatic zero-out: `xor eax, eax` (smaller encoding than `mov eax, 0`, breaks the dependency chain on the renamer). Same idiom works in x64 with `xor rax, rax`.",
        "Побитовое XOR. Идиома обнуления: `xor eax, eax` (короче `mov eax, 0`, разрывает цепочку зависимостей в renamer). Аналогично в x64: `xor rax, rax`.",
        "VMProtect-style obfuscation: `xor reg, X; xor reg, X` cancels out — long sequences of paired XORs with the same constant are dead code introduced for confusion.",
        "VMProtect-обфускация: `xor reg, X; xor reg, X` гасит сама себя — длинные цепочки парных XOR с одной константой — это вставленный мёртвый код для запутывания.",
    )),
    ("not", entry(
        Category::Logic,
        "Bitwise NOT (one's complement). Flags untouched — pair with `inc` for two's complement (`not reg; inc reg` ≡ `neg reg`).",
        "Побитовое НЕ (доп. до единицы). Флаги не меняет — для дополнения до двух нужно `inc` следом (`not reg; inc reg` ≡ `neg reg`).",
    )),
    ("shl", entry(
        Category::Logic,
        "Shift Left logical — fills low bits with 0. Equivalent to multiplying by 2^count for unsigned values. The count operand is masked to 5 bits in x32, 6 bits in x64 (so `shl rax, 64` is actually `shl rax, 0`).",
        "Логический сдвиг влево — младшие биты заполняются нулями. Для беззнаковых = умножение на 2^count. Счётчик маскируется до 5 бит в x32 и 6 бит в x64 (поэтому `shl rax, 64` = `shl rax, 0`).",
    )),
    ("sal", entry(
        Category::Logic,
        "Shift Arithmetic Left — identical encoding to `shl`. The mnemonic is preserved for symmetry with `sar`.",
        "Арифметический сдвиг влево — кодировка та же, что у `shl`. Мнемоника оставлена для симметрии с `sar`.",
    )),
    ("shr", entry(
        Category::Logic,
        "Shift Right logical — fills high bits with 0. Unsigned divide by 2^count. Same count-masking rules as `shl`.",
        "Логический сдвиг вправо — старшие биты заполняются нулями. Беззнаковое деление на 2^count. Маскирование счётчика как у `shl`.",
    )),
    ("sar", entry(
        Category::Logic,
        "Shift Arithmetic Right — replicates the sign bit, preserving sign. Signed divide by 2^count (rounds toward −∞, NOT toward 0).",
        "Арифметический сдвиг вправо — старший бит дублируется, знак сохраняется. Знаковое деление на 2^count (к −∞, НЕ к 0).",
    )),
    ("rol", entry_g(
        Category::Logic,
        "Rotate Left — bits cycled out of the high end re-enter at the low end (no carry chain).",
        "Циклический сдвиг влево — биты, ушедшие со старшего конца, возвращаются с младшего (без участия CF).",
        "`rol reg, 0` is a multi-byte no-op — mutators inject zero-rotate filler that disassembles to a real opcode but does nothing.",
        "`rol reg, 0` — многобайтный no-op; мутаторы вставляют поворот на 0 как заполнитель, выглядит как реальная инструкция, но ничего не делает.",
    )),
    ("ror", entry(
        Category::Logic,
        "Rotate Right — bits cycled out of the low end re-enter at the high end. `ror` is also a building block for crypto-style scrambling in obfuscators.",
        "Циклический сдвиг вправо — биты, ушедшие с младшего конца, возвращаются со старшего. `ror` часто используют в крипто-обфускации.",
    )),
    ("rcl", entry(
        Category::Logic,
        "Rotate through Carry Left — CF participates in the rotation, useful for multi-precision shifts.",
        "Циклический сдвиг влево через CF — флаг переноса участвует, удобно для многоразрядных сдвигов.",
    )),
    ("rcr", entry(
        Category::Logic,
        "Rotate through Carry Right — counterpart to `rcl`.",
        "Циклический сдвиг вправо через CF — парный к `rcl`.",
    )),

    // ── Compare / test ───────────────────────────────────────────────────
    ("cmp", entry(
        Category::Compare,
        "Compare = subtract without storing the result; only the flags update. Followed by a Jcc to branch on the comparison. Identical in x32 and x64.",
        "Сравнение = вычитание без записи результата; меняются только флаги. Обычно за ним идёт Jcc. Одинаково в x32 и x64.",
    )),
    ("test", entry_g(
        Category::Compare,
        "Bitwise AND without storing the result; only the flags update. Idiomatic zero/non-zero check: `test reg, reg` (ZF=1 ⇔ reg=0).",
        "Побитовое И без записи результата; меняются только флаги. Идиоматичная проверка на 0: `test reg, reg` (ZF=1 ⇔ reg=0).",
        "After a `call`, `test eax, eax / je …` is the universal \"check return value for NULL\" pattern in Win API code (handles, pointers).",
        "После `call`, `test eax, eax / je …` — универсальный паттерн \"проверка на NULL\" в Win API коде (handles, указатели).",
    )),

    // ── Bit operations ───────────────────────────────────────────────────
    ("bt", entry(
        Category::BitOp,
        "Bit Test — copy the selected bit of dest into CF. Read-only.",
        "Проверка бита — выбранный бит приёмника копируется в CF. Только чтение.",
    )),
    ("bts", entry(
        Category::BitOp,
        "Bit Test and Set — copy the selected bit into CF, then set it to 1.",
        "Проверить и установить бит — текущее значение в CF, затем выставить в 1.",
    )),
    ("btr", entry(
        Category::BitOp,
        "Bit Test and Reset — copy the selected bit into CF, then clear it.",
        "Проверить и сбросить бит — текущее значение в CF, затем сбросить в 0.",
    )),
    ("btc", entry(
        Category::BitOp,
        "Bit Test and Complement — copy the selected bit into CF, then invert it.",
        "Проверить и инвертировать бит — текущее значение в CF, затем инвертировать.",
    )),
    ("bsf", entry(
        Category::BitOp,
        "Bit Scan Forward — index of the lowest set bit. ZF=1 when source is zero (result undefined).",
        "Поиск младшего единичного бита (его индекс). ZF=1 если источник = 0 (результат не определён).",
    )),
    ("bsr", entry(
        Category::BitOp,
        "Bit Scan Reverse — index of the highest set bit. ZF=1 when source is zero.",
        "Поиск старшего единичного бита (его индекс). ZF=1 если источник = 0.",
    )),
    ("popcnt", entry(
        Category::BitOp,
        "Population count — number of bits set in source. Single-cycle on modern CPUs.",
        "Подсчёт единичных битов в источнике. Одноцикловая на современных CPU.",
    )),
    ("lzcnt", entry(
        Category::BitOp,
        "Leading Zero count — number of zero bits before the highest set bit (BMI1).",
        "Число ведущих нулей до старшего единичного бита (BMI1).",
    )),
    ("tzcnt", entry(
        Category::BitOp,
        "Trailing Zero count — number of zero bits before the lowest set bit (BMI1).",
        "Число завершающих нулей до младшего единичного бита (BMI1).",
    )),

    // ── Control flow — unconditional ─────────────────────────────────────
    ("jmp", entry_g(
        Category::ControlFlow,
        "Unconditional jump. Target may be relative (immediate), register, or memory (indirect). Same encoding family in x32 and x64; near-jumps are RIP-relative in x64.",
        "Безусловный переход. Цель — относительный сдвиг, регистр или память (косвенно). Семейство кодировок общее для x32/x64; близкий переход в x64 — RIP-relative.",
        "Anti-disasm: `jmp short $+2` followed by garbage bytes — the disassembler walks into the garbage; a smart engine follows the jump and skips it. Pair with overlapping decoding for maximum confusion.",
        "Анти-disasm: `jmp short $+2`, а после — мусорные байты. Глупый дизассемблер уйдёт в мусор; умный пройдёт по переходу и пропустит. Часто с overlapping decoding для максимальной путаницы.",
    )),
    ("call", entry_g(
        Category::ControlFlow,
        "Call — push return address (EIP/RIP after this instruction), then jump to target. The callee must `ret` to come back. **x32**: 4-byte return address. **x64**: 8 bytes.",
        "Вызов — положить адрес возврата (EIP/RIP после этой инстр.) в стек и перейти. Возврат через `ret`. **x32**: 4 байта адрес. **x64**: 8.",
        "`call $+5` then `pop reg` is the classic position-independent get-IP idiom in x32 shellcode (x64 uses RIP-relative addressing instead — no need for the trick there).",
        "`call $+5` затем `pop reg` — классическая идиома получения IP в позиционно-независимом x32-шеллкоде (в x64 используется RIP-relative — этот трюк не нужен).",
    )),
    ("ret", entry_g(
        Category::ControlFlow,
        "Return — pop the return address into EIP/RIP. The optional immediate (`ret 16`) frees stack arguments after popping (stdcall convention in x32).",
        "Возврат — снять адрес возврата в EIP/RIP. Необязательный иммедиат (`ret 16`) освобождает аргументы со стека (stdcall в x32).",
        "ROP / JOP exploits chain `pop reg; ret` gadgets. In legitimate code `ret N` with non-zero N is a strong stdcall (x32) signature.",
        "ROP / JOP-эксплоиты цепляют гаджеты `pop reg; ret`. В легитимном коде `ret N` с N≠0 — сильная сигнатура stdcall (x32).",
    )),
    ("retn", entry(
        Category::ControlFlow,
        "Near return (synonym for `ret` in 32/64-bit code).",
        "Ближний возврат (синоним `ret` в 32/64-битном коде).",
    )),
    ("retf", entry(
        Category::ControlFlow,
        "Far return — pops both EIP/RIP and CS. Almost never seen outside legacy 16-bit / kernel code or transitions between code segments.",
        "Дальний возврат — снимает EIP/RIP и CS. Почти не встречается вне 16-битного / ядерного кода или переходов между сегментами.",
    )),
    ("iret", entry(
        Category::ControlFlow,
        "Interrupt return — pops EIP/RIP, CS, EFLAGS/RFLAGS (kernel-only; user-mode hits a #GP).",
        "Возврат из прерывания — снимает EIP/RIP, CS, EFLAGS/RFLAGS (только в ядре; в user mode — #GP).",
    )),
    ("loop", entry(
        Category::ControlFlow,
        "Decrement ECX (x32) or RCX (x64), branch if non-zero. Slower than a manual `dec`/`jnz` pair on modern CPUs; rare in compiler output.",
        "Уменьшить ECX (x32) или RCX (x64), переход если ≠ 0. На современных CPU медленнее ручной пары `dec`/`jnz`; компиляторы используют редко.",
    )),
    ("loope", entry(
        Category::ControlFlow,
        "`loop` plus the ZF=1 condition — keep looping while ECX/RCX ≠ 0 AND ZF=1.",
        "`loop` + условие ZF=1 — продолжать пока ECX/RCX ≠ 0 И ZF=1.",
    )),
    ("loopne", entry(
        Category::ControlFlow,
        "`loop` plus the ZF=0 condition — keep looping while ECX/RCX ≠ 0 AND ZF=0.",
        "`loop` + условие ZF=0 — продолжать пока ECX/RCX ≠ 0 И ZF=0.",
    )),

    // ── Conditional jumps (Jcc) ──────────────────────────────────────────
    ("je", entry(
        Category::ControlFlow,
        "Jump if Equal — taken when ZF=1 (the previous compare reported equality).",
        "Переход если равно — берётся при ZF=1 (предыдущее сравнение показало равенство).",
    )),
    ("jz", entry(
        Category::ControlFlow,
        "Jump if Zero — synonym of `je` (ZF=1).",
        "Переход если ноль — синоним `je` (ZF=1).",
    )),
    ("jne", entry(
        Category::ControlFlow,
        "Jump if Not Equal — taken when ZF=0.",
        "Переход если не равно — берётся при ZF=0.",
    )),
    ("jnz", entry(
        Category::ControlFlow,
        "Jump if Not Zero — synonym of `jne` (ZF=0).",
        "Переход если не ноль — синоним `jne` (ZF=0).",
    )),
    ("jl", entry(
        Category::ControlFlow,
        "Jump if Less (signed) — taken when SF≠OF.",
        "Переход если меньше (знаковое) — при SF≠OF.",
    )),
    ("jnge", entry(
        Category::ControlFlow,
        "Jump if Not Greater-or-Equal (signed) — synonym of `jl` (SF≠OF).",
        "Переход если НЕ больше-или-равно (знаковое) — синоним `jl` (SF≠OF).",
    )),
    ("jle", entry(
        Category::ControlFlow,
        "Jump if Less-or-Equal (signed) — taken when ZF=1 OR SF≠OF.",
        "Переход если меньше-или-равно (знаковое) — при ZF=1 ИЛИ SF≠OF.",
    )),
    ("jng", entry(
        Category::ControlFlow,
        "Jump if Not Greater (signed) — synonym of `jle`.",
        "Переход если НЕ больше (знаковое) — синоним `jle`.",
    )),
    ("jg", entry(
        Category::ControlFlow,
        "Jump if Greater (signed) — taken when ZF=0 AND SF=OF.",
        "Переход если больше (знаковое) — при ZF=0 И SF=OF.",
    )),
    ("jnle", entry(
        Category::ControlFlow,
        "Jump if Not Less-or-Equal (signed) — synonym of `jg`.",
        "Переход если НЕ меньше-или-равно (знаковое) — синоним `jg`.",
    )),
    ("jge", entry(
        Category::ControlFlow,
        "Jump if Greater-or-Equal (signed) — taken when SF=OF.",
        "Переход если больше-или-равно (знаковое) — при SF=OF.",
    )),
    ("jnl", entry(
        Category::ControlFlow,
        "Jump if Not Less (signed) — synonym of `jge` (SF=OF).",
        "Переход если НЕ меньше (знаковое) — синоним `jge` (SF=OF).",
    )),
    ("jb", entry(
        Category::ControlFlow,
        "Jump if Below (unsigned) — taken when CF=1.",
        "Переход если меньше (беззнаковое) — при CF=1.",
    )),
    ("jc", entry(
        Category::ControlFlow,
        "Jump if Carry — synonym of `jb` (CF=1).",
        "Переход если CF=1 — синоним `jb`.",
    )),
    ("jnae", entry(
        Category::ControlFlow,
        "Jump if Not Above-or-Equal (unsigned) — synonym of `jb` (CF=1).",
        "Переход если НЕ больше-или-равно (беззнаковое) — синоним `jb` (CF=1).",
    )),
    ("jbe", entry(
        Category::ControlFlow,
        "Jump if Below-or-Equal (unsigned) — taken when CF=1 OR ZF=1.",
        "Переход если меньше-или-равно (беззнаковое) — при CF=1 ИЛИ ZF=1.",
    )),
    ("jna", entry(
        Category::ControlFlow,
        "Jump if Not Above (unsigned) — synonym of `jbe`.",
        "Переход если НЕ больше (беззнаковое) — синоним `jbe`.",
    )),
    ("ja", entry(
        Category::ControlFlow,
        "Jump if Above (unsigned) — taken when CF=0 AND ZF=0.",
        "Переход если больше (беззнаковое) — при CF=0 И ZF=0.",
    )),
    ("jnbe", entry(
        Category::ControlFlow,
        "Jump if Not Below-or-Equal (unsigned) — synonym of `ja`.",
        "Переход если НЕ меньше-или-равно (беззнаковое) — синоним `ja`.",
    )),
    ("jae", entry(
        Category::ControlFlow,
        "Jump if Above-or-Equal (unsigned) — taken when CF=0.",
        "Переход если больше-или-равно (беззнаковое) — при CF=0.",
    )),
    ("jnc", entry(
        Category::ControlFlow,
        "Jump if No Carry — synonym of `jae` (CF=0).",
        "Переход если CF=0 — синоним `jae`.",
    )),
    ("jnb", entry(
        Category::ControlFlow,
        "Jump if Not Below (unsigned) — synonym of `jae` (CF=0).",
        "Переход если НЕ меньше (беззнаковое) — синоним `jae` (CF=0).",
    )),
    ("js", entry(
        Category::ControlFlow,
        "Jump if Sign — taken when SF=1 (result was negative).",
        "Переход если знак — при SF=1 (результат был отрицательным).",
    )),
    ("jns", entry(
        Category::ControlFlow,
        "Jump if No Sign — taken when SF=0 (result was non-negative).",
        "Переход если нет знака — при SF=0 (результат неотрицательный).",
    )),
    ("jo", entry(
        Category::ControlFlow,
        "Jump if Overflow — taken when OF=1 (signed overflow happened).",
        "Переход при переполнении — OF=1 (произошло знаковое переполнение).",
    )),
    ("jno", entry(
        Category::ControlFlow,
        "Jump if No Overflow — taken when OF=0.",
        "Переход если нет переполнения — OF=0.",
    )),
    ("jp", entry(
        Category::ControlFlow,
        "Jump if Parity (even) — taken when PF=1 (low byte has an even number of set bits).",
        "Переход если чётность — PF=1 (в младшем байте чётное число единиц).",
    )),
    ("jpe", entry(
        Category::ControlFlow,
        "Jump if Parity Even — synonym of `jp`.",
        "Переход при чётности — синоним `jp`.",
    )),
    ("jnp", entry(
        Category::ControlFlow,
        "Jump if No Parity — taken when PF=0 (odd number of set bits).",
        "Переход если нечётность — PF=0 (нечётное число единиц).",
    )),
    ("jpo", entry(
        Category::ControlFlow,
        "Jump if Parity Odd — synonym of `jnp`.",
        "Переход при нечётности — синоним `jnp`.",
    )),
    ("jcxz", entry(
        Category::ControlFlow,
        "Jump if CX=0 (16-bit operand size). Tests CX directly, not flags.",
        "Переход если CX=0 (16-битный режим). Проверяет регистр напрямую, не флаги.",
    )),
    ("jecxz", entry(
        Category::ControlFlow,
        "Jump if ECX=0 (32-bit operand size). Tests ECX directly, not flags.",
        "Переход если ECX=0 (32-битный режим). Проверяет регистр напрямую, не флаги.",
    )),
    ("jrcxz", entry(
        Category::ControlFlow,
        "Jump if RCX=0 (64-bit operand size). Tests RCX directly, not flags. **x64-only**.",
        "Переход если RCX=0 (64-битный режим). Проверяет регистр напрямую, не флаги. **Только x64**.",
    )),

    // ── String operations ────────────────────────────────────────────────
    ("movs", entry(
        Category::String,
        "MOVe String. **x32**: copy [ESI] → [EDI], advance both. **x64**: [RSI] → [RDI]. Element size from suffix (b/w/d/q). DF controls direction. Pair with `rep` for memcpy.",
        "Копирование строки. **x32**: [ESI] → [EDI], оба сдвигаются. **x64**: [RSI] → [RDI]. Размер элемента из суффикса (b/w/d/q). Направление от DF. С `rep` = memcpy.",
    )),
    ("stos", entry(
        Category::String,
        "STOre String. **x32**: write AL/AX/EAX into [EDI], advance EDI. **x64**: same with RAX/[RDI]. Pair with `rep` for memset.",
        "Запись строки. **x32**: AL/AX/EAX в [EDI], сдвинуть EDI. **x64**: то же с RAX/[RDI]. С `rep` = memset.",
    )),
    ("lods", entry(
        Category::String,
        "LOaD String — read [ESI/RSI] into AL/AX/EAX/RAX, advance the index register.",
        "Чтение строки — [ESI/RSI] в AL/AX/EAX/RAX, сдвинуть индексный регистр.",
    )),
    ("scas", entry(
        Category::String,
        "SCAn String — compare AL/AX/EAX/RAX against [EDI/RDI] (sets flags), advance EDI/RDI. Pair with `repne` for `strchr`.",
        "Поиск в строке — сравнить AL/AX/EAX/RAX с [EDI/RDI] (выставить флаги), сдвинуть EDI/RDI. С `repne` = `strchr`.",
    )),
    ("cmps", entry(
        Category::String,
        "CoMPare Strings — compare [ESI/RSI] vs [EDI/RDI], advance both. Pair with `repe` for `memcmp`.",
        "Сравнение строк — [ESI/RSI] vs [EDI/RDI], оба сдвигаются. С `repe` = `memcmp`.",
    )),
    ("rep", entry(
        Category::String,
        "Repeat prefix — execute the string instruction ECX (x32) / RCX (x64) times, decrementing the counter each iteration.",
        "Префикс повторения — выполнить строковую инструкцию ECX (x32) / RCX (x64) раз, уменьшая счётчик на каждой итерации.",
    )),
    ("repe", entry(
        Category::String,
        "Repeat-while-Equal — like `rep` but also exits when ZF=0. Pairs with `cmps` / `scas`.",
        "Повторение пока равно — как `rep`, но выход и при ZF=0. С `cmps` / `scas`.",
    )),
    ("repz", entry(
        Category::String,
        "Repeat-while-Zero — synonym of `repe`.",
        "Повторение пока ZF=1 — синоним `repe`.",
    )),
    ("repne", entry(
        Category::String,
        "Repeat-while-Not-Equal — like `rep` but also exits when ZF=1. Pairs with `scas` for `strlen`.",
        "Повторение пока не равно — как `rep`, но выход и при ZF=1. С `scas` = `strlen`.",
    )),
    ("repnz", entry(
        Category::String,
        "Repeat-while-Not-Zero — synonym of `repne`.",
        "Повторение пока ZF=0 — синоним `repne`.",
    )),

    // ── System / synchronisation ─────────────────────────────────────────
    ("syscall", entry(
        Category::System,
        "Fast system call (x64-only) — switches to ring 0, RIP→RCX, RFLAGS→R11, jumps to LSTAR MSR. Linux x64 system-call path.",
        "Быстрый системный вызов (только x64) — переход в ring 0, RIP→RCX, RFLAGS→R11, прыжок по LSTAR MSR. Путь системных вызовов в Linux x64.",
    )),
    ("sysret", entry(
        Category::System,
        "Return from `syscall` — RCX→RIP, R11→RFLAGS, drops back to ring 3. **x64-only**.",
        "Возврат из `syscall` — RCX→RIP, R11→RFLAGS, обратно в ring 3. **Только x64**.",
    )),
    ("int", entry_g(
        Category::System,
        "Software interrupt — calls the IDT vector. `int 3` is the debugger breakpoint (single-byte 0xCC).",
        "Программное прерывание — вызов вектора IDT. `int 3` = брейкпоинт отладчика (один байт 0xCC).",
        "Anti-debug: `int 3` raises a breakpoint exception even without a software bp set; `int 2D` (Windows kernel debugger probe) goes one further — when no debugger is attached, the next byte is consumed; when one is, it isn't. Both probe \"is a debugger watching\".",
        "Анти-debug: `int 3` вызывает breakpoint-исключение даже без программной точки; `int 2D` (Windows-kernel debugger probe) идёт дальше — без отладчика следующий байт пропускается, с отладчиком — нет. Оба способа проверяют \"следит ли отладчик\".",
    )),
    ("into", entry(
        Category::System,
        "Trap on overflow — fires `int 4` if OF=1. **x32-only** — invalid opcode in x64 mode.",
        "Прерывание при переполнении — `int 4` если OF=1. **Только x32** — в x64 это недействительный опкод.",
    )),
    ("hlt", entry(
        Category::System,
        "Halt the CPU until the next interrupt. Ring-0 only — `#GP` in user mode.",
        "Остановить CPU до следующего прерывания. Только ring 0 — `#GP` в user mode.",
    )),
    ("sti", entry(
        Category::System,
        "Set Interrupt-flag — re-enable maskable interrupts (ring 0).",
        "Установить флаг прерываний — разрешить маскируемые прерывания (ring 0).",
    )),
    ("cli", entry(
        Category::System,
        "Clear Interrupt-flag — disable maskable interrupts (ring 0).",
        "Сбросить флаг прерываний — запретить маскируемые прерывания (ring 0).",
    )),
    ("lock", entry(
        Category::System,
        "LOCK prefix — make the next read-modify-write instruction atomic on the system bus. Used for `Atomic*` operations in C++/Rust on x32 and x64 alike.",
        "Префикс LOCK — сделать следующую RMW-инструкцию атомарной на системной шине. Используется для `Atomic*` в C++/Rust одинаково в x32 и x64.",
    )),
    ("pause", entry(
        Category::System,
        "Hint to spin-wait loops — improves SMT efficiency and saves power; mandatory in well-written `while busy {}` loops.",
        "Подсказка в spin-циклах — повышает эффективность SMT и экономит питание; обязательна в правильных `while busy {}`.",
    )),
    ("cpuid", entry_g(
        Category::System,
        "CPU identification — feature/version query. EAX selects the leaf, results return in EAX/EBX/ECX/EDX. Identical encoding in x32 and x64.",
        "Идентификация CPU — запрос возможностей/версии. EAX задаёт лист, ответы в EAX/EBX/ECX/EDX. Кодировка одинаковая в x32 и x64.",
        "Anti-VM / anti-hypervisor: leaf 1 ECX bit 31 = \"hypervisor present\"; leaves 0x40000000+ return vendor strings (\"VMwareVMware\", \"KVMKVMKVM\", \"Microsoft Hv\", …). Protectors check these to refuse running inside an analysis VM.",
        "Анти-VM / анти-hypervisor: лист 1 ECX бит 31 = \"hypervisor present\"; листы 0x40000000+ возвращают vendor-строки (\"VMwareVMware\", \"KVMKVMKVM\", \"Microsoft Hv\", …). Протекторы проверяют это и отказываются работать в analysis VM.",
    )),
    ("rdtsc", entry_g(
        Category::System,
        "Read TimeStamp Counter — returns the cycle counter in EDX:EAX. Not strictly serialised on its own; pair with `lfence` if you need ordering.",
        "Прочитать счётчик тактов — результат в EDX:EAX. Сам по себе не сериализуем; для упорядочивания нужен `lfence`.",
        "Anti-debug: `rdtsc; … work …; rdtsc; sub` measures elapsed cycles. A debugger's single-step / interrupt overhead inflates the delta, so a large value reveals analysis. `rdtscp` adds an implicit barrier — same trick, slightly more reliable timing.",
        "Анти-debug: `rdtsc; … код …; rdtsc; sub` мерит разницу тактов. Single-step / прерывания отладчика её раздувают — большая дельта выдаёт анализ. `rdtscp` добавляет неявный барьер — тот же приём, чуть надёжнее.",
    )),
    ("rdrand", entry(
        Category::System,
        "Read hardware random number — CF=1 on success. Slower than software PRNGs; rarely seen in hot paths.",
        "Чтение аппаратного случайного числа — CF=1 при успехе. Медленнее программного PRNG; редко в hot path.",
    )),

    // ── Conditional set / move ───────────────────────────────────────────
    ("setz", entry(
        Category::ConditionalMove,
        "Set byte if ZF=1, else clear — turns the equality flag into a 0/1 byte without branching.",
        "Установить байт если ZF=1, иначе обнулить — превращает флаг равенства в 0/1 без перехода.",
    )),
    ("sete", entry(
        Category::ConditionalMove,
        "Set byte if Equal — synonym of `setz`.",
        "Установить байт если равно — синоним `setz`.",
    )),
    ("setnz", entry(
        Category::ConditionalMove,
        "Set byte if ZF=0, else clear.",
        "Установить байт если ZF=0, иначе обнулить.",
    )),
    ("setne", entry(
        Category::ConditionalMove,
        "Set byte if Not Equal — synonym of `setnz`.",
        "Установить байт если не равно — синоним `setnz`.",
    )),
    ("cmove", entry(
        Category::ConditionalMove,
        "Conditional MOVe if Equal (ZF=1) — branch-free assignment. Reads source unconditionally; that can matter for memory operands (no fault avoidance).",
        "Условный перенос если равно (ZF=1) — присваивание без перехода. Источник читается всегда; важно для операндов в памяти (нет защиты от #PF).",
    )),
    ("cmovz", entry(
        Category::ConditionalMove,
        "Conditional MOVe if Zero — synonym of `cmove`.",
        "Условный перенос если ноль — синоним `cmove`.",
    )),
    ("cmovne", entry(
        Category::ConditionalMove,
        "Conditional MOVe if Not Equal (ZF=0).",
        "Условный перенос если не равно (ZF=0).",
    )),
    ("cmovnz", entry(
        Category::ConditionalMove,
        "Conditional MOVe if Not Zero — synonym of `cmovne`.",
        "Условный перенос если не ноль — синоним `cmovne`.",
    )),

    // ── Misc ─────────────────────────────────────────────────────────────
    ("nop", entry_g(
        Category::Misc,
        "No-OPeration — used for alignment padding and hot-patch space. Multi-byte forms exist (0F 1F …) for longer pads.",
        "Пустая инструкция — выравнивание и место под hot-patch. Есть многобайтные формы (0F 1F …) для длинных пропусков.",
        "Mutators inject NOP-equivalent sequences (`xchg eax, eax`, `lea reg, [reg+0]`, `mov reg, reg`, `rol reg, 0`) to inflate code without changing behaviour. A long stretch of \"different\" instructions that all read like NOPs is a strong obfuscation signal.",
        "Мутаторы вставляют NOP-эквиваленты (`xchg eax, eax`, `lea reg, [reg+0]`, `mov reg, reg`, `rol reg, 0`) чтобы раздуть код без смены поведения. Длинная серия \"разных\" инструкций, фактически = NOP — сильный сигнал обфускации.",
    )),
    ("ud2", entry(
        Category::Misc,
        "Undefined Instruction — guaranteed to raise `#UD`. Compilers emit it after unreachable code or trap intrinsics (Rust's `panic!` lands here in release).",
        "Неопределённая инструкция — гарантированно вызывает `#UD`. Компиляторы ставят её после unreachable / trap (Rust `panic!` в release заканчивается здесь).",
    )),
    ("prefetch", entry(
        Category::Misc,
        "Hint the CPU to fetch a cache line — speculative; never faults on a bad address.",
        "Подсказка CPU подгрузить кэш-линию — спекулятивно; не падает при плохом адресе.",
    )),
    ("xlat", entry(
        Category::Misc,
        "Table lookup: AL = [(R/E)BX + AL]. Holdover from the 8086 era; rarely emitted by modern compilers — its appearance in modern code usually means handwritten asm or a niche obfuscator.",
        "Поиск в таблице: AL = [(R/E)BX + AL]. Реликт эпохи 8086; компиляторы почти не используют — её появление в современном коде обычно означает ручной asm или нишевый обфускатор.",
    )),
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
        // Tooltip lines wrap awkwardly past ~280 chars; lock that as a soft cap
        // so future entries stay readable. Dual-mode descriptions (x32 + x64
        // notes) need the headroom over the 200-char baseline.
        for (mnemonic, info) in MNEMONICS {
            assert!(
                info.en.chars().count() <= 320,
                "EN description for `{mnemonic}` is too long ({} chars)",
                info.en.chars().count(),
            );
            assert!(
                info.ru.chars().count() <= 320,
                "RU description for `{mnemonic}` is too long ({} chars)",
                info.ru.chars().count(),
            );
            if let Some(g) = info.gotcha_en {
                assert!(g.chars().count() <= 360, "EN gotcha for `{mnemonic}` too long");
            }
            if let Some(g) = info.gotcha_ru {
                assert!(g.chars().count() <= 360, "RU gotcha for `{mnemonic}` too long");
            }
        }
    }

    #[test]
    fn flow_control_jcc_family_present() {
        // Smoke-test the conditional-jump family — every entry must
        // reference a flag (CF / ZF / SF / OF / PF) so the user sees
        // *why* the branch is taken, not just the synonym.
        for jcc in [
            "je", "jne", "jl", "jle", "jg", "jge", "jb", "jbe", "ja", "jae",
            "js", "jns", "jo", "jno", "jp", "jnp",
        ] {
            let info = lookup(jcc).unwrap_or_else(|| panic!("missing Jcc entry: {jcc}"));
            let en = info.en;
            assert!(
                en.contains("CF") || en.contains("ZF") || en.contains("SF")
                    || en.contains("OF") || en.contains("PF"),
                "Jcc `{jcc}` must mention a flag in its EN description, got: {en}",
            );
        }
    }

    #[test]
    fn key_anti_re_mnemonics_carry_gotchas() {
        // The whole point of the gotcha column is to flag the mnemonics
        // protectors / mutators / anti-debug love. Lock the canonical set
        // so a future "compact this catalogue" pass doesn't strip them.
        for m in [
            "rdtsc", "cpuid", "int", "pushf", "popf", "pushfq", "popfq",
            "xor", "rol", "nop", "lea", "jmp", "call", "ret",
            "div", "push", "xchg", "mov", "inc", "dec", "sbb", "test",
            "enter",
        ] {
            let info = lookup(m).unwrap_or_else(|| panic!("missing entry: {m}"));
            assert!(
                info.gotcha_en.is_some() && info.gotcha_ru.is_some(),
                "anti-RE mnemonic `{m}` must have both EN and RU gotcha lines",
            );
        }
    }

    #[test]
    fn dual_mode_x32_x64_coverage() {
        // Spot-check that mnemonics whose semantics differ across 32-bit
        // and 64-bit modes mention both — the catalogue's "dual-mode
        // cinematic" promise.
        for (mnemonic, regs) in [
            ("push",  &["ESP", "RSP"][..]),
            ("pop",   &["ESP", "RSP"][..]),
            ("call",  &["EIP", "RIP"][..]),
            ("ret",   &["EIP", "RIP"][..]),
            ("syscall", &["x64"][..]),    // x64-only — must say so
            ("movsxd",  &["x64"][..]),    // x64-only
            ("cqo",     &["x64"][..]),    // x64-only
            ("into",    &["x32"][..]),    // x32-only
        ] {
            let info = lookup(mnemonic).unwrap();
            for token in regs {
                assert!(
                    info.en.contains(token),
                    "EN description of `{mnemonic}` should mention `{token}`, got: {}",
                    info.en,
                );
                assert!(
                    info.ru.contains(token),
                    "RU description of `{mnemonic}` should mention `{token}`, got: {}",
                    info.ru,
                );
            }
        }
    }
}
