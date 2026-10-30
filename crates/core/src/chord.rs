//! [`Mods`] and [`Chord`]: a key pressed with some modifiers held.

use std::fmt;
use std::str::FromStr;

use crate::Key;
use crate::key::UnknownName;

/// Modifiers, without telling left from right. Meta is the Windows key, ⌘ Command on macOS and
/// Super on Linux.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods(pub u8);

impl Mods {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1);
    pub const SHIFT: Self = Self(2);
    pub const ALT: Self = Self(4);
    pub const META: Self = Self(8);
    /// Each modifier with its name in settings files, in the order chords are written.
    pub const NAMED: [(Mods, &'static str); 4] =
        [(Mods::CTRL, "Ctrl"), (Mods::SHIFT, "Shift"), (Mods::ALT, "Alt"), (Mods::META, "Meta")];

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub const fn is_empty(self) -> bool {
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
///
/// Written `Ctrl+Shift+KeyT` in settings files: the modifiers, then the key's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub mods: Mods,
    pub key: Key,
}

impl Chord {
    /// The key alone.
    pub const fn key(key: Key) -> Self {
        Self { mods: Mods::NONE, key }
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (m, name) in Mods::NAMED {
            if self.mods.contains(m) {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(self.key.name())
    }
}

impl FromStr for Chord {
    type Err = UnknownName;

    /// `Ctrl+Shift+KeyT`, `CapsLock`. Modifier names ignore case, since people write them by hand.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut parts = text.split('+').map(str::trim);
        let key = parts.next_back().filter(|k| !k.is_empty()).ok_or_else(|| UnknownName(text.to_owned()))?;
        let mut mods = Mods::NONE;
        for part in parts {
            let (m, _) = Mods::NAMED
                .into_iter()
                .find(|(_, name)| name.eq_ignore_ascii_case(part))
                .ok_or_else(|| UnknownName(part.to_owned()))?;
            mods = mods | m;
        }
        Ok(Self { mods, key: key.parse()? })
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Chord {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Chord {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_read_back_as_written() {
        let chord = Chord { mods: Mods::CTRL | Mods::SHIFT, key: Key::KeyT };
        assert_eq!(chord.to_string(), "Ctrl+Shift+KeyT");
        assert_eq!("Ctrl+Shift+KeyT".parse(), Ok(chord));
        assert_eq!("shift + ctrl + KeyT".parse(), Ok(chord));
        assert_eq!("CapsLock".parse(), Ok(Chord::key(Key::CapsLock)));
        assert_eq!(Chord::key(Key::NumpadAdd).to_string(), "NumpadAdd");
        assert_eq!(Chord { mods: Mods::META, key: Key::KeyL }.to_string(), "Meta+KeyL");
    }

    #[test]
    fn nonsense_isnt_a_chord() {
        for text in ["", "Ctrl+", "Hyper+KeyT", "Ctrl+Shift", "KeyT+Ctrl", "capslock"] {
            assert!(text.parse::<Chord>().is_err(), "{text:?}");
        }
    }
}
