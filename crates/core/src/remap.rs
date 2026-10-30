//! What to do with one key event: plain key remaps, shortcut remaps, and the modifier bookkeeping
//! both need. Each system's backend calls [`decide`] for every key on the system, so it never
//! allocates; what that system needs on top (Windows' AltGr and menu quirks) stays in its backend.

use crate::{App, Key, Mods, Rule};

/// The eight modifier keys, left and right; a `u8` bit mask over this list stands for "which are down".
const MODIFIER_KEYS: [Key; 8] = [
    Key::ControlLeft,
    Key::ControlRight,
    Key::ShiftLeft,
    Key::ShiftRight,
    Key::AltLeft,
    Key::AltRight,
    Key::MetaLeft,
    Key::MetaRight,
];

/// One key going down or up, as the backend saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub up: bool,
    /// Synthetic input, ours or another program's: never remapped.
    pub injected: bool,
}

/// How a key the backend injects moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Press,
    Release,
    /// A release of a key the user is still holding down, which a shortcut lets go of for its
    /// target. Systems that act on a lone tap of a modifier need to tell it apart (Windows opens a
    /// menu when Alt goes up with nothing in between).
    Lift,
}

/// This key's bit in a modifier mask, or 0 for any other key.
fn modifier_bit(key: Key) -> u8 {
    MODIFIER_KEYS.iter().position(|&m| m == key).map_or(0, |i| 1 << i)
}

fn generic(mask: u8) -> Mods {
    (0..8).filter(|i| mask & (1 << i) != 0).fold(Mods::NONE, |acc, i| acc | MODIFIER_KEYS[i].modifier())
}

/// The keys of `mask` that count as one of `mods`.
fn keys_of(mask: u8, mods: Mods) -> u8 {
    (0..8)
        .filter(|&i| mask & (1 << i) != 0 && mods.contains(MODIFIER_KEYS[i].modifier()))
        .fold(0, |acc, i| acc | (1 << i))
}

/// The left-hand key of each of `mods`, as a mask: what a shortcut presses for modifiers not held.
fn left_keys(mods: Mods) -> u8 {
    [(Mods::CTRL, 0), (Mods::SHIFT, 2), (Mods::ALT, 4), (Mods::META, 6)]
        .into_iter()
        .filter(|&(m, _)| mods.contains(m))
        .fold(0, |acc, (_, i)| acc | (1 << i))
}

fn keys(mask: u8) -> impl Iterator<Item = Key> {
    (0..8).filter(move |i| mask & (1 << i) != 0).map(|i| MODIFIER_KEYS[i])
}

/// Keys to inject, in order, and whether the original event is swallowed.
/// Fixed capacity, so the hook never allocates.
#[derive(Debug)]
pub struct Out {
    sends: [(Key, Edge); 16],
    len: usize,
    pub swallow: bool,
}

impl Default for Out {
    fn default() -> Self {
        Self { sends: [(Key::KeyA, Edge::Press); 16], len: 0, swallow: false }
    }
}

impl Out {
    /// Swallow the event, inject nothing.
    pub fn swallow() -> Self {
        Self { swallow: true, ..Self::default() }
    }

    fn send(&mut self, key: Key, edge: Edge) {
        if self.len < self.sends.len() {
            self.sends[self.len] = (key, edge);
            self.len += 1;
        }
    }

    fn swallowed(mut self) -> Self {
        self.swallow = true;
        self
    }

    pub fn sends(&self) -> &[(Key, Edge)] {
        &self.sends[..self.len]
    }
}

/// A shortcut whose source key is down: what to undo when it comes back up.
#[derive(Clone, Copy, Debug)]
struct Active {
    source: Key,
    target: Key,
    /// Modifier keys held by the user that the shortcut released (mask over `MODIFIER_KEYS`).
    lifted: u8,
    /// Modifier keys the shortcut pressed that the user isn't holding.
    pressed: u8,
}

/// What the hook remembers between events.
pub struct State {
    /// What each plain-remapped source key sent on its way down, so its release always lifts that
    /// same key even if the mapping changed or the engine was paused in between.
    held: [Option<Key>; 256],
    /// Modifier keys down as the user pressed them, after plain remaps (mask over `MODIFIER_KEYS`).
    mods: u8,
    active: [Option<Active>; 4],
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub const fn new() -> Self {
        Self { held: [None; 256], mods: 0, active: [None; 4] }
    }

