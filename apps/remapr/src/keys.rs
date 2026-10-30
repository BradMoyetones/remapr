//! Human names for keys, as printed on a keyboard. English here; each catalog names them as that
//! language's keyboards print them (context `key`). Keys that type a character show what the
//! layout in use types there, so `Semicolon` is Ñ on Spanish.

use std::borrow::Cow;

use remapr_core::{Chord, Key, Mods};

use crate::i18n::{key, tr};

/// Each modifier as this system's keyboards print it: Win, ⌘ or Super for Meta.
const MODS: [(Mods, &str); 4] = if cfg!(target_os = "macos") {
    [(Mods::CTRL, "⌃"), (Mods::SHIFT, "⇧"), (Mods::ALT, "⌥"), (Mods::META, "⌘")]
} else if cfg!(target_os = "windows") {
    [(Mods::CTRL, "Ctrl"), (Mods::SHIFT, "Shift"), (Mods::ALT, "Alt"), (Mods::META, "Win")]
} else {
    [(Mods::CTRL, "Ctrl"), (Mods::SHIFT, "Shift"), (Mods::ALT, "Alt"), (Mods::META, "Super")]
};

/// The names of a chord's keys, modifiers first: `["Ctrl", "Shift", "T"]`. One keycap each.
pub fn chord_name(chord: Chord) -> Vec<Cow<'static, str>> {
    let mods = MODS.into_iter().filter(|&(m, _)| chord.mods.contains(m)).map(|(_, name)| Cow::Borrowed(name));
    mods.chain(std::iter::once(key_name(chord.key))).collect()
}

/// This system's modifiers, for messages: `Ctrl, Shift, Alt, Win`.
pub fn modifier_names() -> String {
    MODS.map(|(_, name)| name).join(", ")
}

/// `Ctrl + Shift + T`, for messages.
pub fn chord_text(chord: Chord) -> String {
    chord_name(chord).join(" + ")
}

pub fn key_name(key: Key) -> Cow<'static, str> {
    if let Some(name) = fixed_name(key) {
        return name.into();
    }
    if let Some(n) = key.function_number() {
        return format!("F{n}").into();
    }
    if let Some(d) = key.numpad_digit() {
        return format!("Num {d}").into();
    }
    // A key that types a character: what the layout in use types there.
    match remapr_engine::layout_char(key).or(key.us_char()) {
        Some(c) => c.to_uppercase().to_string().into(),
        None => tr!("Key {code}", code = key.name()).into(),
    }
}

/// The names of keys that don't type a character, where this system's keyboards name them.
fn fixed_name(key: Key) -> Option<&'static str> {
    let (left_meta, right_meta) = if cfg!(target_os = "macos") {
        ("Left ⌘", "Right ⌘")
    } else if cfg!(target_os = "windows") {
        (key!("Left Win"), key!("Right Win"))
    } else {
        (key!("Left Super"), key!("Right Super"))
    };
    Some(match key {
        Key::Backspace => key!("Backspace"),
        Key::Tab => "Tab",
        Key::Enter => "Enter",
        Key::NumpadEnter => "Num Enter",
        Key::Escape => "Esc",
        Key::Space => key!("Space"),
        Key::CapsLock => key!("Caps Lock"),
        Key::PrintScreen => key!("Print Screen"),
        Key::ScrollLock => key!("Scroll Lock"),
        Key::Pause => key!("Pause"),
        Key::Insert => "Insert",
        Key::Delete => key!("Delete"),
        Key::Home => key!("Home"),
        Key::End => key!("End"),
        Key::PageUp => key!("Page Up"),
        Key::PageDown => key!("Page Down"),
        Key::ArrowLeft => "←",
        Key::ArrowUp => "↑",
        Key::ArrowRight => "→",
        Key::ArrowDown => "↓",
        Key::NumLock => key!("Num Lock"),
        Key::NumpadDivide => "Num /",
        Key::NumpadMultiply => "Num *",
        Key::NumpadSubtract => "Num −",
        Key::NumpadAdd => "Num +",
        Key::NumpadDecimal => "Num .",
        Key::NumpadEqual => "Num =",
        Key::NumpadComma => "Num ,",
        Key::ContextMenu => key!("Menu"),
        Key::Power => key!("Power"),
        Key::Help => key!("Help"),
        Key::Undo => key!("Undo"),
        Key::Cut => key!("Cut"),
        Key::Copy => key!("Copy"),
        Key::Paste => key!("Paste"),
        Key::Find => key!("Find"),
        Key::KanaMode => "Kana",
        Key::Convert => "Convert",
        Key::NonConvert => "Non-convert",
        Key::Lang1 => "Lang 1",
        Key::Lang2 => "Lang 2",
        Key::ControlLeft => key!("Left Ctrl"),
        Key::ControlRight => key!("Right Ctrl"),
        Key::ShiftLeft => key!("Left Shift"),
        Key::ShiftRight => key!("Right Shift"),
        Key::AltLeft => "Alt",
        Key::AltRight => key!("Right Alt"),
        Key::MetaLeft => left_meta,
        Key::MetaRight => right_meta,
        Key::AudioVolumeMute => key!("Mute"),
        Key::AudioVolumeDown => key!("Volume −"),
        Key::AudioVolumeUp => key!("Volume +"),
        Key::MediaTrackNext => key!("Next track"),
        Key::MediaTrackPrevious => key!("Previous track"),
        Key::MediaStop => key!("Stop"),
        Key::MediaPlayPause => key!("Play/Pause"),
        Key::MediaSelect => key!("Media"),
        Key::LaunchMail => key!("Mail"),
        Key::LaunchApp1 => "App 1",
        Key::LaunchApp2 => "App 2",
        Key::BrowserBack => key!("Back"),
        Key::BrowserForward => key!("Forward"),
        Key::BrowserRefresh => key!("Refresh"),
        Key::BrowserStop => key!("Stop loading"),
        Key::BrowserSearch => key!("Search"),
        Key::BrowserFavorites => key!("Favorites"),
        Key::BrowserHome => key!("Home page"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_common_keys() {
        assert_eq!(key_name(Key::CapsLock), key!("Caps Lock"));
        assert_eq!(key_name(Key::F12), "F12");
        assert_eq!(key_name(Key::F24), "F24");
        assert_eq!(key_name(Key::Numpad5), "Num 5");
        assert_eq!(key_name(Key::NumpadEnter), "Num Enter");
        // The character a key types depends on the layout; the US one where none can be asked.
        assert!(!key_name(Key::KeyQ).is_empty());
        let chord = Chord { mods: Mods::CTRL | Mods::SHIFT, key: Key::F5 };
        assert_eq!(chord_text(chord), "Ctrl + Shift + F5".replace("Ctrl", MODS[0].1).replace("Shift", MODS[1].1));
    }

    #[test]
    fn every_key_has_a_name() {
        for &k in Key::ALL {
            assert!(!key_name(k).is_empty(), "{k}");
        }
    }
}
