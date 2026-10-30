//! Windows' key codes and Remapr's [`Key`]s, as plain tables, tested on every system.
//!
//! Keys that type characters (letters, digits, punctuation) are told apart by scan code, which is
//! the key's position; their virtual-key code depends on the layout (`VK_Q` is the A key on
//! AZERTY). Every other key by its virtual-key code, plus the extended flag where one code covers
//! two keys: Enter and Num Enter, and the navigation keys and the numpad's with Num Lock off.

use remapr_core::Key;

pub const VK_RETURN: u8 = 0x0D;
pub const VK_LCONTROL: u8 = 0xA2;
pub const VK_NUMLOCK: u8 = 0x90;
/// On AltGr layouts (Spanish, German…) Windows sends a fake Left Ctrl with this scan code before
/// Right Alt. It must never be treated as a real Left Ctrl, or remapping Ctrl breaks `@`, `#`, `€`.
const ALTGR_FAKE_CTRL_SCAN: u32 = 0x21D;
/// Tapped before a synthetic release of Alt or Win: not assigned to any key.
pub const VK_DUMMY: u8 = 0xFF;

/// Keys that type characters, by their scan code (set 1).
const CHARS: &[(u16, Key)] = &[
    (0x02, Key::Digit1),
    (0x03, Key::Digit2),
    (0x04, Key::Digit3),
    (0x05, Key::Digit4),
    (0x06, Key::Digit5),
    (0x07, Key::Digit6),
    (0x08, Key::Digit7),
    (0x09, Key::Digit8),
    (0x0A, Key::Digit9),
    (0x0B, Key::Digit0),
    (0x0C, Key::Minus),
    (0x0D, Key::Equal),
    (0x10, Key::KeyQ),
    (0x11, Key::KeyW),
    (0x12, Key::KeyE),
    (0x13, Key::KeyR),
    (0x14, Key::KeyT),
    (0x15, Key::KeyY),
    (0x16, Key::KeyU),
    (0x17, Key::KeyI),
    (0x18, Key::KeyO),
    (0x19, Key::KeyP),
    (0x1A, Key::BracketLeft),
    (0x1B, Key::BracketRight),
    (0x1E, Key::KeyA),
    (0x1F, Key::KeyS),
    (0x20, Key::KeyD),
    (0x21, Key::KeyF),
    (0x22, Key::KeyG),
    (0x23, Key::KeyH),
    (0x24, Key::KeyJ),
    (0x25, Key::KeyK),
    (0x26, Key::KeyL),
    (0x27, Key::Semicolon),
    (0x28, Key::Quote),
    (0x29, Key::Backquote),
    (0x2B, Key::Backslash),
    (0x2C, Key::KeyZ),
    (0x2D, Key::KeyX),
    (0x2E, Key::KeyC),
    (0x2F, Key::KeyV),
    (0x30, Key::KeyB),
    (0x31, Key::KeyN),
    (0x32, Key::KeyM),
    (0x33, Key::Comma),
    (0x34, Key::Period),
    (0x35, Key::Slash),
    (0x56, Key::IntlBackslash),
    (0x73, Key::IntlRo),
    (0x7D, Key::IntlYen),
];

/// The virtual-key codes of keys that type characters, on a US layout. Only for when the layout in
/// use can't be asked (other systems, or a code with no scan code), and to read settings from
/// before Remapr knew positions.
const US_CHARS: &[(u8, Key)] = &[
    (0xBA, Key::Semicolon),
    (0xBB, Key::Equal),
    (0xBC, Key::Comma),
    (0xBD, Key::Minus),
    (0xBE, Key::Period),
    (0xBF, Key::Slash),
    (0xC0, Key::Backquote),
    (0xC1, Key::IntlRo),
    (0xDB, Key::BracketLeft),
    (0xDC, Key::Backslash),
    (0xDD, Key::BracketRight),
    (0xDE, Key::Quote),
    (0xE2, Key::IntlBackslash),
];

