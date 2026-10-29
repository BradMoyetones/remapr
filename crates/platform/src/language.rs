//! The languages the user wants Windows' own text in.

use std::ptr::null_mut;

use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};

/// The user's display languages, most wanted first, as tags like `es-CO` or `en-US`.
pub fn preferred_languages() -> Vec<String> {
    let (mut count, mut len) = (0u32, 0u32);
    // SAFETY: the first call only reports the size; the second writes into a buffer that big.
    unsafe {
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, null_mut(), &mut len) == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u16; len as usize];
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, buf.as_mut_ptr(), &mut len) == 0 {
            return Vec::new();
        }
        // NUL-separated tags, ending in an empty one.
        buf.split(|&c| c == 0).filter(|tag| !tag.is_empty()).map(String::from_utf16_lossy).collect()
    }
}
