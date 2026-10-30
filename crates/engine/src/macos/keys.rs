//! macOS's key codes and Remapr's [`Key`]s, as plain tables, tested on every system.
//!
//! macOS's virtual key codes (`kVK_*`, HIToolbox `Events.h`) name positions on an ANSI keyboard, as
//! [`Key`]s do, so one table serves both ways. Modifier keys arrive as "flags changed" events,
//! which say whether a key is down through per-side bits of the event's flags.

use remapr_core::Key;

/// (key code, key). One code per key.
const KEYS: &[(u16, Key)] = &[
    (0x00, Key::KeyA),
    (0x01, Key::KeyS),
    (0x02, Key::KeyD),
    (0x03, Key::KeyF),
    (0x04, Key::KeyH),
    (0x05, Key::KeyG),
    (0x06, Key::KeyZ),
    (0x07, Key::KeyX),
    (0x08, Key::KeyC),
    (0x09, Key::KeyV),
    (0x0A, Key::IntlBackslash),
    (0x0B, Key::KeyB),
    (0x0C, Key::KeyQ),
    (0x0D, Key::KeyW),
    (0x0E, Key::KeyE),
    (0x0F, Key::KeyR),
    (0x10, Key::KeyY),
    (0x11, Key::KeyT),
    (0x12, Key::Digit1),
    (0x13, Key::Digit2),
    (0x14, Key::Digit3),
    (0x15, Key::Digit4),
    (0x16, Key::Digit6),
    (0x17, Key::Digit5),
    (0x18, Key::Equal),
    (0x19, Key::Digit9),
    (0x1A, Key::Digit7),
    (0x1B, Key::Minus),
    (0x1C, Key::Digit8),
    (0x1D, Key::Digit0),
    (0x1E, Key::BracketRight),
    (0x1F, Key::KeyO),
    (0x20, Key::KeyU),
    (0x21, Key::BracketLeft),
    (0x22, Key::KeyI),
    (0x23, Key::KeyP),
    (0x24, Key::Enter),
    (0x25, Key::KeyL),
    (0x26, Key::KeyJ),
    (0x27, Key::Quote),
    (0x28, Key::KeyK),
    (0x29, Key::Semicolon),
    (0x2A, Key::Backslash),
    (0x2B, Key::Comma),
    (0x2C, Key::Slash),
    (0x2D, Key::KeyN),
    (0x2E, Key::KeyM),
    (0x2F, Key::Period),
    (0x30, Key::Tab),
    (0x31, Key::Space),
    (0x32, Key::Backquote),
    (0x33, Key::Backspace),
    (0x35, Key::Escape),
    (0x36, Key::MetaRight),
    (0x37, Key::MetaLeft),
    (0x38, Key::ShiftLeft),
    (0x39, Key::CapsLock),
    (0x3A, Key::AltLeft),
    (0x3B, Key::ControlLeft),
    (0x3C, Key::ShiftRight),
    (0x3D, Key::AltRight),
    (0x3E, Key::ControlRight),
    (0x40, Key::F17),
    (0x41, Key::NumpadDecimal),
    (0x43, Key::NumpadMultiply),
    (0x45, Key::NumpadAdd),
    // The numpad's Clear key, where PC keyboards have Num Lock.
    (0x47, Key::NumLock),
    (0x48, Key::AudioVolumeUp),
    (0x49, Key::AudioVolumeDown),
    (0x4A, Key::AudioVolumeMute),
    (0x4B, Key::NumpadDivide),
    (0x4C, Key::NumpadEnter),
    (0x4E, Key::NumpadSubtract),
    (0x4F, Key::F18),
    (0x50, Key::F19),
    (0x51, Key::NumpadEqual),
    (0x52, Key::Numpad0),
    (0x53, Key::Numpad1),
    (0x54, Key::Numpad2),
    (0x55, Key::Numpad3),
    (0x56, Key::Numpad4),
    (0x57, Key::Numpad5),
    (0x58, Key::Numpad6),
    (0x59, Key::Numpad7),
    (0x5A, Key::F20),
    (0x5B, Key::Numpad8),
    (0x5C, Key::Numpad9),
    (0x5D, Key::IntlYen),
    (0x5E, Key::IntlRo),
    (0x5F, Key::NumpadComma),
    (0x60, Key::F5),
    (0x61, Key::F6),
    (0x62, Key::F7),
    (0x63, Key::F3),
    (0x64, Key::F8),
    (0x65, Key::F9),
    (0x66, Key::Lang2),
    (0x67, Key::F11),
    (0x68, Key::Lang1),
    (0x69, Key::F13),
    (0x6A, Key::F16),
    (0x6B, Key::F14),
    (0x6D, Key::F10),
    (0x6E, Key::ContextMenu),
    (0x6F, Key::F12),
    (0x71, Key::F15),
    // Help, where PC keyboards have Insert.
    (0x72, Key::Insert),
    (0x73, Key::Home),
    (0x74, Key::PageUp),
    (0x75, Key::Delete),
    (0x76, Key::F4),
    (0x77, Key::End),
    (0x78, Key::F2),
    (0x79, Key::PageDown),
    (0x7A, Key::F1),
    (0x7B, Key::ArrowLeft),
    (0x7C, Key::ArrowRight),
    (0x7D, Key::ArrowDown),
    (0x7E, Key::ArrowUp),
];

