# Security policy

## Supported versions

Only the latest minor release on `master` receives security fixes. Older
minors are not backported — upgrade forward.

## Reporting a vulnerability

If you believe you've found a security issue (RCE via malformed input,
memory-safety violation in `unsafe` blocks, supply-chain concern, …)
please do **not** file a public GitHub issue.

Instead:

1. Open a **private security advisory** via the repo's Security tab
   (GitHub → Security → Advisories → Report a vulnerability).
2. Include a proof-of-concept or stack trace if you have one.
3. Expect an acknowledgement within a few days.

You can also email `nxt8787@gmail.com` for coordination if the GitHub
path is blocked.

## Scope

Relevant issues include:
- Memory-safety bugs in our own `unsafe` blocks
  (`borderless_window/platform.rs` — Win32 cursor + DWM corner APIs).
- Panics on maliciously-crafted input to a parser (hex_viewer offsets,
  disasm_view bytes, code_editor syntax).
- Supply-chain concerns about transitive deps surfaced by `cargo-audit`
  / `cargo-deny`.

Out of scope:
- Bugs in `dear-imgui-rs`, `wgpu`, `winit`, `windows-sys` — report those
  upstream.
- Visual / rendering glitches that don't imply memory corruption — use
  the regular bug report template.
