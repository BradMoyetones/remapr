# Vision

Where Remapr is headed and in what order. [ROADMAP.md](../ROADMAP.md) lists the phases; this
document explains the reasoning behind them: the architecture they build toward, the decisions
already taken, and what each operating system allows. The original research, in Spanish, is in
[INVESTIGATION.md](INVESTIGATION.md).

Written on 2026-10-29, at version 0.4.0. Update it when a phase changes the picture.

## Where Remapr is going

Remapr starts as a keyboard remapper for Windows. One working release at a time, it grows into an
open-source input remapper for Windows, macOS and Linux in the spirit of reWASD: keyboard, mouse,
macros, physical controllers and, where the system allows it, virtual controllers.

Three things don't change along the way:

- It stays pure Rust and MIT.
- It stays as simple to use as it is today: press the key, then press the key it should become.
- Every phase ships an app that works.

## What each system allows

| | Windows | Linux | macOS |
|---|---|---|---|
| Keyboard and mouse remapping | Low-level hooks and `SendInput` | `evdev` and `uinput` | `CGEventTap` and `CGEventPost`, with the Accessibility permission |
| Blocking the physical input | The hook swallows the event | `EVIOCGRAB` | The tap drops the event |
| App in front | `GetForegroundWindow`, WinEvent hooks | Easy on X11; on Wayland it depends on the compositor | `NSWorkspace` |
| Reading controllers | XInput, Windows.Gaming.Input, HID | `evdev` | GameController, IOKit |
| Virtual controllers | Needs a driver (ViGEmBus) | `uinput`, no driver | Not practical: needs DriverKit and an entitlement Apple grants case by case |
| Hiding the physical controller | Needs a driver (HidHide) | `EVIOCGRAB` | Not practical |

On macOS, Remapr will remap the keyboard and mouse, run macros, and turn a physical controller into
keyboard and mouse input. Virtual controllers are Windows and Linux only.

## Where the code stood at 0.4.0

INVESTIGATION.md names two couplings to Windows: `Vk = u8` and the hook. The code had more:

- **Settings stored Win32 key codes.** `config.toml` and export files hold numbers like
  `from = 20`. Moving to a universal key needs a schema version and a migration, not just a
  refactor, or existing users lose their mappings.
- **The "pure" remap logic held two Windows quirks.** One is the phantom Left Ctrl that Windows
  sends with AltGr (scan code `0x21D`). The other is the dummy key `0xFF` tapped before releasing
  Alt or Win. Both belong in the Windows backend.
- **Per-app rules were `.exe` names.** Import rejected anything else. On macOS an app is a bundle
  id (`com.google.Chrome`); on Linux it is a `WM_CLASS` or Wayland `app_id`.
- **The UI read Windows' own fonts and icons.** It loaded Segoe UI and Segoe Fluent Icons from
  `%WINDIR%`, and the tray loaded its icons from the executable's resources. On other systems the
  icons show as empty boxes.
- **No `#[cfg]` anywhere, and CI on Windows only.** Phase 7 fixes this.

## The architecture we build toward

```
crates/
  core/      remapr-core      Key, Chord, Action, Profile, AppMatcher, Capabilities, decide().
                              No unsafe, no OS dependencies.
  engine/    remapr-engine    One backend per system behind one API: windows, macos, linux.
                              Translates VK, CGKeyCode and evdev codes to and from Key.
  platform/  remapr-platform  Settings folder, single instance, start at sign-in, permissions,
                              app in front, dialogs: one facade, one module per system.
  ui/        remapr-ui        Design system: tokens, widgets, sidebar, the gate widget,
                              fonts and icons on every system.
  update/    remapr-update    One package kind per system: MSI, DMG, AppImage or deb.
  gamepad/   (later)          Physical and virtual controllers.
apps/remapr/                  The shell: navigation and modules.
packaging/{windows,macos,linux}/
```

Apps depend on crates, never the other way around. `core` and `engine` never depend on egui;
from Phase 9, `cargo xtask layers` fails the build if they do.

### Capabilities, not operating systems

The UI never asks which system it runs on. It asks the engine what each function can do:

```rust
pub enum Support { Yes, NeedsPermission(Permission), NeedsDriver(Driver), Unsupported(Reason) }

pub struct Capabilities {
    pub keyboard: Support,        // macOS: NeedsPermission(Accessibility) until granted
    pub mouse: Support,
    pub per_app_rules: Support,   // Linux on Wayland: depends on the compositor
    pub gamepad_input: Support,   // Yes on all three
    pub virtual_gamepad: Support, // macOS: Unsupported · Windows: NeedsDriver(ViGEm)
    pub hide_physical: Support,   // Windows: NeedsDriver(HidHide) · Linux: Yes (EVIOCGRAB)
}
```

