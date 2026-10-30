//! Windows: the low-level keyboard hook and `SendInput` (`hook`) and what the layout in use says
//! (`layout`), around plain tables and rules ([`keys`], [`output`]) that compile and are tested on
//! every system.

use remapr_core::{Capabilities, Support};

pub mod keys;
pub mod output;

#[cfg(windows)]
#[allow(unsafe_code)]
mod hook;
#[cfg(windows)]
#[allow(unsafe_code)]
mod layout;

#[cfg(windows)]
pub use layout::layout_char;

/// What Windows' backend does so far. Controllers come in Phases 14 and 15, virtual ones with
/// drivers.
pub const CAPABILITIES: Capabilities = Capabilities {
    keyboard: Support::Yes,
    per_app_rules: Support::Yes,
    mouse: Support::NotYet,
    controllers: Support::NotYet,
    virtual_controllers: Support::NotYet,
    hide_controllers: Support::NotYet,
};

pub fn capabilities() -> Capabilities {
    CAPABILITIES
}

/// Installs the keyboard hook.
#[cfg(windows)]
pub fn start(wake: impl Fn() + Send + 'static) -> std::io::Result<Box<dyn remapr_core::Backend>> {
    Ok(Box::new(hook::Engine::start(wake)?))
}
