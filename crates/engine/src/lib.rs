//! Remapr's engine: each system's backend hooks the keyboard, translates its key codes to
//! [`remapr_core::Key`]s, asks [`remapr_core::decide`] what to do, and injects the result. The app
//! sees it as a [`remapr_core::Backend`].
//!
//! The backends' tables and rules are plain Rust, compiled and tested on every system; only the
//! hooks themselves need their system. Windows and macOS have one; on Linux [`start`] fails with
//! `io::ErrorKind::Unsupported`, and [`capabilities`] says so, until Phase 11.

use std::io;

use remapr_core::{Backend, Capabilities, Key};

// Only each system's hook uses most of its module; the tests run everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos;
#[cfg_attr(not(windows), allow(dead_code))]
mod windows;

#[cfg(not(any(windows, target_os = "macos")))]
mod unsupported;

#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(not(any(windows, target_os = "macos")))]
use unsupported as backend;
#[cfg(windows)]
use windows as backend;

/// What this system's backend can do, started or not.
pub fn capabilities() -> Capabilities {
    backend::capabilities()
}

/// Starts this system's backend. `wake` runs on the backend's own thread whenever it has events,
/// so the app can repaint and poll them. Fails with `io::ErrorKind::Unsupported` where Remapr has
/// no backend yet. Only one backend runs per process.
pub fn start(wake: impl Fn() + Send + 'static) -> io::Result<Box<dyn Backend>> {
    backend::start(wake)
}

/// The character `key` types on the keyboard layout in use, where the system can say.
pub fn layout_char(key: Key) -> Option<char> {
    backend::layout_char(key)
}
