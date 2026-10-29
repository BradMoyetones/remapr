//! One Remapr per Windows session. A second launch asks the running one to show its window, then
//! steps aside.
//!
//! Both kernel objects carry a low integrity label, so a normal launch can still reach a Remapr
//! running as administrator (which is how the installed app runs).

use std::io;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, LocalFree, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForSingleObject};
use windows_sys::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};

use crate::wide;

const MUTEX: &str = "Local\\Remapr.Instance";
const SHOW_EVENT: &str = "Local\\Remapr.Show";
/// Everyone may use the objects, and processes of any integrity level may signal them.
const SDDL: &str = "D:(A;;GA;;;WD)S:(ML;;NW;;;LW)";

pub struct SingleInstance {
    /// A raw `HANDLE`, stored as an integer so the listener thread can own it.
    show_event: usize,
}

impl SingleInstance {
    /// `Some` for the first Remapr in this session. `None` after waking the one already running:
    /// the caller should exit.
    pub fn acquire() -> io::Result<Option<Self>> {
        let security = Security::new()?;
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security.0,
            bInheritHandle: 0,
        };
        // SAFETY: plain Win32 calls; `sa` and the wide strings outlive them.
        unsafe {
            // The event first, by both sides: whoever comes second just opens it, so a second launch
            // racing the first one's startup still gets its "show" delivered.
            let event = CreateEventW(&sa, 0, 0, wide(SHOW_EVENT).as_ptr());
            if event.is_null() {
                return Err(io::Error::last_os_error());
            }
            let mutex = CreateMutexW(&sa, 0, wide(MUTEX).as_ptr());
            let err = GetLastError();
            if mutex.is_null() && err != ERROR_ACCESS_DENIED {
                CloseHandle(event);
                return Err(io::Error::from_raw_os_error(err as i32));
            }
            if mutex.is_null() || err == ERROR_ALREADY_EXISTS {
                // We hold the foreground (the user just launched us); pass it on, or Windows only
                // flashes the running Remapr's taskbar button instead of raising its window.
                AllowSetForegroundWindow(ASFW_ANY);
                SetEvent(event);
                CloseHandle(event);
                if !mutex.is_null() {
                    CloseHandle(mutex);
                }
                return Ok(None);
            }
            // The mutex stays open for the life of the process: holding it is the lock.
            Ok(Some(Self { show_event: event as usize }))
        }
    }

    /// Calls `on_show`, on a background thread, each time another launch asks for the window.
    pub fn listen(self, on_show: impl Fn() + Send + 'static) {
        let event = self.show_event;
        let _ = std::thread::Builder::new().name("remapr-instance".into()).spawn(move || {
            // SAFETY: waiting on an event handle this process owns and never closes.
            while unsafe { WaitForSingleObject(event as HANDLE, INFINITE) } == WAIT_OBJECT_0 {
                on_show();
            }
        });
    }
}

/// A security descriptor built from [`SDDL`], freed on drop.
struct Security(PSECURITY_DESCRIPTOR);

impl Security {
    fn new() -> io::Result<Self> {
        let mut descriptor = null_mut();
        // SAFETY: Windows allocates the descriptor; `Drop` frees it with LocalFree as documented.
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(SDDL).as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(Self(descriptor)) }
    }
}

impl Drop for Security {
    fn drop(&mut self) {
        // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW.
        unsafe { LocalFree(self.0) };
    }
}
