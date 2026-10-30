//! Remapr's engine: each system's backend hooks the keyboard, translates its key codes to
//! [`remapr_core::Key`]s, asks [`remapr_core::decide`] what to do, and injects the result. The app
//! sees it as a [`remapr_core::Backend`].
//!
//! The backends' tables and rules are plain Rust, compiled and tested on every system; only the
//! hooks themselves need their system. Windows is the only backend so far: elsewhere [`start`]
//! fails with `io::ErrorKind::Unsupported`, and [`capabilities`] says so, until Phases 10 and 11
//! bring macOS and Linux.

use std::io;

use remapr_core::{Backend, Capabilities, Key};

// Only the Windows hook uses most of it; its tests run everywhere.
#[cfg_attr(not(windows), allow(dead_code))]
mod windows;

#[cfg(not(windows))]
mod unsupported;

#[cfg(not(windows))]
use unsupported as backend;
#[cfg(windows)]
use windows as backend;

/// What this system's backend can do, started or not.
pub fn capabilities() -> Capabilities {
    backend::CAPABILITIES
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
