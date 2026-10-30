//! [`Key`]: one physical key, the same on every system.
//!
//! Keys are the USB HID usages of the Keyboard/Keypad page (0x07), the standard every keyboard
//! speaks and every system maps from. They are named as the W3C UI Events `code` values (`KeyA`,
//! `CapsLock`, `ArrowLeft`), which follow those usages one to one. Media and browser keys come from
//! the Consumer page (0x0C): they take the Keyboard page's reserved numbers from 0xE8 up, so every
//! key still fits a byte.
//!
//! A key is a position, not a character: `KeyQ` is the key right of Tab, which types Q on QWERTY
//! and A on AZERTY. What a key is called on screen comes from the layout in use.
//!
//! Settings store keys by name. The numbers may change between versions; the names never do.

use std::fmt;
use std::str::FromStr;

macro_rules! keys {
    (
        keyboard { $($k:ident = $ku:literal,)* }
        consumer { $($c:ident = $cv:literal => $cu:literal,)* }
    ) => {
        /// One physical key: see the module documentation.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum Key {
            $($k = $ku,)*
            $($c = $cv,)*
        }

        impl Key {
            /// Every key, Keyboard page first, in HID order.
            pub const ALL: &[Key] = &[$(Key::$k,)* $(Key::$c,)*];

            /// Its name in settings and export files: the W3C `code` value.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Key::$k => stringify!($k),)*
                    $(Key::$c => stringify!($c),)*
                }
            }

            /// Its USB HID usage, as (page, usage).
            pub const fn hid(self) -> (u16, u16) {
                match self {
                    $(Key::$k => (0x07, $ku),)*
                    $(Key::$c => (0x0C, $cu),)*
                }
            }

            /// The key numbered `index` (see [`Key::index`]).
            pub const fn from_index(index: u8) -> Option<Key> {
                match index {
                    $($ku => Some(Key::$k),)*
                    $($cv => Some(Key::$c),)*
                    _ => None,
                }
            }
        }
    };
}

keys! {
    keyboard {
        KeyA = 0x04, KeyB = 0x05, KeyC = 0x06, KeyD = 0x07, KeyE = 0x08, KeyF = 0x09, KeyG = 0x0A,
        KeyH = 0x0B, KeyI = 0x0C, KeyJ = 0x0D, KeyK = 0x0E, KeyL = 0x0F, KeyM = 0x10, KeyN = 0x11,
        KeyO = 0x12, KeyP = 0x13, KeyQ = 0x14, KeyR = 0x15, KeyS = 0x16, KeyT = 0x17, KeyU = 0x18,
        KeyV = 0x19, KeyW = 0x1A, KeyX = 0x1B, KeyY = 0x1C, KeyZ = 0x1D,
        Digit1 = 0x1E, Digit2 = 0x1F, Digit3 = 0x20, Digit4 = 0x21, Digit5 = 0x22, Digit6 = 0x23,
        Digit7 = 0x24, Digit8 = 0x25, Digit9 = 0x26, Digit0 = 0x27,
        Enter = 0x28, Escape = 0x29, Backspace = 0x2A, Tab = 0x2B, Space = 0x2C,
        Minus = 0x2D, Equal = 0x2E, BracketLeft = 0x2F, BracketRight = 0x30, Backslash = 0x31,
        Semicolon = 0x33, Quote = 0x34, Backquote = 0x35, Comma = 0x36, Period = 0x37, Slash = 0x38,
        CapsLock = 0x39,
        F1 = 0x3A, F2 = 0x3B, F3 = 0x3C, F4 = 0x3D, F5 = 0x3E, F6 = 0x3F, F7 = 0x40, F8 = 0x41,
        F9 = 0x42, F10 = 0x43, F11 = 0x44, F12 = 0x45,
        PrintScreen = 0x46, ScrollLock = 0x47, Pause = 0x48, Insert = 0x49, Home = 0x4A,
        PageUp = 0x4B, Delete = 0x4C, End = 0x4D, PageDown = 0x4E,
        ArrowRight = 0x4F, ArrowLeft = 0x50, ArrowDown = 0x51, ArrowUp = 0x52,
        NumLock = 0x53, NumpadDivide = 0x54, NumpadMultiply = 0x55, NumpadSubtract = 0x56,
        NumpadAdd = 0x57, NumpadEnter = 0x58,
        Numpad1 = 0x59, Numpad2 = 0x5A, Numpad3 = 0x5B, Numpad4 = 0x5C, Numpad5 = 0x5D,
        Numpad6 = 0x5E, Numpad7 = 0x5F, Numpad8 = 0x60, Numpad9 = 0x61, Numpad0 = 0x62,
        NumpadDecimal = 0x63, IntlBackslash = 0x64, ContextMenu = 0x65, Power = 0x66,
        NumpadEqual = 0x67,
        F13 = 0x68, F14 = 0x69, F15 = 0x6A, F16 = 0x6B, F17 = 0x6C, F18 = 0x6D, F19 = 0x6E,
        F20 = 0x6F, F21 = 0x70, F22 = 0x71, F23 = 0x72, F24 = 0x73,
        Help = 0x75, Undo = 0x7A, Cut = 0x7B, Copy = 0x7C, Paste = 0x7D, Find = 0x7E,
        AudioVolumeMute = 0x7F, AudioVolumeUp = 0x80, AudioVolumeDown = 0x81,
        NumpadComma = 0x85, IntlRo = 0x87, KanaMode = 0x88, IntlYen = 0x89, Convert = 0x8A,
        NonConvert = 0x8B, Lang1 = 0x90, Lang2 = 0x91,
        ControlLeft = 0xE0, ShiftLeft = 0xE1, AltLeft = 0xE2, MetaLeft = 0xE3,
        ControlRight = 0xE4, ShiftRight = 0xE5, AltRight = 0xE6, MetaRight = 0xE7,
    }
    consumer {
        MediaTrackNext = 0xE8 => 0xB5,
        MediaTrackPrevious = 0xE9 => 0xB6,
        MediaStop = 0xEA => 0xB7,
        MediaPlayPause = 0xEB => 0xCD,
        MediaSelect = 0xEC => 0x183,
        LaunchMail = 0xED => 0x18A,
        LaunchApp2 = 0xEE => 0x192,
        LaunchApp1 = 0xEF => 0x194,
        BrowserSearch = 0xF0 => 0x221,
        BrowserHome = 0xF1 => 0x223,
        BrowserBack = 0xF2 => 0x224,
        BrowserForward = 0xF3 => 0x225,
        BrowserStop = 0xF4 => 0x226,
        BrowserRefresh = 0xF5 => 0x227,
        BrowserFavorites = 0xF6 => 0x22A,
    }
}