One widget in `crates/ui` draws the three states that aren't `Yes`:

| State | Example | What the user sees |
|---|---|---|
| Not available | Virtual controllers on macOS | The section stays visible but dimmed, under a translucent veil, with a card: "Only on Windows and Linux", and a link to why. A lock on its sidebar entry. |
| Needs a permission | Accessibility on macOS, `uinput` on Linux | A card with a button that opens the right settings page. It unlocks by itself once the permission is granted. |
| Needs a driver | ViGEmBus or HidHide on Windows | What gets installed and why, with a button to install it. |

The gate works per function, not per section. On macOS the Controllers section stays usable,
because turning a controller into keyboard and mouse input works there. Only "virtual controller"
as a target and "hide the physical controller" are locked.

## Decisions

1. **`remapr-core` is its own crate** (Phase 8). The compiler then guarantees it stays pure: its
   `Cargo.toml` has no `windows-sys`. Its tests run on every system, and the UI and settings can
   use its types without pulling in OS code.
2. **Keys are positions, identified by HID usage and stored by name** (`from = "CapsLock"`).
   - HID usages are the hardware standard every system maps from. The names are the W3C UI Events
     `code` values, which follow them one to one and which browsers and winit already use.
   - A key is where it sits, not what it types, as on macOS (`CGKeyCode`) and Linux (`evdev`).
     Windows' virtual-key codes for characters depend on the layout, so its backend reads and
     sends characters by scan code. What a key is called on screen comes from the layout in use.
   - Names keep settings readable by hand and portable between systems. Settings carry a
     `version`, and files from 0.5 and earlier, with Win32 numbers, migrate.
3. **ArtCraft is a reference, not a dependency.**
   - We copy and adapt its patterns and, where useful, its code. Its repositories are Apache-2.0.
     Copied code keeps its license header and attribution, says what was changed (Apache-2.0
     §4), and is listed in a third-party notice.
   - We don't depend on `craft-libs` by git. It pins egui and winit to storytold's forks through
     `[patch.crates-io]`, which would tie Remapr to those forks.
4. **macOS remaps through `CGEventTap`, not Karabiner's DriverKit driver.** Karabiner's driver is
   the route kanata takes. Skipping it means there is no system extension to install. It is also
   fragile: there are reports of it no longer receiving the built-in keyboard in a macOS 26.4 beta.
