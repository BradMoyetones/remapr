//! Remapr's integration with Windows itself, kept apart from the keyboard engine.
//! Each module is a thin, `unsafe` wrapper over a few Win32 calls.

#[allow(unsafe_code)]
mod instance;
#[allow(unsafe_code)]
mod window;

pub use instance::SingleInstance;
pub use window::{small_icon_size, use_exe_icon};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