/// Every other key: (virtual-key code, extended flag or `None` for either, key). Where a key has
/// two codes, the first is the one injected.
const FIXED: &[(u8, Option<bool>, Key)] = &[
    (0x08, None, Key::Backspace),
    (0x09, None, Key::Tab),
    (VK_RETURN, Some(false), Key::Enter),
    (VK_RETURN, Some(true), Key::NumpadEnter),
    (0x13, None, Key::Pause),
    (0x14, None, Key::CapsLock),
    (0x15, None, Key::KanaMode),
    (0x19, None, Key::Lang2),
    (0x1B, None, Key::Escape),
    (0x1C, None, Key::Convert),
    (0x1D, None, Key::NonConvert),
    (0x20, None, Key::Space),
    (0x21, Some(true), Key::PageUp),
    (0x22, Some(true), Key::PageDown),
    (0x23, Some(true), Key::End),
    (0x24, Some(true), Key::Home),
    (0x25, Some(true), Key::ArrowLeft),
    (0x26, Some(true), Key::ArrowUp),
    (0x27, Some(true), Key::ArrowRight),
    (0x28, Some(true), Key::ArrowDown),
    (0x2C, None, Key::PrintScreen),
    (0x2D, Some(true), Key::Insert),
    (0x2E, Some(true), Key::Delete),
    (0x2F, None, Key::Help),
    (0x5B, None, Key::MetaLeft),
    (0x5C, None, Key::MetaRight),
    (0x5D, None, Key::ContextMenu),
    (0x60, None, Key::Numpad0),
    (0x61, None, Key::Numpad1),
    (0x62, None, Key::Numpad2),
    (0x63, None, Key::Numpad3),
    (0x64, None, Key::Numpad4),
    (0x65, None, Key::Numpad5),
    (0x66, None, Key::Numpad6),
    (0x67, None, Key::Numpad7),
    (0x68, None, Key::Numpad8),
    (0x69, None, Key::Numpad9),
    (0x6A, None, Key::NumpadMultiply),
    (0x6B, None, Key::NumpadAdd),
    (0x6C, None, Key::NumpadComma),
    (0x6D, None, Key::NumpadSubtract),
    (0x6E, None, Key::NumpadDecimal),
    (0x6F, None, Key::NumpadDivide),
    // The numpad with Num Lock off: the navigation codes, without the extended flag.
    (0x0C, Some(false), Key::Numpad5),
    (0x21, Some(false), Key::Numpad9),
    (0x22, Some(false), Key::Numpad3),
    (0x23, Some(false), Key::Numpad1),
    (0x24, Some(false), Key::Numpad7),
    (0x25, Some(false), Key::Numpad4),
    (0x26, Some(false), Key::Numpad8),
    (0x27, Some(false), Key::Numpad6),
    (0x28, Some(false), Key::Numpad2),
    (0x2D, Some(false), Key::Numpad0),
    (0x2E, Some(false), Key::NumpadDecimal),
    (0x70, None, Key::F1),
    (0x71, None, Key::F2),
    (0x72, None, Key::F3),
    (0x73, None, Key::F4),
    (0x74, None, Key::F5),
    (0x75, None, Key::F6),
    (0x76, None, Key::F7),
    (0x77, None, Key::F8),
    (0x78, None, Key::F9),
    (0x79, None, Key::F10),
    (0x7A, None, Key::F11),
    (0x7B, None, Key::F12),
    (0x7C, None, Key::F13),
    (0x7D, None, Key::F14),
    (0x7E, None, Key::F15),
    (0x7F, None, Key::F16),
    (0x80, None, Key::F17),
    (0x81, None, Key::F18),
    (0x82, None, Key::F19),
    (0x83, None, Key::F20),
    (0x84, None, Key::F21),
    (0x85, None, Key::F22),
    (0x86, None, Key::F23),
    (0x87, None, Key::F24),
    (VK_NUMLOCK, None, Key::NumLock),
    (0x91, None, Key::ScrollLock),
    (0x92, None, Key::NumpadEqual),
    (0xA0, None, Key::ShiftLeft),
    (0xA1, None, Key::ShiftRight),
    (VK_LCONTROL, None, Key::ControlLeft),
    (0xA3, None, Key::ControlRight),
    (0xA4, None, Key::AltLeft),
    (0xA5, None, Key::AltRight),
    (0xA6, None, Key::BrowserBack),
    (0xA7, None, Key::BrowserForward),
    (0xA8, None, Key::BrowserRefresh),
    (0xA9, None, Key::BrowserStop),
    (0xAA, None, Key::BrowserSearch),
    (0xAB, None, Key::BrowserFavorites),
    (0xAC, None, Key::BrowserHome),
    (0xAD, None, Key::AudioVolumeMute),
    (0xAE, None, Key::AudioVolumeDown),
    (0xAF, None, Key::AudioVolumeUp),
    (0xB0, None, Key::MediaTrackNext),
    (0xB1, None, Key::MediaTrackPrevious),
    (0xB2, None, Key::MediaStop),
    (0xB3, None, Key::MediaPlayPause),
    (0xB4, None, Key::LaunchMail),
    (0xB5, None, Key::MediaSelect),
    (0xB6, None, Key::LaunchApp1),
    (0xB7, None, Key::LaunchApp2),
];

/// Whether `vk` names a character, whose key depends on the layout: letters, digits, `VK_OEM_*`.
pub fn is_layout_vk(vk: u8) -> bool {
    matches!(vk, b'0'..=b'9' | b'A'..=b'Z') || US_CHARS.iter().any(|&(v, _)| v == vk)
}

/// The character key at scan code `scan`.
pub fn char_key(scan: u16) -> Option<Key> {
    CHARS.iter().find(|&&(s, _)| s == scan).map(|&(_, k)| k)
}

/// The scan code of a character key, or `None` for any other key.
pub fn char_scan(key: Key) -> Option<u16> {
    CHARS.iter().find(|&&(_, k)| k == key).map(|&(s, _)| s)
}

/// The character key `vk` names on a US layout.
pub fn us_char_key(vk: u8) -> Option<Key> {
    let letter_or_digit = |k: Key| k.us_char() == Some(char::from(vk));
    match vk {
        b'0'..=b'9' | b'A'..=b'Z' => Key::ALL.iter().copied().find(|&k| letter_or_digit(k) && char_scan(k).is_some()),
        _ => US_CHARS.iter().find(|&&(v, _)| v == vk).map(|&(_, k)| k),
    }
}

