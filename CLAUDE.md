# Remapr: notes for contributors and agents

Windows-only key remapper in pure Rust (eframe/egui 0.37, windows-sys). See README.md for the
layout and ROADMAP.md for the phases.

## Rules

- `unsafe` lives only in `crates/engine/src/hook.rs` and `crates/platform`. The workspace denies
  `unsafe_code`; those modules opt out with `#[allow(unsafe_code)]`.
- The hook callback runs for every key on the system and Windows drops slow hooks: no locks,
  allocation, I/O or logging inside it. What it reads lives in thread-locals of the hook thread
  (rules come in as a thread message, the app in front from a WinEvent hook). Put new decisions in
  `remap.rs` with a unit test.
- Never let anything register raw keyboard input (`RegisterRawInputDevices`): while a window of a
  process with that registration is in front, Windows skips that process's own low-level hook.
  winit registers it, so the app removes it at startup (`stop_raw_keyboard_input`).
- Synthetic keys never reach the remap logic, so scripts can't test it. Debug builds started with
  `REMAPR_CAPTURE_INJECTED=1` let synthetic keys complete a capture, to script the capture flow.
- UI code takes colours from `remapr_ui::Palette` and fonts from the theme. No hard-coded colours
  in apps.
- New widgets go in `crates/ui` and report themselves with `widget_info` for screen readers.
- User-facing text is written in English in the code, through `tr!`, `trn!` (counts) and `key!`
  (key names), and translated in `apps/remapr/locales/<lang>.tsv` (craft-i18n's format). The test
  `catalogs_cover_the_code` fails when a catalog lacks a text the code uses. Code, comments and
  docs are English.
- The window theme follows `ctx.set_theme`: egui syncs the native title bar itself
  (`Options::sync_window_theme`). Don't send `ViewportCommand::SetTheme` by hand; it fights that
  sync. `keep_title_bar_theme` re-applies the theme after Windows settings changes, which winit
  resets.
- Icons are generated: edit `assets/logo-*.svg`, run `cargo xtask icons`, commit the outputs.
- The installer runs `remapr.exe --register-task` / `--unregister-task` as SYSTEM: `main` handles
  them before anything that needs a window or a user session.
- Never change the MSI's `UpgradeCode`; upgrades (and the updater) depend on it.
- The updater reads only the version from GitHub and builds download URLs itself; keep it that
  way. Release asset names (`Remapr-<version>-x64.msi` and `.sha256`) are a contract between
  `xtask dist`, the release workflow and `crates/update`.
- Releases are made by pushing a `v<version>` tag matching `Cargo.toml` (see README, Releasing).
- Check window chrome (title bar colour, visibility) with real screen pixels, not `PrintWindow`:
  it redraws the frame and hides bugs.
- A new crate goes in `crates/<name>`, is named `remapr-<name>` and inherits workspace lints.

## Before committing

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
