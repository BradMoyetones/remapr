//! What the keyboard layout in use says about keys: the character one types, and which key an old
//! virtual-key code meant.

use remapr_core::Key;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MAPVK_VK_TO_CHAR, MAPVK_VK_TO_VSC, MAPVK_VSC_TO_VK, MapVirtualKeyW,
};

use super::keys;

/// The character `key` types on the current layout (`Ñ` for `Semicolon` on Spanish), for the keys
/// that type one.
pub fn layout_char(key: Key) -> Option<char> {
    let scan = keys::char_scan(key)?;
    // SAFETY: pure lookups with no pointers.
    let c = unsafe { MapVirtualKeyW(MapVirtualKeyW(u32::from(scan), MAPVK_VSC_TO_VK), MAPVK_VK_TO_CHAR) } & 0xFFFF;
    char::from_u32(c).filter(|c| !c.is_control())
}

/// The key a setting from Remapr 0.5 or earlier meant by `vk`. A character's code is read through
/// the current layout, so it is the key the user pressed to record it (`VK_OEM_3` is Ñ, at
/// `Semicolon`, on Spanish).
pub fn key_from_vk(vk: u8) -> Option<Key> {
    if keys::is_layout_vk(vk) {
        // SAFETY: a pure lookup with no pointers.
        let scan = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC) } as u16;
        if let Some(key) = keys::char_key(scan) {
            return Some(key);
        }
    }
    keys::key_of_old_vk(vk)
}
