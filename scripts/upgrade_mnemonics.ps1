#!/usr/bin/env pwsh
# Upgrade 36 mnemonic entries from entry/entry_g to entry_t/entry_gt

$file = "D:\GitHub\Rust_Projects\Dear-imgui-custom-mod\crate\src\disasm_view\mnemonic.rs"
$content = Get-Content $file -Raw -Encoding UTF8
$report = [System.Collections.Generic.List[string]]::new()

function rep {
    param([string]$n, [string]$o, [string]$nw, [ref]$c)
    if ($c.Value.Contains($o)) {
        $c.Value = $c.Value.Replace($o, $nw)
        $script:report.Add("[OK] $n")
    } else {
        $script:report.Add("[MISS] $n")
    }
}

# ── BATCH A: leave, retn, retf, iret ──────────────────────────────────────────

rep "leave" @'
    (
        "leave",
        entry(
            Category::Stack,
            "Tear down a stack frame. **x32**: ESP=EBP, pop EBP. **x64**: RSP=RBP, pop RBP. Counterpart to `enter`; pairs with `ret` for the standard function epilogue.",
            "Свернуть стек-фрейм. **x32**: ESP=EBP, pop EBP. **x64**: RSP=RBP, pop RBP. Парная к `enter`; вместе с `ret` — стандартный эпилог функции.",
        ),
    ),
'@ @'
    (
        "leave",
        entry_t(
            Category::Stack,
            "Tear down a stack frame. **x32**: ESP=EBP, pop EBP. **x64**: RSP=RBP, pop RBP. Counterpart to `enter`; pairs with `ret` for the standard function epilogue.",
            "Свернуть стек-фрейм. **x32**: ESP=EBP, pop EBP. **x64**: RSP=RBP, pop RBP. Парная к `enter`; вместе с `ret` — стандартный эпилог функции.",
            super::HintTiers {
                compact_en: "Tear down frame: SP=BP then pop BP; pairs with `ret` in epilogue.",
                compact_ru: "Свернуть фрейм: SP=BP, затем pop BP; пара `ret` в эпилоге.",
                educational_en: "\
`leave` collapses the current stack frame in two micro-ops: `mov rsp, rbp` (restore the \
stack pointer, freeing all local storage), then `pop rbp` (restore the caller's frame \
pointer). In **x32**: `ESP = EBP; pop EBP`. In **x64**: `RSP = RBP; pop RBP`. \
It is the counterpart to `enter` but, unlike `enter`, `leave` is routinely emitted by \
GCC, Clang, and MSVC in function epilogues before `ret`. \
A `leave; ret` sequence is the canonical epilogue when frame-pointer omission (`-fomit-\
frame-pointer`) is disabled.",
                educational_ru: "\
`leave` сворачивает стек-фрейм двумя микрооперациями: `mov rsp, rbp` (восстанавливает \
указатель стека, освобождая локальные переменные), затем `pop rbp` (восстанавливает \
frame pointer вызывающего). В **x32**: `ESP = EBP; pop EBP`. В **x64**: `RSP = RBP; pop RBP`. \
Симметрична `enter`, но, в отличие от неё, генерируется GCC, Clang и MSVC в эпилогах \
функций. Пара `leave; ret` — канонический эпилог при отключённом `-fomit-frame-pointer`.",
            },
        ),
    ),
'@ ([ref]$content)

rep "retn" @'
    (
        "retn",
        entry(
            Category::ControlFlow,
            "Near return (synonym for `ret` in 32/64-bit code).",
            "Ближний возврат (синоним `ret` в 32/64-битном коде).",
        ),
    ),
'@ @'
    (
        "retn",
        entry_t(
            Category::ControlFlow,
            "Near return (synonym for `ret` in 32/64-bit code).",
            "Ближний возврат (синоним `ret` в 32/64-битном коде).",
            super::HintTiers {
                compact_en: "Near return — pop RIP from stack; IDA/Ghidra spelling of `ret`.",
                compact_ru: "Ближний возврат — снять RIP со стека; написание IDA/Ghidra для `ret`.",
                educational_en: "\
`retn` is the IDA Pro / Ghidra disassembler spelling of the near-return instruction, \
identical in encoding to `ret` (`0xC3` or `0xC2 imm16`). \
In **x32**: `EIP = [ESP]; ESP += 4`. In **x64**: `RIP = [RSP]; RSP += 8`. \
The optional immediate (`retn N`) is the stdcall callee-cleanup byte count: \
`EIP = [ESP]; ESP += 4 + N`. \
Treat `retn` and `ret` identically for analysis — the name difference is purely \
a disassembler convention.",
                educational_ru: "\
`retn` — написание ближнего возврата в дизассемблерах IDA Pro / Ghidra, идентичное по \
кодировке (`0xC3` или `0xC2 imm16`) стандартной мнемонике `ret`. \
В **x32**: `EIP = [ESP]; ESP += 4`. В **x64**: `RIP = [RSP]; RSP += 8`. \
Опциональный иммедиат (`retn N`) — очистка стека по stdcall: `EIP = [ESP]; ESP += 4 + N`. \
Для анализа `retn` и `ret` эквивалентны — разница лишь в соглашении дизассемблера.",
            },
        ),
    ),
'@ ([ref]$content)

rep "retf" @'
    (
        "retf",
        entry(
            Category::ControlFlow,
            "Far return — pops both EIP/RIP and CS. Almost never seen outside legacy 16-bit / kernel code or transitions between code segments.",
            "Дальний возврат — снимает EIP/RIP и CS. Почти не встречается вне 16-битного / ядерного кода или переходов между сегментами.",
        ),
    ),
'@ @'
    (
        "retf",
        entry_t(
            Category::ControlFlow,
            "Far return — pops both EIP/RIP and CS. Almost never seen outside legacy 16-bit / kernel code or transitions between code segments.",
            "Дальний возврат — снимает EIP/RIP и CS. Почти не встречается вне 16-битного / ядерного кода или переходов между сегментами.",
            super::HintTiers {
                compact_en: "Far return — pop EIP/RIP then CS; crosses privilege/segment boundary.",
                compact_ru: "Дальний возврат — pop EIP/RIP и CS; пересекает границу привилегий.",
                educational_en: "\
`retf` pops two items from the stack: the return offset into EIP/RIP, then the segment \
selector into CS. In protected-mode x32 this also performs a CPL check — used for \
OS ring transitions (e.g. ring 0 to ring 3 on task return). \
In x64 segment registers are largely vestigial; `retf` appears mainly in hypervisor code, \
firmware, or deliberate obfuscation. \
`retf` in a user-mode x64 binary strongly suggests obfuscation or a hand-written shellcode stub.",
                educational_ru: "\
`retf` снимает со стека два значения: смещение возврата в EIP/RIP, затем селектор \
сегмента в CS. В защищённом режиме x32 дополнительно проверяет CPL — применяется при \
переходах привилегий ОС (например, ring 0 → ring 3). \
В x64 сегментные регистры рудиментарны; `retf` встречается главным образом в коде \
гипервизора, прошивке или намеренной обфускации. \
`retf` в user-mode x64-бинаре — признак обфускации или ручного шеллкода.",
            },
        ),
    ),
'@ ([ref]$content)

rep "iret" @'
    (
        "iret",
        entry(
            Category::ControlFlow,
            "Interrupt return — pops EIP/RIP, CS, EFLAGS/RFLAGS (kernel-only; user-mode hits a #GP).",
            "Возврат из прерывания — снимает EIP/RIP, CS, EFLAGS/RFLAGS (только в ядре; в user mode — #GP).",
        ),
    ),
'@ @'
    (
        "iret",
        entry_t(
            Category::ControlFlow,
            "Interrupt return — pops EIP/RIP, CS, EFLAGS/RFLAGS (kernel-only; user-mode hits a #GP).",
            "Возврат из прерывания — снимает EIP/RIP, CS, EFLAGS/RFLAGS (только в ядре; в user mode — #GP).",
            super::HintTiers {
                compact_en: "Interrupt return — pop EIP/RIP, CS, EFLAGS/RFLAGS (ring 0 only).",
                compact_ru: "Возврат из прерывания — pop EIP/RIP, CS, EFLAGS/RFLAGS (ring 0).",
                educational_en: "\
`iret` (`iretd` in 32-bit, `iretq` in 64-bit) atomically pops EIP/RIP, then CS, then \
EFLAGS/RFLAGS from the kernel stack — the counterpart to the IDT entry push sequence. \
In x32 protected mode the CPU validates the new CS RPL for privilege transitions. \
In x64 (`iretq`) an additional RSP and SS are popped when returning to a less-privileged \
level, restoring the full inter-privilege frame. \
Executing `iret` in user mode raises #GP; its presence in user-land code signals \
obfuscation (it faults unless behind an SEH/signal handler that catches #GP).",
                educational_ru: "\
`iret` (`iretd` в 32-бит, `iretq` в 64-бит) атомарно снимает EIP/RIP, CS и EFLAGS/RFLAGS \
с ядерного стека — пара к последовательности push в точке входа IDT. \
В x32 (защищённый режим) CPU проверяет RPL нового CS для перехода привилегий. \
В x64 (`iretq`) при возврате на менее привилегированный уровень дополнительно снимаются \
RSP и SS, восстанавливая полный межуровневый фрейм. \
В user mode вызывает #GP; `iret` в пользовательском коде — признак обфускации.",
            },
        ),
    ),
'@ ([ref]$content)

# ── BATCH B: string ops ────────────────────────────────────────────────────────

rep "movs" @'
    (
        "movs",
        entry(
            Category::String,
            "MOVe String. **x32**: copy [ESI] → [EDI], advance both. **x64**: [RSI] → [RDI]. Element size from suffix (b/w/d/q). DF controls direction. Pair with `rep` for memcpy.",
            "Копирование строки. **x32**: [ESI] → [EDI], оба сдвигаются. **x64**: [RSI] → [RDI]. Размер элемента из суффикса (b/w/d/q). Направление от DF. С `rep` = memcpy.",
        ),
    ),
'@ @'
    (
        "movs",
        entry_t(
            Category::String,
            "MOVe String. **x32**: copy [ESI] → [EDI], advance both. **x64**: [RSI] → [RDI]. Element size from suffix (b/w/d/q). DF controls direction. Pair with `rep` for memcpy.",
            "Копирование строки. **x32**: [ESI] → [EDI], оба сдвигаются. **x64**: [RSI] → [RDI]. Размер элемента из суффикса (b/w/d/q). Направление от DF. С `rep` = memcpy.",
            super::HintTiers {
                compact_en: "Move string (DS:RSI → ES:RDI, advance per DF).",
                compact_ru: "Копировать строку (DS:RSI → ES:RDI, сдвиг по DF).",
                educational_en: "\
`movs` copies one element from implicit source DS:RSI (x32: DS:ESI) to implicit destination \
ES:RDI (x32: ES:EDI), then advances both pointers by the element size. \
Suffix selects size: `movsb` (1 byte), `movsw` (2), `movsd` (4), `movsq` (8, x64 only). \
The Direction Flag (`DF`) controls the advance direction: `DF=0` (cleared by `cld`) \
increments both pointers; `DF=1` (set by `std`) decrements them for reverse copies. \
Paired with `rep` (RCX iterations), `rep movsb` is the modern fast-path for medium \
`memcpy` on Intel CPUs with ERMS / FSRM microcode acceleration.",
                educational_ru: "\
`movs` копирует один элемент из неявного источника DS:RSI (x32: DS:ESI) в неявный \
приёмник ES:RDI (x32: ES:EDI) и сдвигает оба указателя на размер элемента. \
Суффикс задаёт размер: `movsb` (1 байт), `movsw` (2), `movsd` (4), `movsq` (8, x64). \
Флаг `DF` определяет направление: `DF=0` (сброшен `cld`) — оба растут; \
`DF=1` (установлен `std`) — уменьшаются, что даёт реверсное копирование. \
В паре с `rep` (`RCX` итераций) `rep movsb` — быстрый путь компилятора для `memcpy` \
среднего размера на Intel с поддержкой ERMS / FSRM.",
            },
        ),
    ),
