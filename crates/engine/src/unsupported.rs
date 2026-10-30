//! Systems without a backend yet: nothing to start, and their capabilities say so.

use std::io;

use remapr_core::{Backend, Capabilities, Key, Support};

/// macOS will never make virtual game controllers (Apple doesn't let apps); everything else comes
/// with Phases 10 (macOS) and 11 (Linux).
pub const CAPABILITIES: Capabilities =
    Capabilities::none(if cfg!(target_os = "macos") { Support::NotOnThisSystem } else { Support::NotYet });

pub fn start(_wake: impl Fn() + Send + 'static) -> io::Result<Box<dyn Backend>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "remapping isn't available on this system yet"))
}

/// No layout to ask here: callers fall back to [`Key::us_char`].
pub fn layout_char(_key: Key) -> Option<char> {
    None
}
