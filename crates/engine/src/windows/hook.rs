//! The Win32 side: a `WH_KEYBOARD_LL` hook on its own high-priority thread, and `SendInput`.
//!
//! The hook callback runs for every key pressed anywhere on the system, and Windows silently
//! removes hooks that take too long. So all it reads lives on the hook thread itself: no locks, no
//! allocation, no I/O. New rules arrive as a thread message, and the app in front is tracked by a
//! WinEvent hook on the same thread, between key events.
//!
//! Each event becomes a [`Key`] through [`keys`] and goes to [`decide`]; Windows' own quirks stay
//! here: AltGr's fake Left Ctrl, and the dummy tap before lifting Alt or Win ([`output`]).
//!
//! The app sees an [`Engine`] as a [`Backend`]. Nothing is logged from the hook callback: only when
//! the hook thread starts and stops.

use std::cell::RefCell;
use std::io;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering::Relaxed};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use remapr_core::{
    App, Backend, Capabilities, Chord, Command, Event, Key, KeyEvent, Mods, Out, Rule, State, decide, remaps_plainly,
    sort_rules,
};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_HIGHEST,
};
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    MAPVK_VK_TO_VSC_EX, MapVirtualKeyW, SendInput,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, EVENT_SYSTEM_FOREGROUND, GetForegroundWindow, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT,
    LLKHF_EXTENDED, LLKHF_INJECTED, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WINEVENT_OUTOFCONTEXT, WM_APP, WM_KEYUP, WM_QUIT, WM_SYSKEYUP,
};

use super::keys::{self, VK_DUMMY, VK_NUMLOCK};
use super::output::{self, Stroke};

static RUNNING: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(true);
static CAPTURING: AtomicBool = AtomicBool::new(false);
/// The captured chord, packed as `mods << 8 | key index`; 0 while there is none.
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
    /// Tells the app there are events to poll.
    static WAKE: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// The running hook. Dropping it removes the hook and releases any key it is holding down.
/// Only one can run per process.
pub struct Engine {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
    /// Events the hook thread reports outside key events (the hook stopping).
    events: mpsc::Receiver<Event>,
}

impl Engine {
    /// Installs the hook. `wake` runs on the hook thread when there are events to poll: a finished
    /// capture (keys are swallowed while capturing, so the app sees no input of its own).
    pub fn start(wake: impl Fn() + Send + 'static) -> io::Result<Self> {
        if RUNNING.swap(true, Relaxed) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "the engine is already running"));
        }
        CAPTURE_INJECTED
            .store(cfg!(debug_assertions) && std::env::var_os("REMAPR_CAPTURE_INJECTED").is_some(), Relaxed);
        let (tx, rx) = mpsc::channel();
        let (events_tx, events) = mpsc::channel();
        let thread =
            thread::Builder::new().name("remapr-hook".into()).spawn(move || run(tx, events_tx, Box::new(wake)));
        let thread = match thread {
            Ok(t) => t,
            Err(e) => {
                RUNNING.store(false, Relaxed);
                return Err(e);
            }
        };
        match rx.recv() {
            Ok(Ok(thread_id)) => {
                log::info!("keyboard hook installed");
                Ok(Self { thread_id, thread: Some(thread), events })
            }
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
    fn set_rules(&self, mut rules: Vec<Rule>) {
        log::debug!("{} rules", rules.len());
        sort_rules(&mut rules);
        let rules = Box::into_raw(Box::new(rules));
        // SAFETY: the hook thread takes the box back; if the post fails, we take it back here.
        unsafe {
            if PostThreadMessageW(self.thread_id, WM_SET_RULES, 0, rules as LPARAM) == 0 {
                drop(Box::from_raw(rules));
            }
        }
    }

    fn start_capture(&self) {
        CAPTURED.store(0, Relaxed);
        CAPTURE_GENERATION.fetch_add(1, Relaxed);
        CAPTURING.store(true, Relaxed);
    }

    fn cancel_capture(&self) {
        CAPTURING.store(false, Relaxed);
        CAPTURED.store(0, Relaxed);
    }

    /// The chord a finished capture caught, once.
    fn captured(&self) -> Option<Chord> {
        if CAPTURING.load(Relaxed) {
            return None;
        }
        let packed = CAPTURED.swap(0, Relaxed);
        let key = Key::from_index(packed as u8)?;
        Some(Chord { mods: Mods((packed >> 8) as u8), key })
    }
}

