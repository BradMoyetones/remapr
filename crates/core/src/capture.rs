//! Catching the key or chord the user presses, to make a mapping of it: a key with the modifiers
//! held before it, or a lone modifier. It finishes once every key is up again. A backend feeds it
//! every key event while capturing and swallows them, so nothing reaches any app.

use crate::{Chord, Key, KeyEvent, Mods};

/// What a capture has seen so far. Doesn't allocate: backends feed it from their hooks.
#[derive(Clone, Debug)]
pub struct Capture {
    mods: Mods,
    /// The first modifier pressed, which is the capture if no other key comes.
    first_modifier: Option<Key>,
    chord: Option<Chord>,
    down: [bool; 256],
}

impl Default for Capture {
    fn default() -> Self {
        Self::new()
    }
}

impl Capture {
    pub const fn new() -> Self {
        Self { mods: Mods::NONE, first_modifier: None, chord: None, down: [false; 256] }
    }

    /// Takes one key event; the chord, once every key pressed is up again. Releases of keys
    /// pressed before the capture began are ignored.
    pub fn feed(&mut self, ev: KeyEvent) -> Option<Chord> {
        let i = usize::from(ev.key.index());
        if !ev.up {
            if !self.down[i] && self.chord.is_none() {
                let m = ev.key.modifier();
                if m.is_empty() {
                    self.chord = Some(Chord { mods: self.mods, key: ev.key });
                } else {
                    self.mods = self.mods | m;
                    self.first_modifier.get_or_insert(ev.key);
                }
            }
            self.down[i] = true;
            return None;
        }
        if !self.down[i] {
            return None;
        }
        self.down[i] = false;
        if self.down.contains(&true) {
            return None;
        }
        let caught = self.chord.or(self.first_modifier.map(Chord::key));
        *self = Self::new();
        caught
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(c: &mut Capture, key: Key, up: bool) -> Option<Chord> {
        c.feed(KeyEvent { key, up, injected: false })
    }

    #[test]
    fn a_chord_is_caught_once_every_key_is_up() {
        let mut c = Capture::new();
        assert_eq!(feed(&mut c, Key::ControlLeft, false), None);
        assert_eq!(feed(&mut c, Key::ShiftRight, false), None);
        assert_eq!(feed(&mut c, Key::KeyT, false), None);
        assert_eq!(feed(&mut c, Key::KeyT, false), None, "key repeat");
        assert_eq!(feed(&mut c, Key::KeyT, true), None);
        assert_eq!(feed(&mut c, Key::ControlLeft, true), None);
        let chord = Chord { mods: Mods::CTRL | Mods::SHIFT, key: Key::KeyT };
        assert_eq!(feed(&mut c, Key::ShiftRight, true), Some(chord));
    }

    #[test]
    fn a_lone_modifier_is_its_own_chord() {
        let mut c = Capture::new();
        feed(&mut c, Key::AltRight, false);
        feed(&mut c, Key::ShiftLeft, false);
        feed(&mut c, Key::ShiftLeft, true);
        assert_eq!(feed(&mut c, Key::AltRight, true), Some(Chord::key(Key::AltRight)));
    }

    #[test]
    fn keys_held_before_the_capture_dont_count() {
        let mut c = Capture::new();
        assert_eq!(feed(&mut c, Key::KeyA, true), None);
        feed(&mut c, Key::F5, false);
        assert_eq!(feed(&mut c, Key::F5, true), Some(Chord::key(Key::F5)));
        // And it starts afresh.
        feed(&mut c, Key::Escape, false);
        assert_eq!(feed(&mut c, Key::Escape, true), Some(Chord::key(Key::Escape)));
    }
}
