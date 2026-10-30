//! The user's settings, in `%APPDATA%\Remapr\config.toml` on Windows; other systems use their own
//! settings folder (`remapr_platform::config_dir`).
//!
//! Files say which `version` of the format they are in. Version 1 (Remapr 0.1 to 0.5, with no
//! `version` line) stored Windows key codes: it is converted on load ([`v1`]) and the original
//! kept beside the new file as `config.v1.toml`, for going back to an older Remapr.

mod v1;

use std::io;
use std::path::PathBuf;

use remapr_core::{AppMatcher, Chord, Key, Rule};
use serde::{Deserialize, Serialize};

use crate::i18n::{tr, trn};

/// The format this Remapr writes.
pub const VERSION: i64 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The format's version, [`VERSION`]; first, so it opens the file.
    pub version: i64,
    pub enabled: bool,
    pub theme: Theme,
    pub mappings: Vec<Mapping>,
    /// The user has been told that closing the window keeps Remapr running in the tray.
    pub tray_hint_seen: bool,
    /// The version that last ran, to say "Remapr se actualizó" once after an update.
    pub last_version: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: VERSION,
            enabled: true,
            theme: Theme::System,
            mappings: Vec::new(),
            tray_hint_seen: false,
            last_version: String::new(),
        }
    }
}

/// Pressing `from` sends `to`, everywhere or only in `app`. In the file:
/// `from = "Ctrl+Shift+KeyT"`, `to = "F5"`, `app = { exe = "code.exe" }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub from: Chord,
    pub to: Chord,
    /// Only while this app is in front; every app when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<AppMatcher>,
}

impl Mapping {
    pub fn new(from: Chord, to: Chord) -> Self {
        Self { from, to, app: None }
    }

    pub fn rule(&self) -> Rule {
        Rule { from: self.from, to: self.to, app: self.app.clone() }
    }

    /// Whether `other` fires on the same keys in the same place, so only one of them can stay.
    pub fn clashes_with(&self, other: &Self) -> bool {
        self.from == other.from && self.app == other.app
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

fn path() -> PathBuf {
    let base = remapr_platform::config_dir().unwrap_or_else(std::env::temp_dir);
    // Debug builds keep their own settings, so trying things never touches the installed copy's.
    let folder = if cfg!(debug_assertions) { "Remapr-dev" } else { "Remapr" };
    base.join(folder).join("config.toml")
}

/// What a settings file turned out to be.
#[derive(Debug, PartialEq)]
enum Read {
    Current(Config),
    /// A version 1 file, converted; `dropped` of its mappings had codes that are no key.
    Converted {
        config: Config,
        dropped: usize,
    },
    /// Written by a newer Remapr, in a format this one doesn't know.
    Newer,
}

/// Reads a settings file of any version. `key_from_vk` converts version 1's key codes.
fn read(text: &str, key_from_vk: &dyn Fn(u8) -> Option<Key>) -> Result<Read, toml::de::Error> {
    let table: toml::Table = text.parse()?;
    match version(&table) {
        1 => v1::config(table, key_from_vk).map(|(config, dropped)| Read::Converted { config, dropped }),
        VERSION => Ok(Read::Current(table.try_into()?)),
        v if v > VERSION => Ok(Read::Newer),
        v => Err(serde::de::Error::custom(format!("unknown settings version {v}"))),
    }
}

/// A file's format version: 1 when it doesn't say.
fn version(table: &toml::Table) -> i64 {
    table.get("version").and_then(toml::Value::as_integer).unwrap_or(1)
}

impl Config {
    /// The saved config, or defaults, with a message for the user when something needs saying.
    ///
    /// A file that can't be read is set aside as `config.toml.bad`, never overwritten. A version 1
    /// file is converted and saved, its original kept as `config.v1.toml`. A file from a newer
    /// Remapr is copied to `config.newer.toml` before this one starts with defaults.
    pub fn load() -> (Self, Option<String>) {
        let path = path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => return (Self::default(), Some(tr!("Couldn't read the settings: {error}", error = e))),
        };
        match read(&text, &remapr_engine::key_from_vk) {
            Ok(Read::Current(config)) => (config, None),
            Ok(Read::Converted { config, dropped }) => {
                let old = path.with_file_name("config.v1.toml");
                if !old.exists() {
                    let _ = std::fs::write(&old, &text);
                }
                let message = match config.save() {
                    Err(e) => Some(tr!("Couldn't save the settings: {error}", error = e)),
                    Ok(()) if dropped > 0 => Some(tr!(
                        "{count} couldn't be brought to this version and were left out. The old settings are in {file}",
                        count = trn!("{n} mapping|{n} mappings", dropped),
                        file = old.display()
                    )),
                    Ok(()) => None,
                };
                (config, message)
            }
            Ok(Read::Newer) => {
                let newer = path.with_file_name("config.newer.toml");
                let _ = std::fs::copy(&path, &newer);
                let msg =
                    tr!("The settings are from a newer Remapr. A copy was kept as {file}", file = newer.display());
                (Self::default(), Some(msg))
            }
            Err(_) => {
                let bad = path.with_extension("toml.bad");
                let _ = std::fs::rename(&path, &bad);
                let msg = tr!("The settings were damaged. A copy was saved as {file}", file = bad.display());
                (Self::default(), Some(msg))
            }
        }
    }

    /// Writes beside the file, then swaps it in, so a crash mid-save never leaves a broken config.
    pub fn save(&self) -> io::Result<()> {
        let path = path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(io::Error::other)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path)
    }
}

