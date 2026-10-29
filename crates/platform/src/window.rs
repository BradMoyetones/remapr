//! Native window icons and title bar.

use std::ptr::null;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GetForegroundWindow, GetSystemMetrics, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR,
    LoadImageW, PostMessageW, SM_CXICON, SM_CXSMICON, SWP_NOACTIVATE, SWP_SHOWWINDOW, SendMessageW, WINDOWPOS, WM_APP,
    WM_NCACTIVATE, WM_SETICON, WM_SHOWWINDOW, WM_WINDOWPOSCHANGING,
};

const SUBCLASS_ID: usize = 1;
const WM_REPAINT_TITLE_BAR: u32 = WM_APP + 0x52;
static HIDE_FIRST_SHOW: AtomicBool = AtomicBool::new(false);

/// Repaints the title bar of `hwnd` each time the window is shown.
///
/// egui gives the window its light or dark theme before it first appears, but Windows paints that
/// first title bar in the system theme and only corrects it on the next activation change: a light
/// app on a dark system showed a dark title bar until the user switched away and back.
pub fn repaint_title_bar_when_shown(hwnd: isize) {
    // SAFETY: subclassing a window this process owns; installing again only updates it.
    unsafe { SetWindowSubclass(hwnd as _, Some(window_subclass), SUBCLASS_ID, 0) };
}

/// Keeps `hwnd` hidden the first time something shows it, without taking the focus.
///
/// eframe always shows the window after painting its first frame; a Remapr started at sign-in must
/// stay in the tray. The app must still send `ViewportCommand::Visible(false)` on its first frame,
/// so winit knows the window is hidden and shows it for real next time.
pub fn hide_first_show(hwnd: isize) {
    HIDE_FIRST_SHOW.store(true, Relaxed);
    // SAFETY: as in `repaint_title_bar_when_shown`; both share one subclass.
    unsafe { SetWindowSubclass(hwnd as _, Some(window_subclass), SUBCLASS_ID, 0) };
}

unsafe extern "system" fn window_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: plain messages to our own window; everything else is forwarded untouched.
    unsafe {
        if msg == WM_REPAINT_TITLE_BAR {
            // A round trip through the other activation state and back repaints the frame. Straight
            // to DefWindowProc: winit reads WM_NCACTIVATE as a focus change, and a fake one left
            // egui believing the window had lost the focus.
            let active = usize::from(GetForegroundWindow() == hwnd);
            DefWindowProcW(hwnd, WM_NCACTIVATE, active ^ 1, 0);
            DefWindowProcW(hwnd, WM_NCACTIVATE, active, 0);
            return 0;
        }
        if msg == WM_WINDOWPOSCHANGING && HIDE_FIRST_SHOW.load(Relaxed) {
            // For WM_WINDOWPOSCHANGING, `lparam` points to the WINDOWPOS about to be applied.
            let pos = &mut *(lparam as *mut WINDOWPOS);
            if pos.flags & SWP_SHOWWINDOW != 0 {
                pos.flags = (pos.flags & !SWP_SHOWWINDOW) | SWP_NOACTIVATE;
                HIDE_FIRST_SHOW.store(false, Relaxed);
            }
        }
        if msg == WM_SHOWWINDOW && wparam != 0 {
            // Posted, because the window is only on screen once WM_SHOWWINDOW returns.
            PostMessageW(hwnd, WM_REPAINT_TITLE_BAR, 0, 0);
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

/// Whether `hwnd` is the window the user is typing into.
pub fn is_foreground(hwnd: isize) -> bool {
    // SAFETY: a pure query.
    unsafe { GetForegroundWindow() as isize == hwnd }
}

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
