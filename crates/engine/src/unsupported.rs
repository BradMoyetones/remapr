//! The engine on systems without a backend yet: it never starts, so no `Engine` value exists.

use std::io;

use remapr_core::{Chord, Key, Rule};

/// Can't be created here: [`Engine::start`] always fails.
pub enum Engine {}

impl Engine {
    pub fn start(_on_capture: impl Fn() + Send + 'static) -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "remapping isn't available on this system yet"))
    }

    pub fn set_rules(&self, _rules: Vec<Rule>) {
        match *self {}
    }

    pub fn set_enabled(&self, _enabled: bool) {
        match *self {}
    }

    pub fn start_capture(&self) {
        match *self {}
    }

    pub fn cancel_capture(&self) {
        match *self {}
    }

    pub fn captured(&self) -> Option<Chord> {
        match *self {}
    }
}

/// No layout to ask here: callers fall back to [`Key::us_char`].
pub fn layout_char(_key: Key) -> Option<char> {
    None
}
