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

## Phase 3: Installer and administrator mode (done)

- MSI installer (WiX 5, `packaging/windows/remapr.wxs`), per machine into `Program Files`; asks
  for administrator rights once, at install time, and opens Remapr when done.
- At sign-in Remapr starts hidden in the tray through a scheduled task that runs with the
  highest rights the user has: elevated for administrators, no UAC prompt on every boot.
- A plain launch of the installed app hands over to that elevated copy.
- "Iniciar con Windows" switch in the app (keeps the task, drops its sign-in trigger).
- Clean uninstall: ends Remapr, removes the task, files and shortcut; keeps the user's settings.
- `cargo xtask dist` builds the release executable, the MSI and its SHA-256; CI builds it too.

## Phase 4: Auto-update (done)

- Installer wizard (WixUI_InstallDir, Spanish): licence, folder choice, Remapr's own art.
- `crates/update`: checks GitHub Releases of `BradMoyetones/remapr`, compares versions.
- A "Nueva versión disponible" banner in the app; one click downloads the MSI, verifies its
  SHA-256, installs it and restarts Remapr. "Novedades" opens the release page, as ArtCraft does.
- Since Remapr already runs elevated, updating needs no extra UAC prompt.
- After an update, a notice says so once, with a link to the notes.
- Release workflow: pushing a `v*` tag runs `cargo xtask dist` and publishes the MSI and its
  checksum as a GitHub release.

## Phase 5: Shortcuts and per-app rules (done)

- Key to shortcut (`F1` to `Ctrl+Shift+T`), shortcut to key, and shortcut to shortcut, captured
  by pressing them.
- Mappings that only apply while a given app is in the foreground, picked from the apps open.
- Overlaps resolved, not warned about: the same keys in the same place replace the old mapping
  (with a notice); one app's mappings win over global ones, and shortcuts over single keys.

## Phase 6: Polish (done)

- English and Spanish UI, following Windows' display language; translations are catalogs as in
  ArtCraft's craft-i18n, so another language is one file.
- Import and export of mappings.
- "Buscar actualizaciones" button in the footer.
- The title bar keeps the app's theme after Windows settings changes; the window icon stays sharp
  across monitors with different DPI.
- Debug builds run beside the installed app, with their own settings.

## Later

- Code signing, so SmartScreen does not warn on first install (needs a certificate).
- The installer in English too (it is Spanish only).
- A language picker, if following Windows isn't enough for someone.
