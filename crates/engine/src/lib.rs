//! Remapr's engine: a low-level keyboard hook that swaps keys and shortcuts system-wide, globally or
//! only while a given app is in front.
//!
//! Only [`hook`] touches Win32 and `unsafe`; the decision of what to do with each key lives in
//! [`remap`], which is plain Rust and fully unit-tested on every system.
//!
//! The hook is Windows-only for now. Elsewhere [`Engine::start`] fails with
//! `io::ErrorKind::Unsupported`, until Phases 10 and 11 bring macOS and Linux backends.
//!
//! Keys are identified by their Windows virtual-key code (`VK_*`, `u8`).

#[cfg(windows)]
#[allow(unsafe_code)]
mod hook;
// Only the Windows hook calls it for now; its tests run everywhere.
#[cfg_attr(not(windows), allow(dead_code))]
mod remap;
#[cfg(not(windows))]
mod unsupported;

#[cfg(windows)]
pub use hook::{Engine, layout_char};
#[cfg(not(windows))]
pub use unsupported::{Engine, layout_char};

/// A virtual-key code.
pub type Vk = u8;

/// Modifiers, without telling left from right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods(pub u8);

impl Mods {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1);
    pub const SHIFT: Self = Self(2);
    pub const ALT: Self = Self(4);
    pub const WIN: Self = Self(8);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Mods {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A key pressed with some modifiers held: `Ctrl + Shift + T`. No modifiers is just the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub mods: Mods,
    pub key: Vk,
}

/// Pressing `from` sends `to` instead, everywhere or only while `app` is in front.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub from: Chord,
    pub to: Chord,
    /// The executable's file name, like `chrome.exe`; `None` for every app.
    pub app: Option<String>,
}

impl Rule {
    /// Key to key, with no modifiers on either side: it applies whatever modifiers are held, which
    /// pass through (Shift + Caps Lock with Caps Lock to Esc is Shift + Esc).
    pub fn is_plain(&self) -> bool {
        self.from.mods.is_empty() && self.to.mods.is_empty()
    }
}