'@ ([ref]$content)

rep "stos" @'
    (
        "stos",
        entry(
            Category::String,
            "STOre String. **x32**: write AL/AX/EAX into [EDI], advance EDI. **x64**: same with RAX/[RDI]. Pair with `rep` for memset.",
            "Запись строки. **x32**: AL/AX/EAX в [EDI], сдвинуть EDI. **x64**: то же с RAX/[RDI]. С `rep` = memset.",
        ),
    ),
'@ @'
    (
        "stos",
        entry_t(
            Category::String,
            "STOre String. **x32**: write AL/AX/EAX into [EDI], advance EDI. **x64**: same with RAX/[RDI]. Pair with `rep` for memset.",
            "Запись строки. **x32**: AL/AX/EAX в [EDI], сдвинуть EDI. **x64**: то же с RAX/[RDI]. С `rep` = memset.",
            super::HintTiers {
                compact_en: "Store AL/AX/EAX/RAX → ES:RDI, advance RDI per DF.",
                compact_ru: "Записать AL/AX/EAX/RAX → ES:RDI, сдвинуть RDI по DF.",
                educational_en: "\
`stos` writes the accumulator (AL / AX / EAX / RAX, chosen by suffix) to the memory \
location at ES:RDI (x32: ES:EDI), then advances RDI by the element size per `DF`. \
The data source is solely the accumulator — no second memory operand exists. \
Paired with `rep` and RCX holding the byte count, `rep stosb` is the canonical `memset` \
lowering: load the fill byte into AL, the count into RCX, emit one `rep stosb`. \
On Intel ERMS / FSRM-capable CPUs this outperforms scalar loops for buffers in the \
kilobyte range.",
                educational_ru: "\
`stos` записывает аккумулятор (AL / AX / EAX / RAX — по суффиксу) по адресу ES:RDI \
(x32: ES:EDI) и сдвигает RDI на размер элемента по `DF`. \
Источник данных — только аккумулятор, второго операнда в памяти нет. \
В паре с `rep` и счётчиком в RCX `rep stosb` — канонический `memset`: \
кладём байт-заполнитель в AL, длину в RCX, испускаем один `rep stosb`. \
На Intel с ERMS / FSRM это быстрее скалярного цикла для буферов от нескольких килобайт.",
            },
        ),
    ),
'@ ([ref]$content)

rep "lods" @'
    (
        "lods",
        entry(
            Category::String,
            "LOaD String — read [ESI/RSI] into AL/AX/EAX/RAX, advance the index register.",
            "Чтение строки — [ESI/RSI] в AL/AX/EAX/RAX, сдвинуть индексный регистр.",
        ),
    ),
'@ @'
    (
        "lods",
        entry_t(
            Category::String,
            "LOaD String — read [ESI/RSI] into AL/AX/EAX/RAX, advance the index register.",
            "Чтение строки — [ESI/RSI] в AL/AX/EAX/RAX, сдвинуть индексный регистр.",
            super::HintTiers {
                compact_en: "Load DS:RSI → AL/AX/EAX/RAX, advance RSI per DF.",
                compact_ru: "Загрузить DS:RSI → AL/AX/EAX/RAX, сдвинуть RSI по DF.",
                educational_en: "\
`lods` reads one element from DS:RSI (x32: DS:ESI) into the accumulator AL / AX / EAX / \
RAX (suffix-selected: `lodsb`, `lodsw`, `lodsd`, `lodsq`), then advances RSI per `DF`. \
Unlike `movs` and `stos`, `lods` is rarely used with `rep` because each iteration \
overwrites the accumulator, leaving only the last value. \
It appears in hand-written loops that load and transform each element in sequence, \
often paired with a subsequent computation and `stos` write.",
                educational_ru: "\
`lods` читает один элемент из DS:RSI (x32: DS:ESI) в аккумулятор AL / AX / EAX / RAX \
(по суффиксу: `lodsb`, `lodsw`, `lodsd`, `lodsq`) и сдвигает RSI по `DF`. \
В отличие от `movs` и `stos`, с `rep` почти не используется: каждая итерация \
перезаписывает аккумулятор, оставляя лишь последнее значение. \
Встречается в рукописных циклах, загружающих и преобразующих элементы поочерёдно, \
нередко в паре с вычислением и записью через `stos`.",
            },
        ),
    ),
'@ ([ref]$content)

rep "scas" @'
    (
        "scas",
        entry(
            Category::String,
            "SCAn String — compare AL/AX/EAX/RAX against [EDI/RDI] (sets flags), advance EDI/RDI. Pair with `repne` for `strchr`.",
            "Поиск в строке — сравнить AL/AX/EAX/RAX с [EDI/RDI] (выставить флаги), сдвинуть EDI/RDI. С `repne` = `strchr`.",
        ),
    ),
'@ @'
    (
        "scas",
        entry_t(
            Category::String,
            "SCAn String — compare AL/AX/EAX/RAX against [EDI/RDI] (sets flags), advance EDI/RDI. Pair with `repne` for `strchr`.",
            "Поиск в строке — сравнить AL/AX/EAX/RAX с [EDI/RDI] (выставить флаги), сдвинуть EDI/RDI. С `repne` = `strchr`.",
            super::HintTiers {
                compact_en: "Scan: compare accumulator vs ES:RDI, set flags, advance RDI.",
                compact_ru: "Сканирование: аккумулятор vs ES:RDI, флаги, сдвинуть RDI.",
                educational_en: "\
`scas` subtracts the memory byte/word/dword/qword at ES:RDI from the accumulator and sets \
flags exactly as `cmp` would, without storing the result, then advances RDI per `DF`. \
The accumulator is the search key; RDI scans the target buffer. \
Paired with `repne`/`repnz` (repeat while ZF=0), `repne scasb` implements `strchr` and \
`memchr`: scan until AL matches the byte at RDI (ZF=1) or RCX reaches zero. \
After the loop, RDI is one past the found element; subtract the element size for the \
exact address.",
                educational_ru: "\
`scas` вычитает байт/слово из ES:RDI из аккумулятора и выставляет флаги как `cmp`, \
не сохраняя результат, затем сдвигает RDI по `DF`. \
Аккумулятор — искомый ключ; RDI обходит целевой буфер. \
В паре с `repne`/`repnz` (повтор пока ZF=0) `repne scasb` реализует `strchr` и \
`memchr`: сканирование до совпадения AL с байтом по RDI (ZF=1) или исчерпания RCX. \
После цикла RDI стоит на один элемент дальше найденного.",
            },
        ),
    ),
'@ ([ref]$content)

rep "cmps" @'
    (
        "cmps",
        entry(
            Category::String,
            "CoMPare Strings — compare [ESI/RSI] vs [EDI/RDI], advance both. Pair with `repe` for `memcmp`.",
            "Сравнение строк — [ESI/RSI] vs [EDI/RDI], оба сдвигаются. С `repe` = `memcmp`.",
        ),
    ),
'@ @'
    (
        "cmps",
        entry_t(
            Category::String,
            "CoMPare Strings — compare [ESI/RSI] vs [EDI/RDI], advance both. Pair with `repe` for `memcmp`.",
            "Сравнение строк — [ESI/RSI] vs [EDI/RDI], оба сдвигаются. С `repe` = `memcmp`.",
            super::HintTiers {
                compact_en: "Compare DS:RSI vs ES:RDI, set flags, advance both per DF.",
                compact_ru: "Сравнить DS:RSI с ES:RDI, флаги, сдвинуть оба по DF.",
                educational_en: "\
`cmps` subtracts ES:RDI from DS:RSI, sets flags identically to `cmp`, discards the \
difference, then advances both RSI and RDI per `DF`. \
Paired with `repe`/`repz` (repeat while ZF=1, i.e. bytes are equal), `repe cmpsb` \
implements `memcmp`: advance through two buffers in lock-step until a difference is found \
or RCX reaches zero. \
After the loop: ZF=1 means all bytes matched; ZF=0 means a difference was found, with \
CF/SF providing the sign for a three-way comparison result.",
                educational_ru: "\
`cmps` вычитает ES:RDI из DS:RSI, выставляет флаги как `cmp`, отбрасывает результат \
и сдвигает оба указателя по `DF`. \
В паре с `repe`/`repz` (повтор пока ZF=1, байты равны) `repe cmpsb` реализует \
`memcmp`: синхронный обход двух буферов до первого несовпадения или RCX=0. \
После цикла: ZF=1 — все байты совпали; ZF=0 — найдено различие, CF/SF дают знак \
для трёхзначного результата сравнения.",
            },
        ),
    ),
'@ ([ref]$content)

# ── BATCH C: REP prefixes ──────────────────────────────────────────────────────

rep "rep" @'
    (
        "rep",
        entry(
            Category::String,
            "Repeat prefix — execute the string instruction ECX (x32) / RCX (x64) times, decrementing the counter each iteration.",
            "Префикс повторения — выполнить строковую инструкцию ECX (x32) / RCX (x64) раз, уменьшая счётчик на каждой итерации.",
        ),
    ),
'@ @'
    (
        "rep",
        entry_t(
            Category::String,
            "Repeat prefix — execute the string instruction ECX (x32) / RCX (x64) times, decrementing the counter each iteration.",
            "Префикс повторения — выполнить строковую инструкцию ECX (x32) / RCX (x64) раз, уменьшая счётчик на каждой итерации.",
            super::HintTiers {
                compact_en: "REP prefix — repeat string op RCX times (unconditional).",
                compact_ru: "Префикс REP — повторить строковую op RCX раз (безусловно).",
                educational_en: "\
`rep` is a prefix byte (`0xF3`) that turns a string instruction into a counted loop: the \
CPU decrements ECX (x32) or RCX (x64) after each iteration and stops when the counter \
reaches zero. No ZF condition applies — this is the unconditional form. \
For `movs` and `stos` the exit condition is purely the counter, making `rep movsb` the \
canonical `memcpy` and `rep stosb` the canonical `memset` lowering. \
On Intel CPUs with ERMS (Enhanced REP MOVSB/STOSB) or FSRM (Fast Short REP MOV), the \
microcode handles cache-line bursts internally, often outperforming hand-unrolled loops \
for buffers in the kilobyte range.",
                educational_ru: "\
`rep` — байт-префикс (`0xF3`), превращающий строковую инструкцию в счётный цикл: CPU \
уменьшает ECX (x32) или RCX (x64) после каждой итерации и останавливается при нулевом \
счётчике. Условия по ZF нет — это безусловная форма. \
Для `movs` и `stos`: `rep movsb` — канонический `memcpy`, `rep stosb` — `memset`. \
На Intel с ERMS / FSRM микрокод обрабатывает кэш-линии пакетами, часто обгоняя \
развёрнутые вручную циклы для буферов в диапазоне килобайт.",
            },
        ),
    ),
'@ ([ref]$content)

rep "repe" @'
    (
        "repe",
        entry(
            Category::String,
            "Repeat-while-Equal — like `rep` but also exits when ZF=0. Pairs with `cmps` / `scas`.",
            "Повторение пока равно — как `rep`, но выход и при ZF=0. С `cmps` / `scas`.",
        ),
    ),
