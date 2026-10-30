//! What Remapr's rules and [`decide`](remapr_core::decide)'s keys become on macOS, as plain logic
//! tested on every system.
//!
//! Two layers share the rules. macOS's own key mapping (`hidutil`'s `UserKeyMapping`, Apple's
//! Technical Note TN2450) swaps keys below every app: it needs no permission, keeps working while
//! Secure Input is on, and is the only way Caps Lock can be remapped, since its lock toggles before
//! any event tap sees it. It takes every global key-to-key rule whose key no other rule uses. The
//! event tap does the rest: shortcuts, per-app rules, and keys other rules share.
//!
//! macOS apps read modifiers from each event's flags, not from earlier key events, so the tap
//! keeps [`Injector`]: the modifier keys apps see held, after Remapr's changes. It sets the flags
//! of the events Remapr posts and of the keys it lets through.

use remapr_core::{Edge, Key, Rule};

use super::keys::{self, MODIFIER_FLAGS, MODIFIERS};

/// The rules macOS's key mapping takes, as (from, to) pairs, and the rules left for the tap.
pub fn split(rules: &[Rule]) -> (Vec<(Key, Key)>, Vec<Rule>) {
    let shared = |r: &Rule| rules.iter().filter(|o| o.from.key == r.from.key).count() > 1;
    let (hid, tap): (Vec<&Rule>, Vec<&Rule>) =
        rules.iter().partition(|r| r.is_plain() && r.app.is_none() && r.from != r.to && !shared(r));
    (hid.iter().map(|r| (r.from.key, r.to.key)).collect(), tap.into_iter().cloned().collect())
}

/// `pairs` as `hidutil property --set`'s argument. An empty list clears the mapping.
pub fn user_key_mapping(pairs: &[(Key, Key)]) -> String {
    let usage = |key: Key| {
        let (page, usage) = key.hid();
        u64::from(page) << 32 | u64::from(usage)
    };
    let entries: Vec<String> = pairs
        .iter()
        .map(|&(from, to)| {
            format!(
                r#"{{"HIDKeyboardModifierMappingSrc":{},"HIDKeyboardModifierMappingDst":{}}}"#,
                usage(from),
                usage(to)
            )
        })
        .collect();
    format!(r#"{{"UserKeyMapping":[{}]}}"#, entries.join(","))
}

/// How a key event the tap posts moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Down,
    Up,
    /// A modifier key going down or up: macOS reports those as changes of the flags.
    FlagsChanged,
}

/// One key event for `CGEventPost`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    pub keycode: u16,
    pub kind: Kind,
    pub flags: u64,
}

/// Room for what one event sends ([`remapr_core::Out`] holds 16, and Caps Lock sends two events'
/// worth).
pub const MAX: usize = 32;

/// A fixed-size list, so the tap's callback builds it without allocating.
pub struct Strokes {
    items: [Stroke; MAX],
    len: usize,
}

impl Strokes {
    pub const fn new() -> Self {
        Self { items: [Stroke { keycode: 0, kind: Kind::Up, flags: 0 }; MAX], len: 0 }
    }

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

/// The modifier keys apps see held: what reached them from the keyboard, with Remapr's changes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Injector {
    /// A bit per [`MODIFIERS`] entry.
    held: u8,
}

impl Injector {
    pub const fn new() -> Self {
        Self { held: 0 }
    }

    /// A modifier key went down or up as apps see it.
    pub fn observe(&mut self, key: Key, down: bool) {
        if let Some(i) = keys::modifier_index(key) {
            if down { self.held |= 1 << i } else { self.held &= !(1 << i) }
        }
    }

    /// The modifier flags of the held keys.
    pub fn flags(&self) -> u64 {
        (0..MODIFIERS.len()).filter(|i| self.held & (1 << i) != 0).fold(0, |f, i| f | MODIFIERS[i].1 | MODIFIERS[i].2)
    }

    /// `flags` of an event Remapr lets through, with the modifiers apps see held instead of the
    /// keyboard's.
    pub fn pass(&self, flags: u64) -> u64 {
        flags & !MODIFIER_FLAGS | self.flags()
    }