/// What an export file holds: just the mappings, in the config's own format.
#[derive(Serialize)]
struct Exported<'a> {
    version: i64,
    mappings: &'a [Mapping],
}

/// Mappings as an export file's text.
pub fn export(mappings: &[Mapping]) -> String {
    let body = toml::to_string_pretty(&Exported { version: VERSION, mappings }).unwrap_or_default();
    format!("# {}\n\n{body}", tr!("Remapr: exported mappings. Import them from Remapr's ⋯ menu."))
}

/// The usable mappings in an export file someone hands us, from this version or an earlier one.
/// Rows that make no sense (keys that aren't keys, the same keys on both sides, an app that isn't
/// one) are left out.
pub fn import(text: &str) -> Result<Vec<Mapping>, String> {
    import_with(text, &remapr_engine::key_from_vk)
}

fn import_with(text: &str, key_from_vk: &dyn Fn(u8) -> Option<Key>) -> Result<Vec<Mapping>, String> {
    const MAX_BYTES: usize = 1 << 20;
    const MAX_MAPPINGS: usize = 1000;
    if text.len() > MAX_BYTES {
        return Err(tr!("The file is too big to be a Remapr export.").into());
    }
    let not_export = || tr!("The file isn't a Remapr export.").to_owned();
    let table: toml::Table = text.parse().map_err(|_| not_export())?;
    let version = version(&table);
    if version > VERSION {
        return Err(tr!("The file is from a newer Remapr.").into());
    }
    let rows = table.get("mappings").and_then(toml::Value::as_array).ok_or_else(not_export)?;
    let row = |row: &toml::Value| match version {
        1 => v1::mapping(row.clone(), key_from_vk),
        _ => Mapping::deserialize(row.clone()).ok(),
    };
    let mappings: Vec<Mapping> = rows
        .iter()
        .filter_map(row)
        .filter(|m| m.from != m.to && m.app.as_ref().is_none_or(is_app))
        .take(MAX_MAPPINGS)
        .collect();
    if mappings.is_empty() { Err(tr!("The file has no usable mappings.").into()) } else { Ok(mappings) }
}

/// Whether an imported app is one its system could name.
fn is_app(app: &AppMatcher) -> bool {
    let name = app.name();
    let plausible = !name.is_empty() && name.len() <= 260 && !name.chars().any(char::is_control);
    plausible
        && match app {
            AppMatcher::Exe(exe) => exe.to_ascii_lowercase().ends_with(".exe") && !exe.contains(['\\', '/', ':']),
            AppMatcher::BundleId(id) => id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
            AppMatcher::WindowClass(_) => true,
        }
}

#[cfg(test)]
mod tests {
    use remapr_core::Mods;

    use super::*;

    const CONFIG_0_5: &str = include_str!("../../tests/fixtures/config-0.5.toml");
    const EXPORT_0_5: &str = include_str!("../../tests/fixtures/export-0.5.toml");

    fn us(vk: u8) -> Option<Key> {
        remapr_engine::us_key_from_vk(vk)
    }

    fn chord(mods: Mods, key: Key) -> Chord {
        Chord { mods, key }
    }

    fn mapping(from: Chord, to: Chord, app: Option<&str>) -> Mapping {
        Mapping { from, to, app: app.map(|a| AppMatcher::Exe(a.into())) }
    }