'@ @'
    (
        "repe",
        entry_t(
            Category::String,
            "Repeat-while-Equal — like `rep` but also exits when ZF=0. Pairs with `cmps` / `scas`.",
            "Повторение пока равно — как `rep`, но выход и при ZF=0. С `cmps` / `scas`.",
            super::HintTiers {
                compact_en: "REPE — repeat while RCX>0 AND ZF=1 (equal); used with cmps/scas.",
                compact_ru: "REPE — повтор пока RCX>0 И ZF=1 (равно); с cmps/scas.",
                educational_en: "\
`repe` (synonym `repz`, prefix `0xF3`) adds a ZF condition: the loop continues only while \
RCX > 0 AND ZF = 1 (last comparison set \"equal\"). \
The primary use is `repe cmpsb` — compare two byte buffers element by element, stopping \
at the first differing byte or when RCX hits zero — the classic `memcmp` lowering. \
After the loop: ZF=1 means all bytes matched; ZF=0 means RSI-1 and RDI-1 point to the \
first differing pair. `repe scasb` scans while bytes equal the accumulator.",
                educational_ru: "\
`repe` (синоним `repz`, префикс `0xF3`) добавляет условие ZF: цикл продолжается только \
пока RCX > 0 И ZF = 1 (последнее сравнение дало «равно»). \
Основное применение — `repe cmpsb`: синхронный обход двух буферов до первого \
несовпадения или RCX=0 — классическая реализация `memcmp`. \
ZF=1 — все байты совпали; ZF=0 — RSI-1 и RDI-1 указывают на первую несовпадающую пару. \
`repe scasb` сканирует, пока байты равны аккумулятору.",
            },
        ),
    ),
'@ ([ref]$content)

rep "repz" @'
    (
        "repz",
        entry(
            Category::String,
            "Repeat-while-Zero — synonym of `repe`.",
            "Повторение пока ZF=1 — синоним `repe`.",
        ),
    ),
'@ @'
    (
        "repz",
        entry_t(
            Category::String,
            "Repeat-while-Zero — synonym of `repe`.",
            "Повторение пока ZF=1 — синоним `repe`.",
            super::HintTiers {
                compact_en: "REPZ — synonym of REPE (0xF3); repeat while RCX>0 AND ZF=1.",
                compact_ru: "REPZ — синоним REPE (0xF3); повтор пока RCX>0 И ZF=1.",
                educational_en: "\
`repz` is the assembler alias for `repe` — both assemble to prefix byte `0xF3`. \
The name emphasises the ZF=1 (\"zero-difference\" subtraction) exit condition. \
Disassemblers choose between `repz` and `repe` based on convention or context; both \
mean identical behaviour. See `repe` for the full description.",
                educational_ru: "\
`repz` — синоним `repe`; оба кодируются байтом `0xF3`. \
Название акцентирует условие ZF=1 («нулевая» разность вычитания). \
Дизассемблеры выбирают между `repz` и `repe` по соглашению; поведение идентично. \
Полное описание — в статье `repe`.",
            },
        ),
    ),
'@ ([ref]$content)

rep "repne" @'
    (
        "repne",
        entry(
            Category::String,
            "Repeat-while-Not-Equal — like `rep` but also exits when ZF=1. Pairs with `scas` for `strlen`.",
            "Повторение пока не равно — как `rep`, но выход и при ZF=1. С `scas` = `strlen`.",
        ),
    ),
'@ @'
    (
        "repne",
        entry_t(
            Category::String,
            "Repeat-while-Not-Equal — like `rep` but also exits when ZF=1. Pairs with `scas` for `strlen`.",
            "Повторение пока не равно — как `rep`, но выход и при ZF=1. С `scas` = `strlen`.",
            super::HintTiers {
                compact_en: "REPNE — repeat while RCX>0 AND ZF=0 (not equal); strlen/memchr.",
                compact_ru: "REPNE — повтор пока RCX>0 И ZF=0 (не равно); strlen/memchr.",
                educational_en: "\
`repne` (synonym `repnz`, prefix `0xF2`) continues while RCX > 0 AND ZF = 0 (last \
comparison was not equal). \
`repne scasb` with AL=0 is the classic `strlen` idiom: set RCX to a large sentinel, scan \
until a NUL byte is found; length = sentinel - RCX - 1. \
`repne scasb` with AL=target_byte implements `memchr`. \
After the loop: ZF=1 — match found (RDI-1 is the address); ZF=0 — buffer exhausted.",
                educational_ru: "\
`repne` (синоним `repnz`, префикс `0xF2`) продолжает пока RCX > 0 И ZF = 0 (не равно). \
`repne scasb` с AL=0 — классическая реализация `strlen`: задаём большой RCX, сканируем \
до NUL-байта; длина = sentinel − RCX − 1. \
`repne scasb` с AL=нужный_байт реализует `memchr`. \
После цикла: ZF=1 — совпадение найдено (RDI-1 — адрес); ZF=0 — буфер исчерпан.",
            },
        ),
    ),
'@ ([ref]$content)

rep "repnz" @'
    (
        "repnz",
        entry(
            Category::String,
            "Repeat-while-Not-Zero — synonym of `repne`.",
            "Повторение пока ZF=0 — синоним `repne`.",
        ),
    ),
'@ @'
    (
        "repnz",
        entry_t(
            Category::String,
            "Repeat-while-Not-Zero — synonym of `repne`.",
            "Повторение пока ZF=0 — синоним `repne`.",
            super::HintTiers {
                compact_en: "REPNZ — synonym of REPNE (0xF2); repeat while RCX>0 AND ZF=0.",
                compact_ru: "REPNZ — синоним REPNE (0xF2); повтор пока RCX>0 И ZF=0.",
                educational_en: "\
`repnz` is the assembler alias for `repne` — both assemble to prefix byte `0xF2`. \
The name emphasises the ZF=0 (\"non-zero\" subtraction result) exit condition. \
Both names appear in disassembler output for `repnz cmpsb`, `repnz scasb`, etc. \
See `repne` for the full description including the `strlen` and `memchr` idioms.",
                educational_ru: "\
`repnz` — синоним `repne`; оба кодируются байтом `0xF2`. \
Название акцентирует условие ZF=0 («ненулевой» результат вычитания). \
Оба имени встречаются в выводе дизассемблера. \
Полное описание с `strlen` и `memchr` — в статье `repne`.",
            },
        ),
    ),
'@ ([ref]$content)

# ── BATCH D: syscall, sysret, into, hlt, sti, cli, lock, pause, rdrand ────────

rep "syscall" @'
    (
        "syscall",
        entry(
            Category::System,
            "Fast system call (x64-only) — switches to ring 0, RIP→RCX, RFLAGS→R11, jumps to LSTAR MSR. Linux x64 system-call path.",
            "Быстрый системный вызов (только x64) — переход в ring 0, RIP→RCX, RFLAGS→R11, прыжок по LSTAR MSR. Путь системных вызовов в Linux x64.",
        ),
    ),
'@ @'
    (
        "syscall",
        entry_t(
            Category::System,
            "Fast system call (x64-only) — switches to ring 0, RIP→RCX, RFLAGS→R11, jumps to LSTAR MSR. Linux x64 system-call path.",
            "Быстрый системный вызов (только x64) — переход в ring 0, RIP→RCX, RFLAGS→R11, прыжок по LSTAR MSR. Путь системных вызовов в Linux x64.",
            super::HintTiers {
                compact_en: "Fast syscall (x64): RIP→RCX, RFLAGS→R11, jump to LSTAR.",
                compact_ru: "Быстрый syscall (x64): RIP→RCX, RFLAGS→R11, прыжок по LSTAR.",
                educational_en: "\
`syscall` is the x64-only fast system-call instruction, replacing the legacy `int 0x80` path. \
On entry the CPU saves RIP into RCX and RFLAGS into R11, masks RFLAGS per SFMASK MSR, \
then jumps to the address in LSTAR MSR (the kernel's syscall handler). \
Under the System V ABI: syscall number in RAX; arguments in RDI, RSI, RDX, R10, R8, R9 \
(note R10 instead of RCX, which is clobbered by the hardware save). \
On Windows x64, the same instruction is used with a different ABI and LSTAR target \
(KiSystemCall64 in ntoskrnl). `sysret` reverses: RCX→RIP, R11→RFLAGS, drops to ring 3.",
                educational_ru: "\
`syscall` — только-x64 быстрый системный вызов, замена устаревшего `int 0x80`. \
При входе CPU сохраняет RIP в `RCX` и RFLAGS в `R11`, маскирует RFLAGS по MSR SFMASK \
и прыгает по адресу из MSR LSTAR (обработчик ядра). \
По ABI System V: номер вызова в RAX, аргументы в RDI, RSI, RDX, `R10`, R8, R9 \
(`R10` вместо RCX — тот затирается аппаратно). \
В Windows x64 та же инструкция, другой ABI и цель LSTAR (KiSystemCall64 в ntoskrnl). \
`sysret` выполняет обратное: RCX→RIP, R11→RFLAGS, понижает до ring 3.",
            },
        ),
    ),
'@ ([ref]$content)

rep "sysret" @'
    (
        "sysret",
        entry(
            Category::System,
            "Return from `syscall` — RCX→RIP, R11→RFLAGS, drops back to ring 3. **x64-only**.",
            "Возврат из `syscall` — RCX→RIP, R11→RFLAGS, обратно в ring 3. **Только x64**.",
        ),
    ),
'@ @'
    (
        "sysret",
        entry_t(
            Category::System,
            "Return from `syscall` — RCX→RIP, R11→RFLAGS, drops back to ring 3. **x64-only**.",
            "Возврат из `syscall` — RCX→RIP, R11→RFLAGS, обратно в ring 3. **Только x64**.",
            super::HintTiers {
                compact_en: "Syscall return (x64): RCX→RIP, R11→RFLAGS, ring 0→3.",
                compact_ru: "Возврат из syscall (x64): RCX→RIP, R11→RFLAGS, ring 0→3.",
                educational_en: "\
`sysret` is the kernel-side complement to `syscall`: it copies RCX back into RIP and R11 \
back into RFLAGS, restoring the user-space instruction pointer and flags, then drops \
privilege back to ring 3. Valid in ring 0 only; executing in user mode raises #GP. \
Under Linux, RAX holds the syscall return value before `sysret` executes. \
A speculative-execution path through `sysret` was at the heart of Spectre-v1/v2 \
mitigations (IBRS, retpoline) because the CPU mispredicts the restored RIP target.",
                educational_ru: "\
`sysret` — ядерная пара к `syscall`: копирует `RCX` обратно в RIP, `R11` — в RFLAGS, \
восстанавливая указатель инструкции и флаги user-space, и снижает привилегию до ring 3. \
Допустима только в ring 0; в user mode вызывает #GP. \
В Linux RAX содержит возвращаемое значение вызова до выполнения `sysret`. \
Спекулятивный путь через `sysret` лежал в основе уязвимостей Spectre-v1/v2 \
(IBRS, retpoline): CPU неверно предсказывает восстановленный адрес RIP.",
            },
        ),
    ),
'@ ([ref]$content)

rep "into" @'
    (
        "into",
        entry(
            Category::System,
            "Trap on overflow — fires `int 4` if OF=1. **x32-only** — invalid opcode in x64 mode.",
            "Прерывание при переполнении — `int 4` если OF=1. **Только x32** — в x64 это недействительный опкод.",
        ),
    ),
