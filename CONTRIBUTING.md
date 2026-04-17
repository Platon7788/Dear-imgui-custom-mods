# Contributing to dear-imgui-custom-mod

Thanks for considering a contribution. This document is short on purpose —
the codebase is small enough that `src/` + `CLAUDE.md` already tell most
of the story.

## Setting up

```bash
git clone https://github.com/Platon7788/Dear-imgui-custom-mods
cd Dear-imgui-custom-mods
cargo build --all-features
cargo test  --all-features
```

Windows and Linux are both tested in CI. On Linux you'll need X11 / Wayland
dev headers (`libx11-dev libxcursor-dev libxi-dev libxrandr-dev
libxkbcommon-dev libwayland-dev` on Debian / Ubuntu).

`rust-toolchain.toml` pins the compiler — `rustup` will auto-install the
matching version on first `cargo` invocation.

## Running a demo

Each widget has a `demo_*` example under `examples/`:

```bash
cargo run --example demo_confirm_dialog
cargo run --example demo_borderless
cargo run --example demo_nav_panel
# …and so on
```

## Before you push

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test  --workspace --all-features
```

CI runs the same three checks + `cargo doc` + `cargo-deny` + MSRV build +
`cargo-semver-checks` on PRs. Run the local triad and you'll pass the
first four by construction.

## Code style

- **Comments** — English only. User-facing messages can be in any
  language; the source stays English for cross-team readability.
- **One logical change per commit.** Refactor noise (rename variables,
  reindent, reshuffle modules) goes in its own commit.
- **Conventional-ish commit prefixes** — `feat:`, `fix:`, `docs:`,
  `refactor:`, `deps:`, `ci:`, `chore:`. Not enforced, just convention.
- **Breaking changes** go in the subject line tagged `BREAKING:` and
  in the CHANGELOG under a new minor-version header.
- **Doc comments** on every public item. `#![warn(missing_docs)]` is on
  in `lib.rs` — CI fails if you skip one.

## Layout

```
src/
├── <component>/
│   ├── mod.rs        # public API + rendering
│   ├── config.rs     # builder types
│   ├── state.rs      # stateful bits (animation, selection, …)
│   └── theme.rs      # palette struct (no enum — themes live in theme/)
├── theme/
│   ├── mod.rs        # unified Theme enum
│   └── {dark,light,midnight,solarized,monokai}.rs
├── fonts/            # shared TTF blobs + font installers
├── input/            # keyboard translation helpers (winit → ImGui)
└── utils/            # color packing, text measurement, exports
```

New widget: copy an existing component's layout (e.g.
`src/confirm_dialog/`), swap the rendering body, wire the public API
through `src/lib.rs`, add a doc file under `docs/<name>.md`, and a
`examples/demo_<name>.rs`.

## Testing

- **Unit tests** — `#[cfg(test)] mod tests` at the bottom of the module.
  Covers pure logic (color math, layout calculations, state transitions).
- **Integration tests** — `tests/*.rs` — test the public API surface
  end-to-end. Don't require a live ImGui context; reason about types
  and builder chains.
- **Benches** — `benches/*.rs` via Criterion. Add one for any hot-path
  render function you touch.
- **Doc tests** — fenced code blocks in rustdoc. Use `no_run` when the
  example needs a live `Ui` — compile-only proves the signature.

## Themes

Adding a new theme:
1. Create `src/theme/<name>.rs` with `titlebar_colors`, `nav_colors`,
   `dialog_colors`, `statusbar_config`, `apply_imgui_style`.
2. Register in `src/theme/mod.rs`: add a `Theme::<name>` variant and
   hook up the five dispatch methods.
3. Update `Theme::ALL` and the doc comment.

Test: `cargo run --example demo_app_window`, hit the theme picker.

## Filing bugs

Use the bug report template; include environment (OS, Rust version,
GPU backend) — rendering bugs are often backend-specific.

## License

The repo has no explicit LICENSE file yet; contributions are under an
informal "use it" understanding with the owner. If you need clarity
for your organization, open an issue and we'll add a proper license.