impl Backend for Engine {
    fn send(&mut self, command: Command) {
        match command {
            Command::SetRules(rules) => self.set_rules(rules),
            // Keys held down when pausing are still released correctly: `decide` tracks them.
            Command::SetEnabled(enabled) => ENABLED.store(enabled, Relaxed),
            Command::StartCapture => self.start_capture(),
            Command::CancelCapture => self.cancel_capture(),
        }
    }

    fn poll(&mut self) -> Vec<Event> {
        let captured = self.captured().map(Event::Captured);
        captured.into_iter().chain(self.events.try_iter()).collect()
    }

    fn capabilities(&self) -> Capabilities {
        super::CAPABILITIES
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: posting to a thread id we own; a failure only means the thread already exited.
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        log::info!("keyboard hook removed");
    }
}

fn run(ready: mpsc::Sender<io::Result<u32>>, events: mpsc::Sender<Event>, wake: Box<dyn Fn()>) {
    WAKE.set(Some(wake));
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
        // Low-level hooks are delivered while the installing thread waits for messages. 0 is
        // WM_QUIT, from `Drop`; -1 a failure of the message loop, which ends the hook too.
        let failure = loop {
            match GetMessageW(&mut msg, null_mut(), 0, 0) {
                0 => break None,
                -1 => break Some(io::Error::last_os_error()),
                _ if msg.message == WM_SET_RULES => RULES.set(*Box::from_raw(msg.lParam as *mut Vec<Rule>)),
                _ => {}
            }
        };
        UnhookWinEvent(foreground);
        UnhookWindowsHookEx(hook);
        if let Some(e) = failure {
            log::error!("the keyboard hook stopped: {e}");
            let _ = events.send(Event::Stopped(e.to_string()));
            WAKE.with_borrow(|wake| wake.as_ref().map(|wake| wake()));
        }
    }
    let stuck: Vec<Stroke> = STATE.with_borrow(|s| s.stuck().map(|key| Stroke::Key { key, up: true }).collect());
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
        let out = handle(kb, matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP));
        // Sent outside the borrows: our own synthetic keys come back through this hook.
        send_all(output::strokes(out.sends().iter().copied()).as_slice());
        if out.swallow {
            return 1;
        }
    }
    // SAFETY: forwarding the exact arguments we were given.
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

/// What to do with one key event of the hook.
fn handle(kb: &KBDLLHOOKSTRUCT, up: bool) -> Out {
    let (vk, injected) = (kb.vkCode as u8, kb.flags & LLKHF_INJECTED != 0);
    let capturing = CAPTURING.load(Relaxed);
    let enabled = ENABLED.load(Relaxed);
    if keys::is_altgr_phantom(vk, kb.scanCode) {
        // Not a real Ctrl: never counted, captured or remapped as one. A remapped AltGr would
        // otherwise arrive with a phantom Ctrl held.
        let altgr_remapped =
            || RULES.with_borrow(|r| FOREGROUND.with_borrow(|app| remaps_plainly(r, App::Exe(app), Key::AltRight)));
        return if capturing || (enabled && altgr_remapped()) { Out::swallow() } else { Out::default() };
    }
    let Some(key) = keys::key_of(vk, kb.scanCode as u16, kb.flags & LLKHF_EXTENDED != 0) else {
        // A key Remapr doesn't know can't be mapped; while capturing, it still reaches no app.
        return if capturing && !injected { Out::swallow() } else { Out::default() };
    };
    let ev = KeyEvent { key, up, injected };
    STATE.with_borrow_mut(|s| {
        // A key remapped before the capture started still has to lift what it pressed.
        let capturable = (!ev.injected || CAPTURE_INJECTED.load(Relaxed)) && !(ev.up && s.is_replacing(ev.key));
        if capturing && capturable {
            capture(ev);
            return Out::swallow();
        }
        RULES.with_borrow(|rules| FOREGROUND.with_borrow(|app| decide(ev, enabled, rules, App::Exe(app), s)))
    })
}