'@ @'
    (
        "into",
        entry_t(
            Category::System,
            "Trap on overflow — fires `int 4` if OF=1. **x32-only** — invalid opcode in x64 mode.",
            "Прерывание при переполнении — `int 4` если OF=1. **Только x32** — в x64 это недействительный опкод.",
            super::HintTiers {
                compact_en: "Overflow trap: call int 4 if OF=1; x32-only (#UD in x64).",
                compact_ru: "Ловушка переполнения: int 4 если OF=1; только x32 (#UD в x64).",
                educational_en: "\
`into` checks the Overflow Flag (OF) and, if set, raises software interrupt 4 (the \
overflow handler). If OF=0 it is a no-op. In x32 mode the opcode is `0xCE`. \
In x64 (64-bit long mode) `0xCE` is an invalid opcode and raises #UD, making `into` \
architecturally unavailable in modern user-mode binaries. \
Pascal and older Borland C++ used `into` after signed arithmetic for checked-arithmetic \
traps. In modern x32 code it signals hand-written asm or a legacy compiler; in x64 \
binaries it is invariably obfuscation (dead byte or deliberate #UD lure).",
                educational_ru: "\
`into` проверяет флаг переполнения (OF) и при его установке вызывает программное \
прерывание 4. При OF=0 — нет-оп. В x32 опкод `0xCE`. \
В x64 (64-битный long mode) `0xCE` — недействительный опкод (#UD), поэтому `into` \
недоступна в современных user-mode бинарях. \
Pascal и Borland C++ использовали `into` для проверяемой арифметики. В современном x32 \
— признак рукописного asm; в x64-бинарях — обфускация (мёртвый байт или ловушка #UD).",
            },
        ),
    ),
'@ ([ref]$content)

rep "hlt" @'
    (
        "hlt",
        entry(
            Category::System,
            "Halt the CPU until the next interrupt. Ring-0 only — `#GP` in user mode.",
            "Остановить CPU до следующего прерывания. Только ring 0 — `#GP` в user mode.",
        ),
    ),
'@ @'
    (
        "hlt",
        entry_t(
            Category::System,
            "Halt the CPU until the next interrupt. Ring-0 only — `#GP` in user mode.",
            "Остановить CPU до следующего прерывания. Только ring 0 — `#GP` в user mode.",
            super::HintTiers {
                compact_en: "Halt CPU until next interrupt (ring 0 only; #GP in user mode).",
                compact_ru: "Остановить CPU до следующего прерывания (ring 0; #GP в user mode).",
                educational_en: "\
`hlt` suspends the processor in a low-power state until an unmasked interrupt or NMI \
wakes it. Privileged (CPL=0 required); in user mode raises #GP(0). \
In OS kernels `hlt` appears in the idle loop: `sti; hlt` enables interrupts then halts, \
ensuring any pending interrupt is delivered before the CPU enters the wait state. \
On SMP systems each core independently executes `hlt` in its idle thread. \
`hlt` in unexpected places (non-idle kernel paths) hints at a custom hypervisor layer.",
                educational_ru: "\
`hlt` переводит процессор в режим пониженного энергопотребления до прихода прерывания \
или NMI. Привилегированная (CPL=0); в user mode вызывает #GP(0). \
В ядрах ОС встречается в idle-цикле: `sti; hlt` разрешает прерывания и затем \
останавливается, гарантируя доставку ожидающих прерываний перед переходом в ожидание. \
В SMP каждое ядро независимо выполняет `hlt` в своём idle-потоке. \
`hlt` в нетипичных местах намекает на нестандартный слой гипервизора.",
            },
        ),
    ),
'@ ([ref]$content)

rep "sti" @'
    (
        "sti",
        entry(
            Category::System,
            "Set Interrupt-flag — re-enable maskable interrupts (ring 0).",
            "Установить флаг прерываний — разрешить маскируемые прерывания (ring 0).",
        ),
    ),
'@ @'
    (
        "sti",
        entry_t(
            Category::System,
            "Set Interrupt-flag — re-enable maskable interrupts (ring 0).",
            "Установить флаг прерываний — разрешить маскируемые прерывания (ring 0).",
            super::HintTiers {
                compact_en: "Set IF — re-enable maskable interrupts (ring 0 only).",
                compact_ru: "Установить IF — разрешить маскируемые прерывания (ring 0).",
                educational_en: "\
`sti` sets the Interrupt Flag (IF) in EFLAGS/RFLAGS, re-enabling delivery of maskable \
hardware interrupts. Requires CPL=0 (or IOPL=3); user mode raises #GP. \
The canonical `sti; hlt` pair in kernel idle loops ensures any pending interrupt is \
delivered immediately before the CPU halts, avoiding a race between the readiness check \
and the halt. `cli` is the counterpart — the pair brackets critical sections in interrupt \
handlers.",
                educational_ru: "\
`sti` устанавливает флаг прерываний (IF) в EFLAGS/RFLAGS, разрешая доставку маскируемых \
аппаратных прерываний. Требует CPL=0 (или IOPL=3); в user mode — #GP. \
Канонический паттерн `sti; hlt` в idle-цикле ядра: `sti` гарантирует доставку ожидающих \
прерываний сразу перед остановкой, исключая гонку между проверкой и остановкой. \
`cli` — парная инструкция; вместе обрамляют критические секции.",
            },
        ),
    ),
'@ ([ref]$content)

rep "cli" @'
    (
        "cli",
        entry(
            Category::System,
            "Clear Interrupt-flag — disable maskable interrupts (ring 0).",
            "Сбросить флаг прерываний — запретить маскируемые прерывания (ring 0).",
        ),
    ),
'@ @'
    (
        "cli",
        entry_t(
            Category::System,
            "Clear Interrupt-flag — disable maskable interrupts (ring 0).",
            "Сбросить флаг прерываний — запретить маскируемые прерывания (ring 0).",
            super::HintTiers {
                compact_en: "Clear IF — disable maskable interrupts (ring 0 only).",
                compact_ru: "Сбросить IF — запретить маскируемые прерывания (ring 0).",
                educational_en: "\
`cli` clears the Interrupt Flag (IF) in EFLAGS/RFLAGS, blocking delivery of maskable \
hardware interrupts until `sti` restores it. Requires CPL=0 (or IOPL=3). \
In kernel code, `cli` opens an interrupt-disabled critical section when accessing per-CPU \
data that must not be modified concurrently by an interrupt handler. \
On SMP systems `cli` only masks interrupts on the local CPU; cross-CPU atomicity still \
requires spinlocks or `lock`-prefixed RMW instructions. \
`pushf; cli` / `popf` saves and restores the full EFLAGS including IF.",
                educational_ru: "\
`cli` сбрасывает флаг прерываний (IF) в EFLAGS/RFLAGS, блокируя маскируемые прерывания \
до восстановления через `sti`. Требует CPL=0 (или IOPL=3). \
В коде ядра `cli` открывает критическую секцию с запрещёнными прерываниями — для \
per-CPU структур, которые нельзя изменять одновременно из обработчика прерывания. \
В SMP маскирует только локальное ядро; для межъядерной атомарности нужны spinlock'и \
или `lock`-префиксные RMW. `pushf; cli` / `popf` — сохранение/восстановление EFLAGS.",
            },
        ),
    ),
'@ ([ref]$content)

rep "lock" @'
    (
        "lock",
        entry(
            Category::System,
            "LOCK prefix — make the next read-modify-write instruction atomic on the system bus. Used for `Atomic*` operations in C++/Rust on x32 and x64 alike.",
            "Префикс LOCK — сделать следующую RMW-инструкцию атомарной на системной шине. Используется для `Atomic*` в C++/Rust одинаково в x32 и x64.",
        ),
    ),
'@ @'
    (
        "lock",
        entry_t(
            Category::System,
            "LOCK prefix — make the next read-modify-write instruction atomic on the system bus. Used for `Atomic*` operations in C++/Rust on x32 and x64 alike.",
            "Префикс LOCK — сделать следующую RMW-инструкцию атомарной на системной шине. Используется для `Atomic*` в C++/Rust одинаково в x32 и x64.",
            super::HintTiers {
                compact_en: "LOCK prefix — atomic RMW on system bus; RFO cache-line cost.",
                compact_ru: "Префикс LOCK — атомарная RMW на системной шине; стоимость RFO.",
                educational_en: "\
`lock` is prefix byte `0xF0` that asserts bus-lock during the next RMW instruction, making \
the read-modify-write appear atomic to all processors. Valid only before: `bts`, `btr`, \
`btc`, `xadd`, `xchg`, `cmpxchg`, `cmpxchg8b`, `inc`, `dec`, `add`, `sub`, `adc`, `sbb`, \
`and`, `or`, `xor`, `not`, `neg`. On modern MESI-coherent CPUs the bus is not physically \
locked; instead the CPU acquires exclusive ownership (RFO — Read For Ownership) of the \
cache line. `lock xchg` and `lock cmpxchg` underlie `AtomicXxx::fetch_add` and \
`compare_exchange` in C++ and Rust.",
                educational_ru: "\
`lock` — байт-префикс `0xF0`, удерживающий сигнал блокировки шины во время следующей \
RMW-инструкции. Допустим перед: `bts`, `btr`, `btc`, `xadd`, `xchg`, `cmpxchg`, \
`cmpxchg8b`, `inc`, `dec`, `add`, `sub`, `adc`, `sbb`, `and`, `or`, `xor`, `not`, `neg`. \
На современных CPU с MESI физической блокировки шины нет: CPU захватывает эксклюзивное \
владение кэш-линией (RFO). `lock xchg` и `lock cmpxchg` лежат в основе \
`AtomicXxx::fetch_add` и `compare_exchange` в C++ и Rust.",
            },
        ),
    ),
'@ ([ref]$content)

rep "pause" @'
    (
        "pause",
        entry(
            Category::System,
            "Hint to spin-wait loops — improves SMT efficiency and saves power; mandatory in well-written `while busy {}` loops.",
            "Подсказка в spin-циклах — повышает эффективность SMT и экономит питание; обязательна в правильных `while busy {}`.",
        ),
    ),
'@ @'
    (
        "pause",
        entry_t(
            Category::System,
            "Hint to spin-wait loops — improves SMT efficiency and saves power; mandatory in well-written `while busy {}` loops.",
            "Подсказка в spin-циклах — повышает эффективность SMT и экономит питание; обязательна в правильных `while busy {}`.",
            super::HintTiers {
                compact_en: "Spin-wait hint for tight CAS loops; reduces SMT contention.",
                compact_ru: "Подсказка для CAS-циклов; снижает конкуренцию SMT, экономит питание.",
                educational_en: "\
`pause` is a 2-byte NOP (`F3 90`) with microarchitectural semantics: it hints to the CPU \
that the current thread is in a spin-wait loop. \
Without `pause`, a tight spin loop saturates the reorder buffer (ROB) with speculative \
iterations; when the lock is released the pipeline must flush all mispredicted loads, \
adding latency. `pause` serialises the pipeline slightly, reducing power and letting the \
sibling SMT thread use more execution resources. \
All well-written spinlocks (`std::sync::Mutex` spin phase, Linux `cpu_relax()`, \
Windows `YieldProcessor()`) emit `pause` in their retry loops.",
                educational_ru: "\
`pause` — двухбайтный NOP (`F3 90`) с микроархитектурной семантикой: подсказывает CPU, \
что поток выполняет spin-wait цикл. \
Без `pause` плотный цикл насыщает ROB спекулятивными итерациями; при освобождении \
блокировки конвейер сбрасывает неверно предсказанные загрузки, добавляя задержку. \
`pause` слегка сериализует конвейер, снижает потребление и позволяет SMT-партнёру \
использовать больше ресурсов. Все правильные спинлоки (Linux `cpu_relax()`, \
Windows `YieldProcessor()`) испускают `pause` в петле повтора.",
            },
        ),
    ),
'@ ([ref]$content)

rep "rdrand" @'
    (
        "rdrand",
        entry(
            Category::System,
            "Read hardware random number — CF=1 on success. Slower than software PRNGs; rarely seen in hot paths.",
            "Чтение аппаратного случайного числа — CF=1 при успехе. Медленнее программного PRNG; редко в hot path.",
        ),
    ),