pub fn key_of(keycode: u16) -> Option<Key> {
    KEYS.iter().find(|&&(c, _)| c == keycode).map(|&(_, k)| k)
}

pub fn keycode_of(key: Key) -> Option<u16> {
    KEYS.iter().find(|&&(_, k)| k == key).map(|&(c, _)| c)
}

/// Caps Lock's flag: whether it is locked, not whether it is down.
pub const ALPHA_SHIFT: u64 = 0x0001_0000;
const SHIFT: u64 = 0x0002_0000;
const CONTROL: u64 = 0x0004_0000;
const ALTERNATE: u64 = 0x0008_0000;
const COMMAND: u64 = 0x0010_0000;
const NUMERIC_PAD: u64 = 0x0020_0000;
const SECONDARY_FN: u64 = 0x0080_0000;

/// The eight modifier keys, each with its own bit in an event's flags (`NX_DEVICE*KEYMASK`) and
/// the bit it shares with its twin.
pub const MODIFIERS: [(Key, u64, u64); 8] = [
    (Key::ControlLeft, 0x0001, CONTROL),
    (Key::ShiftLeft, 0x0002, SHIFT),
    (Key::ShiftRight, 0x0004, SHIFT),
    (Key::MetaLeft, 0x0008, COMMAND),
    (Key::MetaRight, 0x0010, COMMAND),
    (Key::AltLeft, 0x0020, ALTERNATE),
    (Key::AltRight, 0x0040, ALTERNATE),
    (Key::ControlRight, 0x2000, CONTROL),
];

/// Every bit modifier keys set in an event's flags, held or not.
pub const MODIFIER_FLAGS: u64 = 0x2000 | 0x7F | SHIFT | CONTROL | ALTERNATE | COMMAND;

/// This modifier key's place in [`MODIFIERS`].
pub fn modifier_index(key: Key) -> Option<usize> {
    MODIFIERS.iter().position(|&(k, _, _)| k == key)
}

/// Whether a "flags changed" event for modifier `key` says it went down.
pub fn is_down(key: Key, flags: u64) -> bool {
    modifier_index(key).is_some_and(|i| flags & MODIFIERS[i].1 != 0)
}

/// Flags real events of `key` carry besides the modifiers: arrows are on the "numeric pad",
/// function and navigation keys need Fn on small keyboards, the numpad is the numpad.
pub fn extra_flags(key: Key) -> u64 {
    match key {
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => NUMERIC_PAD | SECONDARY_FN,
        Key::Home | Key::End | Key::PageUp | Key::PageDown | Key::Delete | Key::Insert => SECONDARY_FN,
        _ if key.function_number().is_some() => SECONDARY_FN,
        _ if key.numpad_digit().is_some() => NUMERIC_PAD,
        Key::NumpadDecimal
        | Key::NumpadMultiply
        | Key::NumpadAdd
        | Key::NumpadDivide
        | Key::NumpadEnter
        | Key::NumpadSubtract
        | Key::NumpadEqual
        | Key::NumpadComma
        | Key::NumLock => NUMERIC_PAD,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_keys_go_both_ways() {
        for &(code, key) in KEYS {
            assert_eq!(key_of(code), Some(key), "{code:#04x}");
            assert_eq!(keycode_of(key), Some(code), "{key}");
        }
        let mut keys: Vec<Key> = KEYS.iter().map(|&(_, k)| k).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), KEYS.len(), "a key with two codes");
        // Fn has no HID usage of its own: it isn't a Remapr key.
        assert_eq!(key_of(0x3F), None);
    }

    #[test]
    fn every_modifier_has_a_code_and_its_own_bit() {
        for (i, &(key, bit, shared)) in MODIFIERS.iter().enumerate() {
            assert!(keycode_of(key).is_some(), "{key}");
            assert!(!key.modifier().is_empty(), "{key}");
            assert_eq!(MODIFIERS.iter().filter(|m| m.1 == bit).count(), 1);
            assert_eq!(MODIFIER_FLAGS & (bit | shared), bit | shared);
            assert_eq!(modifier_index(key), Some(i));
        }
        assert!(is_down(Key::ShiftRight, 0x0002_0004));
        assert!(!is_down(Key::ShiftLeft, 0x0002_0004));
        assert!(!is_down(Key::KeyA, u64::MAX));
    }
}
