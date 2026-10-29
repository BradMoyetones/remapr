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
- Add a mapping by pressing the keys, not by picking from a list.
- Pause all remapping with one switch; keys held down while pausing are released correctly.
- Safe with AltGr layouts (Spanish, German, ...): `@`, `#`, `€` keep working.
- Lives in the notification area: closing the window keeps your keys remapped; the tray icon
  opens, pauses or quits Remapr, and dims while paused.
- One instance per session: launching Remapr again brings the running window forward.
- Light, dark, or follow Windows, including the native title bar.
- Settings saved to `%APPDATA%\Remapr\config.toml`, written atomically.

See [ROADMAP.md](ROADMAP.md) for what comes next (installer, start with Windows, auto-update,
shortcuts).

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
have no console. Only one Remapr runs at a time: if `cargo run` seems to do nothing, an earlier
copy is still in the tray; quit it from the tray menu.

Run the tests, lints and formatter, the same checks CI runs:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

### Remapping inside admin windows

Windows does not let a normal process change keys typed into elevated windows (Task Manager,
installers, terminals run as administrator). To test that, start your terminal as administrator
before `cargo run`. The installer (phase 3) will set this up automatically for users.

### Changing the logo

The logo lives in `assets/logo-light.svg` and `assets/logo-dark.svg`. After editing them,
regenerate the app icons (every size of `icon.ico`, the paused variant and `icon-256.png`) and
commit the results:

```sh
cargo xtask icons
```

## Building a release

```sh
cargo build --release
```

The executable is `target\release\remapr.exe`. It is self-contained: copy it anywhere and run it.

## Project layout

```
remapr/
├── apps/
│   └── remapr/          The executable: main window, capture flow, tray icon, settings file
├── assets/              Logo (SVG, light and dark) and the generated app icons
├── crates/
│   ├── engine/          Global keyboard hook and key injection
│   ├── platform/        Windows integration: single instance, native window icon
│   └── ui/              Design system: light/dark tokens, fonts, Kbd, buttons, switch, cards
├── xtask/               Repository chores (`cargo xtask icons`)
└── .github/workflows/   CI
```

Apps depend on crates, never the other way around. Each crate has one job:

- **engine** knows nothing about the UI. It installs a `WH_KEYBOARD_LL` hook on a dedicated
  high-priority thread, swaps keys through `SendInput`, and exposes a small API (`set_mappings`,
  `set_enabled`, `start_capture`). The decision for each key event is plain Rust in `remap.rs`,
  unit-tested without Windows.
- **platform** wraps the few Win32 calls the app needs outside the engine. Together with the
  engine's hook module, it is the only place with `unsafe` code.
- **ui** knows nothing about keys. Screens take every colour from `Palette` and every font from
  the theme, so light and dark mode stay consistent. It uses the fonts already on Windows
  (Segoe UI and Segoe Fluent Icons), which adds nothing to the binary.
- **apps/remapr** wires them together and owns the settings file and the tray icon.

## How it works

Windows sends every key press to the hook before any application sees it. If the key is mapped,
Remapr swallows it and injects the target key instead. Injected keys are ignored by the hook, so a
mapping can never loop. The callback only reads atomics and a fixed table: no locks, allocation or
I/O, which keeps added latency in the microseconds.

## License

[MIT](LICENSE). You can use, modify and redistribute Remapr, including commercially, as long as
you keep the copyright notice and credit Brad Moyetones as the original author.