'@ @'
    (
        "rdrand",
        entry_t(
            Category::System,
            "Read hardware random number — CF=1 on success. Slower than software PRNGs; rarely seen in hot paths.",
            "Чтение аппаратного случайного числа — CF=1 при успехе. Медленнее программного PRNG; редко в hot path.",
            super::HintTiers {
                compact_en: "Read DRNG entropy into register; CF=1 success, CF=0 retry.",
                compact_ru: "Читать аппаратную энтропию DRNG в регистр; CF=1 успех, CF=0 повтор.",
                educational_en: "\
`rdrand` reads a hardware-generated random number from Intel's on-die DRNG (Digital \
Random Number Generator) into the destination register (16 / 32 / 64-bit). \
CF=1 means a valid sample was available; CF=0 means the entropy pool was temporarily \
exhausted — the canonical retry pattern is `rdrand reg; jnc $-2`. \
The DRNG sources entropy from a thermal-noise amplifier but the instruction costs \
~50-100 cycles, far slower than software PRNGs. \
Used for seeding CSPRNGs, key generation, and nonce production in cryptographic code.",
                educational_ru: "\
`rdrand` читает аппаратно-сгенерированное случайное число из DRNG Intel (Digital Random \
Number Generator) в регистр-назначение (16 / 32 / 64 бита). \
CF=1 — выборка доступна; CF=0 — пул энтропии временно исчерпан; канонический повтор: \
`rdrand reg; jnc $-2`. \
DRNG получает энтропию от усилителя теплового шума, но инструкция стоит ~50-100 тактов — \
на порядки медленнее программных PRNG. \
Применяется для засева CSPRNG, генерации ключей и nonce в криптографическом коде.",
            },
        ),
    ),
'@ ([ref]$content)

# ── BATCH E: ud2, prefetch, xlat ──────────────────────────────────────────────

rep "ud2" @'
    (
        "ud2",
        entry(
            Category::Misc,
            "Undefined Instruction — guaranteed to raise `#UD`. Compilers emit it after unreachable code or trap intrinsics (Rust's `panic!` lands here in release).",
            "Неопределённая инструкция — гарантированно вызывает `#UD`. Компиляторы ставят её после unreachable / trap (Rust `panic!` в release заканчивается здесь).",
        ),
    ),
'@ @'
    (
        "ud2",
        entry_t(
            Category::Misc,
            "Undefined Instruction — guaranteed to raise `#UD`. Compilers emit it after unreachable code or trap intrinsics (Rust's `panic!` lands here in release).",
            "Неопределённая инструкция — гарантированно вызывает `#UD`. Компиляторы ставят её после unreachable / trap (Rust `panic!` в release заканчивается здесь).",
            super::HintTiers {
                compact_en: "Guaranteed #UD (0F 0B) — compiler unreachable / panic marker.",
                compact_ru: "Гарантированный #UD (0F 0B) — маркер unreachable / panic компилятора.",
                educational_en: "\
`ud2` (opcode `0F 0B`) is the architecturally guaranteed undefined instruction: the Intel \
SDM explicitly promises it will always raise #UD (Invalid Opcode exception) on all past \
and future processors. \
Compilers emit `ud2` after truly unreachable code: GCC / Clang after \
`__builtin_unreachable()`, Rust after `panic!` in release mode (where the panic handler \
aborts without unwinding), and LLVM after `llvm.trap`. \
The Linux kernel's `BUG_ON()` macro expands to `ud2` on x86. \
`ud2` mid-function in optimised code signals either a dead post-panic path or deliberate \
anti-analysis padding — it halts naive emulators and JIT tracers.",
                educational_ru: "\
`ud2` (опкод `0F 0B`) — архитектурно гарантированная неопределённая инструкция: \
Intel SDM прямо обещает, что она всегда вызывает #UD (Invalid Opcode) на всех \
процессорах. \
Компиляторы испускают `ud2` после безусловно недостижимого кода: GCC / Clang после \
`__builtin_unreachable()`, Rust после `panic!` в release (без раскрутки стека), \
LLVM после `llvm.trap`. Макрос `BUG_ON()` в ядре Linux раскрывается в `ud2`. \
`ud2` в середине функции — либо мёртвый путь после паники, либо намеренный \
антиотладочный наполнитель, останавливающий наивные эмуляторы и JIT-трассировщики.",
            },
        ),
    ),
'@ ([ref]$content)

rep "prefetch" @'
    (
        "prefetch",
        entry(
            Category::Misc,
            "Hint the CPU to fetch a cache line — speculative; never faults on a bad address.",
            "Подсказка CPU подгрузить кэш-линию — спекулятивно; не падает при плохом адресе.",
        ),
    ),
'@ @'
    (
        "prefetch",
        entry_t(
            Category::Misc,
            "Hint the CPU to fetch a cache line — speculative; never faults on a bad address.",
            "Подсказка CPU подгрузить кэш-линию — спекулятивно; не падает при плохом адресе.",
            super::HintTiers {
                compact_en: "Prefetch hint: bring cache line to Ln; never faults on bad address.",
                compact_ru: "Подсказка prefetch: принести кэш-линию в Ln; не падает на bad addr.",
                educational_en: "\
`prefetch` and its variants (`prefetcht0` L1/L2/L3, `prefetcht1` L2/L3, `prefetcht2` L3, \
`prefetchnta` non-temporal) hint the CPU to speculatively load the target cache line. \
The instruction is purely advisory — the processor may ignore it, and it never raises a \
fault or exception regardless of address validity. \
Typical use: in a loop processing element `i`, issue `prefetch [ptr + N*stride]` so \
element `i+N` is warm in cache when needed. \
Over-aggressive or mispredicted prefetches cause cache pollution and can degrade \
performance.",
                educational_ru: "\
`prefetch` и варианты (`prefetcht0` L1/L2/L3, `prefetcht1` L2/L3, `prefetcht2` L3, \
`prefetchnta` non-temporal) подсказывают CPU спекулятивно загрузить целевую кэш-линию. \
Носит чисто рекомендательный характер — процессор может проигнорировать, и никогда \
не вызывает исключений, независимо от корректности адреса. \
Типичное применение: при обработке элемента `i` выдать `prefetch [ptr + N*stride]`, \
чтобы элемент `i+N` был в кэше заранее. \
Избыточные или неверные prefetch засоряют кэш и снижают производительность.",
            },
        ),
    ),
'@ ([ref]$content)

rep "xlat" @'
    (
        "xlat",
        entry(
            Category::Misc,
            "Table lookup: AL = [(R/E)BX + AL]. Holdover from the 8086 era; rarely emitted by modern compilers — its appearance in modern code usually means handwritten asm or a niche obfuscator.",
            "Поиск в таблице: AL = [(R/E)BX + AL]. Реликт эпохи 8086; компиляторы почти не используют — её появление в современном коде обычно означает ручной asm или нишевый обфускатор.",
        ),
    ),
'@ @'
    (
        "xlat",
        entry_t(
            Category::Misc,
            "Table lookup: AL = [(R/E)BX + AL]. Holdover from the 8086 era; rarely emitted by modern compilers — its appearance in modern code usually means handwritten asm or a niche obfuscator.",
            "Поиск в таблице: AL = [(R/E)BX + AL]. Реликт эпохи 8086; компиляторы почти не используют — её появление в современном коде обычно означает ручной asm или нишевый обфускатор.",
            super::HintTiers {
                compact_en: "Table lookup: AL = DS:[(R/E)BX + AL]; 8086-era holdover.",
                compact_ru: "Поиск в таблице: AL = DS:[(R/E)BX + AL]; реликт эпохи 8086.",
                educational_en: "\
`xlat` (also `xlatb`) performs a single-byte table lookup: reads the byte at address \
DS:BX + AL (x32: DS:EBX + AL; x64: RBX + AL) and stores it back into AL, replacing the \
original index. No flags are modified. \
The instruction is a relic of the 8086 era, designed for substitution-cipher tables and \
character translation (`tolower`, code-page conversion). \
Modern compilers never emit `xlat` — `movzx eax, byte [rbx + rax]` is faster on all \
microarchitectures and supports 32/64-bit base registers. \
`xlat` in modern binaries signals hand-written asm, legacy code, or an obfuscator \
exploiting obscure single-byte opcodes.",
                educational_ru: "\
`xlat` (также `xlatb`) — однобайтовый поиск в таблице: читает байт по адресу \
DS:BX + AL (x32: DS:EBX + AL; x64: RBX + AL) и записывает его обратно в AL, \
заменяя исходный индекс. Флаги не меняются. \
Реликт эпохи 8086, предназначавшийся для таблиц подстановочного шифра и преобразования \
символов (`tolower`, code-page). Современные компиляторы `xlat` не генерируют — \
`movzx eax, byte [rbx + rax]` быстрее и поддерживает 32/64-битные регистры. \
`xlat` в современных бинарях — рукописный asm, унаследованный код или обфускатор.",
            },
        ),
    ),
'@ ([ref]$content)

# ── BATCH F: entry_g → entry_gt (preserve gotcha, add tiers) ──────────────────

rep "pushf→gt" @'
    (
        "pushf",
        entry_g(
            Category::Stack,
            "Push EFLAGS (16 or 32 bits). The 64-bit form is `pushfq` — the assembler picks based on operand-size prefix.",
            "Положить EFLAGS в стек (16 или 32 бита). 64-битная форма — `pushfq`; ассемблер выбирает по префиксу размера операнда.",
            "Anti-debug: `pushf; pop reg; test reg, 0x100` reads the Trap Flag (TF). If TF=1 a single-step debugger is attached.",
            "Анти-debug: `pushf; pop reg; test reg, 0x100` читает Trap Flag (TF). TF=1 — подключён single-step отладчик.",
        ),
    ),
'@ @'
    (
        "pushf",
        entry_gt(
            Category::Stack,
            "Push EFLAGS (16 or 32 bits). The 64-bit form is `pushfq` — the assembler picks based on operand-size prefix.",
            "Положить EFLAGS в стек (16 или 32 бита). 64-битная форма — `pushfq`; ассемблер выбирает по префиксу размера операнда.",
            "Anti-debug: `pushf; pop reg; test reg, 0x100` reads the Trap Flag (TF). If TF=1 a single-step debugger is attached.",
            "Анти-debug: `pushf; pop reg; test reg, 0x100` читает Trap Flag (TF). TF=1 — подключён single-step отладчик.",
            super::HintTiers {
                compact_en: "Push EFLAGS (16/32-bit); use `pushfq` for full 64-bit RFLAGS.",
                compact_ru: "Поместить EFLAGS (16/32 бита) в стек; `pushfq` — полный 64-бит.",
                educational_en: "\
`pushf` decrements SP by 2 (16-bit) or 4 (32-bit) and writes the lower 16 or 32 bits of \
EFLAGS to the new stack top. The x64 form is `pushfq`, which writes all 64 bits of RFLAGS. \
EFLAGS contains CF, PF, AF, ZF, SF, TF, IF, DF, OF, IOPL, NT, and (in RFLAGS) RF, VM, \
AC, VIF, VIP, ID. \
Reading EFLAGS via `pushf; pop reg` is the standard technique for detecting single-step \
mode (TF bit 8), CPUID availability (ID bit 21), and the AC alignment check flag.",
                educational_ru: "\
`pushf` уменьшает SP на 2 (16-бит) или 4 (32-бит) и записывает младшие 16 или 32 бита \
EFLAGS на новую вершину стека. Форма x64 — `pushfq`, записывающая все 64 бита RFLAGS. \
EFLAGS содержит CF, PF, AF, ZF, SF, TF, IF, DF, OF, IOPL, NT и (в RFLAGS) RF, VM, \
AC, VIF, VIP, ID. \
Чтение EFLAGS через `pushf; pop reg` — стандартный способ определить режим single-step \
(бит TF=8), поддержку CPUID (бит ID=21) и флаг выравнивания AC.",
            },
            super::HintTiers {
                compact_en: "TF bit 8 in EFLAGS: `pushf; pop reg; test reg, 0x100` detects single-step.",
                compact_ru: "Бит TF=8 в EFLAGS: `pushf; pop reg; test reg, 0x100` — детект single-step.",
                educational_en: "\
The Trap Flag (TF, bit 8 of EFLAGS) is set by a debugger to single-step execution. \
`pushf; pop reg; test reg, 0x100` reads it without changing it; if non-zero, a \
single-step debugger is attached. \
Protectors extend this: `pushf; or dword [esp], 0x100; popf` arms TF deliberately — the \
next instruction raises a #DB exception that the protector's SEH handler catches to \
detect a debugger. This is the classic \"TF-arming\" anti-debug loop.",
                educational_ru: "\
Trap Flag (TF, бит 8 EFLAGS) устанавливается отладчиком для пошагового выполнения. \
`pushf; pop reg; test reg, 0x100` читает его без изменения; ненулевое значение — \
признак single-step отладчика. \
Протекторы идут дальше: `pushf; or dword [esp], 0x100; popf` вручную взводит TF — \
следующая инструкция вызовет #DB, которое обработчик SEH протектора ловит для \
обнаружения отладчика.",
            },
        ),
    ),
