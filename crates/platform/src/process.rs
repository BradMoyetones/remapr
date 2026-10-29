//! Which program a window belongs to, and which programs have windows open.

use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GWL_EXSTYLE, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, WS_EX_TOOLWINDOW,
};

/// The file name of the program that owns `hwnd`, lowercase (`chrome.exe`). `None` when Windows won't
/// say: a program running as administrator, while Remapr isn't.
pub fn exe_name(hwnd: isize) -> Option<String> {
    let mut pid = 0;
    // SAFETY: plain queries; the process handle is closed before returning.
    unsafe {
        GetWindowThreadProcessId(hwnd as HWND, &mut pid);
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(process);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().map(str::to_lowercase)
    }
}

/// A program with a window open, as the app picker lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningApp {
    /// Lowercase file name, what a rule stores: `chrome.exe`.
    pub exe: String,
    /// The title of one of its windows, to help recognise it.
    pub title: String,
}

/// Hosts of other apps (Store apps run inside it), not something a user would pick.
const NOT_APPS: [&str; 1] = ["applicationframehost.exe"];

/// Programs with a visible, titled top-level window, other than Remapr; one entry each, by name.
/// The same windows Alt+Tab shows: cloaked ones (background Store apps, the touch keyboard) are out.
///
/// shortcut: a Store app's window belongs to ApplicationFrameHost, so a rule can't target one Store
/// app; resolve the hosted process (`GetWindow` of the child core window) if users ask for that.
pub fn running_apps() -> Vec<RunningApp> {
    let mut apps: Vec<RunningApp> = Vec::new();
    // SAFETY: `apps` outlives EnumWindows, which calls back synchronously on this thread.
    unsafe { EnumWindows(Some(collect), &raw mut apps as LPARAM) };
    apps.sort_by(|a, b| a.exe.cmp(&b.exe));
    apps.dedup_by(|a, b| a.exe == b.exe);
    apps
}

unsafe extern "system" fn collect(hwnd: HWND, apps: LPARAM) -> i32 {
    // SAFETY: `apps` is the Vec `running_apps` passed; the rest are plain queries.
    unsafe {
        let apps = &mut *(apps as *mut Vec<RunningApp>);
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let tool = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0;
        let len = GetWindowTextLengthW(hwnd);
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() || tool || len == 0 {
            return 1;
        }
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, (&raw mut cloaked).cast(), size_of::<u32>() as u32);
        if pid == GetCurrentProcessId() || cloaked != 0 {
            return 1;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if let Some(exe) = exe_name(hwnd as isize).filter(|exe| !NOT_APPS.contains(&exe.as_str())) {
            apps.push(RunningApp { exe, title: String::from_utf16_lossy(&buf[..copied as usize]) });
        }
        1
    }
}
