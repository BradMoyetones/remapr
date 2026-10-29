//! Where the installer put Remapr, as it records in the registry.

use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr::null_mut;

use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};

use crate::wide;

/// The folder the MSI installed Remapr into (`HKLM\Software\Remapr\InstallDir`), if it did.
pub fn install_dir() -> Option<PathBuf> {
    let (key, value) = (wide(r"Software\Remapr"), wide("InstallDir"));
    let mut buf = [0u16; 1024];
    let mut len = size_of_val(&buf) as u32;
    // SAFETY: `buf` and `len` describe a writable buffer; the strings are NUL-terminated.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buf.as_mut_ptr().cast(),
            &mut len,
        )
    };
    if status != 0 {
        return None;
    }
    // `len` counts bytes, including the terminating NUL.
    let chars = (len as usize / 2).saturating_sub(1);
    Some(PathBuf::from(std::ffi::OsString::from_wide(&buf[..chars])))
}
