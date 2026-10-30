//! Settings and export files of format version 1, from Remapr 0.1 to 0.5: keys as Windows
//! virtual-key codes, modifiers as bits (1 Ctrl, 2 Shift, 4 Alt, 8 Win), the app as a program file
//! name. Read only, to convert them; `tests/fixtures` holds files as those versions wrote them.

use remapr_core::{AppMatcher, Chord, Key, Mods};
use serde::Deserialize;

use super::{Config, Mapping, Theme, VERSION};

/// A version 1 `config.toml`.
#[derive(Deserialize)]
#[serde(default)]
struct File {
    enabled: bool,
    theme: Theme,
    /// Read one by one, so a row that can't be converted leaves the others.
    mappings: Vec<toml::Value>,
    tray_hint_seen: bool,
    last_version: String,
}

impl Default for File {
    fn default() -> Self {
        let Config { enabled, theme, tray_hint_seen, last_version, .. } = Config::default();
        Self { enabled, theme, mappings: Vec::new(), tray_hint_seen, last_version }
    }
}

/// A version 1 mapping row.
#[derive(Deserialize)]
struct Row {
    from: u8,
    to: u8,
    #[serde(default)]
    from_mods: u8,
    #[serde(default)]
    to_mods: u8,
    #[serde(default)]
    app: Option<String>,
}

/// A version 1 settings file in today's format, and how many of its mappings couldn't be
/// converted (a code that is no key).
pub fn config(table: toml::Table, key_from_vk: &dyn Fn(u8) -> Option<Key>) -> Result<(Config, usize), toml::de::Error> {
    let file: File = table.try_into()?;
    let total = file.mappings.len();
    let mappings: Vec<Mapping> = file.mappings.into_iter().filter_map(|row| mapping(row, key_from_vk)).collect();
    let dropped = total - mappings.len();
    let config = Config {
        version: VERSION,
        enabled: file.enabled,
        theme: file.theme,
        mappings,
        tray_hint_seen: file.tray_hint_seen,
        last_version: file.last_version,
    };
    Ok((config, dropped))
}

/// One version 1 mapping row in today's format, if it is one and its codes are keys.
pub fn mapping(row: toml::Value, key_from_vk: &dyn Fn(u8) -> Option<Key>) -> Option<Mapping> {
    let row: Row = row.try_into().ok()?;
    // Unknown bits meant nothing then either.
    let chord = |vk, mods: u8| Some(Chord { mods: Mods(mods & 0x0F), key: key_from_vk(vk)? });
    Some(Mapping {
        from: chord(row.from, row.from_mods)?,
        to: chord(row.to, row.to_mods)?,
        app: row.app.map(AppMatcher::Exe),
    })
}
