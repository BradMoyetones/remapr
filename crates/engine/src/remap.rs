//! What to do with one key event: plain key remaps, shortcut remaps, and the modifier bookkeeping
//! both need. Pure logic, called from the hook for every key on the system, so it never allocates.

use crate::{Mods, Rule, Vk};

const VK_LCONTROL: Vk = 0xA2;
const VK_RMENU: Vk = 0xA5;
/// On AltGr layouts (Spanish, German…) Windows sends a fake Left Ctrl with this scan code before
/// Right Alt. It must never be treated as a real Left Ctrl, or remapping Ctrl breaks `@`, `#`, `€`.
const ALTGR_FAKE_CTRL_SCAN: u32 = 0x21D;
/// Tapped before a synthetic Alt or Win release, so Windows doesn't take the release for a tap of
/// that key and open the window menu or the Start menu. 0xFF is not assigned to any key.
pub(crate) const DUMMY: Vk = 0xFF;

/// The eight modifier keys, left and right; a `u8` bit mask over this list stands for "which are down".
const MODIFIER_KEYS: [Vk; 8] = [0xA2, 0xA3, 0xA0, 0xA1, 0xA4, 0xA5, 0x5B, 0x5C];
/// The generic modifier each of `MODIFIER_KEYS` counts as.
const MODIFIER_OF: [Mods; 8] =
    [Mods::CTRL, Mods::CTRL, Mods::SHIFT, Mods::SHIFT, Mods::ALT, Mods::ALT, Mods::WIN, Mods::WIN];
const ALT_OR_WIN_KEYS: u8 = 0b1111_0000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Event {
    pub vk: Vk,
    pub scan: u32,
    pub up: bool,
    pub injected: bool,
}

pub(crate) fn is_altgr_phantom(ev: Event) -> bool {
    ev.vk == VK_LCONTROL && ev.scan == ALTGR_FAKE_CTRL_SCAN
}

/// This key's bit in a modifier mask, or 0 for any other key.
pub(crate) fn modifier_bit(vk: Vk) -> u8 {
    MODIFIER_KEYS.iter().position(|&m| m == vk).map_or(0, |i| 1 << i)
}

/// The modifier `vk` is (Left Ctrl is `Mods::CTRL`), or none.
pub(crate) fn modifier_mods(vk: Vk) -> Mods {
    generic(modifier_bit(vk))
}

fn generic(mask: u8) -> Mods {
    (0..8).filter(|i| mask & (1 << i) != 0).fold(Mods::NONE, |acc, i| acc | MODIFIER_OF[i])
}

/// The keys of `mask` that count as one of `mods`.
fn keys_of(mask: u8, mods: Mods) -> u8 {
    (0..8).filter(|&i| mask & (1 << i) != 0 && mods.contains(MODIFIER_OF[i])).fold(0, |acc, i| acc | (1 << i))
}

/// The left-hand key of each of `mods`, as a mask: what a shortcut presses for modifiers not held.
fn left_keys(mods: Mods) -> u8 {
    [(Mods::CTRL, 0), (Mods::SHIFT, 2), (Mods::ALT, 4), (Mods::WIN, 6)]
        .into_iter()
        .filter(|&(m, _)| mods.contains(m))
        .fold(0, |acc, (_, i)| acc | (1 << i))
}

fn keys(mask: u8) -> impl Iterator<Item = Vk> {
    (0..8).filter(move |i| mask & (1 << i) != 0).map(|i| MODIFIER_KEYS[i])
}

/// Key events to inject, in order, and whether the original event is swallowed.
/// Fixed capacity, so the hook never allocates.
#[derive(Debug, Default)]
pub(crate) struct Out {
    sends: [(Vk, bool); 16],
    len: usize,
    pub swallow: bool,
}

impl Out {
    /// Swallow the event, inject nothing.
    pub fn swallow() -> Self {
        Self { swallow: true, ..Self::default() }
    }

    fn send(&mut self, vk: Vk, up: bool) {
        if self.len < self.sends.len() {
            self.sends[self.len] = (vk, up);
            self.len += 1;
        }
    }

    fn swallowed(mut self) -> Self {
        self.swallow = true;
        self
    }

    /// (key, is release) pairs.
    pub fn sends(&self) -> &[(Vk, bool)] {
        &self.sends[..self.len]
    }
}