/// What a capture has seen so far.
struct Capture {
    generation: u32,
    mods: Mods,
    /// The first modifier pressed, which is the capture if no other key comes.
    first_modifier: Option<Key>,
    chord: Option<Chord>,
    down: [bool; 256],
}

impl Capture {
    const fn new(generation: u32) -> Self {
        Self { generation, mods: Mods::NONE, first_modifier: None, chord: None, down: [false; 256] }
    }
}

/// Collects a key with the modifiers held before it (or a lone modifier) and finishes once every
/// key is up again. All of it is swallowed, so nothing reaches any app.
fn capture(ev: KeyEvent) {
    CAPTURE.with_borrow_mut(|c| {
        let generation = CAPTURE_GENERATION.load(Relaxed);
        if c.generation != generation {
            *c = Capture::new(generation);
        }
        let i = usize::from(ev.key.index());
        if !ev.up {
            if !c.down[i] && c.chord.is_none() {
                let m = ev.key.modifier();
                if m.is_empty() {
                    c.chord = Some(Chord { mods: c.mods, key: ev.key });
                } else {
                    c.mods = c.mods | m;
                    c.first_modifier.get_or_insert(ev.key);
                }
            }
            c.down[i] = true;
        } else if c.down[i] {
            c.down[i] = false;
            if c.down.contains(&true) {
                return;
            }
            if let Some(chord) = c.chord.or(c.first_modifier.map(Chord::key)) {
                CAPTURED.store(u16::from(chord.mods.0) << 8 | u16::from(chord.key.index()), Relaxed);
                CAPTURING.store(false, Relaxed);
                WAKE.with_borrow(|wake| wake.as_ref().map(|wake| wake()));
            }
            *c = Capture::new(generation);
        }
    });
}

/// Injects `strokes` in one go per batch, so nothing can slip in between them.
fn send_all(strokes: &[Stroke]) {
    for batch in strokes.chunks(output::MAX) {
        // SAFETY: INPUT is plain data; all-zero is a valid value, then filled in below.
        let mut inputs = [unsafe { std::mem::zeroed::<INPUT>() }; output::MAX];
        let mut n = 0;
        for input in batch.iter().filter_map(|&s| keyboard_input(s)) {
            inputs[n] = input;
            n += 1;
        }
        if n > 0 {
            // SAFETY: `n` initialised INPUTs.
            unsafe { SendInput(n as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
        }
    }
}

fn keyboard_input(stroke: Stroke) -> Option<INPUT> {
    let (key, up) = match stroke {
        Stroke::Dummy { up } => return Some(input(u16::from(VK_DUMMY), 0, up, 0)),
        Stroke::Key { key, up } => (key, up),
    };
    if let Some(scan) = keys::char_scan(key) {
        // A character key, by position: Windows takes its virtual-key code from the layout of the
        // app it reaches, as for a real key.
        return Some(input(0, scan, up, KEYEVENTF_SCANCODE));
    }
    let (vk, extended) = keys::vk_of(key)?;
    // SAFETY: a pure lookup with no pointers.
    let scan = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC_EX) };
    // Arrows, Insert/Delete, right Ctrl/Alt… share scan codes with other keys; E0 tells them apart.
    let extended = extended.unwrap_or(scan & 0xFF00 == 0xE000) || vk == VK_NUMLOCK;
    Some(input(u16::from(vk), (scan & 0xFF) as u16, up, if extended { KEYEVENTF_EXTENDEDKEY } else { 0 }))
}

fn input(vk: u16, scan: u16, up: bool, flags: u32) -> INPUT {
    let flags = flags | if up { KEYEVENTF_KEYUP } else { 0 };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}
