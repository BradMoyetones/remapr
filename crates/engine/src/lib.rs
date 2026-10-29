//! Remapr's engine: a low-level keyboard hook that swaps one key for another system-wide.
//!
//! Only [`hook`] touches Win32 and `unsafe`; the decision of what to do with each key lives in
//! [`remap`], which is plain Rust and fully unit-tested.
//!
//! Keys are identified by their Windows virtual-key code (`VK_*`, `u8`).

#[allow(unsafe_code)]
mod hook;
mod keys;
mod remap;

pub use hook::Engine;
pub use keys::key_name;

/// A virtual-key code.
pub type Vk = u8;