    fn held(&mut self, key: Key) -> &mut Option<Key> {
        &mut self.held[usize::from(key.index())]
    }

    /// Whether `key` is down as the source of a remap or shortcut, so its release must be handled.
    pub fn is_replacing(&self, key: Key) -> bool {
        self.held[usize::from(key.index())].is_some() || self.active.iter().flatten().any(|a| a.source == key)
    }

    fn active(&mut self, source: Key) -> Option<&mut Option<Active>> {
        self.active.iter_mut().find(|a| a.is_some_and(|a| a.source == source))
    }

    /// Keys this state is holding down that the user isn't: released when the engine stops.
    pub fn stuck(&self) -> impl Iterator<Item = Key> + '_ {
        let pressed = self.active.iter().flatten().flat_map(|a| std::iter::once(a.target).chain(keys(a.pressed)));
        self.held.iter().flatten().copied().chain(pressed)
    }
}

/// The rule for `key` with modifiers `mods` held, in `app`. `rules` is in [`sort_rules`]' order.
///
/// [`sort_rules`]: crate::sort_rules
fn find<'a>(rules: &'a [Rule], app: App<'_>, key: Key, mods: Mods) -> Option<&'a Rule> {
    rules.iter().find(|r| {
        r.from.key == key && r.app.as_ref().is_none_or(|a| a.matches(app)) && (r.is_plain() || r.from.mods == mods)
    })
}

/// Whether `key` alone is plainly remapped in `app`, so its own events are replaced.
pub fn remaps_plainly(rules: &[Rule], app: App<'_>, key: Key) -> bool {
    find(rules, app, key, Mods::NONE).is_some_and(Rule::is_plain)
}

/// Decides what to do with one key event. `rules` is in [`sort_rules`]' order.
///
/// [`sort_rules`]: crate::sort_rules
pub fn decide(ev: KeyEvent, enabled: bool, rules: &[Rule], app: App<'_>, s: &mut State) -> Out {
    let out = Out::default();
    // Ours or another program's synthetic input: never remap it.
    if ev.injected {
        return out;
    }
    if ev.up { release(ev.key, s, out) } else { press(ev.key, enabled, rules, app, s, out) }
}

fn release(key: Key, s: &mut State, mut out: Out) -> Out {
    let bit = modifier_bit(key);
    // A plain remap's release lifts what its press sent.
    if let Some(target) = s.held(key).take() {
        s.mods &= !modifier_bit(target);
        out.send(target, Edge::Release);
        return out.swallowed();
    }
    // A shortcut's key: lift its target and put the modifiers back as the user holds them.
    if let Some(slot) = s.active(key) {
        let a = slot.take().expect("found by source");
        out.send(a.target, Edge::Release);
        keys(a.pressed).for_each(|m| out.send(m, Edge::Release));
        keys(a.lifted & s.mods).for_each(|m| out.send(m, Edge::Press));
        return out.swallowed();
    }
    s.mods &= !bit;
    // A modifier a shortcut already released: apps saw it go up then.
    if let Some(a) = s.active.iter_mut().flatten().find(|a| a.lifted & bit != 0) {
        a.lifted &= !bit;
        return out.swallowed();
    }
    out
}

