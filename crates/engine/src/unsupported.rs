//! Systems without a backend yet (Linux, until Phase 11): nothing to start, and their
//! capabilities say so.

use std::io;

use remapr_core::{Backend, Capabilities, Key, Support};

pub fn capabilities() -> Capabilities {
    Capabilities::none(Support::NotYet)
}

pub fn start(_wake: impl Fn() + Send + 'static) -> io::Result<Box<dyn Backend>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "remapping isn't available on this system yet"))
}

/// No layout to ask here: callers fall back to [`Key::us_char`].
pub fn layout_char(_key: Key) -> Option<char> {
    None
}
