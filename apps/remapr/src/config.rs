//! The user's settings, in `%APPDATA%\Remapr\config.toml`.

use std::io;
use std::path::PathBuf;

use remapr_engine::Vk;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub theme: Theme,
    pub mappings: Vec<Mapping>,
    /// The user has been told that closing the window keeps Remapr running in the tray.
    pub tray_hint_seen: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { enabled: true, theme: Theme::System, mappings: Vec::new(), tray_hint_seen: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub from: Vk,
    pub to: Vk,
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
    appdata.join("Remapr").join("config.toml")
}

impl Config {
    /// The saved config, or defaults. A file that can't be read is set aside as
    /// `config.toml.bad` (never overwritten) and the returned message says so.
    pub fn load() -> (Self, Option<String>) {
        let path = path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => return (Self::default(), Some(format!("No se pudo leer la configuración: {e}"))),
        };
        match toml::from_str(&text) {
            Ok(config) => (config, None),
            Err(_) => {
                let bad = path.with_extension("toml.bad");
                let _ = std::fs::rename(&path, &bad);
                let msg = format!("La configuración estaba dañada. Se guardó una copia en {}", bad.display());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_fills_missing_fields() {
        let config = Config {
            enabled: false,
            theme: Theme::Dark,
            mappings: vec![Mapping { from: 0x14, to: 0x1B }],
            tray_hint_seen: true,
        };
        let text = toml::to_string_pretty(&config).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), config);
        assert_eq!(toml::from_str::<Config>("").unwrap(), Config::default());
    }
}
