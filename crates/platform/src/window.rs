//! Native window icons.

use std::ptr::null;

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON,
    SendMessageW, WM_SETICON,
};

/// The side, in pixels, of a small icon (title bar, notification area) at the system DPI.
pub fn small_icon_size() -> u32 {
    // SAFETY: a pure query.
    unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as u32
}

/// Gives the window `hwnd` the executable's icon resource `id`, at the exact sizes its DPI asks for.
/// (eframe can only set one bitmap, which Windows rescales blurrily.)
///
/// shortcut: sizes are picked once; moving the window to a monitor with another DPI keeps them.
/// Upgrade by calling this again on `WM_DPICHANGED` if that ever looks soft.
pub fn use_exe_icon(hwnd: isize, id: u16) {
    let hwnd = hwnd as _;
    // SAFETY: plain Win32 calls on a window owned by this process; a failed load is skipped.
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        let module = GetModuleHandleW(null());
        for (which, metric) in [(ICON_SMALL, SM_CXSMICON), (ICON_BIG, SM_CXICON)] {
            let size = GetSystemMetricsForDpi(metric, dpi);
            // MAKEINTRESOURCE: a numeric resource id travels as the pointer value.
            let icon = LoadImageW(module, usize::from(id) as _, IMAGE_ICON, size, size, LR_DEFAULTCOLOR);
            if !icon.is_null() {
                SendMessageW(hwnd, WM_SETICON, which as usize, icon as isize);
            }
        }
    }
}
