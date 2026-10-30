//! Remapr's engine: each system's backend hooks the keyboard, translates its key codes to
//! [`remapr_core::Key`]s, asks [`remapr_core::decide`] what to do, and injects the result.
//!
//! The backends' tables and rules are plain Rust, compiled and tested on every system; only the
//! hooks themselves need their system. Windows is the only backend so far: elsewhere
//! [`Engine::start`] fails with `io::ErrorKind::Unsupported`, until Phases 10 and 11 bring macOS
//! and Linux.

// Only the Windows hook uses most of it; its tests run everywhere.
#[cfg_attr(not(windows), allow(dead_code))]
mod windows;

#[cfg(not(windows))]
mod unsupported;

#[cfg(not(windows))]
pub use unsupported::{Engine, key_from_vk, layout_char};
#[cfg(windows)]
pub use windows::{Engine, key_from_vk, layout_char};

/// The key a setting from Remapr 0.5 or earlier meant by the virtual-key code `vk`, read as a US
/// layout would. [`key_from_vk`] reads it through the layout in use where there is one.
pub fn us_key_from_vk(vk: u8) -> Option<remapr_core::Key> {
    windows::keys::key_of_old_vk(vk)
}
