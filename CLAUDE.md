# Remapr: notes for contributors and agents

Open-source input remapper in pure Rust (eframe/egui 0.37, windows-sys). It remaps keys on Windows
today; it builds, lints and tests on macOS and Linux too, which get their backends in later phases. See README.md for the
layout, ROADMAP.md for the phases and docs/VISION.md for the architecture they build toward.

## Rules

- `unsafe` lives only in the engine's system modules (`windows/{hook,layout}.rs`,
  `macos/{tap,system,hid}.rs`) and `crates/platform`. The workspace denies `unsafe_code`; those
  modules opt out with `#[allow(unsafe_code)]`. `remapr-core` forbids it.
- Keys are `remapr_core::Key`: a physical position, by USB HID usage, named as the W3C `code`
  values in files. Never store or compare a system's key codes outside its backend. Each system's
  translation is a plain table in `crates/engine/src/<system>/keys.rs`, tested on every system;
  on Windows, characters go by scan code (position) and other keys by virtual-key code.
- The settings format has a version (`config::VERSION`). Changing it means bumping it, a reader
  for the old version, a file as the old version wrote it in `apps/remapr/tests/fixtures` (never
  edited afterwards), and a test that converts it. Users' settings are never dropped silently.
- Code for one system sits in its own `#[cfg]` module (`crates/platform/src/windows/`,
  `crates/platform/src/unix.rs`, the engine's `windows/` and `macos/` and `unsupported.rs`), not scattered
  through the app. `remapr-platform`'s `lib.rs` lists every public item once, and each system's
  module provides all of them: an `Unsupported` error, `None` or a no-op where it can't do the job.
  Dependencies one system needs are target-specific (`[target.'cfg(windows)'.dependencies]`).
- The hook callback runs for every key on the system and Windows drops slow hooks: no locks,
  allocation, I/O or logging inside it. What it reads lives in thread-locals of the hook thread
  (rules come in as a thread message, the app in front from a WinEvent hook). Put new decisions in
  `crates/core/src/remap.rs` with a unit test; what only one system needs (Windows' AltGr and
  Alt-tap quirks) stays in its backend.
- On macOS, global key swaps go to macOS's own key mapping (`hidutil`) and everything else to the
  event tap (`macos/output.rs::split`). Remapr owns `UserKeyMapping` while it runs and clears it
  however it stops: on drop, from the signal handler in `macos/hid.rs`, and on the next start.
- The macOS tap callback may create the `CGEvent`s it posts (macOS allocates them), and nothing
  else that allocates, locks or logs. It rewrites the event in place when one key becomes one key:
  a posted event queues behind keys already waiting.
- Never let anything register raw keyboard input (`RegisterRawInputDevices`): while a window of a
  process with that registration is in front, Windows skips that process's own low-level hook.
  winit registers it, so the app removes it at startup (`stop_raw_keyboard_input`).
- Synthetic keys never reach the remap logic, so scripts can't test it. Debug builds started with
  `REMAPR_CAPTURE_INJECTED=1` let synthetic keys complete a capture, to script the capture flow.
- UI code takes colours from `remapr_ui::Palette` and fonts from the theme. No hard-coded colours
  in apps. The `palette_contrast` test checks every pairing screens use (AAA for body text, AA for
  other text and status colours, 3:1 for controls); a new token gets its rows there.
- Text is Inter on every system (cut down: `crates/ui/assets/fonts/README.md`). Icons are Lucide
  SVGs in `crates/ui/assets/icons`, listed in `remapr_ui::icon`; add new ones unchanged, from the
  same Lucide version.
- Code or assets taken from another project keep their licence notice and get a row in
  ATTRIBUTION.md.
- The title bar follows ArtCraft's craft-window rule. Windows keeps its native frame, tinted with
  the palette (`tint_title_bar`). macOS runs the header under a transparent title bar, keeps
  `MACOS_TITLE_BAR` points clear for the window buttons, and lets the header's empty part drag the
  window (`remapr_ui::title_bar`). Linux keeps the desktop's frame.
- New widgets also go in the design snapshots (`crates/ui/tests/gallery.rs`): `cargo test -p
  remapr-ui --features snapshots` draws them, light and dark, with wgpu.
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
- A new crate goes in `crates/<name>`, is named `remapr-<name>`, inherits workspace lints and gets
  its layer in `xtask/src/layers.rs`; `cargo xtask layers` fails until it has one.
- The app talks to the engine only as a `remapr_core::Backend` (commands in, events drained once
  per frame). The UI decides what to offer from `Capabilities`, never from `cfg!(target_os)`;
  `cfg!` is for system matters outside the engine (elevation, the title bar).
- What happens to the mappings goes in `apps/remapr/src/session.rs`, tested with its fake backend;
  `app.rs` draws, saves and talks to the system.
- Libraries log through the `log` facade; only the app installs a logger (`logging.rs`). Never log
  from a hook callback.

## Before committing

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask layers
```

CI runs these on Windows, macOS and Linux. To check another system's code from yours, add its
target (`rustup target add x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`…) and run clippy
with `--target`.