5. **Virtual controllers on Windows go through a trait.** ViGEmBus was archived in November 2023
   and is no longer maintained, though it works and is widely installed. Remapr will detect it and
   help install it, and keep it behind a trait so another driver can replace it. Whether to write
   our own driver (which needs EV signing and Microsoft's attestation) is decided in Phase 15, not
   before.
6. **One look on every system, not each system's own.**
   - Text is Inter, the logo's typeface and the one ArtCraft's suite ships. Icons are Lucide,
     ArtCraft's set. SF Symbols can't be used outside Apple's platforms, and Heroicons has no
     controller or mouse icons.
   - The palette's contrast is a test. The dark theme is near black (`#09090B`): deep on OLED,
     with its layers still apart.
   - The title bar follows ArtCraft's rule. Windows and macOS keep the system's frame; on macOS
     the header runs under it, transparent. Linux keeps the desktop's frame, except on GNOME
     Wayland, which draws none.
7. **reWASD is studied, never reverse-engineered.**
   - On Windows, we look at its public interface and features and describe them in our own words
     in `docs/research/rewasd.md`. Screenshots stay private, and none of its text, icons or
     binaries come into the repository.
   - kanata is LGPL-3.0: read it for ideas, don't copy its code into an MIT project.

## Patterns taken from ArtCraft

These are read from the repositories of [storytold](https://github.com/storytold) on 2026-10-29.

| Pattern | Where it is in ArtCraft | Where it goes in Remapr |
|---|---|---|
| Pure logic plus one `#[cfg(target_os)]` module per system, in one crate. The OS code only translates; what a system can't do returns `Unsupported`. | `photocraft:crates/tablet` | `crates/platform` (Phase 7), the engine backends (Phase 9) |
| A `Backend` trait with a `Capabilities` struct, and `Option` services that gray out a function when missing. | `craft-launcher:crates/core/src/protocol.rs`, `vectorcraft:crates/ui-egui/src/services.rs` | The engine contract and the gate widget (Phases 9 and 12) |
| Layers checked by xtask: nothing below the UI depends on egui. | `vectorcraft:xtask/src/layers.rs` | `cargo xtask layers` (Phase 9) |
| Settings with `SCHEMA_VERSION` and `migrate()`, atomic saves, recovery of damaged files. | `craft-libs:crates/settings` | The settings migration (Phase 8) |
| The chords each system keeps for itself: Alt+Tab and Alt+F4 on Windows, Cmd+Tab on macOS, Ctrl+Alt+F1…F12 on Linux. | `craft-libs:crates/keymap` (`reserved::system`) | Warnings when mapping them (Phase 12) |
| One update flow over many package kinds (MSI, DMG, AppImage, deb…). | `craft-libs:crates/update` | `crates/update` (Phases 10 and 11) |
| Logging through the `log` facade to a rotated file in the settings folder. | `vectorcraft:apps/vectorcraft/src/logging.rs` | Phase 9 |
| SVG icon sets (Lucide) instead of a system icon font, and Inter as the suite's typeface. | `craft-libs:crates/ui` (`icons::SvgIconSet`), `photocraft:assets/icons`, `craft-fonts` | Remapr's icons and fonts (Phase 7) |
| Who draws the window frame on each system, and an app strip that acts as the title bar. | `craft-libs:crates/window` (`title_bar`) | Windows and macOS (Phase 7), Linux (Phase 11) |
| Screenshot tests of the UI with `egui_kittest`. | `craft-libs:crates/ui` (`screen-testing`) | Design snapshots (Phase 7) |
| `packaging/{windows,macos,linux}` with Info.plist, entitlements, `.desktop` and nfpm files. | `vectorcraft:packaging` | Phases 10 and 11 |
| Translations as TSV catalogs keyed by the English text. | `craft-i18n` | Already in Remapr since Phase 6 |

## Horizons

- **Short term: foundations (Phases 7 to 9).** No new features. The workspace builds everywhere,
  keys become universal, and the app talks to one engine contract. Windows keeps working exactly
  as before.
- **Medium term: the keyboard on all three systems, and the app as a shell (Phases 10 to 13).**
  macOS and Linux get keyboard remapping. The app gets a sidebar, profiles and the gate widget.
  Then the mouse and macros arrive on all three systems.
- **Long term: controllers (Phases 14 and 15, and beyond).** First physical controllers to
  keyboard and mouse, on all three systems. Then virtual controllers, starting with Linux, where
  no driver is needed. After that, toward what reWASD offers: profiles per device, combos, gyro
  to mouse, rumble, shared profiles, and a `remapr-cli` to switch profiles from scripts.

## Rules for every phase

- **A phase is done when** it is merged, CI is green, and it has been used on real hardware.
- **The hot path stays clean.** No locks, allocation or I/O inside any backend's callback.
  Decisions live in `core`, each with a unit test.
- **Settings are versioned.** Migrations are tested against real files from older versions.
- **The updater's file names are a contract.** Installed copies look for
  `Remapr-<version>-x64.msi`. If release names move to ArtCraft's `<app>-<ver>-<os>-<arch>`, the
  Windows release keeps publishing the old name too.
- **Docs, code and comments are in English** (CLAUDE.md). INVESTIGATION.md stays as written.

## Working between the Mac and Windows

- **On the Mac:** Phases 7 to 10 and 12, and all the UI work. CI on Windows catches regressions
  while refactoring from the Mac.
- **On Windows:** every refactor is checked with a real keyboard before merging, above all Phase
  8, which migrates users' settings. The reWASD study happens there, and later the Windows
  controller work.

## Sources

- [ViGEm End of Life (Nefarius)](https://docs.nefarius.at/projects/ViGEm/End-of-Life/)
- [nefarius/ViGEmBus on GitHub (archived)](https://github.com/nefarius/vigembus)
- [Karabiner-DriverKit-VirtualHIDDevice](https://www.github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice)
- [Apple Developer Forums: macOS 26.4 beta and DriverKit virtual HID devices](https://developer.apple.com/forums/thread/817009)
- [kanata, macOS module](https://docs.rs/crate/kanata/1.11.0/source/src/oskbd/macos.rs)
- [storytold on GitHub (ArtCraft)](https://github.com/storytold)
