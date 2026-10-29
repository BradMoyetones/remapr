//! The engine on systems without a backend yet: it never starts, so no `Engine` value exists.

use std::io;

use crate::{Chord, Rule, Vk};

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

/// Key codes are Windows' here, so no layout can be asked: callers fall back to the code.
pub fn layout_char(_vk: Vk) -> Option<char> {
    None
}