'@ ([ref]$content)

rep "pushfq→gt" @'
    (
        "pushfq",
        entry_g(
            Category::Stack,
            "Push the full 64-bit RFLAGS register onto the stack (x64-only).",
            "Положить весь 64-битный RFLAGS в стек (только x64).",
            "Same TF-reading anti-debug as `pushf` but with the full 64-bit flags word.",
            "Та же анти-debug проверка TF, что у `pushf`, но с полным 64-битным словом флагов.",
        ),
    ),
'@ @'
    (
        "pushfq",
        entry_gt(
            Category::Stack,
            "Push the full 64-bit RFLAGS register onto the stack (x64-only).",
            "Положить весь 64-битный RFLAGS в стек (только x64).",
            "Same TF-reading anti-debug as `pushf` but with the full 64-bit flags word.",
            "Та же анти-debug проверка TF, что у `pushf`, но с полным 64-битным словом флагов.",
            super::HintTiers {
                compact_en: "Push full 64-bit RFLAGS to stack (x64-only form of pushf).",
                compact_ru: "Поместить полный 64-бит RFLAGS в стек (x64-форма pushf).",
                educational_en: "\
`pushfq` is the x64-only form of `pushf`: it decrements RSP by 8 and writes all 64 bits \
of RFLAGS to the stack. In 64-bit mode the assembler emits `pushfq` by default when you \
write `pushf`; the REX.W prefix is implied. \
The full RFLAGS word includes all EFLAGS bits plus reserved upper bits. \
`pushfq; pop rax` is the standard x64 way to inspect any RFLAGS bit, including TF (bit 8) \
for single-step detection and ID (bit 21) for CPUID availability.",
                educational_ru: "\
`pushfq` — x64-форма `pushf`: уменьшает RSP на 8 и записывает все 64 бита RFLAGS \
в стек. В 64-битном режиме ассемблер генерирует `pushfq` по умолчанию при написании \
`pushf`; префикс REX.W подразумевается. \
Слово RFLAGS включает все биты EFLAGS плюс зарезервированные старшие биты. \
`pushfq; pop rax` — стандартный x64-способ прочитать любой бит RFLAGS, в том числе \
TF (бит 8) для определения single-step и ID (бит 21) для проверки CPUID.",
            },
            super::HintTiers {
                compact_en: "Same TF-read anti-debug as `pushf`; operates on full 64-bit word.",
                compact_ru: "Та же TF-детекция что у `pushf`; работает с полным 64-битным словом.",
                educational_en: "\
The anti-debug technique is identical to `pushf`: read RFLAGS via `pushfq; pop rax; test \
rax, 0x100` to check TF. In x64, `pushfq` is preferred over `pushf` because it preserves \
the full flags register including bits 32-63. \
TF-arming also works: `pushfq; or qword [rsp], 0x100; popfq` arms the Trap Flag in x64.",
                educational_ru: "\
Антиотладочная техника идентична `pushf`: читаем RFLAGS через `pushfq; pop rax; \
test rax, 0x100` для проверки TF. В x64 `pushfq` предпочтительнее `pushf`, поскольку \
сохраняет полный регистр флагов, включая биты 32-63. \
Взвод TF тоже работает: `pushfq; or qword [rsp], 0x100; popfq` устанавливает TF в x64.",
            },
        ),
    ),
'@ ([ref]$content)

rep "popf→gt" @'
    (
        "popf",
        entry_g(
            Category::Stack,
            "Pop the stack into EFLAGS (16 or 32 bits).",
            "Снять со стека в регистр флагов EFLAGS (16/32-битная форма).",
            "Anti-debug: `or [esp], 0x100; popf` arms the Trap Flag manually — the next instruction raises a single-step exception that the protector handles in its own SEH chain to detect debuggers.",
            "Анти-debug: `or [esp], 0x100; popf` вручную выставляет Trap Flag — следующая инструкция вызовет single-step исключение, которое протектор перехватит в своём SEH, чтобы обнаружить отладчик.",
        ),
    ),
'@ @'
    (
        "popf",
        entry_gt(
            Category::Stack,
            "Pop the stack into EFLAGS (16 or 32 bits).",
            "Снять со стека в регистр флагов EFLAGS (16/32-битная форма).",
            "Anti-debug: `or [esp], 0x100; popf` arms the Trap Flag manually — the next instruction raises a single-step exception that the protector handles in its own SEH chain to detect debuggers.",
            "Анти-debug: `or [esp], 0x100; popf` вручную выставляет Trap Flag — следующая инструкция вызовет single-step исключение, которое протектор перехватит в своём SEH, чтобы обнаружить отладчик.",
            super::HintTiers {
                compact_en: "Pop stack into EFLAGS (16/32-bit); `popfq` for full 64-bit RFLAGS.",
                compact_ru: "Снять стек в EFLAGS (16/32 бита); `popfq` — для полного RFLAGS.",
                educational_en: "\
`popf` reads the value at the current stack top and writes it into the lower 16 or 32 bits \
of EFLAGS, then increments SP. In user mode, certain bits are protected: VM and RF cannot \
be set via `popf`; IOPL can only be modified at ring 0. \
`pushf; popf` is a common idiom for saving and restoring flags around a code region that \
would otherwise clobber them. \
Paired with bit-manipulation on the stack word, `popf` is used to arm or clear specific \
flags such as TF, DF, or AC.",
                educational_ru: "\
`popf` читает значение с вершины стека, записывает его в младшие 16 или 32 бита EFLAGS \
и увеличивает SP. В user mode часть битов защищена: VM и RF нельзя установить через \
`popf`; IOPL изменяется только в ring 0. \
`pushf; popf` — типичный паттерн сохранения и восстановления флагов вокруг кода, \
который их затирает. \
В паре с манипуляцией битами слова стека `popf` используется для взвода/сброса \
конкретных флагов: TF, DF или AC.",
            },
            super::HintTiers {
                compact_en: "TF-arming: `or [esp], 0x100; popf` arms single-step to trap a debugger.",
                compact_ru: "Взвод TF: `or [esp], 0x100; popf` — ловушка для отладчика.",
                educational_en: "\
`or [esp], 0x100; popf` sets TF in EFLAGS without triggering a single-step on the `popf` \
itself. The very next instruction will then raise a #DB exception. \
A protector registers its own SEH (x32) or VEH (x64) handler: if its handler receives the \
#DB, no external debugger is present and the protector handles it silently; if an external \
debugger is attached it intercepts the #DB first, revealing itself.",
                educational_ru: "\
`or [esp], 0x100; popf` устанавливает TF в EFLAGS, не вызывая single-step на самой \
инструкции `popf`. Следующая же инструкция вызовет исключение #DB. \
Протектор регистрирует собственный обработчик SEH (x32) или VEH (x64): если его \
обработчик получает #DB — внешнего отладчика нет, и он обрабатывает его незаметно; \
если внешний отладчик перехватит #DB первым — он себя обнаружит.",
            },
        ),
    ),
'@ ([ref]$content)

rep "popfq→gt" @'
    (
        "popfq",
        entry_g(
            Category::Stack,
            "Pop the stack into the full 64-bit RFLAGS register (x64-only).",
            "Снять со стека в полный 64-битный RFLAGS (только x64).",
            "Same TF-arming anti-debug as `popf`.",
            "Та же анти-debug установка TF, что у `popf`.",
        ),
    ),
'@ @'
    (
        "popfq",
        entry_gt(
            Category::Stack,
            "Pop the stack into the full 64-bit RFLAGS register (x64-only).",
            "Снять со стека в полный 64-битный RFLAGS (только x64).",
            "Same TF-arming anti-debug as `popf`.",
            "Та же анти-debug установка TF, что у `popf`.",
            super::HintTiers {
                compact_en: "Pop 8 bytes from stack into full 64-bit RFLAGS (x64-only).",
                compact_ru: "Снять 8 байт со стека в полный 64-бит RFLAGS (только x64).",
                educational_en: "\
`popfq` reads 8 bytes from the stack top into the full 64-bit RFLAGS register, then \
increments RSP by 8. It is the x64 counterpart to `popf`. \
As with `popf`, certain bits cannot be set from user mode (VM, RF), and IOPL modification \
requires ring 0. \
`pushfq; popfq` is the x64 idiom for a complete flags save/restore cycle.",
                educational_ru: "\
`popfq` читает 8 байт с вершины стека в полный 64-битный RFLAGS и увеличивает RSP на 8. \
x64-аналог `popf`. Как и `popf`, в user mode не позволяет установить VM и RF; \
изменение IOPL требует ring 0. \
`pushfq; popfq` — x64-идиома для полного цикла сохранения и восстановления флагов.",
            },
            super::HintTiers {
                compact_en: "Same TF-arming anti-debug as `popf`; operates on full RFLAGS.",
                compact_ru: "Та же TF-ловушка что у `popf`; работает с полным RFLAGS.",
                educational_en: "\
TF-arming in x64: `pushfq; or qword [rsp], 0x100; popfq` sets TF in RFLAGS. The next \
instruction raises #DB. Otherwise identical to the `popf` anti-debug pattern but operates \
on the full 64-bit flags word.",
                educational_ru: "\
Взвод TF в x64: `pushfq; or qword [rsp], 0x100; popfq` устанавливает TF в RFLAGS. \
Следующая инструкция вызовет #DB. В остальном идентично паттерну `popf`, \
но работает с полным 64-битным словом флагов.",
            },
        ),
    ),
'@ ([ref]$content)

rep "enter→gt" @'
    (
        "enter",
        entry_g(
            Category::Stack,
            "Build a stack frame. **x32**: pushes EBP, sets EBP=ESP, allocates locals. **x64**: same with RBP/RSP. Slow on modern CPUs; compilers prefer `push rbp; mov rbp, rsp; sub rsp, N`.",
            "Построить стек-фрейм. **x32**: push EBP, EBP=ESP, выделение под локальные. **x64**: то же с RBP/RSP. На современных CPU медленнее ручной пары `push rbp; mov rbp, rsp; sub rsp, N` — компиляторы её и предпочитают.",
            "Seeing `enter` in modern code = handwritten asm or a deliberate signature. MSVC / GCC / Clang never emit it.",
            "`enter` в современном коде — рукописный asm или преднамеренная сигнатура. MSVC / GCC / Clang её не генерируют.",
        ),
    ),
