//! macOS and Linux, until Phases 10 and 11 give each its own module. What is simple is done here
//! (the settings folder, opening links, the display language); the rest reports that it isn't
//! available: an `Unsupported` error, `None`, `false` or nothing at all.
//!
//! Functions taking an `hwnd` belong to Windows' windows: the app only has one on Windows, so here
//! they are never reached with a real window.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Once, OnceLock};
use std::time::Duration;

use crate::{FileKind, RunningApp, TitleBarDoubleClick};

fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "not available on this system yet")
}

/// `~/Library/Application Support` on macOS; `$XDG_CONFIG_HOME`, or `~/.config`, elsewhere.
pub fn config_dir() -> Option<PathBuf> {
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home().map(|home| home.join("Library").join("Application Support"));
    }
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|dir| dir.is_absolute());
    xdg.or_else(|| home().map(|home| home.join(".config")))
}

/// Opens `url` in the default browser.
pub fn open_in_browser(url: &str) {
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    if let Ok(mut child) = Command::new(opener).arg(url).spawn() {
        // Reaped on a thread, so it doesn't linger as a zombie process.
        let _ = std::thread::Builder::new().name("remapr-open-url".into()).spawn(move || child.wait());
    }
}

/// The languages of the user's locale variables, most wanted first, as tags like `es-CO`. On macOS
/// these are set in a terminal but not for apps opened from the Finder, which then get English.
pub fn preferred_languages() -> Vec<String> {
    let vars = ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"];
    language_tags(vars.iter().filter_map(|var| std::env::var(var).ok()))
}

/// Tags from locale values: `LANGUAGE` lists several (`es_CO:es`), the others name one
/// (`es_CO.UTF-8`). Encodings and modifiers are dropped; `C` and `POSIX` aren't languages.
fn language_tags(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for value in values {
        for locale in value.split(':') {
            let name = locale.split(['.', '@']).next().unwrap_or_default();
            if name.is_empty() || name == "C" || name == "POSIX" {
                continue;
            }
            let tag = name.replace('_', "-");
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    tags
}

/// No native dialogs yet: always as if cancelled.
pub fn save_file_dialog(_owner: Option<isize>, _title: &str, _file_name: &str, _kind: &FileKind) -> Option<PathBuf> {
    None
}

/// No native dialogs yet: always as if cancelled.
pub fn open_file_dialog(_owner: Option<isize>, _title: &str, _kind: &FileKind) -> Option<PathBuf> {
    None
}

/// Administrator rights are a Windows matter for Remapr.
pub fn is_elevated() -> bool {
    false
}

/// Not on this system yet: there is never a lock to hold, so each launch runs on its own.
pub enum SingleInstance {}

impl SingleInstance {
    pub fn acquire() -> io::Result<Option<Self>> {
        Err(unsupported())
    }

    pub fn listen(self, _on_show: impl Fn() + Send + 'static) {
        match self {}
    }

    pub fn request_show(_timeout: Duration) -> bool {
        false
    }
}

pub fn exe_name(_hwnd: isize) -> Option<String> {
    None
}

/// Not listed yet: the app picker offers only "All apps".
pub fn running_apps() -> Vec<RunningApp> {
    Vec::new()
}

/// Windows' raw input registration: nothing to undo here.
pub fn stop_raw_keyboard_input() -> bool {
    true
}

/// No installer yet, so never installed.
pub fn install_dir() -> Option<PathBuf> {
    None
}

pub fn repaint_title_bar_when_shown(_hwnd: isize) {}

pub fn hide_first_show(_hwnd: isize) {}

pub fn keep_title_bar_theme(_dark: Option<bool>) {}

pub fn is_foreground(_hwnd: isize) -> bool {
    false
}

pub fn small_icon_size() -> u32 {
    16
}

pub fn use_exe_icon(_hwnd: isize, _id: u16) {}

/// Windows' title bar colours.
pub fn tint_title_bar(_hwnd: isize, _caption: [u8; 3], _text: [u8; 3], _border: [u8; 3]) {}

/// macOS: the user's setting, read once on a background thread (the first call starts it and zooms
/// until it is known). Elsewhere a zoom.
pub fn title_bar_double_click() -> TitleBarDoubleClick {
    static ACTION: OnceLock<TitleBarDoubleClick> = OnceLock::new();
    static READ: Once = Once::new();
    if cfg!(target_os = "macos") {
        READ.call_once(|| {
            let _ = std::thread::Builder::new().name("remapr-title-bar-setting".into()).spawn(|| {
                let out = Command::new("/usr/bin/defaults").args(["read", "-g", "AppleActionOnDoubleClick"]).output();
                // Absent (never changed) or unreadable: macOS's default, zoom.
                let action = match out {
                    Ok(out) if out.status.success() => double_click_action(&String::from_utf8_lossy(&out.stdout)),
                    _ => TitleBarDoubleClick::Zoom,
                };
                let _ = ACTION.set(action);
            });
        });
    }
    ACTION.get().copied().unwrap_or(TitleBarDoubleClick::Zoom)
}

/// The global default `AppleActionOnDoubleClick` as `defaults read` prints it. Anything but
/// "Minimize" or "None" zooms: macOS's default "Maximize", and macOS 15's "Fill", which zooming
/// comes closest to. (After ArtCraft's craft-window, Apache-2.0.)
fn double_click_action(defaults_output: &str) -> TitleBarDoubleClick {
    match defaults_output.trim() {
        "Minimize" => TitleBarDoubleClick::Minimize,
        "None" => TitleBarDoubleClick::Nothing,
        _ => TitleBarDoubleClick::Zoom,
    }
}

pub mod startup {
    use super::*;

    pub fn register(_exe: &Path, _at_sign_in: bool) -> io::Result<()> {
        Err(unsupported())
    }

    pub fn unregister() -> io::Result<()> {
        Err(unsupported())
    }

    pub fn run() -> io::Result<()> {
        Err(unsupported())
    }

    pub fn starts_at_sign_in() -> Option<bool> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_values_become_language_tags() {
        let values = ["es_CO:es", "", "en_US.UTF-8", "C.UTF-8", "de_DE@euro", "es"];
        let tags = language_tags(values.iter().map(|v| v.to_string()));
        assert_eq!(tags, ["es-CO", "es", "en-US", "de-DE"]);
    }

    #[test]
    fn reads_the_double_click_setting() {
        assert_eq!(double_click_action("Maximize\n"), TitleBarDoubleClick::Zoom);
        assert_eq!(double_click_action("Minimize\n"), TitleBarDoubleClick::Minimize);
        assert_eq!(double_click_action("None\n"), TitleBarDoubleClick::Nothing);
        for junk in ["", "Fill", "minimize", "1"] {
            assert_eq!(double_click_action(junk), TitleBarDoubleClick::Zoom, "{junk:?}");
        }
    }
}
