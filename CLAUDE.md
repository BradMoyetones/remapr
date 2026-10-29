# Remapr: notes for contributors and agents

Windows-only key remapper in pure Rust (eframe/egui 0.37, windows-sys). See README.md for the
layout and ROADMAP.md for the phases.

## Rules

- `unsafe` lives only in `crates/engine/src/hook.rs`. The workspace denies `unsafe_code`.
- The hook callback runs for every key on the system and Windows drops slow hooks: no locks,
  allocation, I/O or logging inside it. Put new decisions in `remap.rs` with a unit test.
- UI code takes colours from `remapr_ui::Palette` and fonts from the theme. No hard-coded colours
  in apps.
- New widgets go in `crates/ui` and report themselves with `widget_info` for screen readers.
- User-facing text is Spanish for now; code, comments and docs are English.
- A new crate goes in `crates/<name>`, is named `remapr-<name>` and inherits workspace lints.

## Before committing

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
