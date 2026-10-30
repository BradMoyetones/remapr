//! Windows: the low-level keyboard hook and `SendInput` (`hook`) and what the layout in use says
//! (`layout`), around plain tables and rules ([`keys`], [`output`]) that compile and are tested on
//! every system.

pub mod keys;
pub mod output;

#[cfg(windows)]
#[allow(unsafe_code)]
mod hook;
#[cfg(windows)]
#[allow(unsafe_code)]
mod layout;

#[cfg(windows)]
pub use hook::Engine;
#[cfg(windows)]
pub use layout::layout_char;