    #[test]
    fn settings_from_0_5_are_converted() {
        let Ok(Read::Converted { config, dropped }) = read(CONFIG_0_5, &us) else { panic!("not converted") };
        let ctrl_shift = Mods::CTRL | Mods::SHIFT;
        let expected = Config {
            version: VERSION,
            enabled: true,
            theme: Theme::Dark,
            mappings: vec![
                mapping(Chord::key(Key::CapsLock), Chord::key(Key::Escape), None),
                mapping(Chord::key(Key::F1), chord(ctrl_shift, Key::KeyT), None),
                mapping(chord(ctrl_shift, Key::KeyT), chord(Mods::CTRL, Key::F5), Some("code.exe")),
                mapping(Chord::key(Key::Backquote), Chord::key(Key::Delete), None),
                mapping(Chord::key(Key::AltRight), Chord::key(Key::ControlLeft), Some("chrome.exe")),
                mapping(Chord::key(Key::ArrowLeft), Chord::key(Key::Home), None),
                mapping(chord(Mods::META, Key::KeyL), chord(Mods::CTRL, Key::KeyL), None),
            ],
            tray_hint_seen: true,
            last_version: "0.5.0".into(),
        };
        assert_eq!(config, expected);
        assert_eq!(dropped, 1, "VK_PACKET is no key");
        // And reads back as itself once saved.
        let text = toml::to_string_pretty(&config).unwrap();
        assert_eq!(read(&text, &us), Ok(Read::Current(config)));
    }

    #[test]
    fn the_file_reads_by_hand() {
        let config = Config {
            mappings: vec![mapping(chord(Mods::CTRL | Mods::SHIFT, Key::KeyT), Chord::key(Key::F5), Some("code.exe"))],
            ..Config::default()
        };
        let text = toml::to_string_pretty(&config).unwrap();
        assert!(text.starts_with("version = 2\n"), "{text}");
        assert!(text.contains(r#"from = "Ctrl+Shift+KeyT""#), "{text}");
        assert!(text.contains(r#"to = "F5""#), "{text}");
        assert!(text.contains(r#"exe = "code.exe""#), "{text}");
        assert_eq!(read(&text, &us), Ok(Read::Current(config)));
        assert_eq!(read("version = 2\n", &us), Ok(Read::Current(Config::default())));
    }

    #[test]
    fn newer_and_broken_files_are_told_apart() {
        assert_eq!(read("version = 3\nmappings = \"whatever\"\n", &us), Ok(Read::Newer));
        assert!(read("version = 0\n", &us).is_err());
        assert!(read("not toml at all {", &us).is_err());
        assert!(read("version = 2\n[[mappings]]\nfrom = \"CapsLok\"\nto = \"Escape\"\n", &us).is_err());
        // An empty version 1 file was a fresh config.
        assert_eq!(read("", &us), Ok(Read::Converted { config: Config::default(), dropped: 0 }));
    }

    #[test]
    fn exports_from_0_5_and_from_now_import() {
        let old = import_with(EXPORT_0_5, &us).unwrap();
        assert_eq!(
            old,
            vec![
                mapping(Chord::key(Key::CapsLock), Chord::key(Key::Escape), None),
                mapping(Chord::key(Key::F1), chord(Mods::CTRL | Mods::SHIFT, Key::KeyT), Some("Code.exe")),
            ]
        );
        assert_eq!(import_with(&export(&old), &us).unwrap(), old);
        assert!(export(&old).starts_with("# "));
    }

    #[test]
    fn import_drops_nonsense() {
        let file = r#"
            version = 2
            [[mappings]]
            from = "CapsLock"
            to = "CapsLock"
            [[mappings]]
            from = "CapsLock"
            to = "Escape"
            app = { exe = "C:\\evil\\x.exe" }
            [[mappings]]
            from = "Hyper+KeyT"
            to = "Escape"
            [[mappings]]
            from = "CapsLock"
            to = "Escape"
            app = { bundle_id = "com.example.App" }
            [[mappings]]
            from = "F1"
            to = "Escape"
        "#;
        let imported = import_with(file, &us).unwrap();
        let bundle = Some(AppMatcher::BundleId("com.example.App".into()));
        assert_eq!(imported.iter().map(|m| m.app.clone()).collect::<Vec<_>>(), [bundle, None]);

        let v1 = "[[mappings]]\nfrom = 20\nto = 20\n\n[[mappings]]\nfrom = 0\nto = 27\n";
        assert!(import_with(v1, &us).is_err());
        assert!(import_with("not toml at all {", &us).is_err());
        assert!(import_with("version = 3\n[[mappings]]\n", &us).is_err());
        assert!(import_with("enabled = true\n", &us).is_err());
    }
}