impl Key {
    /// A number for tables indexed by key: `[T; 256]` holds one entry per key.
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// What it types on a US layout, for the keys that type something: the label to show when the
    /// system can't say what the layout in use types.
    pub fn us_char(self) -> Option<char> {
        const A: u8 = Key::KeyA.index();
        const Z: u8 = Key::KeyZ.index();
        const ONE: u8 = Key::Digit1.index();
        const NINE: u8 = Key::Digit9.index();
        let i = self.index();
        Some(match (i, self) {
            (A..=Z, _) => char::from(b'A' + (i - A)),
            (ONE..=NINE, _) => char::from(b'1' + (i - ONE)),
            (_, Key::Digit0) => '0',
            (_, Key::Minus) => '-',
            (_, Key::Equal) => '=',
            (_, Key::BracketLeft) => '[',
            (_, Key::BracketRight) => ']',
            (_, Key::Backslash | Key::IntlBackslash) => '\\',
            (_, Key::Semicolon) => ';',
            (_, Key::Quote) => '\'',
            (_, Key::Backquote) => '`',
            (_, Key::Comma) => ',',
            (_, Key::Period) => '.',
            (_, Key::Slash | Key::IntlRo) => '/',
            (_, Key::IntlYen) => '¥',
            _ => return None,
        })
    }

    /// `n` for the function key Fn.
    pub fn function_number(self) -> Option<u8> {
        const F1: u8 = Key::F1.index();
        const F12: u8 = Key::F12.index();
        const F13: u8 = Key::F13.index();
        const F24: u8 = Key::F24.index();
        match self.index() {
            i @ F1..=F12 => Some(i - F1 + 1),
            i @ F13..=F24 => Some(i - F13 + 13),
            _ => None,
        }
    }

    /// The digit of a numpad digit key.
    pub fn numpad_digit(self) -> Option<u8> {
        const ONE: u8 = Key::Numpad1.index();
        const NINE: u8 = Key::Numpad9.index();
        match self.index() {
            i @ ONE..=NINE => Some(i - ONE + 1),
            _ if self == Key::Numpad0 => Some(0),
            _ => None,
        }
    }

    /// The modifier this key is (Left Ctrl is [`Mods::CTRL`](crate::Mods::CTRL)), or none.
    pub const fn modifier(self) -> crate::Mods {
        use crate::Mods;
        match self {
            Key::ControlLeft | Key::ControlRight => Mods::CTRL,
            Key::ShiftLeft | Key::ShiftRight => Mods::SHIFT,
            Key::AltLeft | Key::AltRight => Mods::ALT,
            Key::MetaLeft | Key::MetaRight => Mods::META,
            _ => Mods::NONE,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A name that isn't a [`Key`]'s, nor (for a chord) a modifier's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownName(pub String);

impl fmt::Display for UnknownName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown key {:?}", self.0)
    }
}

impl std::error::Error for UnknownName {}

impl FromStr for Key {
    type Err = UnknownName;

    /// By name, as written: `CapsLock`, not `capslock`. Not on the hot path: a scan of [`Key::ALL`].
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Key::ALL.iter().copied().find(|k| k.name() == name).ok_or_else(|| UnknownName(name.to_owned()))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Key {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Key {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_numbers_round_trip() {
        for &key in Key::ALL {
            assert_eq!(key.name().parse::<Key>(), Ok(key));
            assert_eq!(Key::from_index(key.index()), Some(key));
        }
        assert!("capslock".parse::<Key>().is_err());
        assert_eq!(Key::from_index(0), None);
    }

    #[test]
    fn every_key_has_its_own_usage() {
        let mut usages: Vec<(u16, u16)> = Key::ALL.iter().map(|k| k.hid()).collect();
        usages.sort_unstable();
        usages.dedup();
        assert_eq!(usages.len(), Key::ALL.len());
        assert_eq!(Key::CapsLock.hid(), (0x07, 0x39));
        assert_eq!(Key::MediaPlayPause.hid(), (0x0C, 0xCD));
    }

    #[test]
    fn labels_and_numbers_of_key_families() {
        assert_eq!(Key::KeyQ.us_char(), Some('Q'));
        assert_eq!(Key::Digit0.us_char(), Some('0'));
        assert_eq!(Key::Digit7.us_char(), Some('7'));
        assert_eq!(Key::Semicolon.us_char(), Some(';'));
        assert_eq!(Key::Enter.us_char(), None);
        assert_eq!(Key::F12.function_number(), Some(12));
        assert_eq!(Key::F13.function_number(), Some(13));
        assert_eq!(Key::F24.function_number(), Some(24));
        assert_eq!(Key::Numpad0.numpad_digit(), Some(0));
        assert_eq!(Key::Numpad9.numpad_digit(), Some(9));
        assert_eq!(Key::NumpadEnter.numpad_digit(), None);
    }
}
