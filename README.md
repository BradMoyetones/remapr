<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img src="assets/logo-light.svg" width="128" alt="Remapr logo: a keycap with an R">
  </picture>
</p>

<h1 align="center">Remapr</h1>

<p align="center">A fast, native key remapper for Windows, written entirely in Rust.</p>

Press a key, press the key it should become, done. No web views, no runtime, no background services
beyond a single small process.

## Features

- Remap any key to any other key, system-wide.
- Shortcuts too: a key to a shortcut (`F1` to `Ctrl + Shift + T`), a shortcut to a key, or one
  shortcut to another.
- Any mapping can apply everywhere or only while one app is in front (`chrome.exe`, `code.exe`…).
- Add a mapping by pressing the keys, not by picking from a list.
- Pause all remapping with one switch; keys held down while pausing are released correctly.
- Safe with AltGr layouts (Spanish, German, ...): `@`, `#`, `€` keep working.
- Lives in the notification area: closing the window keeps your keys remapped; the tray icon
  opens, pauses or quits Remapr, and dims while paused.
- One instance per session: launching Remapr again brings the running window forward.
- Starts with Windows as administrator, so it also remaps keys inside elevated windows, without a
  UAC prompt at every sign-in. Can be turned off from the app.
- Updates itself: when a new release is out, a banner offers it; one click downloads it, checks
  its SHA-256, installs it and restarts Remapr.
- Light, dark, or follow Windows, including the native title bar.
- In English or Spanish, following Windows' display language.
- Settings saved to `%APPDATA%\Remapr\config.toml`, written atomically.

See [ROADMAP.md](ROADMAP.md) for the phases and [docs/VISION.md](docs/VISION.md) for where Remapr is
going: macOS, Linux, mouse, macros and controllers.

## Installing

Download `Remapr-<version>-x64.msi` and run it. A short wizard (in Spanish) shows the licence and
lets you pick the folder; Windows asks for administrator rights once, when installing starts. The
installer:

- puts Remapr in `C:\Program Files\Remapr` (or the folder you chose), with a Start menu entry;
- registers a scheduled task, `Remapr`, that starts it hidden in the tray when you sign in, with
  the highest rights your account has (so remapping reaches admin windows too);
- opens Remapr when it finishes.

Uninstall it from Settings > Apps. Your mappings stay in `%APPDATA%\Remapr` in case you come back.

## Requirements

