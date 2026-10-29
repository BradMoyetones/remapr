//! The Win32 side: a `WH_KEYBOARD_LL` hook on its own high-priority thread, and `SendInput`.
//!
//! The hook callback runs for every key pressed anywhere on the system, and Windows silently
//! removes hooks that take too long. So all it reads lives on the hook thread itself: no locks, no
//! allocation, no I/O. New rules arrive as a thread message, and the app in front is tracked by a
//! WinEvent hook on the same thread, between key events.

use std::cell::RefCell;
use std::io;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering::Relaxed};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_HIGHEST,
};
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_CHAR,
    MAPVK_VK_TO_VSC_EX, MapVirtualKeyW, SendInput, VK_NUMLOCK,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, EVENT_SYSTEM_FOREGROUND, GetForegroundWindow, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WINEVENT_OUTOFCONTEXT, WM_APP, WM_KEYUP, WM_QUIT, WM_SYSKEYUP,
};

use crate::remap::{Event, Out, State, decide, is_altgr_phantom, modifier_mods};
use crate::{Chord, Mods, Rule, Vk};

static RUNNING: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(true);
static CAPTURING: AtomicBool = AtomicBool::new(false);
/// The captured chord, packed as `mods << 8 | key`; 0 while there is none.
static CAPTURED: AtomicU16 = AtomicU16::new(0);
/// Bumped by each `start_capture`, so the hook thread starts its capture bookkeeping afresh.
static CAPTURE_GENERATION: AtomicU32 = AtomicU32::new(0);
/// Debug builds only: let synthetic keys complete a capture, so the capture flow can be scripted.
static CAPTURE_INJECTED: AtomicBool = AtomicBool::new(false);

/// Carries a `Box<Vec<Rule>>` to the hook thread.
const WM_SET_RULES: u32 = WM_APP + 1;

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };
    static RULES: RefCell<Vec<Rule>> = const { RefCell::new(Vec::new()) };
    /// The executable of the window in front, lowercase (`chrome.exe`).
    static FOREGROUND: RefCell<String> = const { RefCell::new(String::new()) };
    static CAPTURE: RefCell<Capture> = const { RefCell::new(Capture::new(0)) };
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
        CAPTURE_INJECTED
            .store(cfg!(debug_assertions) && std::env::var_os("REMAPR_CAPTURE_INJECTED").is_some(), Relaxed);
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

    /// Replaces every rule. Rules for one app win over global ones, and shortcuts over plain remaps.
    pub fn set_rules(&self, mut rules: Vec<Rule>) {
        for rule in &mut rules {
            if let Some(app) = &mut rule.app {
                app.make_ascii_lowercase();
            }
        }
        rules.sort_by_key(|r| (r.app.is_none(), r.is_plain()));
        let rules = Box::into_raw(Box::new(rules));
        // SAFETY: the hook thread takes the box back; if the post fails, we take it back here.
        unsafe {
            if PostThreadMessageW(self.thread_id, WM_SET_RULES, 0, rules as LPARAM) == 0 {
                drop(Box::from_raw(rules));
            }
        }
    }

    /// Pauses or resumes remapping. Keys held down when pausing are still released correctly.
    pub fn set_enabled(&self, enabled: bool) {
        ENABLED.store(enabled, Relaxed);
    }

    /// Swallows all keys until a key or chord is pressed and every key released again, then
    /// reports it via [`Self::captured`].
    pub fn start_capture(&self) {
        CAPTURED.store(0, Relaxed);
        CAPTURE_GENERATION.fetch_add(1, Relaxed);
        CAPTURING.store(true, Relaxed);
    }

    pub fn cancel_capture(&self) {
        CAPTURING.store(false, Relaxed);
        CAPTURED.store(0, Relaxed);
    }

    /// The chord a finished capture caught, once.
    pub fn captured(&self) -> Option<Chord> {
        if CAPTURING.load(Relaxed) {
            return None;
        }
        let packed = CAPTURED.swap(0, Relaxed);
        (packed != 0).then_some(Chord { mods: Mods((packed >> 8) as u8), key: packed as u8 })
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
    // SAFETY: plain Win32 calls; `msg` outlives every call that writes to it, and the rules box
    // was made by `set_rules`.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), GetModuleHandleW(std::ptr::null()), 0);
        if hook.is_null() {
            let _ = ready.send(Err(io::Error::last_os_error()));
            RUNNING.store(false, Relaxed);
            return;
        }
        // Out of context: delivered through this thread's message loop, between key events.
        let foreground = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            null_mut(),
            Some(on_foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        on_foreground(null_mut(), EVENT_SYSTEM_FOREGROUND, GetForegroundWindow(), 0, 0, 0, 0);
        let mut msg: MSG = std::mem::zeroed();
        // Make sure this thread has a message queue before anyone posts to it.
        PeekMessageW(&mut msg, null_mut(), 0, 0, PM_NOREMOVE);
        let _ = ready.send(Ok(GetCurrentThreadId()));
        // Low-level hooks are delivered while the installing thread waits for messages.
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.message == WM_SET_RULES {
                RULES.set(*Box::from_raw(msg.lParam as *mut Vec<Rule>));
            }
        }
        UnhookWinEvent(foreground);
        UnhookWindowsHookEx(hook);
    }
    let stuck: Vec<(Vk, bool)> = STATE.with_borrow(|s| s.stuck().map(|vk| (vk, true)).collect());
    send_all(&stuck);
    CAPTURING.store(false, Relaxed);
    RUNNING.store(false, Relaxed);
}

