//! Windows' own open and save dialogs, for files of one kind.

use std::path::PathBuf;
use std::ptr::null_mut;

use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT,
    OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

/// What the dialogs offer: files ending in `.{extension}`, described as `name`.
pub struct FileKind<'a> {
    pub name: &'a str,
    pub extension: &'a str,
}

/// Asks where to save, suggesting `file_name`; asks before replacing a file. `None` if cancelled.
pub fn save_file_dialog(owner: Option<isize>, title: &str, file_name: &str, kind: &FileKind) -> Option<PathBuf> {
    run(owner, title, file_name, kind, OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST, GetSaveFileNameW)
}

/// Asks for an existing file. `None` if cancelled.
pub fn open_file_dialog(owner: Option<isize>, title: &str, kind: &FileKind) -> Option<PathBuf> {
    run(owner, title, "", kind, OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST, GetOpenFileNameW)
}

fn run(
    owner: Option<isize>,
    title: &str,
    file_name: &str,
    kind: &FileKind,
    flags: u32,
    show: unsafe extern "system" fn(*mut OPENFILENAMEW) -> i32,
) -> Option<PathBuf> {
    // Pairs of NUL-terminated strings, ending in an extra NUL.
    let filter: Vec<u16> =
        format!("{} (*.{ext})\0*.{ext}\0", kind.name, ext = kind.extension).encode_utf16().chain([0]).collect();
    let mut file = [0u16; 1024];
    for (slot, c) in file.iter_mut().zip(file_name.encode_utf16().take(1023)) {
        *slot = c;
    }
    let (title, extension) = (crate::wide(title), crate::wide(kind.extension));
    let mut dialog = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner.map_or(null_mut(), |h| h as _),
        lpstrFilter: filter.as_ptr(),
        lpstrFile: file.as_mut_ptr(),
        nMaxFile: file.len() as u32,
        lpstrTitle: title.as_ptr(),
        lpstrDefExt: extension.as_ptr(),
        Flags: flags | OFN_EXPLORER | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // SAFETY: every pointer in `dialog` points at a buffer that outlives the modal call.
    if unsafe { show(&mut dialog) } == 0 {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    Some(PathBuf::from(String::from_utf16_lossy(&file[..len])))
}
