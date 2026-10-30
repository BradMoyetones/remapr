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

## Phase 7: Builds and looks the same everywhere (done)

The first step toward macOS and Linux (see [docs/VISION.md](docs/VISION.md)). No new remapping
features.

- The workspace builds, lints and tests on Windows, macOS and Linux. CI runs on all three; the
  installer is still built on Windows.
- `crates/platform` is one facade over a module per system. The Win32 code moves to
  `platform/src/windows/`. macOS and Linux get what is simple (settings folder, opening links,
  display language) and report the rest as unsupported.
- The keyboard hook and the tray icon stay Windows-only. On macOS and Linux the window opens and
  says that remapping isn't available on that system yet.
- One look on every system:
  - Inter, the logo's typeface, and Lucide icons, embedded.
  - A full palette: info, success, warning and danger, controls, and a scrim for modals. A test
    checks its contrast.
- A title bar fit to each system, as ArtCraft's apps do. On Windows the native frame takes the
  app's colours. On macOS the header runs under a transparent title bar, beside the window
  buttons, and dragging it moves the window.
- Snapshots of the design system in both themes, drawn by CI for review.
- Remapping on Windows behaves exactly as in 0.4.

## Phase 8: Universal keys

- `crates/core` (`remapr-core`) holds chords, modifiers, rules and the remap decision, with no
  OS code. Its tests run on every system.
- A key is a position, identified by USB HID usage and stored by its W3C `code` name
  (`from = "CapsLock"`, `"Ctrl+Shift+KeyT"`). On Windows, characters are read and sent by scan
  code, so a mapping stays on its key whatever the layout. Modifiers are Ctrl, Shift, Alt and Meta,
  shown as Win, ⌘ or Super.
- The AltGr phantom Ctrl and the dummy key tapped before releasing Alt or Win move to the Windows
  backend.
- A per-app rule matches a program file, a macOS bundle id or a Linux window class.
- Settings and export files carry a version. Files from 0.5 and earlier, with Win32 key numbers,
  migrate through the layout in use, and the old file is kept as `config.v1.toml`. The migration
  is tested against files as those versions wrote them.

## Phase 9: One engine contract

- The app talks to the engine through one API: start, rules, pause, capture and capabilities,
  with events back (capture done, app in front changed, permission changed, error).
- `Capabilities` says, per function, whether it works, needs a permission, needs a driver or
  isn't available. The UI reads that instead of asking which system it runs on.
- A fake backend tests the capture flow and the UI's states on any system.
- `cargo xtask layers` fails if `core` or the engine depend on egui.
- Logs go to a rotated file in the settings folder.

## Phase 10: macOS keyboard

- Remapping through `CGEventTap` and `CGEventPost`. The app in front comes from `NSWorkspace`
  (bundle id). The tap is turned back on if macOS disables it for being slow.
- A first-run screen asks for the Accessibility and Input Monitoring permissions and notices
  when they are granted. A notice explains when Secure Input (password fields) pauses remapping.
- Menu bar icon, one instance at a time, native file dialogs (import and export), start at login,
  and the display language from the system rather than the terminal's variables.
- Texts no longer name Windows where they mean the system.
- `cargo xtask dist` builds `Remapr.app` and a DMG. The updater offers new versions and opens
  their page.

## Phase 11: Linux keyboard

- `evdev` with `EVIOCGRAB` and a `uinput` virtual keyboard; keyboards plugged in later are picked
  up.
- A udev rule ships in the packages. The first-run screen checks the permissions.
- The app in front: on X11 first, then on Wayland compositor by compositor. Per-app rules are
  marked as partial where the app in front can't be known.
- Tray icon, one instance at a time, file dialogs, start at login (XDG autostart).
- The window on Wayland with decorations of its own (GNOME draws none for it).
- AppImage and deb.

## Phase 12: App shell and profiles

- A sidebar: Keyboard, Mouse, Macros, Controllers, Profiles, Settings.
- One gate widget for what needs a permission, needs a driver or isn't available on this
  system: the section stays visible, dimmed under a translucent veil, with a card saying why.
- Profiles: named sets of mappings, switched by the app in front or by a shortcut. The tray shows
  the active one.
- A warning when mapping a shortcut the system keeps for itself (Ctrl+Alt+Del, Cmd+Tab…).

## Phase 13: Mouse and macros

- Mouse buttons, wheel and clicks, as source and as target, on all three systems.
- Actions beyond a chord: sequences with pauses, typing text, turbo, toggle, switching profile.
  They play on their own thread, never inside the hook.
- A macro recorder built on capture.
- Then dual-role keys (Caps Lock: Esc on tap, Ctrl on hold) and layers.

## Phase 14: Controllers to keyboard and mouse

- Physical controllers are read on all three systems, macOS included: `gilrs` first, Remapr's
  own backends where grabbing or hiding them needs it.
- A live view of the controller (Xbox, DualSense, Switch Pro) drawn in egui. A button is mapped
  by pressing it, like a key.
- Sticks to mouse, with dead zones and response curves.
- Shaped by the study of reWASD's features (`docs/research/rewasd.md`, see the vision).

## Phase 15: Virtual controllers

- Linux first: a virtual Xbox 360 or DualShock 4 through `uinput`, with the physical controller
  grabbed. No driver needed.
- Windows: through ViGEmBus when installed (detected, with help to install it), and HidHide to
  hide the physical controller. Both sit behind a trait, so another driver can replace them.
- macOS: the section is shown locked, with the reason.

## Later

- Code signing with Azure Trusted Signing, so SmartScreen does not warn on first install: sign
  `remapr.exe` and the MSI in the release workflow, with the account's credentials as secrets.
- Maybe: an Apple Developer ID (99 USD a year) to sign and notarize the macOS app, so Gatekeeper
  opens it without a warning.
- The installer in English too (it is Spanish only).
- A language picker, if following Windows isn't enough for someone.
- Beyond Phase 15: profiles per device, combos, gyro to mouse, rumble, shared profiles and a
  `remapr-cli` to switch profiles from scripts.
