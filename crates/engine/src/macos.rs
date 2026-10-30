//! macOS: an event tap and `CGEventPost` (`tap`), macOS's own key mapping through `hidutil`
//! (`hid`) and what the tap asks the system (`system`), around plain tables and rules ([`keys`],
//! [`output`]) that compile and are tested on every system.

pub mod keys;
pub mod output;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod hid;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod system;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod tap;

#[cfg(target_os = "macos")]
pub use tap::capabilities;

/// Starts the tap thread.
#[cfg(target_os = "macos")]
pub fn start(wake: impl Fn() + Send + 'static) -> std::io::Result<Box<dyn remapr_core::Backend>> {
    Ok(Box::new(tap::Engine::start(wake)?))
}

/// Not asked yet (Phase 10 continues): callers fall back to the US label.
#[cfg(target_os = "macos")]
pub fn layout_char(_key: remapr_core::Key) -> Option<char> {
    None
}
