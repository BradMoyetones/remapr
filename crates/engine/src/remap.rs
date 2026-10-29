//! What to do with one key event. Pure logic, called from the hook for every key on the system.

use crate::Vk;

const VK_LCONTROL: Vk = 0xA2;
const VK_RMENU: Vk = 0xA5;
/// On AltGr layouts (Spanish, German…) Windows sends a fake Left Ctrl with this scan code before
/// Right Alt. It must never be treated as a real Left Ctrl, or remapping Ctrl breaks `@`, `#`, `€`.
const ALTGR_FAKE_CTRL_SCAN: u32 = 0x21D;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Pass,
    Swallow,
    Send { vk: Vk, up: bool },
}

/// `target(vk)` is the key `vk` maps to, or 0. `held[vk]` remembers what each source key sent on
/// its way down, so its release always lifts that same key even if the mapping changed or the
/// engine was paused in between: a key is never left stuck down.
pub(crate) fn decide(ev: Event, enabled: bool, target: impl Fn(Vk) -> Vk, held: &mut [Vk; 256]) -> Action {
    // Ours (marked) or another program's synthetic input: never remap it.
    if ev.injected {
        return Action::Pass;
    }
    let slot = &mut held[usize::from(ev.vk)];
    if ev.up && *slot != 0 {
        let vk = std::mem::take(slot);
        return Action::Send { vk, up: true };
    }
    if !enabled {
        return Action::Pass;
    }
    if is_altgr_phantom(ev) {
        // Remapped AltGr would otherwise arrive with a phantom Ctrl held.
        return if target(VK_RMENU) != 0 { Action::Swallow } else { Action::Pass };
    }
    match target(ev.vk) {
        0 => Action::Pass,
        t if t == ev.vk => Action::Pass,
        // A release whose press we didn't remap belongs to the original key.
        _ if ev.up => Action::Pass,
        t => {
            *slot = t;
            Action::Send { vk: t, up: false }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPS: Vk = 0x14;
    const ESC: Vk = 0x1B;

    fn ev(vk: Vk, up: bool) -> Event {
        Event { vk, scan: 0, up, injected: false }
    }

    fn caps_to_esc(vk: Vk) -> Vk {
        if vk == CAPS { ESC } else { 0 }
    }

    #[test]
    fn remaps_press_and_release() {
        let mut held = [0; 256];
        assert_eq!(decide(ev(CAPS, false), true, caps_to_esc, &mut held), Action::Send { vk: ESC, up: false });
        assert_eq!(decide(ev(CAPS, true), true, caps_to_esc, &mut held), Action::Send { vk: ESC, up: true });
        assert_eq!(held, [0; 256]);
    }

    #[test]
    fn unmapped_and_injected_keys_pass() {
        let mut held = [0; 256];
        assert_eq!(decide(ev(0x41, false), true, caps_to_esc, &mut held), Action::Pass);
        let injected = Event { injected: true, ..ev(CAPS, false) };
        assert_eq!(decide(injected, true, caps_to_esc, &mut held), Action::Pass);
    }

    #[test]
    fn release_lifts_what_was_pressed_even_after_pause_or_remap() {
        let mut held = [0; 256];
        decide(ev(CAPS, false), true, caps_to_esc, &mut held);
        // Paused, and the mapping is gone: the release still lifts Esc.
        assert_eq!(decide(ev(CAPS, true), false, |_| 0, &mut held), Action::Send { vk: ESC, up: true });
        // Paused with nothing held: everything passes.
        assert_eq!(decide(ev(CAPS, false), false, caps_to_esc, &mut held), Action::Pass);
    }

    #[test]
    fn release_without_remapped_press_passes() {
        let mut held = [0; 256];
        assert_eq!(decide(ev(CAPS, true), true, caps_to_esc, &mut held), Action::Pass);
    }

    #[test]
    fn altgr_phantom_ctrl_is_never_remapped_as_ctrl() {
        let mut held = [0; 256];
        let phantom = Event { scan: ALTGR_FAKE_CTRL_SCAN, ..ev(VK_LCONTROL, false) };
        let ctrl_to_esc = |vk| if vk == VK_LCONTROL { ESC } else { 0 };
        assert_eq!(decide(phantom, true, ctrl_to_esc, &mut held), Action::Pass);
        let altgr_to_esc = |vk| if vk == VK_RMENU { ESC } else { 0 };
        assert_eq!(decide(phantom, true, altgr_to_esc, &mut held), Action::Swallow);
    }
}