/// A shortcut whose source key is down: what to undo when it comes back up.
#[derive(Clone, Copy, Debug)]
struct Active {
    source: Vk,
    target: Vk,
    /// Modifier keys held by the user that the shortcut released (mask over `MODIFIER_KEYS`).
    lifted: u8,
    /// Modifier keys the shortcut pressed that the user isn't holding.
    pressed: u8,
}

/// What the hook remembers between events.
pub(crate) struct State {
    /// What each plain-remapped source key sent on its way down, so its release always lifts that
    /// same key even if the mapping changed or the engine was paused in between.
    held: [Vk; 256],
    /// Modifier keys down as the user pressed them, after plain remaps (mask over `MODIFIER_KEYS`).
    mods: u8,
    active: [Option<Active>; 4],
}

impl State {
    pub const fn new() -> Self {
        Self { held: [0; 256], mods: 0, active: [None; 4] }
    }

    /// Whether `vk` is down as the source of a remap or shortcut, so its release must be handled.
    pub fn is_replacing(&self, vk: Vk) -> bool {
        self.held[usize::from(vk)] != 0 || self.active.iter().flatten().any(|a| a.source == vk)
    }

    fn active(&mut self, source: Vk) -> Option<&mut Option<Active>> {
        self.active.iter_mut().find(|a| a.is_some_and(|a| a.source == source))
    }

    /// Keys this state is holding down that the user isn't: released when the engine stops.
    pub fn stuck(&self) -> impl Iterator<Item = Vk> + '_ {
        let pressed = self.active.iter().flatten().flat_map(|a| std::iter::once(a.target).chain(keys(a.pressed)));
        self.held.iter().copied().filter(|&vk| vk != 0).chain(pressed)
    }
}

/// The rule for `vk` with modifiers `mods` held, in `app`. `rules` is sorted as `Engine::set_rules`
/// leaves it: rules for one app before global ones, shortcuts before plain remaps.
fn find<'a>(rules: &'a [Rule], app: &str, vk: Vk, mods: Mods) -> Option<&'a Rule> {
    rules.iter().find(|r| {
        r.from.key == vk
            && r.app.as_deref().is_none_or(|a| a.eq_ignore_ascii_case(app))
            && (r.is_plain() || r.from.mods == mods)
    })
}

/// Decides what to do with one key event. `app` is the executable in front, lowercase.
pub(crate) fn decide(ev: Event, enabled: bool, rules: &[Rule], app: &str, s: &mut State) -> Out {
    let out = Out::default();
    // Ours or another program's synthetic input: never remap it.
    if ev.injected {
        return out;
    }
    if is_altgr_phantom(ev) {
        // Not a real Ctrl: never counted or remapped as one. A remapped AltGr would otherwise
        // arrive with a phantom Ctrl held.
        let altgr_remapped = enabled && find(rules, app, VK_RMENU, Mods::NONE).is_some_and(Rule::is_plain);
        return if altgr_remapped { out.swallowed() } else { out };
    }
    if ev.up { release(ev.vk, s, out) } else { press(ev.vk, enabled, rules, app, s, out) }
}