/// The key a non-character `vk` stands for, given its extended flag.
pub fn fixed_key(vk: u8, extended: bool) -> Option<Key> {
    FIXED.iter().find(|&&(v, e, _)| v == vk && e.is_none_or(|e| e == extended)).map(|&(_, _, k)| k)
}

/// The key a hook event is about. Characters go by position (scan code); a code with no known scan
/// code (another program's synthetic input) falls back to its US meaning.
pub fn key_of(vk: u8, scan: u16, extended: bool) -> Option<Key> {
    if is_layout_vk(vk) {
        return char_key(scan).or_else(|| us_char_key(vk));
    }
    fixed_key(vk, extended)
}

/// The virtual-key code that injects a non-character key, and whether it must be marked extended
/// (`None`: as the system says for that code).
pub fn vk_of(key: Key) -> Option<(u8, Option<bool>)> {
    FIXED.iter().find(|&&(_, _, k)| k == key).map(|&(v, e, _)| (v, e))
}

/// The key a setting from Remapr 0.5 or earlier, which stored virtual-key codes, meant, read as a
/// US layout would. Navigation codes meant the navigation keys, not the numpad's.
pub fn key_of_old_vk(vk: u8) -> Option<Key> {
    if is_layout_vk(vk) {
        return us_char_key(vk);
    }
    fixed_key(vk, vk != VK_RETURN).or_else(|| fixed_key(vk, false))
}

/// The fake Left Ctrl that AltGr layouts send with Right Alt.
pub fn is_altgr_phantom(vk: u8, scan: u32) -> bool {
    vk == VK_LCONTROL && scan == ALTGR_FAKE_CTRL_SCAN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_one_way_in_and_out() {
        for &key in Key::ALL {
            match (char_scan(key), vk_of(key)) {
                (Some(scan), None) => assert_eq!(char_key(scan), Some(key)),
                (None, Some((vk, ext))) => assert_eq!(fixed_key(vk, ext.unwrap_or(false)), Some(key), "{key}"),
                // Keys Windows gives no virtual-key code of their own.
                (None, None) => assert!(
                    matches!(key, Key::Power | Key::Undo | Key::Cut | Key::Copy | Key::Paste | Key::Find | Key::Lang1),
                    "{key} can't be read or sent on Windows"
                ),
                (Some(_), Some(_)) => panic!("{key} is both a character and a fixed key"),
            }
        }
    }

    #[test]
    fn characters_go_by_position_others_by_code() {
        // VK_A pressed where QWERTY has Q (AZERTY): the key is KeyQ.
        assert_eq!(key_of(b'A', 0x10, false), Some(Key::KeyQ));
        // Ñ on Spanish is VK_OEM_3 at the US semicolon position.
        assert_eq!(key_of(0xC0, 0x27, false), Some(Key::Semicolon));
        // No scan code (another program's input): the US meaning.
        assert_eq!(key_of(b'A', 0, false), Some(Key::KeyA));
        assert_eq!(key_of(0x25, 0x4B, true), Some(Key::ArrowLeft));
        assert_eq!(key_of(0x25, 0x4B, false), Some(Key::Numpad4));
        assert_eq!(key_of(VK_RETURN, 0x1C, false), Some(Key::Enter));
        assert_eq!(key_of(VK_RETURN, 0x1C, true), Some(Key::NumpadEnter));
        assert_eq!(key_of(0x13, 0x45, false), Some(Key::Pause));
        assert_eq!(key_of(VK_NUMLOCK, 0x45, true), Some(Key::NumLock));
        assert_eq!(key_of(0xB3, 0x22, true), Some(Key::MediaPlayPause));
        assert_eq!(key_of(0xE7, 0, false), None, "VK_PACKET");
    }

    #[test]
    fn old_settings_read_as_us_keys() {
        assert_eq!(key_of_old_vk(0x14), Some(Key::CapsLock));
        assert_eq!(key_of_old_vk(0x1B), Some(Key::Escape));
        assert_eq!(key_of_old_vk(b'T'), Some(Key::KeyT));
        assert_eq!(key_of_old_vk(b'7'), Some(Key::Digit7));
        assert_eq!(key_of_old_vk(0xBA), Some(Key::Semicolon));
        assert_eq!(key_of_old_vk(0x25), Some(Key::ArrowLeft));
        assert_eq!(key_of_old_vk(VK_RETURN), Some(Key::Enter));
        assert_eq!(key_of_old_vk(0x0C), Some(Key::Numpad5));
        assert_eq!(key_of_old_vk(0xA5), Some(Key::AltRight));
        assert_eq!(key_of_old_vk(0x00), None);
    }

    #[test]
    fn the_altgr_phantom_is_only_that_scan_code() {
        assert!(is_altgr_phantom(VK_LCONTROL, 0x21D));
        assert!(!is_altgr_phantom(VK_LCONTROL, 0x1D));
        assert!(!is_altgr_phantom(0xA5, 0x21D));
    }
}
