//! Human names for keys, as printed on a keyboard. English here; each catalog names them as that
//! language's keyboards print them (context `key`).

use std::borrow::Cow;

use remapr_engine::{Chord, Mods, Vk};

use crate::i18n::{key, tr};

/// The names of a chord's keys, modifiers first: `["Ctrl", "Shift", "T"]`. One keycap each.
pub fn chord_name(chord: Chord) -> Vec<Cow<'static, str>> {
    let mods = [(Mods::CTRL, "Ctrl"), (Mods::SHIFT, "Shift"), (Mods::ALT, "Alt"), (Mods::WIN, "Win")];
    let mods = mods.into_iter().filter(|&(m, _)| chord.mods.contains(m)).map(|(_, name)| Cow::Borrowed(name));
    mods.chain(std::iter::once(key_name(chord.key))).collect()
}

/// `Ctrl + Shift + T`, for messages.
pub fn chord_text(chord: Chord) -> String {
    chord_name(chord).join(" + ")
}

pub fn key_name(vk: Vk) -> Cow<'static, str> {
    let fixed = match vk {
        0x08 => key!("Backspace"),
        0x09 => "Tab",
        0x0D => "Enter",
        0x10 => "Shift",
        0x11 => "Ctrl",
        0x12 => "Alt",
        0x13 => key!("Pause"),
        0x14 => key!("Caps Lock"),
        0x1B => "Esc",
        0x20 => key!("Space"),
        0x21 => key!("Page Up"),
        0x22 => key!("Page Down"),
        0x23 => key!("End"),
        0x24 => key!("Home"),
        0x25 => "←",
        0x26 => "↑",
        0x27 => "→",
        0x28 => "↓",
        0x2C => key!("Print Screen"),
        0x2D => "Insert",
        0x2E => key!("Delete"),
        0x5B => key!("Left Win"),
        0x5C => key!("Right Win"),
        0x5D => key!("Menu"),
        0x6A => "Num *",
        0x6B => "Num +",
        0x6D => "Num −",
        0x6E => "Num .",
        0x6F => "Num /",
        0x90 => key!("Num Lock"),
        0x91 => key!("Scroll Lock"),
        0xA0 => key!("Left Shift"),
        0xA1 => key!("Right Shift"),
        0xA2 => key!("Left Ctrl"),
        0xA3 => key!("Right Ctrl"),
        0xA4 => "Alt",
        0xA5 => key!("Right Alt"),
        0xA6 => key!("Back"),
        0xA7 => key!("Forward"),
        0xA8 => key!("Refresh"),
        0xAD => key!("Mute"),
        0xAE => key!("Volume −"),
        0xAF => key!("Volume +"),
        0xB0 => key!("Next track"),
        0xB1 => key!("Previous track"),
        0xB2 => key!("Stop"),
        0xB3 => key!("Play/Pause"),
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.into();
    }
    match vk {
        b'0'..=b'9' | b'A'..=b'Z' => char::from(vk).to_string().into(),
        0x60..=0x69 => format!("Num {}", vk - 0x60).into(),
        0x70..=0x87 => format!("F{}", vk - 0x6F).into(),
        // OEM keys depend on the layout: ask Windows what this one types.
        _ => remapr_engine::layout_char(vk)
            .map(|c| c.to_uppercase().to_string().into())
            .unwrap_or_else(|| tr!("Key {code}", code = format!("{vk:#04X}")).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_common_keys() {
        assert_eq!(key_name(0x14), key!("Caps Lock"));
        assert_eq!(key_name(b'Q'), "Q");
        assert_eq!(key_name(0x7B), "F12");
        assert_eq!(key_name(0x87), "F24");
        assert_eq!(key_name(0x65), "Num 5");
        let chord = Chord { mods: Mods::CTRL | Mods::SHIFT, key: b'T' };
        assert_eq!(chord_text(chord), "Ctrl + Shift + T");
    }
}