'@ @'
    (
        "enter",
        entry_gt(
            Category::Stack,
            "Build a stack frame. **x32**: pushes EBP, sets EBP=ESP, allocates locals. **x64**: same with RBP/RSP. Slow on modern CPUs; compilers prefer `push rbp; mov rbp, rsp; sub rsp, N`.",
            "Построить стек-фрейм. **x32**: push EBP, EBP=ESP, выделение под локальные. **x64**: то же с RBP/RSP. На современных CPU медленнее ручной пары `push rbp; mov rbp, rsp; sub rsp, N` — компиляторы её и предпочитают.",
            "Seeing `enter` in modern code = handwritten asm or a deliberate signature. MSVC / GCC / Clang never emit it.",
            "`enter` в современном коде — рукописный asm или преднамеренная сигнатура. MSVC / GCC / Clang её не генерируют.",
            super::HintTiers {
                compact_en: "Build stack frame; compilers emit `push rbp; mov rbp,rsp; sub rsp,N` instead.",
                compact_ru: "Построить стек-фрейм; компиляторы предпочитают `push rbp; mov rbp,rsp; sub rsp,N`.",
                educational_en: "\
`enter imm16, imm8` pushes EBP/RBP, sets EBP/RBP = ESP/RSP, and subtracts `imm16` bytes \
from ESP/RSP to allocate local storage. The second operand (lexical nesting level) was \
intended for Pascal nested procedures and is almost always 0. \
On modern CPUs `enter` is a slow microcoded sequence; compilers universally prefer the \
3-instruction prologue `push rbp; mov rbp, rsp; sub rsp, N` which is ~3x faster. \
`enter` in modern compiled output is a strong signal of either hand-written asm or a \
legacy compiler (old Borland/Watcom); protectors may inject it as an anomaly detection \
lure.",
                educational_ru: "\
`enter imm16, imm8` помещает EBP/RBP в стек, устанавливает EBP/RBP = ESP/RSP и \
вычитает `imm16` байт из ESP/RSP для выделения локальных переменных. Второй операнд \
(уровень вложенности для Pascal) почти всегда равен 0. \
На современных CPU `enter` — медленная микрокодированная последовательность; компиляторы \
предпочитают трёхинструкционный пролог `push rbp; mov rbp, rsp; sub rsp, N`, \
который примерно в 3 раза быстрее. \
`enter` в современном скомпилированном коде — признак рукописного asm или устаревшего \
компилятора; протекторы могут внедрять её как аномалию.",
            },
            super::HintTiers {
                compact_en: "`enter` in modern compiled code = handwritten asm; MSVC/GCC/Clang never emit it.",
                compact_ru: "`enter` в современном коде = рукописный asm; MSVC/GCC/Clang не генерируют.",
                educational_en: "\
No modern optimising compiler (MSVC, GCC, Clang, Rust/LLVM) emits `enter` — the \
equivalent `push rbp; mov rbp, rsp; sub rsp, N` prologue is strictly faster on every \
micro-architecture since the Pentium Pro. \
Seeing `enter` in a binary means handwritten asm, a legacy compiler (Borland C++ 3.x, \
Watcom), or deliberate injection by a protector to confuse ABI-aware reverse-engineering \
tools.",
                educational_ru: "\
Ни один современный оптимизирующий компилятор (MSVC, GCC, Clang, Rust/LLVM) не \
генерирует `enter` — эквивалентный пролог `push rbp; mov rbp, rsp; sub rsp, N` \
быстрее на каждой микроархитектуре начиная с Pentium Pro. \
`enter` в бинаре означает рукописный asm, устаревший компилятор (Borland C++ 3.x, \
Watcom) или намеренное внедрение протектором для запутывания ABI-ориентированных \
инструментов реверс-инжиниринга.",
            },
        ),
    ),
'@ ([ref]$content)

rep "int→gt" @'
    (
        "int",
        entry_g(
            Category::System,
            "Software interrupt — calls the IDT vector. `int 3` is the debugger breakpoint (single-byte 0xCC).",
            "Программное прерывание — вызов вектора IDT. `int 3` = брейкпоинт отладчика (один байт 0xCC).",
            "Anti-debug: `int 3` raises a breakpoint exception even without a software bp set; `int 2D` (Windows kernel debugger probe) goes one further — when no debugger is attached, the next byte is consumed; when one is, it isn't. Both probe \"is a debugger watching\".",
            "Анти-debug: `int 3` вызывает breakpoint-исключение даже без программной точки; `int 2D` (Windows-kernel debugger probe) идёт дальше — без отладчика следующий байт пропускается, с отладчиком — нет. Оба способа проверяют \"следит ли отладчик\".",
        ),
    ),
'@ @'
    (
        "int",
        entry_gt(
            Category::System,
            "Software interrupt — calls the IDT vector. `int 3` is the debugger breakpoint (single-byte 0xCC).",
            "Программное прерывание — вызов вектора IDT. `int 3` = брейкпоинт отладчика (один байт 0xCC).",
            "Anti-debug: `int 3` raises a breakpoint exception even without a software bp set; `int 2D` (Windows kernel debugger probe) goes one further — when no debugger is attached, the next byte is consumed; when one is, it isn't. Both probe \"is a debugger watching\".",
            "Анти-debug: `int 3` вызывает breakpoint-исключение даже без программной точки; `int 2D` (Windows-kernel debugger probe) идёт дальше — без отладчика следующий байт пропускается, с отладчиком — нет. Оба способа проверяют \"следит ли отладчик\".",
            super::HintTiers {
                compact_en: "Software interrupt n — call IDT[n]; `int 3` (0xCC) = debugger BP.",
                compact_ru: "Программное прерывание n — вызов IDT[n]; `int 3` (0xCC) = BP отладчика.",
                educational_en: "\
`int n` triggers a software interrupt by looking up vector `n` in the IDT (Interrupt \
Descriptor Table) and calling the corresponding handler. \
`int 3` (opcode `0xCC`, 1 byte) is architecturally the breakpoint exception (#BP): \
debuggers patch code with `0xCC` to install software breakpoints. \
`int 0x80` is the legacy Linux x32 syscall path (EAX = syscall number, arguments in \
EBX/ECX/EDX/ESI/EDI/EBP). \
`int 0x2E` is the Windows legacy syscall gate. \
`int 0x2D` is a Windows kernel debugger probe (see gotcha).",
                educational_ru: "\
`int n` вызывает программное прерывание, находя вектор `n` в IDT и передавая управление \
соответствующему обработчику. \
`int 3` (опкод `0xCC`, 1 байт) — архитектурное исключение точки останова (#BP): \
отладчики патчат код байтом `0xCC` для программных breakpoint'ов. \
`int 0x80` — устаревший путь syscall в Linux x32 (EAX = номер вызова, аргументы в \
EBX/ECX/EDX/ESI/EDI/EBP). \
`int 0x2E` — устаревший шлюз syscall Windows. \
`int 0x2D` — зонд ядерного отладчика Windows (см. gotcha).",
            },
            super::HintTiers {
                compact_en: "`int 3` = instant BP; `int 2D` = Win kernel debug probe (byte-skip trick).",
                compact_ru: "`int 3` — мгновенный BP; `int 2D` — зонд Win kernel debugger (байт-пропуск).",
                educational_en: "\
`int 3` (`0xCC`) raises #BP regardless of whether a software breakpoint is set — executing \
it directly signals the debugger. \
`int 2D` (`0xCD 0x2D`) is a Windows kernel debugger probe: if no kernel debugger is \
attached the instruction behaves normally and EIP advances past the next byte (consuming \
it); if KD is attached, EIP is not advanced, so the byte after `int 2D` is re-executed. \
This creates a divergence: code executed differently under a kernel debugger vs. without, \
used to detect analysis environments.",
                educational_ru: "\
`int 3` (`0xCC`) вызывает #BP независимо от наличия программной точки останова — \
прямое его исполнение немедленно сигнализирует отладчику. \
`int 2D` (`0xCD 0x2D`) — зонд ядерного отладчика Windows: без KD инструкция выполняется \
нормально и EIP продвигается через следующий байт (пропуская его); при подключённом KD \
EIP не продвигается, и следующий байт исполняется повторно. \
Это создаёт дивергенцию поведения: код работает по-разному с ядерным отладчиком и без, \
что используется для обнаружения аналитических сред.",
            },
        ),
    ),
'@ ([ref]$content)

rep "nop→gt" @'
    (
        "nop",
        entry_g(
            Category::Misc,
            "No-OPeration — used for alignment padding and hot-patch space. Multi-byte forms exist (0F 1F …) for longer pads.",
            "Пустая инструкция — выравнивание и место под hot-patch. Есть многобайтные формы (0F 1F …) для длинных пропусков.",
            "Mutators inject NOP-equivalent sequences (`xchg eax, eax`, `lea reg, [reg+0]`, `mov reg, reg`, `rol reg, 0`) to inflate code without changing behaviour. A long stretch of \"different\" instructions that all read like NOPs is a strong obfuscation signal.",
            "Мутаторы вставляют NOP-эквиваленты (`xchg eax, eax`, `lea reg, [reg+0]`, `mov reg, reg`, `rol reg, 0`) чтобы раздуть код без смены поведения. Длинная серия \"разных\" инструкций, фактически = NOP — сильный сигнал обфускации.",
        ),
    ),
'@ @'
    (
        "nop",
        entry_gt(
            Category::Misc,
            "No-OPeration — used for alignment padding and hot-patch space. Multi-byte forms exist (0F 1F …) for longer pads.",
            "Пустая инструкция — выравнивание и место под hot-patch. Есть многобайтные формы (0F 1F …) для длинных пропусков.",
            "Mutators inject NOP-equivalent sequences (`xchg eax, eax`, `lea reg, [reg+0]`, `mov reg, reg`, `rol reg, 0`) to inflate code without changing behaviour. A long stretch of \"different\" instructions that all read like NOPs is a strong obfuscation signal.",
            "Мутаторы вставляют NOP-эквиваленты (`xchg eax, eax`, `lea reg, [reg+0]`, `mov reg, reg`, `rol reg, 0`) чтобы раздуть код без смены поведения. Длинная серия \"разных\" инструкций, фактически = NOP — сильный сигнал обфускации.",
            super::HintTiers {
                compact_en: "NOP (0x90 = xchg eax,eax); multi-byte `0F 1F` pads for alignment.",
                compact_ru: "NOP (0x90 = xchg eax,eax); многобайтный `0F 1F` для выравнивания.",
                educational_en: "\
The 1-byte `nop` is opcode `0x90`, which is actually the encoding of `xchg eax, eax` — \
a historical accident that is now architecturally defined as a no-op on all x86 CPUs. \
Multi-byte NOPs (`0F 1F /0` + ModRM + optional SIB + displacement) fill 2-to-9 byte gaps \
without wasting decode bandwidth: the CPU treats them as a single micro-op that does nothing. \
Compilers insert NOPs for: (1) function alignment to cache-line / branch-predictor \
boundaries, (2) hot-patch space (1–5 bytes before a call that MSVC can replace at runtime), \
and (3) loop padding to avoid decode stalls.",
                educational_ru: "\
Однобайтный `nop` — опкод `0x90`, который исторически является кодировкой `xchg eax, eax` \
— архитектурно определён как нет-оп на всех x86 CPU. \
Многобайтные NOP (`0F 1F /0` + ModRM + опциональные SIB и смещение) заполняют зазоры \
в 2-9 байт без потери полосы декодирования: CPU обрабатывает их как одну микрооперацию \
без действия. \
Компиляторы вставляют NOP для: (1) выравнивания функций к границам кэш-линии / \
предсказателя переходов, (2) пространства для hot-patch (MSVC), (3) заполнения \
циклов для предотвращения задержек декодирования.",
            },
            super::HintTiers {
                compact_en: "NOP-equivalent sequences (`xchg eax,eax`, `lea r,[r+0]`) signal mutator obfuscation.",
                compact_ru: "NOP-эквиваленты (`xchg eax,eax`, `lea r,[r+0]`) — признак мутаторной обфускации.",
                educational_en: "\
Obfuscating mutators replace genuine NOPs with semantically equivalent but syntactically \
varied instructions: `xchg eax, eax` (the canonical `0x90` NOP), `lea reg, [reg+0]` \
(flag-free add-zero), `mov reg, reg` (identity copy), `rol reg, 0` (rotate by zero). \
A long run of such \"different\" instructions that each leave all outputs unchanged is a \
reliable obfuscation signal — taint-track the outputs to confirm they are all dead.",
                educational_ru: "\
Обфусцирующие мутаторы заменяют настоящие NOP семантически эквивалентными, но \
синтаксически разнообразными инструкциями: `xchg eax, eax` (канонический `0x90`), \
`lea reg, [reg+0]` (прибавление нуля без флагов), `mov reg, reg` (копирование самого \
себя), `rol reg, 0` (поворот на ноль). \
Длинная серия таких «разных» инструкций, каждая из которых не меняет выходы, — \
надёжный сигнал обфускации; отслеживайте taint выходов для подтверждения мёртвого кода.",
            },
        ),
    ),
