//! Remapr's integration with the operating system, kept apart from the keyboard engine.
//!
//! One facade, one module per system behind it: every function and type below exists on every
//! system, so the app builds anywhere without `#[cfg]`s of its own, and CI on each system catches
//! one that falls behind. `windows` wraps a few Win32 calls; `unix` serves macOS and Linux for now.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as os;
#[cfg(windows)]
use windows as os;

pub use os::{
    SingleInstance, config_dir, exe_name, hide_first_show, install_dir, is_elevated, is_foreground,
    keep_title_bar_theme, open_file_dialog, open_in_browser, preferred_languages, repaint_title_bar_when_shown,
    running_apps, save_file_dialog, small_icon_size, stop_raw_keyboard_input, use_exe_icon,
};

/// Starting Remapr when the user signs in.
pub mod startup {
    /// Remapr started with this argument starts hidden in the tray.
    pub const BACKGROUND_ARG: &str = "--background";

    pub use crate::os::startup::{register, run, starts_at_sign_in, unregister};
}

/// What the file dialogs offer: files ending in `.{extension}`, described as `name`.
pub struct FileKind<'a> {
    pub name: &'a str,
    pub extension: &'a str,
}

/// A program with a window open, as the app picker lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningApp {
    /// Lowercase file name, what a rule stores: `chrome.exe`.
    pub exe: String,
    /// The title of one of its windows, to help recognise it.
    pub title: String,
}
