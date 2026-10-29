//! Remapr's integration with Windows itself, kept apart from the keyboard engine.
//! Each module is a thin wrapper over a few Win32 calls (or, for `startup`, over schtasks.exe).

mod browser;
#[allow(unsafe_code)]
mod dialogs;
#[allow(unsafe_code)]
mod elevation;
#[allow(unsafe_code)]
mod instance;
#[allow(unsafe_code)]
mod language;
#[allow(unsafe_code)]
mod process;
#[allow(unsafe_code)]
mod raw_input;
#[allow(unsafe_code)]
mod registry;
pub mod startup;
#[allow(unsafe_code)]
mod window;

pub use browser::open_in_browser;
pub use dialogs::{FileKind, open_file_dialog, save_file_dialog};
pub use elevation::is_elevated;
pub use instance::SingleInstance;
pub use language::preferred_languages;
pub use process::{RunningApp, exe_name, running_apps};
pub use raw_input::stop_raw_keyboard_input;
pub use registry::install_dir;
pub use window::{
    hide_first_show, is_foreground, keep_title_bar_theme, repaint_title_bar_when_shown, small_icon_size, use_exe_icon,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
