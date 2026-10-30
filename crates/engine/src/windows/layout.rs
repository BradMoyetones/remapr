//! What the keyboard layout in use says about keys: the character one types.

use remapr_core::Key;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MAPVK_VK_TO_CHAR, MAPVK_VSC_TO_VK, MapVirtualKeyW};

use super::keys;

/// The character `key` types on the current layout (`Ñ` for `Semicolon` on Spanish), for the keys
/// that type one.
pub fn layout_char(key: Key) -> Option<char> {
    let scan = keys::char_scan(key)?;
    // SAFETY: pure lookups with no pointers.
    let c = unsafe { MapVirtualKeyW(MapVirtualKeyW(u32::from(scan), MAPVK_VSC_TO_VK), MAPVK_VK_TO_CHAR) } & 0xFFFF;
    char::from_u32(c).filter(|c| !c.is_control())
}