fn press(key: Key, enabled: bool, rules: &[Rule], app: App<'_>, s: &mut State, mut out: Out) -> Out {
    // Key repeat of a key we are replacing.
    if let Some(target) = *s.held(key) {
        out.send(target, Edge::Press);
        return out.swallowed();
    }
    if let Some(a) = s.active(key).and_then(|a| *a) {
        out.send(a.target, Edge::Press);
        return out.swallowed();
    }
    let rule = enabled.then(|| find(rules, app, key, generic(s.mods))).flatten();
    let Some(rule) = rule.filter(|r| r.to != r.from) else {
        s.mods |= modifier_bit(key);
        return out;
    };
    if rule.is_plain() {
        *s.held(key) = Some(rule.to.key);
        s.mods |= modifier_bit(rule.to.key);
        out.send(rule.to.key, Edge::Press);
        return out.swallowed();
    }
    let Some(slot) = s.active.iter_mut().find(|a| a.is_none()) else {
        // Four shortcuts held at once: let this key through rather than lose track of one.
        return out;
    };
    let lifted = keys_of(s.mods, rule.from.mods.without(rule.to.mods));
    let pressed = left_keys(rule.to.mods.without(generic(s.mods)));
    keys(lifted).for_each(|m| out.send(m, Edge::Lift));
    keys(pressed).for_each(|m| out.send(m, Edge::Press));
    out.send(rule.to.key, Edge::Press);
    *slot = Some(Active { source: key, target: rule.to.key, lifted, pressed });
    out.swallowed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppMatcher, Chord, sort_rules};

    use Edge::{Lift, Press, Release};
    use Key::{AltLeft, CapsLock, ControlLeft, Escape, F1, F5, KeyA, KeyT, MetaLeft, ShiftLeft, ShiftRight};

    fn ev(key: Key, up: bool) -> KeyEvent {
        KeyEvent { key, up, injected: false }
    }

    fn rule(from: (Mods, Key), to: (Mods, Key), app: Option<&str>) -> Rule {
        Rule {
            from: Chord { mods: from.0, key: from.1 },
            to: Chord { mods: to.0, key: to.1 },
            app: app.map(|a| AppMatcher::Exe(a.into())),
        }
    }

    fn run(s: &mut State, rules: &[Rule], key: Key, up: bool) -> (bool, Vec<(Key, Edge)>) {
        let out = decide(ev(key, up), true, rules, App::Exe("notepad.exe"), s);
        (out.swallow, out.sends().to_vec())
    }

    #[test]
    fn plain_remap_presses_and_releases() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CapsLock), (Mods::NONE, Escape), None)]);
        assert_eq!(run(&mut s, &rules, CapsLock, false), (true, vec![(Escape, Press)]));
        assert_eq!(run(&mut s, &rules, CapsLock, true), (true, vec![(Escape, Release)]));
    }

    #[test]
    fn plain_remap_lets_held_modifiers_through() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CapsLock), (Mods::NONE, Escape), None)]);
        assert_eq!(run(&mut s, &rules, ShiftLeft, false), (false, vec![]));
        assert_eq!(run(&mut s, &rules, CapsLock, false), (true, vec![(Escape, Press)]));
    }

    #[test]
    fn unmapped_and_injected_keys_pass() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CapsLock), (Mods::NONE, Escape), None)]);
        assert_eq!(run(&mut s, &rules, KeyA, false), (false, vec![]));
        let injected = KeyEvent { injected: true, ..ev(CapsLock, false) };
        assert!(!decide(injected, true, &rules, App::Unknown, &mut s).swallow);
    }

    #[test]
    fn release_lifts_what_was_pressed_even_after_pause_or_remap() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CapsLock), (Mods::NONE, Escape), None)]);
        run(&mut s, &rules, CapsLock, false);
        // Paused, and the mapping is gone: the release still lifts Esc.
        let out = decide(ev(CapsLock, true), false, &[], App::Unknown, &mut s);
        assert_eq!((out.swallow, out.sends()), (true, &[(Escape, Release)][..]));
        assert!(!decide(ev(CapsLock, false), false, &rules, App::Unknown, &mut s).swallow);
    }

    #[test]
    fn release_without_remapped_press_passes() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CapsLock), (Mods::NONE, Escape), None)]);
        assert_eq!(run(&mut s, &rules, CapsLock, true), (false, vec![]));
    }

    #[test]
    fn key_to_shortcut_presses_and_releases_the_modifiers() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, F1), (Mods::CTRL | Mods::SHIFT, KeyT), None)]);
        let down = vec![(ControlLeft, Press), (ShiftLeft, Press), (KeyT, Press)];
        assert_eq!(run(&mut s, &rules, F1, false), (true, down));
        assert_eq!(run(&mut s, &rules, F1, false), (true, vec![(KeyT, Press)]), "repeat");
        let up = vec![(KeyT, Release), (ControlLeft, Release), (ShiftLeft, Release)];
        assert_eq!(run(&mut s, &rules, F1, true), (true, up));
    }

    #[test]
    fn shortcut_to_shortcut_changes_only_the_modifiers_that_differ() {
        let rules = [rule((Mods::CTRL | Mods::SHIFT, KeyT), (Mods::CTRL, F5), None)];
        let mut s = State::new();
        run(&mut s, &rules, ControlLeft, false);
        run(&mut s, &rules, ShiftLeft, false);
        // Shift goes up for the target, as a lift: the user still holds it. Ctrl stays.
        assert_eq!(run(&mut s, &rules, KeyT, false), (true, vec![(ShiftLeft, Lift), (F5, Press)]));
        // Releasing T puts Shift back, since the user still holds it.
        assert_eq!(run(&mut s, &rules, KeyT, true), (true, vec![(F5, Release), (ShiftLeft, Press)]));
    }

    #[test]
    fn modifier_released_first_is_not_pressed_again() {
        let rules = [rule((Mods::CTRL, KeyT), (Mods::NONE, F5), None)];
        let mut s = State::new();
        run(&mut s, &rules, ControlLeft, false);
        assert_eq!(run(&mut s, &rules, KeyT, false), (true, vec![(ControlLeft, Lift), (F5, Press)]));
        // Apps already saw Ctrl go up: the physical release is swallowed.
        assert_eq!(run(&mut s, &rules, ControlLeft, true), (true, vec![]));
        assert_eq!(run(&mut s, &rules, KeyT, true), (true, vec![(F5, Release)]));
    }

    #[test]
    fn lifting_alt_or_meta_is_marked_as_a_lift() {
        let rules = [rule((Mods::ALT | Mods::META, KeyT), (Mods::NONE, F5), None)];
        let mut s = State::new();
        run(&mut s, &rules, AltLeft, false);
        run(&mut s, &rules, MetaLeft, false);
        let (_, sends) = run(&mut s, &rules, KeyT, false);
        assert_eq!(sends, vec![(AltLeft, Lift), (MetaLeft, Lift), (F5, Press)]);
    }

    #[test]
    fn shortcuts_need_exactly_their_modifiers() {
        let rules = [rule((Mods::CTRL, KeyT), (Mods::NONE, F5), None)];
        let mut s = State::new();
        assert_eq!(run(&mut s, &rules, KeyT, false), (false, vec![]), "no Ctrl");
        run(&mut s, &rules, KeyT, true);
        run(&mut s, &rules, ControlLeft, false);
        run(&mut s, &rules, ShiftRight, false);
        assert_eq!(run(&mut s, &rules, KeyT, false), (false, vec![]), "Ctrl+Shift is another shortcut");
    }

    #[test]
    fn app_rules_apply_only_in_their_app_and_win_over_global_ones() {
        let mut rules = vec![
            rule((Mods::NONE, CapsLock), (Mods::NONE, Escape), None),
            rule((Mods::NONE, CapsLock), (Mods::NONE, F5), Some("chrome.exe")),
        ];
        sort_rules(&mut rules);
        let mut s = State::new();
        let out = decide(ev(CapsLock, false), true, &rules, App::Exe("Chrome.exe"), &mut s);
        assert_eq!(out.sends(), &[(F5, Press)]);
        decide(ev(CapsLock, true), true, &rules, App::Exe("chrome.exe"), &mut s);
        let out = decide(ev(CapsLock, false), true, &rules, App::Exe("notepad.exe"), &mut s);
        assert_eq!(out.sends(), &[(Escape, Press)]);
    }

    #[test]
    fn remapped_modifier_counts_for_shortcuts() {
        let rules = [
            rule((Mods::CTRL, KeyT), (Mods::NONE, F5), None),
            rule((Mods::NONE, CapsLock), (Mods::NONE, ControlLeft), None),
        ];
        let mut s = State::new();
        assert_eq!(run(&mut s, &rules, CapsLock, false), (true, vec![(ControlLeft, Press)]));
        assert_eq!(run(&mut s, &rules, KeyT, false), (true, vec![(ControlLeft, Lift), (F5, Press)]));
    }

    #[test]
    fn stopping_releases_what_is_held() {
        let rules = [
            rule((Mods::NONE, CapsLock), (Mods::NONE, Escape), None),
            rule((Mods::NONE, F1), (Mods::CTRL, KeyT), None),
        ];
        let mut s = State::new();
        run(&mut s, &rules, CapsLock, false);
        run(&mut s, &rules, F1, false);
        let mut stuck: Vec<Key> = s.stuck().collect();
        stuck.sort();
        assert_eq!(stuck, [KeyT, Escape, ControlLeft]);
    }

    #[test]
    fn plain_remaps_are_known_by_key() {
        let rules = [rule((Mods::NONE, Key::AltRight), (Mods::NONE, Escape), None)];
        assert!(remaps_plainly(&rules, App::Unknown, Key::AltRight));
        assert!(!remaps_plainly(&rules, App::Unknown, AltLeft));
    }
}