unsafe extern "system" fn on_foreground(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    FOREGROUND.set(remapr_platform::exe_name(hwnd as isize).unwrap_or_default());
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
        let out = STATE.with_borrow_mut(|s| {
            // A key remapped before the capture started still has to lift what it pressed.
            let capturable = (!ev.injected || CAPTURE_INJECTED.load(Relaxed)) && !(ev.up && s.is_replacing(ev.vk));
            if CAPTURING.load(Relaxed) && capturable {
                capture(ev);
                return Out::swallow();
            }
            RULES.with_borrow(|rules| FOREGROUND.with_borrow(|app| decide(ev, ENABLED.load(Relaxed), rules, app, s)))
        });
        // Sent outside the borrows: our own synthetic keys come back through this hook.
        send_all(out.sends());
        if out.swallow {
            return 1;
        }
    }
    // SAFETY: forwarding the exact arguments we were given.
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

/// What a capture has seen so far.
struct Capture {
    generation: u32,
    mods: Mods,
    /// The first modifier pressed, which is the capture if no other key comes.
    first_modifier: Vk,
    chord: Option<Chord>,
    down: [bool; 256],
}

impl Capture {
    const fn new(generation: u32) -> Self {
        Self { generation, mods: Mods::NONE, first_modifier: 0, chord: None, down: [false; 256] }
    }
}

/// Collects a key with the modifiers held before it (or a lone modifier) and finishes once every
/// key is up again. All of it is swallowed, so nothing reaches any app.
fn capture(ev: Event) {
    if is_altgr_phantom(ev) {
        return;
    }
    CAPTURE.with_borrow_mut(|c| {
        let generation = CAPTURE_GENERATION.load(Relaxed);
        if c.generation != generation {
            *c = Capture::new(generation);
        }
        let i = usize::from(ev.vk);
        if !ev.up {
            if !c.down[i] && c.chord.is_none() {
                let m = modifier_mods(ev.vk);
                if m.is_empty() {
                    c.chord = Some(Chord { mods: c.mods, key: ev.vk });
                } else {
                    c.mods = c.mods | m;
                    if c.first_modifier == 0 {
                        c.first_modifier = ev.vk;
                    }
                }
            }
            c.down[i] = true;
        } else if c.down[i] {
            c.down[i] = false;
            if c.down.contains(&true) {
                return;
            }
            let lone_modifier = (c.first_modifier != 0).then_some(Chord { mods: Mods::NONE, key: c.first_modifier });
            if let Some(chord) = c.chord.or(lone_modifier) {
                CAPTURED.store(u16::from(chord.mods.0) << 8 | u16::from(chord.key), Relaxed);
                CAPTURING.store(false, Relaxed);
                ON_CAPTURE.with_borrow(|f| f.as_ref().map(|f| f()));
            }
            *c = Capture::new(generation);
        }
    });
}

/// Injects `keys` ((key, is release) pairs) in one go, so nothing can slip in between them.
fn send_all(keys: &[(Vk, bool)]) {
    if keys.is_empty() {
        return;
    }
    // SAFETY: INPUT is plain data; all-zero is a valid value, then filled in below.
    let mut inputs = [unsafe { std::mem::zeroed::<INPUT>() }; 16];
    for (input, &(vk, up)) in inputs.iter_mut().zip(keys) {
        *input = keyboard_input(vk, up);
    }
    let n = keys.len().min(inputs.len());
    // SAFETY: `n` initialised INPUTs.
    unsafe { SendInput(n as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
}

fn keyboard_input(vk: Vk, up: bool) -> INPUT {
    // SAFETY: a pure lookup with no pointers.
    let scan = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC_EX) };
    let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
    // Arrows, Insert/Delete, right Ctrl/Alt… share scan codes with other keys; E0 tells them apart.
    if scan & 0xFF00 == 0xE000 || u16::from(vk) == VK_NUMLOCK {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: u16::from(vk), wScan: (scan & 0xFF) as u16, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    }
}

/// The character `vk` types on the current keyboard layout (`Ñ` for `VK_OEM_3` on Spanish).
pub(crate) fn layout_char(vk: Vk) -> Option<char> {
    // SAFETY: a pure lookup with no pointers.
    let c = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_CHAR) } & 0xFFFF;
    char::from_u32(c).filter(|c| !c.is_control())
}
