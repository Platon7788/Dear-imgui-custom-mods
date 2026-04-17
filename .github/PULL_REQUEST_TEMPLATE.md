## What

<!-- One or two sentences. "Adds X to Y" / "Fixes regression in Z". -->

## Why

<!-- The motivation — real use case, issue number, or benchmark result. -->

## Checklist

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features`
- [ ] Added / updated doc comments on any new public item
- [ ] Added / updated CHANGELOG.md entry under `[Unreleased]`
- [ ] For breaking API changes: minor version bumped in Cargo.toml and
      migration note added to CHANGELOG.md

## Notes for reviewers

<!-- Anything reviewers should look at first (tricky logic, perf trade-off,
deliberate deviation from previous convention). -->
