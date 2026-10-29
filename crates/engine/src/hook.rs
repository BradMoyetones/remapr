//! The Win32 side: a `WH_KEYBOARD_LL` hook on its own high-priority thread, and `SendInput`.
//!
//! The hook callback runs for every key pressed anywhere on the system, and Windows silently
//! removes hooks that take too long. So the callback only reads atomics and a thread-local table:
//! no locks, no allocation, no I/O.

use std::cell::RefCell;
use std::io;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_HIGHEST,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_CHAR,
    MAPVK_VK_TO_VSC_EX, MapVirtualKeyW, SendInput, VK_NUMLOCK,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PM_NOREMOVE, PeekMessageW,
    PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYUP, WM_QUIT, WM_SYSKEYUP,
};

use crate::Vk;
use crate::remap::{Action, Event, decide, is_altgr_phantom};

static RUNNING: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(true);
static TABLE: [AtomicU8; 256] = [const { AtomicU8::new(0) }; 256];
static CAPTURING: AtomicBool = AtomicBool::new(false);
static CAPTURED: AtomicU8 = AtomicU8::new(0);

thread_local! {
    static HELD: RefCell<[Vk; 256]> = const { RefCell::new([0; 256]) };
    static ON_CAPTURE: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// The running hook. Dropping it removes the hook and releases any key it is holding down.
/// Only one can run per process.
pub struct Engine {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    /// Installs the hook. `on_capture` runs on the hook thread when a key capture completes, to
    /// wake the UI (keys are swallowed while capturing, so the UI sees no input of its own).
    pub fn start(on_capture: impl Fn() + Send + 'static) -> io::Result<Self> {
        if RUNNING.swap(true, Relaxed) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "the engine is already running"));
        }
        let (tx, rx) = mpsc::channel();
        let thread = thread::Builder::new().name("remapr-hook".into()).spawn(move || run(tx, Box::new(on_capture)));
        let thread = match thread {
            Ok(t) => t,
            Err(e) => {
                RUNNING.store(false, Relaxed);
                return Err(e);
            }
        };
        match rx.recv() {
            Ok(Ok(thread_id)) => Ok(Self { thread_id, thread: Some(thread) }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err(io::Error::other("the hook thread stopped while starting"))
            }
        }
    }

    /// Replaces every mapping with `pairs` of (from, to).
    pub fn set_mappings(&self, pairs: impl IntoIterator<Item = (Vk, Vk)>) {
        TABLE.iter().for_each(|t| t.store(0, Relaxed));
        for (from, to) in pairs {
            TABLE[usize::from(from)].store(to, Relaxed);
        }
    }

    /// Pauses or resumes remapping. Keys held down when pausing are still released correctly.
    pub fn set_enabled(&self, enabled: bool) {
        ENABLED.store(enabled, Relaxed);
    }

    /// Swallows all keys until one is pressed and released, then reports it via [`Self::captured`].
    pub fn start_capture(&self) {
        CAPTURED.store(0, Relaxed);
        CAPTURING.store(true, Relaxed);
    }

    pub fn cancel_capture(&self) {
        CAPTURING.store(false, Relaxed);
        CAPTURED.store(0, Relaxed);
    }

    /// The key a finished capture caught, once.
    pub fn captured(&self) -> Option<Vk> {
        if CAPTURING.load(Relaxed) {
            return None;
        }
        Some(CAPTURED.swap(0, Relaxed)).filter(|&vk| vk != 0)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: posting to a thread id we own; a failure only means the thread already exited.
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(ready: mpsc::Sender<io::Result<u32>>, on_capture: Box<dyn Fn()>) {
    ON_CAPTURE.set(Some(on_capture));
    // SAFETY: plain Win32 calls; `msg` outlives every call that writes to it.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), GetModuleHandleW(std::ptr::null()), 0);
        if hook.is_null() {
            let _ = ready.send(Err(io::Error::last_os_error()));
            RUNNING.store(false, Relaxed);
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        // Make sure this thread has a message queue before anyone posts WM_QUIT to it.
        PeekMessageW(&mut msg, null_mut(), 0, 0, PM_NOREMOVE);
        let _ = ready.send(Ok(GetCurrentThreadId()));
        // Low-level hooks are delivered while the installing thread waits for messages.
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {}
        UnhookWindowsHookEx(hook);
    }
    HELD.with_borrow_mut(|held| {
        for vk in held.iter_mut().filter(|vk| **vk != 0) {
            send(std::mem::take(vk), true);
        }
    });
    CAPTURING.store(false, Relaxed);
    RUNNING.store(false, Relaxed);
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, `lparam` points to a KBDLLHOOKSTRUCT valid for this call.
        let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let ev = Event {
            vk: kb.vkCode as Vk,
            scan: kb.scanCode,
            up: matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP),
            injected: kb.flags & LLKHF_INJECTED != 0,
        };
        let action = HELD.with_borrow_mut(|held| {
            // A remapped key released mid-capture still has to lift what it pressed.
            let releasing_held = ev.up && held[usize::from(ev.vk)] != 0;
            if CAPTURING.load(Relaxed) && !ev.injected && !releasing_held {
                capture(ev)
            } else {
                decide(ev, ENABLED.load(Relaxed), |vk| TABLE[usize::from(vk)].load(Relaxed), held)
            }
        });
        match action {
            Action::Pass => {}
            Action::Swallow => return 1,
            Action::Send { vk, up } => {
                send(vk, up);
                return 1;
            }
        }
    }
    // SAFETY: forwarding the exact arguments we were given.
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

/// The whole press, down and up, is swallowed, so the captured key never reaches any app.
fn capture(ev: Event) -> Action {
    if is_altgr_phantom(ev) {
        return Action::Swallow;
    }
    if !ev.up {
        let _ = CAPTURED.compare_exchange(0, ev.vk, Relaxed, Relaxed);
    } else if CAPTURED.load(Relaxed) == ev.vk {
        CAPTURING.store(false, Relaxed);
        ON_CAPTURE.with_borrow(|f| f.as_ref().map(|f| f()));
    }
    Action::Swallow
}

fn send(vk: Vk, up: bool) {
    // SAFETY: plain Win32 calls with a fully initialised INPUT.
    unsafe {
        let scan = MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC_EX);
        let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
        // Arrows, Insert/Delete, right Ctrl/Alt… share scan codes with other keys; E0 tells them apart.
        if scan & 0xFF00 == 0xE000 || u16::from(vk) == VK_NUMLOCK {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: u16::from(vk),
                    wScan: (scan & 0xFF) as u16,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(1, &input, size_of::<INPUT>() as i32);
    }
}

/// The character `vk` types on the current keyboard layout (`Ñ` for `VK_OEM_3` on Spanish).
pub(crate) fn layout_char(vk: Vk) -> Option<char> {
    // SAFETY: a pure lookup with no pointers.
    let c = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_CHAR) } & 0xFFFF;
    char::from_u32(c).filter(|c| !c.is_control())
}