    /// `sends` as events to post, after `out`'s earlier strokes. `locks` are the flags that come
    /// from the keyboard's state rather than from keys held (Caps Lock).
    pub fn strokes(&mut self, out: &mut Strokes, sends: &[(Key, Edge)], locks: u64) {
        for &(key, edge) in sends {
            let Some(keycode) = keys::keycode_of(key) else { continue };
            let down = edge == Edge::Press;
            if keys::modifier_index(key).is_some() {
                self.observe(key, down);
                out.push(Stroke { keycode, kind: Kind::FlagsChanged, flags: self.flags() | locks });
            } else {
                let kind = if down { Kind::Down } else { Kind::Up };
                out.push(Stroke { keycode, kind, flags: self.flags() | locks | keys::extra_flags(key) });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use remapr_core::{AppMatcher, Chord, Mods};

    use super::*;

    fn rule(from: Chord, to: Chord, app: Option<&str>) -> Rule {
        Rule { from, to, app: app.map(|a| AppMatcher::BundleId(a.into())) }
    }

    #[test]
    fn global_key_swaps_go_to_macos_and_the_rest_to_the_tap() {
        let caps_to_esc = rule(Chord::key(Key::CapsLock), Chord::key(Key::Escape), None);
        let swap_cmd = rule(Chord::key(Key::MetaLeft), Chord::key(Key::ControlLeft), None);
        let f1_in_app = rule(Chord::key(Key::F1), Chord::key(Key::F2), Some("com.apple.Safari"));
        let f1_everywhere = rule(Chord::key(Key::F1), Chord::key(Key::F3), None);
        let shortcut = rule(Chord { mods: Mods::META, key: Key::KeyQ }, Chord::key(Key::Escape), None);
        let rules = [caps_to_esc, swap_cmd, f1_in_app.clone(), f1_everywhere.clone(), shortcut.clone()];
        let (hid, tap) = split(&rules);
        assert_eq!(hid, [(Key::CapsLock, Key::Escape), (Key::MetaLeft, Key::ControlLeft)]);
        // F1 has two rules: the tap needs both to choose by app.
        assert_eq!(tap, [f1_in_app, f1_everywhere, shortcut]);
    }

    #[test]
    fn the_mapping_names_hid_usages() {
        assert_eq!(user_key_mapping(&[]), r#"{"UserKeyMapping":[]}"#);
        let caps_to_esc = r#"{"UserKeyMapping":[{"HIDKeyboardModifierMappingSrc":30064771129,"HIDKeyboardModifierMappingDst":30064771113}]}"#;
        assert_eq!(user_key_mapping(&[(Key::CapsLock, Key::Escape)]), caps_to_esc);
        assert!(user_key_mapping(&[(Key::F5, Key::MediaPlayPause)]).contains(&(0xC_0000_00CDu64).to_string()));
    }

    #[test]
    fn injected_keys_carry_the_modifiers_apps_see() {
        let mut injector = Injector::default();
        // The user holds Shift, which reached apps.
        injector.observe(Key::ShiftLeft, true);
        let mut out = Strokes::new();
        let sends = [(Key::ShiftLeft, Edge::Lift), (Key::ControlLeft, Edge::Press), (Key::KeyT, Edge::Press)];
        injector.strokes(&mut out, &sends, keys::ALPHA_SHIFT);
        let [shift_up, ctrl_down, t_down] = out.as_slice() else { panic!("{:?}", out.as_slice()) };
        assert_eq!((shift_up.keycode, shift_up.kind, shift_up.flags), (0x38, Kind::FlagsChanged, keys::ALPHA_SHIFT));
        assert_eq!((ctrl_down.kind, ctrl_down.flags), (Kind::FlagsChanged, 0x0004_0001 | keys::ALPHA_SHIFT));
        assert_eq!((t_down.keycode, t_down.kind, t_down.flags), (0x11, Kind::Down, 0x0004_0001 | keys::ALPHA_SHIFT));
        // A key let through afterwards shows Ctrl held, and Shift not, whatever the keyboard said.
        assert_eq!(injector.pass(0x0002_0002 | 0x100), 0x0004_0001 | 0x100);
    }

    #[test]
    fn arrows_and_function_keys_carry_their_own_flags() {
        let mut out = Strokes::new();
        Injector::default().strokes(&mut out, &[(Key::ArrowLeft, Edge::Press), (Key::F5, Edge::Release)], 0);
        assert_eq!(out.as_slice()[0].flags, 0x00A0_0000);
        assert_eq!(out.as_slice()[1].flags, 0x0080_0000);
        assert_eq!(out.as_slice()[1].kind, Kind::Up);
    }
}
