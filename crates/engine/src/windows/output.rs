//! What [`decide`](remapr_core::decide)'s keys become on Windows: the same keys, plus a tap of an
//! unassigned key before lifting an Alt or Win key the user holds. Otherwise Windows takes that
//! release for a lone tap of the key and opens the window menu or the Start menu.

use remapr_core::{Edge, Key};

/// One key event for `SendInput`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stroke {
    Key {
        key: Key,
        up: bool,
    },
    /// The unassigned key, [`VK_DUMMY`](super::keys::VK_DUMMY).
    Dummy {
        up: bool,
    },
}

/// Room for what one event sends ([`remapr_core::Out`] holds 16) plus the dummy tap.
pub const MAX: usize = 18;

/// A fixed-size list, so the hook never allocates.
pub struct Strokes {
    items: [Stroke; MAX],
    len: usize,
}

impl Strokes {
    fn push(&mut self, stroke: Stroke) {
        if self.len < MAX {
            self.items[self.len] = stroke;
            self.len += 1;
        }
    }

    pub fn as_slice(&self) -> &[Stroke] {
        &self.items[..self.len]
    }
}

/// `sends` as Windows key events.
pub fn strokes(sends: impl IntoIterator<Item = (Key, Edge)>) -> Strokes {
    let mut out = Strokes { items: [Stroke::Dummy { up: false }; MAX], len: 0 };
    let mut tapped = false;
    for (key, edge) in sends {
        let alt_or_win = matches!(key, Key::AltLeft | Key::AltRight | Key::MetaLeft | Key::MetaRight);
        if edge == Edge::Lift && alt_or_win && !tapped {
            out.push(Stroke::Dummy { up: false });
            out.push(Stroke::Dummy { up: true });
            tapped = true;
        }
        out.push(Stroke::Key { key, up: edge != Edge::Press });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use Edge::{Lift, Press, Release};

    fn key(key: Key, up: bool) -> Stroke {
        Stroke::Key { key, up }
    }

    #[test]
    fn lifting_alt_or_win_taps_the_dummy_first_once() {
        let sends = [(Key::ControlLeft, Lift), (Key::AltLeft, Lift), (Key::MetaLeft, Lift), (Key::F5, Press)];
        let dummy = |up| Stroke::Dummy { up };
        assert_eq!(
            strokes(sends).as_slice(),
            [
                key(Key::ControlLeft, true),
                dummy(false),
                dummy(true),
                key(Key::AltLeft, true),
                key(Key::MetaLeft, true),
                key(Key::F5, false),
            ]
        );
    }

    #[test]
    fn plain_releases_of_alt_are_left_alone() {
        // Caps Lock remapped to Alt and tapped: a real Alt tap, which may open the menu as usual.
        assert_eq!(strokes([(Key::AltLeft, Release)]).as_slice(), [key(Key::AltLeft, true)]);
        assert_eq!(strokes([(Key::ControlLeft, Lift)]).as_slice(), [key(Key::ControlLeft, true)]);
    }
}
