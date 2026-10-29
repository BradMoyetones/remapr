# Third-party work in Remapr

Remapr is MIT-licensed (see [LICENSE](LICENSE)). These parts come from others and keep their own
licences.

| What | Where in Remapr | Author | Source | Licence |
|---|---|---|---|---|
| Inter 4.1, Regular and SemiBold, cut down (see the folder's README) | `crates/ui/assets/fonts/*.ttf`; the logo's R (`assets/logo-*.svg`), as a path | Rasmus Andersson | <https://github.com/rsms/inter> | SIL OFL 1.1, [`OFL-inter.txt`](crates/ui/assets/fonts/OFL-inter.txt) |
| Lucide icons 1.64.0, unchanged | `crates/ui/assets/icons/*.svg` | Lucide Icons and Contributors; some derived from Feather by Cole Bemis | <https://github.com/lucide-icons/lucide> (`icons/<name>.svg`) | ISC (Feather: MIT), [`LICENSE-lucide.txt`](crates/ui/assets/icons/LICENSE-lucide.txt) |
| The title-bar drag detection, and how macOS's double-click setting is read | `crates/ui/src/title_bar.rs`, `crates/platform/src/unix.rs` (`title_bar_double_click`) | ArtCraft (storytold) | `craft-libs/crates/window/src/title_bar.rs` in <https://github.com/storytold/craft-libs> | Apache-2.0; adapted (see the comments there for what changed) |
