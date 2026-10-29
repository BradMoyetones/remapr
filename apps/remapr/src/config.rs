//! The user's settings, in `%APPDATA%\Remapr\config.toml`.

use std::io;
use std::path::PathBuf;

use remapr_engine::{Chord, Mods, Rule, Vk};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
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
            enabled: true,
            theme: Theme::System,
            mappings: Vec::new(),
            tray_hint_seen: false,
            last_version: String::new(),
        }
    }
}

/// Pressing `from` (with `from_mods` held) sends `to` (with `to_mods`), everywhere or only in `app`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub from: Vk,
    pub to: Vk,
    /// A shortcut's modifiers, as `remapr_engine::Mods` bits; left out for a single key.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub from_mods: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub to_mods: u8,
    /// Only while this program is in front (`chrome.exe`); every app when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

impl Mapping {
    pub fn new(from: Chord, to: Chord) -> Self {
        Self { from: from.key, to: to.key, from_mods: from.mods.0, to_mods: to.mods.0, app: None }
    }

    pub fn source(&self) -> Chord {
        Chord { mods: Mods(self.from_mods), key: self.from }
    }

    pub fn target(&self) -> Chord {
        Chord { mods: Mods(self.to_mods), key: self.to }
    }

    pub fn rule(&self) -> Rule {
        Rule { from: self.source(), to: self.target(), app: self.app.clone() }
    }

    /// Whether `other` fires on the same keys in the same place, so only one of them can stay.
    pub fn clashes_with(&self, other: &Self) -> bool {
        self.source() == other.source() && self.app == other.app
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
    let appdata = std::env::var_os("APPDATA").map_or_else(std::env::temp_dir, PathBuf::from);
    // Debug builds keep their own settings, so trying things never touches the installed copy's.
    let folder = if cfg!(debug_assertions) { "Remapr-dev" } else { "Remapr" };
    appdata.join(folder).join("config.toml")
}

impl Config {
    /// The saved config, or defaults. A file that can't be read is set aside as
    /// `config.toml.bad` (never overwritten) and the returned message says so.
    pub fn load() -> (Self, Option<String>) {
        let path = path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => return (Self::default(), Some(tr!("Couldn't read the settings: {error}", error = e))),
        };
        match toml::from_str(&text) {
            Ok(config) => (config, None),
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
#[derive(Serialize, Deserialize)]
struct Exported {
    mappings: Vec<Mapping>,
}

/// Mappings as an export file's text.
pub fn export(mappings: &[Mapping]) -> String {
    let body = toml::to_string_pretty(&Exported { mappings: mappings.to_vec() }).unwrap_or_default();
    format!("# {}\n\n{body}", tr!("Remapr: exported mappings. Import them from Remapr's ⋯ menu."))
}

/// The usable mappings in an export file someone hands us. Rows that make no sense (no key, the
/// same keys on both sides, an app that isn't a program's file name) are left out.
pub fn import(text: &str) -> Result<Vec<Mapping>, String> {
    const MAX_BYTES: usize = 1 << 20;
    const MAX_MAPPINGS: usize = 1000;
    if text.len() > MAX_BYTES {
        return Err(tr!("The file is too big to be a Remapr export.").into());
    }
    let exported: Exported = toml::from_str(text).map_err(|_| tr!("The file isn't a Remapr export.").to_owned())?;
    let valid_app =
        |app: &str| app.len() <= 260 && app.to_ascii_lowercase().ends_with(".exe") && !app.contains(['\\', '/', ':']);
    let mappings: Vec<Mapping> = exported
        .mappings
        .into_iter()
        .filter(|m| m.from != 0 && m.to != 0 && m.source() != m.target())
        .filter(|m| m.from_mods < 16 && m.to_mods < 16 && m.app.as_deref().is_none_or(valid_app))
        .take(MAX_MAPPINGS)
        .collect();
    if mappings.is_empty() { Err(tr!("The file has no usable mappings.").into()) } else { Ok(mappings) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_reads_back_and_import_drops_nonsense() {
        let good = Mapping { from: 0x14, to: 0x1B, from_mods: 0, to_mods: 0, app: Some("code.exe".into()) };
        assert_eq!(import(&export(std::slice::from_ref(&good))).unwrap(), vec![good.clone()]);

        let file = "[[mappings]]\nfrom = 20\nto = 20\n\n\
                    [[mappings]]\nfrom = 20\nto = 27\napp = \"C:\\\\evil\\\\x.exe\"\n\n\
                    [[mappings]]\nfrom = 0\nto = 27\n\n\
                    [[mappings]]\nfrom = 20\nto = 27\n";
        let imported = import(file).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].app, None);

        assert!(import("not toml at all {").is_err());
        assert!(import("[[mappings]]\nfrom = 20\nto = 20\n").is_err());
    }

    #[test]
    fn round_trips_and_fills_missing_fields() {
        let config = Config {
            enabled: false,
            theme: Theme::Dark,
            mappings: vec![
                Mapping { from: 0x14, to: 0x1B, from_mods: 0, to_mods: 0, app: None },
                Mapping { from: b'T', to: 0x74, from_mods: 3, to_mods: 1, app: Some("chrome.exe".into()) },
            ],
            tray_hint_seen: true,
            last_version: "0.1.0".into(),
        };
        let text = toml::to_string_pretty(&config).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), config);
        assert_eq!(toml::from_str::<Config>("").unwrap(), Config::default());
        // A 0.1 config, single keys only, still reads.
        let old: Config = toml::from_str("[[mappings]]\nfrom = 20\nto = 27\n").unwrap();
        assert_eq!(old.mappings[0].rule().from, Chord { mods: Mods::NONE, key: 20 });
        assert_eq!(old.mappings[0].app, None);
    }
}