'@ ([ref]$content)

rep "cpuid→gt" @'
    (
        "cpuid",
        entry_g(
            Category::System,
            "CPU identification — feature/version query. EAX selects the leaf, results return in EAX/EBX/ECX/EDX. Identical encoding in x32 and x64.",
            "Идентификация CPU — запрос возможностей/версии. EAX задаёт лист, ответы в EAX/EBX/ECX/EDX. Кодировка одинаковая в x32 и x64.",
            "Anti-VM / anti-hypervisor: leaf 1 ECX bit 31 = \"hypervisor present\"; leaves 0x40000000+ return vendor strings (\"VMwareVMware\", \"KVMKVMKVM\", \"Microsoft Hv\", …). Protectors check these to refuse running inside an analysis VM.",
            "Анти-VM / анти-hypervisor: лист 1 ECX бит 31 = \"hypervisor present\"; листы 0x40000000+ возвращают vendor-строки (\"VMwareVMware\", \"KVMKVMKVM\", \"Microsoft Hv\", …). Протекторы проверяют это и отказываются работать в analysis VM.",
        ),
    ),
'@ @'
    (
        "cpuid",
        entry_gt(
            Category::System,
            "CPU identification — feature/version query. EAX selects the leaf, results return in EAX/EBX/ECX/EDX. Identical encoding in x32 and x64.",
            "Идентификация CPU — запрос возможностей/версии. EAX задаёт лист, ответы в EAX/EBX/ECX/EDX. Кодировка одинаковая в x32 и x64.",
            "Anti-VM / anti-hypervisor: leaf 1 ECX bit 31 = \"hypervisor present\"; leaves 0x40000000+ return vendor strings (\"VMwareVMware\", \"KVMKVMKVM\", \"Microsoft Hv\", …). Protectors check these to refuse running inside an analysis VM.",
            "Анти-VM / анти-hypervisor: лист 1 ECX бит 31 = \"hypervisor present\"; листы 0x40000000+ возвращают vendor-строки (\"VMwareVMware\", \"KVMKVMKVM\", \"Microsoft Hv\", …). Протекторы проверяют это и отказываются работать в analysis VM.",
            super::HintTiers {
                compact_en: "CPUID: EAX=leaf, ECX=subleaf; results in EAX/EBX/ECX/EDX.",
                compact_ru: "CPUID: EAX=лист, ECX=подлист; результаты в EAX/EBX/ECX/EDX.",
                educational_en: "\
`cpuid` is a serialising instruction (`0F A2`) that queries CPU features and version info. \
EAX selects the leaf; for leaves that have subleaves (e.g. leaf 4, 7, 0x0D) ECX selects \
the subleaf. Results return in EAX, EBX, ECX, EDX. \
Key leaves: 0 (max leaf + vendor string), 1 (family/model/stepping + feature bits), \
7/0 (AVX2/AVX512/SHA extensions), 0x80000001 (extended feature bits). \
Hypervisors intercept `cpuid` and can return any values — all CPUID output in a VM is \
soft and may be spoofed.",
                educational_ru: "\
`cpuid` — сериализующая инструкция (`0F A2`), запрашивающая характеристики CPU. \
EAX задаёт лист; для листов с подлистами (4, 7, 0x0D) ECX задаёт подлист. \
Результаты возвращаются в EAX, EBX, ECX, EDX. \
Ключевые листы: 0 (максимальный лист + vendor-строка), 1 (family/model/stepping + биты \
возможностей), 7/0 (AVX2/AVX512/SHA), 0x80000001 (расширенные биты). \
Гипервизоры перехватывают `cpuid` и могут возвращать любые значения — весь вывод CPUID \
в VM является программным и может быть подделан.",
            },
            super::HintTiers {
                compact_en: "Leaf 1 ECX[31]=hypervisor; 0x40000000+ = vendor string (VMware/KVM/Hyper-V).",
                compact_ru: "Лист 1 ECX[31]=гипервизор; 0x40000000+ = vendor-строка (VMware/KVM/Hyper-V).",
                educational_en: "\
Leaf 1, ECX bit 31 (\"hypervisor present\") is set by all major hypervisors to advertise \
their presence. Leaves 0x40000000 and above are the hypervisor-specific range: \
0x40000000 returns a 4-byte signature in EBX+ECX+EDX — \"VMwareVMware\" for VMware, \
\"KVMKVMKVM\" for KVM, \"Microsoft Hv\" for Hyper-V / Windows Sandbox. \
Protectors read these to detect analysis VMs and refuse to run or behave differently. \
Patching the hypervisor leaf to return zeros defeats this check.",
                educational_ru: "\
Лист 1, бит ECX[31] («hypervisor present») устанавливается всеми крупными гипервизорами. \
Листы 0x40000000 и выше — диапазон гипервизора: лист 0x40000000 возвращает 4-байтовую \
сигнатуру в EBX+ECX+EDX — \"VMwareVMware\" для VMware, \"KVMKVMKVM\" для KVM, \
\"Microsoft Hv\" для Hyper-V / Windows Sandbox. \
Протекторы читают эти значения для обнаружения аналитических VM и отказываются \
запускаться или меняют поведение. Патчинг листа гипервизора на нули обходит эту проверку.",
            },
        ),
    ),
'@ ([ref]$content)

rep "rdtsc→gt" @'
    (
        "rdtsc",
        entry_g(
            Category::System,
            "Read TimeStamp Counter — returns the cycle counter in EDX:EAX. Not strictly serialised on its own; pair with `lfence` if you need ordering.",
            "Прочитать счётчик тактов — результат в EDX:EAX. Сам по себе не сериализуем; для упорядочивания нужен `lfence`.",
            "Anti-debug: `rdtsc; … work …; rdtsc; sub` measures elapsed cycles. A debugger's single-step / interrupt overhead inflates the delta, so a large value reveals analysis. `rdtscp` adds an implicit barrier — same trick, slightly more reliable timing.",
            "Анти-debug: `rdtsc; … код …; rdtsc; sub` мерит разницу тактов. Single-step / прерывания отладчика её раздувают — большая дельта выдаёт анализ. `rdtscp` добавляет неявный барьер — тот же приём, чуть надёжнее.",
        ),
    ),
'@ @'
    (
        "rdtsc",
        entry_gt(
            Category::System,
            "Read TimeStamp Counter — returns the cycle counter in EDX:EAX. Not strictly serialised on its own; pair with `lfence` if you need ordering.",
            "Прочитать счётчик тактов — результат в EDX:EAX. Сам по себе не сериализуем; для упорядочивания нужен `lfence`.",
            "Anti-debug: `rdtsc; … work …; rdtsc; sub` measures elapsed cycles. A debugger's single-step / interrupt overhead inflates the delta, so a large value reveals analysis. `rdtscp` adds an implicit barrier — same trick, slightly more reliable timing.",
            "Анти-debug: `rdtsc; … код …; rdtsc; sub` мерит разницу тактов. Single-step / прерывания отладчика её раздувают — большая дельта выдаёт анализ. `rdtscp` добавляет неявный барьер — тот же приём, чуть надёжнее.",
            super::HintTiers {
                compact_en: "Read 64-bit TSC into EDX:EAX; not serialising — pair with lfence.",
                compact_ru: "Читать 64-бит TSC в EDX:EAX; не сериализует — нужен lfence.",
                educational_en: "\
`rdtsc` reads the 64-bit Time Stamp Counter into EDX:EAX (high 32 bits in EDX, low in EAX). \
The TSC counts CPU cycles and is monotonically non-decreasing; on modern CPUs it is an \
invariant TSC (scales with the nominal frequency, not the actual P-state frequency). \
`rdtsc` is not serialising: the CPU may execute instructions before it out-of-order. \
For accurate timing use `lfence; rdtsc` or the serialising `rdtscp` (`0F 01 F9`, also \
returns the TSC auxiliary register in ECX). \
In virtualised environments the TSC may be offset or scaled by the hypervisor.",
                educational_ru: "\
`rdtsc` читает 64-битный Time Stamp Counter в EDX:EAX (старшие 32 бита в EDX, младшие \
в EAX). TSC считает такты CPU и монотонно неубывает; на современных CPU это инвариантный \
TSC (масштабируется по номинальной частоте, а не текущей P-state). \
`rdtsc` не сериализует: CPU может выполнять инструкции перед ним не по порядку. \
Для точного измерения используйте `lfence; rdtsc` или сериализующий `rdtscp` (`0F 01 F9`, \
также возвращает вспомогательный регистр TSC в ECX). \
В виртуализированных средах TSC может быть смещён или масштабирован гипервизором.",
            },
            super::HintTiers {
                compact_en: "TSC delta timing: large delta under single-step reveals debugger presence.",
                compact_ru: "Разница TSC: большая дельта при single-step выдаёт отладчик.",
                educational_en: "\
`rdtsc; <work>; rdtsc; sub` measures the elapsed cycles. A debugger's single-step or \
breakpoint overhead adds thousands of cycles, inflating the delta well above the expected \
value. Protectors compare the delta to a threshold; if exceeded, they assume a debugger is \
present and abort or alter behaviour. `rdtscp` is a stronger variant (implicit serialisation \
barrier), making the measurement harder to defeat by reordering. \
Countermeasure: patch `rdtsc` to `xor edx,edx; xor eax,eax` or use a kernel driver to \
virtualise TSC reads.",
                educational_ru: "\
`rdtsc; <код>; rdtsc; sub` измеряет прошедшие такты. Одиночный шаг или прерывание \
отладчика добавляют тысячи тактов, раздувая дельту выше ожидаемого порога. \
Протекторы сравнивают дельту с пороговым значением; при превышении предполагают \
наличие отладчика и прерывают работу или меняют поведение. `rdtscp` — более стойкий \
вариант (неявный барьер сериализации), затрудняющий обход через переупорядочивание. \
Контрмера: патч `rdtsc` → `xor edx,edx; xor eax,eax` или драйвер ядра для \
виртуализации чтений TSC.",
            },
        ),
    ),
'@ ([ref]$content)

# Save
$report | ForEach-Object { Write-Output $_ }
[System.IO.File]::WriteAllText($file, $content, [System.Text.UTF8Encoding]::new($false))
Write-Output "=== SAVED ==="