fn release(vk: Vk, s: &mut State, mut out: Out) -> Out {
    let bit = modifier_bit(vk);
    // A plain remap's release lifts what its press sent.
    let target = std::mem::take(&mut s.held[usize::from(vk)]);
    if target != 0 {
        s.mods &= !modifier_bit(target);
        out.send(target, true);
        return out.swallowed();
    }
    // A shortcut's key: lift its target and put the modifiers back as the user holds them.
    if let Some(slot) = s.active(vk) {
        let a = slot.take().expect("found by source");
        out.send(a.target, true);
        keys(a.pressed).for_each(|m| out.send(m, true));
        keys(a.lifted & s.mods).for_each(|m| out.send(m, false));
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

fn press(vk: Vk, enabled: bool, rules: &[Rule], app: &str, s: &mut State, mut out: Out) -> Out {
    // Key repeat of a key we are replacing.
    let target = s.held[usize::from(vk)];
    if target != 0 {
        out.send(target, false);
        return out.swallowed();
    }
    if let Some(a) = s.active(vk).and_then(|a| *a) {
        out.send(a.target, false);
        return out.swallowed();
    }
    let rule = enabled.then(|| find(rules, app, vk, generic(s.mods))).flatten();
    let Some(rule) = rule.filter(|r| r.to != r.from) else {
        s.mods |= modifier_bit(vk);
        return out;
    };
    if rule.is_plain() {
        s.held[usize::from(vk)] = rule.to.key;
        s.mods |= modifier_bit(rule.to.key);
        out.send(rule.to.key, false);
        return out.swallowed();
    }
    let Some(slot) = s.active.iter_mut().find(|a| a.is_none()) else {
        // Four shortcuts held at once: let this key through rather than lose track of one.
        return out;
    };
    let lifted = keys_of(s.mods, rule.from.mods.without(rule.to.mods));
    let pressed = left_keys(rule.to.mods.without(generic(s.mods)));
    if lifted & ALT_OR_WIN_KEYS != 0 {
        out.send(DUMMY, false);
        out.send(DUMMY, true);
    }
    keys(lifted).for_each(|m| out.send(m, true));
    keys(pressed).for_each(|m| out.send(m, false));
    out.send(rule.to.key, false);
    *slot = Some(Active { source: vk, target: rule.to.key, lifted, pressed });
    out.swallowed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Chord;

    const CAPS: Vk = 0x14;
    const ESC: Vk = 0x1B;
    const LCTRL: Vk = 0xA2;
    const LSHIFT: Vk = 0xA0;
    const LALT: Vk = 0xA4;
    const T: Vk = b'T';
    const F1: Vk = 0x70;
    const F5: Vk = 0x74;

    fn ev(vk: Vk, up: bool) -> Event {
        Event { vk, scan: 0, up, injected: false }
    }

    fn rule(from: (Mods, Vk), to: (Mods, Vk), app: Option<&str>) -> Rule {
        Rule {
            from: Chord { mods: from.0, key: from.1 },
            to: Chord { mods: to.0, key: to.1 },
            app: app.map(Into::into),
        }
    }

    fn run(s: &mut State, rules: &[Rule], vk: Vk, up: bool) -> (bool, Vec<(Vk, bool)>) {
        let out = decide(ev(vk, up), true, rules, "notepad.exe", s);
        (out.swallow, out.sends().to_vec())
    }

    #[test]
    fn plain_remap_presses_and_releases() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CAPS), (Mods::NONE, ESC), None)]);
        assert_eq!(run(&mut s, &rules, CAPS, false), (true, vec![(ESC, false)]));
        assert_eq!(run(&mut s, &rules, CAPS, true), (true, vec![(ESC, true)]));
    }

    #[test]
    fn plain_remap_lets_held_modifiers_through() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CAPS), (Mods::NONE, ESC), None)]);
        assert_eq!(run(&mut s, &rules, LSHIFT, false), (false, vec![]));
        assert_eq!(run(&mut s, &rules, CAPS, false), (true, vec![(ESC, false)]));
    }

    #[test]
    fn unmapped_and_injected_keys_pass() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CAPS), (Mods::NONE, ESC), None)]);
        assert_eq!(run(&mut s, &rules, b'A', false), (false, vec![]));
        let injected = Event { injected: true, ..ev(CAPS, false) };
        assert!(!decide(injected, true, &rules, "", &mut s).swallow);
    }

    #[test]
    fn release_lifts_what_was_pressed_even_after_pause_or_remap() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CAPS), (Mods::NONE, ESC), None)]);
        run(&mut s, &rules, CAPS, false);
        // Paused, and the mapping is gone: the release still lifts Esc.
        let out = decide(ev(CAPS, true), false, &[], "", &mut s);
        assert_eq!((out.swallow, out.sends()), (true, &[(ESC, true)][..]));
        assert!(!decide(ev(CAPS, false), false, &rules, "", &mut s).swallow);
    }

    #[test]
    fn release_without_remapped_press_passes() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, CAPS), (Mods::NONE, ESC), None)]);
        assert_eq!(run(&mut s, &rules, CAPS, true), (false, vec![]));
    }

    #[test]
    fn altgr_phantom_ctrl_is_never_remapped_as_ctrl() {
        let mut s = State::new();
        let phantom = Event { scan: ALTGR_FAKE_CTRL_SCAN, ..ev(VK_LCONTROL, false) };
        let ctrl_to_esc = [rule((Mods::NONE, LCTRL), (Mods::NONE, ESC), None)];
        assert!(!decide(phantom, true, &ctrl_to_esc, "", &mut s).swallow);
        let altgr_to_esc = [rule((Mods::NONE, VK_RMENU), (Mods::NONE, ESC), None)];
        assert!(decide(phantom, true, &altgr_to_esc, "", &mut s).swallow);
        // Nor counted as a held Ctrl.
        assert_eq!(s.mods, 0);
    }

    #[test]
    fn key_to_shortcut_presses_and_releases_the_modifiers() {
        let (mut s, rules) = (State::new(), [rule((Mods::NONE, F1), (Mods::CTRL | Mods::SHIFT, T), None)]);
        assert_eq!(run(&mut s, &rules, F1, false), (true, vec![(LCTRL, false), (LSHIFT, false), (T, false)]));
        assert_eq!(run(&mut s, &rules, F1, false), (true, vec![(T, false)]), "repeat");
        assert_eq!(run(&mut s, &rules, F1, true), (true, vec![(T, true), (LCTRL, true), (LSHIFT, true)]));
    }

    #[test]
    fn shortcut_to_shortcut_changes_only_the_modifiers_that_differ() {
        let rules = [rule((Mods::CTRL | Mods::SHIFT, T), (Mods::CTRL, F5), None)];
        let mut s = State::new();
        run(&mut s, &rules, LCTRL, false);
        run(&mut s, &rules, LSHIFT, false);
        // Shift goes up for the target; Ctrl stays.
        assert_eq!(run(&mut s, &rules, T, false), (true, vec![(LSHIFT, true), (F5, false)]));
        // Releasing T puts Shift back, since the user still holds it.
        assert_eq!(run(&mut s, &rules, T, true), (true, vec![(F5, true), (LSHIFT, false)]));
    }

    #[test]
    fn modifier_released_first_is_not_pressed_again() {
        let rules = [rule((Mods::CTRL, T), (Mods::NONE, F5), None)];
        let mut s = State::new();
        run(&mut s, &rules, LCTRL, false);
        assert_eq!(run(&mut s, &rules, T, false), (true, vec![(LCTRL, true), (F5, false)]));
        // Apps already saw Ctrl go up: the physical release is swallowed.
        assert_eq!(run(&mut s, &rules, LCTRL, true), (true, vec![]));
        assert_eq!(run(&mut s, &rules, T, true), (true, vec![(F5, true)]));
    }

    #[test]
    fn lifting_alt_taps_a_dummy_key_first() {
        let rules = [rule((Mods::ALT, T), (Mods::NONE, F5), None)];
        let mut s = State::new();
        run(&mut s, &rules, LALT, false);
        let (_, sends) = run(&mut s, &rules, T, false);
        assert_eq!(sends, vec![(DUMMY, false), (DUMMY, true), (LALT, true), (F5, false)]);
    }

    #[test]
    fn shortcuts_need_exactly_their_modifiers() {
        let rules = [rule((Mods::CTRL, T), (Mods::NONE, F5), None)];
        let mut s = State::new();
        assert_eq!(run(&mut s, &rules, T, false), (false, vec![]), "no Ctrl");
        run(&mut s, &rules, T, true);
        run(&mut s, &rules, LCTRL, false);
        run(&mut s, &rules, LSHIFT, false);
        assert_eq!(run(&mut s, &rules, T, false), (false, vec![]), "Ctrl+Shift is another shortcut");
    }

    #[test]
    fn app_rules_apply_only_in_their_app_and_win_over_global_ones() {
        let rules = [
            rule((Mods::NONE, CAPS), (Mods::NONE, F5), Some("chrome.exe")),
            rule((Mods::NONE, CAPS), (Mods::NONE, ESC), None),
        ];
        let mut s = State::new();
        let out = decide(ev(CAPS, false), true, &rules, "chrome.exe", &mut s);
        assert_eq!(out.sends(), &[(F5, false)]);
        decide(ev(CAPS, true), true, &rules, "chrome.exe", &mut s);
        let out = decide(ev(CAPS, false), true, &rules, "notepad.exe", &mut s);
        assert_eq!(out.sends(), &[(ESC, false)]);
    }

    #[test]
    fn remapped_modifier_counts_for_shortcuts() {
        let rules =
            [rule((Mods::CTRL, T), (Mods::NONE, F5), None), rule((Mods::NONE, CAPS), (Mods::NONE, LCTRL), None)];
        let mut s = State::new();
        assert_eq!(run(&mut s, &rules, CAPS, false), (true, vec![(LCTRL, false)]));
        assert_eq!(run(&mut s, &rules, T, false), (true, vec![(LCTRL, true), (F5, false)]));
    }
}