- Windows 10 or 11 (x64).
- Rust 1.95 or newer with the MSVC toolchain: install from [rustup.rs](https://rustup.rs).
- Microsoft C++ Build Tools (the "Desktop development with C++" workload): the MSVC linker, and
  `rc.exe` from the Windows SDK, which embeds the icons into the executable.

## Development

Run the app in debug mode:

```sh
cargo run
```

Debug builds keep a console window attached so `println!` and panics are visible. Release builds
have no console. Debug builds are a separate app: their own settings (`%APPDATA%\Remapr-dev`) and
their own single instance, so they run beside an installed Remapr without touching it. If
`cargo run` seems to do nothing, an earlier debug copy is still in the tray; quit it from there.

Run the tests, lints and formatter, the same checks CI runs:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

### On macOS and Linux

The workspace builds, lints and tests on macOS and Linux too, and CI runs on all three. There the
window opens and says remapping isn't available yet: the keyboard engine, the tray icon and the
installer are Windows-only until Phases 10 (macOS) and 11 (Linux). Settings go to
`~/Library/Application Support/Remapr-dev` on macOS and `~/.config/Remapr-dev` on Linux.

From either, you can check the Windows code before pushing:

```sh
rustup target add x86_64-pc-windows-msvc
cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings
```

That build goes without the app icons (embedding them needs `rc.exe`, so a Windows host).

### Remapping inside admin windows

Windows does not let a normal process change keys typed into elevated windows (Task Manager,
installers, terminals run as administrator). To test that, start your terminal as administrator
before `cargo run`. Installed copies always run elevated through the sign-in task: a plain launch
from the Start menu hands over to it.

### Translations

The code holds the English text (`tr!("Add")`, `trn!("{n} mapping|{n} mappings", n)` for counts,
`key!("Caps Lock")` for key names). Each other language is a catalog in `apps/remapr/locales/`, in
the format of ArtCraft's craft-i18n: one line per text, `context <TAB> English <TAB> translation`.
Remapr picks the first of Windows' display languages it has a catalog for (`es-CO` uses `es`), and
shows English for anything a catalog lacks.

To add a language, copy `es.tsv` to `<code>.tsv`, translate the third column, and add it to
`CATALOGS` in `apps/remapr/src/i18n.rs`; if its plurals aren't "one is 1, other for the rest", add
its rule to `i18n::plural`. `cargo test` fails while a catalog misses a text or a `{placeholder}`.
A debug build can be started in any language with `REMAPR_LANG=<code>`.

### Changing the logo

The logo lives in `assets/logo-light.svg` and `assets/logo-dark.svg`. After editing them,
regenerate the app icons (every size of `icon.ico`, the paused variant and `icon-256.png`) and
commit the results:

```sh
cargo xtask icons
```

## Building the installer

```sh
cargo xtask dist
```

This builds the release executable and `target\dist\Remapr-<version>-x64.msi`, with its SHA-256
in a `.sha256` file beside it. The first run downloads WiX 5 from NuGet into `target\wix` (version
and checksum pinned in `xtask`); WiX runs on the .NET runtime, 6 or newer, which most Windows
machines already have. The installer source is `packaging/windows/remapr.wxs`.

CI builds the installer on every push: download it from the run's artifacts to test a change.

`target\release\remapr.exe` alone is self-contained too: copy it anywhere and run it, without the
sign-in task.

## Releasing

1. Bump `version` in the root `Cargo.toml` and commit.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.

The `Release` workflow checks that the tag matches the version, runs `cargo xtask dist` and
publishes the MSI and its `.sha256` as a GitHub release, with notes generated from the commits
(edit them on GitHub if you like). Installed copies check GitHub a few seconds after starting and
every 6 hours, and offer the update; its "Novedades" link opens that release page.

To try the update flow without publishing, start a debug build with `REMAPR_UPDATES_DIR` pointing
at a folder laid out like GitHub's (`latest.json` plus `v<version>/` with the files); see
`remapr_update::Source::local`.

## Project layout

```
remapr/
├── apps/
│   └── remapr/          The executable: main window, capture flow, tray icon, settings file
├── assets/              Logo (SVG, light and dark) and the generated app icons
├── docs/                Vision and research
├── crates/
│   ├── engine/          Global keyboard hook and key injection
│   ├── platform/        OS integration: settings folder, single instance, elevation, sign-in task
│   ├── ui/              Design system: light/dark tokens, fonts, Kbd, buttons, switch, cards
│   └── update/          Release check, download, SHA-256 check and install (GitHub Releases)
├── packaging/windows/   The installer (WiX)
├── xtask/               Repository chores (`cargo xtask icons`, `cargo xtask dist`)
└── .github/workflows/   CI
```

Apps depend on crates, never the other way around. Each crate has one job:

- **engine** knows nothing about the UI. It installs a `WH_KEYBOARD_LL` hook on a dedicated
  high-priority thread, swaps keys and shortcuts through `SendInput`, and exposes a small API
  (`set_rules`, `set_enabled`, `start_capture`). The decision for each key event is plain Rust in `remap.rs`,
  unit-tested without Windows.
- **platform** wraps the few system calls the app needs outside the engine. Every function exists
  on every system: `src/windows/` holds the Win32 code, `src/unix.rs` what macOS and Linux do so
  far, and what a system can't do yet reports itself unsupported. Together with the engine's hook
  module, it is the only place with `unsafe` code.
- **ui** knows nothing about keys. Screens take every colour from `Palette` and every font from
  the theme, so light and dark mode stay consistent. It uses the fonts already on Windows
  (Segoe UI and Segoe Fluent Icons), which adds nothing to the binary.
- **apps/remapr** wires them together and owns the settings file and the tray icon.

## How it works

Windows sends every key press to the hook before any application sees it. If the key is mapped,
Remapr swallows it and injects the target key instead. Injected keys are ignored by the hook, so a
mapping can never loop. For a shortcut it also lifts the modifiers the target doesn't use and
presses the ones it adds, in a single `SendInput`, and puts them back on release. Everything the
callback reads lives on the hook thread itself (rules arrive as a thread message, the app in front
from a WinEvent hook): no locks, allocation or I/O, which keeps added latency in the microseconds.

## License

[MIT](LICENSE). You can use, modify and redistribute Remapr, including commercially, as long as
you keep the copyright notice and credit Brad Moyetones as the original author.
