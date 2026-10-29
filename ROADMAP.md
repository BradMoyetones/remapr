# Roadmap

Each phase ships a working app. A phase is done when it is merged, CI is green and it has been
used on a real keyboard.

## Phase 1: Foundations (done)

- Cargo workspace: `apps/remapr`, `crates/engine`, `crates/ui`.
- Engine: low-level keyboard hook, key to key remapping, no stuck keys, AltGr-safe, pause.
- Key capture: add a mapping by pressing the two keys.
- Design system: light/dark tokens, Segoe UI and Fluent icons, `Kbd`, primary and icon buttons,
  switch, cards, banners.
- Settings in `%APPDATA%\Remapr\config.toml` with atomic saves and recovery of a damaged file.
- CI on Windows: fmt, clippy, tests.

## Phase 2: Live in the background (done)

- Logo: a keycap with an R, in light and dark SVG; `cargo xtask icons` renders the app icons.
- App icon, paused tray icon and version info embedded in the executable.
- Tray icon: closing the window keeps remapping on (explained the first time); the tray menu
  opens, pauses or quits, and the icon dims while paused.
- Single instance: launching again brings the existing window forward, even across elevation.

## Phase 3: Installer and administrator mode

- MSI installer (WiX, in `packaging/windows/`) that installs per machine to `Program Files` and
  asks for administrator rights once, at install time.
- At sign-in Remapr starts hidden in the tray through a scheduled task with "run with highest
  privileges", so it runs as administrator (remaps keys inside elevated windows too) without a
  UAC prompt on every boot.
- Clean uninstall: removes the task, files and shortcuts; keeps the user's settings unless asked.
- `cargo xtask dist` builds the release executable and the MSI.

## Phase 4: Auto-update

- `crates/update`: checks GitHub Releases of `BradMoyetones/remapr`, compares versions.
- An unobtrusive "New version available" banner in the app; one click downloads the MSI,
  verifies its SHA-256, installs it and restarts Remapr.
- Since Remapr already runs elevated, updating needs no extra UAC prompt.
- Release workflow: pushing a `v*` tag builds, packages and publishes the MSI and its checksum.

## Phase 5: Shortcuts and per-app rules

- Key to shortcut (`F1` to `Ctrl+Shift+T`) and shortcut to shortcut.
- Mappings that only apply while a given app is in the foreground.
- Conflict warnings when two mappings overlap.

## Phase 6: Polish

- English and Spanish UI.
- Code signing, so SmartScreen does not warn on first install.
- Import and export of mappings.
