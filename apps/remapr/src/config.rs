//! The user's settings, in `%APPDATA%\Remapr\config.toml` on Windows; other systems use their own
//! settings folder (`remapr_platform::config_dir`).
//!
//! Files say which `version` of the format they are in, so a later change of the format can convert
//! the files before it. Version 2 is the first: Remapr 0.5 and earlier wrote Windows key codes, and
//! nobody but its author had settings to bring along.

use std::io;
use std::path::PathBuf;

use remapr_core::{AppMatcher, Chord, Rule};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;

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
    /// Written by a newer Remapr, in a format this one doesn't know.
    Newer,
}

/// Reads a settings file.
fn read(text: &str) -> Result<Read, toml::de::Error> {
    let table: toml::Table = text.parse()?;
    match version(&table) {
        VERSION => Ok(Read::Current(table.try_into()?)),
        v if v > VERSION => Ok(Read::Newer),
        v => Err(serde::de::Error::custom(format!("unknown settings version {v}"))),
    }
}

/// A file's format version; 0 when it doesn't say.
fn version(table: &toml::Table) -> i64 {
    table.get("version").and_then(toml::Value::as_integer).unwrap_or(0)
}

impl Config {
    /// The saved config, or defaults, with a message for the user when something needs saying.
    ///
    /// A file that can't be read is set aside as `config.toml.bad`, never overwritten. A file from a
    /// newer Remapr is copied to `config.newer.toml` before this one starts with defaults.
    pub fn load() -> (Self, Option<String>) {
        let path = path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => return (Self::default(), Some(tr!("Couldn't read the settings: {error}", error = e))),
        };
        match read(&text) {
            Ok(Read::Current(config)) => (config, None),
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

/// The usable mappings in an export file someone hands us. Rows that make no sense (keys that
/// aren't keys, the same keys on both sides, an app that isn't one) are left out.
pub fn import(text: &str) -> Result<Vec<Mapping>, String> {
    const MAX_BYTES: usize = 1 << 20;
    const MAX_MAPPINGS: usize = 1000;
    if text.len() > MAX_BYTES {
        return Err(tr!("The file is too big to be a Remapr export.").into());
    }
    let not_export = || tr!("The file isn't a Remapr export.").to_owned();
    let table: toml::Table = text.parse().map_err(|_| not_export())?;
    match version(&table) {
        VERSION => {}
        v if v > VERSION => return Err(tr!("The file is from a newer Remapr.").into()),
        _ => return Err(not_export()),
    }
    let rows = table.get("mappings").and_then(toml::Value::as_array).ok_or_else(not_export)?;
    let row = |row: &toml::Value| Mapping::deserialize(row.clone()).ok();
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
    use remapr_core::{Key, Mods};

    use super::*;

    fn mapping(from: Chord, to: Chord, app: Option<&str>) -> Mapping {
        Mapping { from, to, app: app.map(|a| AppMatcher::Exe(a.into())) }
    }

    fn sample() -> Vec<Mapping> {
        let ctrl_shift_t = Chord { mods: Mods::CTRL | Mods::SHIFT, key: Key::KeyT };
        vec![
            mapping(Chord::key(Key::CapsLock), Chord::key(Key::Escape), None),
            mapping(ctrl_shift_t, Chord { mods: Mods::CTRL, key: Key::F5 }, Some("code.exe")),
        ]
    }

    #[test]
    fn the_file_reads_back_and_by_hand() {
        let config =
            Config { theme: Theme::Dark, mappings: sample(), last_version: "0.6.0".into(), ..Config::default() };
        let text = toml::to_string_pretty(&config).unwrap();
        assert!(text.starts_with("version = 2\n"), "{text}");
        assert!(text.contains(r#"from = "Ctrl+Shift+KeyT""#), "{text}");
        assert!(text.contains(r#"to = "Ctrl+F5""#), "{text}");
        assert!(text.contains(r#"exe = "code.exe""#), "{text}");
        assert_eq!(read(&text), Ok(Read::Current(config)));
        assert_eq!(read("version = 2\n"), Ok(Read::Current(Config::default())));
    }

    #[test]
    fn newer_and_unreadable_files_are_told_apart() {
        assert_eq!(read("version = 3\nmappings = \"whatever\"\n"), Ok(Read::Newer));
        // Remapr 0.5's format: no version line, keys as numbers.
        assert!(read("[[mappings]]\nfrom = 20\nto = 27\n").is_err());
        assert!(read("not toml at all {").is_err());
        assert!(read("version = 2\n[[mappings]]\nfrom = \"CapsLok\"\nto = \"Escape\"\n").is_err());
    }

    #[test]
    fn exports_import() {
        let mappings = sample();
        let file = export(&mappings);
        assert!(file.starts_with("# "));
        assert_eq!(import(&file).unwrap(), mappings);
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
        let imported = import(file).unwrap();
        let bundle = Some(AppMatcher::BundleId("com.example.App".into()));
        assert_eq!(imported.iter().map(|m| m.app.clone()).collect::<Vec<_>>(), [bundle, None]);

        assert!(import("[[mappings]]\nfrom = 20\nto = 27\n").is_err(), "0.5's format");
        assert!(import("not toml at all {").is_err());
        assert!(import("version = 3\n[[mappings]]\n").is_err());
        assert!(import("version = 2\nenabled = true\n").is_err());
        assert!(import("version = 2\n[[mappings]]\nfrom = \"F1\"\nto = \"F1\"\n").is_err());
    }
}
