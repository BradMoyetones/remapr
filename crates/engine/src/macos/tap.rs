//! The macOS side: a `CGEventTap` on a thread of its own, with its own run loop, and
//! `CGEventPost`. Commands reach that thread through a run loop source, between key events; a
//! timer keeps the app in front, Secure Input and the permission to change keys up to date.
//!
//! The tap's callback runs for every key on the system, and macOS turns off a tap that keeps it
//! waiting (it is turned back on here). So it reads only this thread's own state and takes no
//! locks; posting a key does make a `CGEvent`, which macOS allocates. Without the Accessibility
//! permission there is no tap: global key swaps still work through macOS's own key mapping
//! ([`super::output::split`]), and the tap starts once the permission is granted.

use std::cell::RefCell;
use std::ffi::c_void;
use std::io;
use std::ptr::{NonNull, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering::Relaxed};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext,
    CFRunLoopTimer, kCFRunLoopCommonModes,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy,
    CGEventType,
};
use remapr_core::{
    App, Backend, Capabilities, Capture, Chord, Command, Edge, Event, Key, KeyEvent, Mods, Out, Permission, Rule,
    State, Support, decide, sort_rules,
};

use super::hid::Hid;
use super::keys;
use super::output::{self, Injector, Kind, Strokes};
use super::system;

/// In `kCGEventSourceUserData` of every event Remapr posts, so its own tap lets them through.
const MARKER: i64 = 0x5245_4D50;
/// How often the timer looks at the app in front; permissions and Secure Input every fourth time.
const TICK_SECONDS: f64 = 0.25;

static RUNNING: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(true);
static CAPTURING: AtomicBool = AtomicBool::new(false);
/// The captured chord, packed as `mods << 8 | key index`; 0 while there is none.
static CAPTURED: AtomicU16 = AtomicU16::new(0);
/// Bumped by each capture, so the tap thread starts its capture bookkeeping afresh.
static CAPTURE_GENERATION: AtomicU32 = AtomicU32::new(0);
/// Whether the tap is installed: the Accessibility permission was granted.
static TAP_ON: AtomicBool = AtomicBool::new(false);

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };
    static RULES: RefCell<Vec<Rule>> = const { RefCell::new(Vec::new()) };
    /// The bundle identifier of the app in front.
    static FRONT: RefCell<String> = const { RefCell::new(String::new()) };
    /// The capture under way, and the `CAPTURE_GENERATION` it belongs to.
    static CAPTURE: RefCell<(u32, Capture)> = const { RefCell::new((0, Capture::new())) };
    static INJECTOR: RefCell<Injector> = const { RefCell::new(Injector::new()) };
    static TAP: RefCell<Option<CFRetained<CFMachPort>>> = const { RefCell::new(None) };
    /// The commands source, signalled from the callback once a capture ends.
    static SOURCE: RefCell<Option<CFRetained<CFRunLoopSource>>> = const { RefCell::new(None) };
    static THREAD: RefCell<Option<TapThread>> = const { RefCell::new(None) };
}

/// What the tap thread owns besides the state its callback reads.
struct TapThread {
    commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
    /// Tells the app there are events to poll.
    wake: Box<dyn Fn()>,
    hid: Hid,
    /// The rules macOS's key mapping takes.
    pairs: Vec<(Key, Key)>,
    secure_input: bool,
    ticks: u32,
}

impl TapThread {
    fn report(&self, event: Event) {
        let _ = self.events.send(event);
        (self.wake)();
    }
}

/// The tap thread's run loop and commands source, as other threads use them.
struct Shared {
    run_loop: CFRetained<CFRunLoop>,
    source: CFRetained<CFRunLoopSource>,
}

// SAFETY: run loops and their sources are thread-safe for what other threads do with them here:
// signalling the source, waking and stopping the run loop.
unsafe impl Send for Shared {}

/// What this system's backend can do now.
pub fn capabilities() -> Capabilities {
    let keyboard =
        if TAP_ON.load(Relaxed) { Support::Yes } else { Support::NeedsPermission(Permission::Accessibility) };
    Capabilities {
        keyboard,
        per_app_rules: keyboard,
        mouse: Support::NotYet,
        controllers: Support::NotYet,
        // Apple lets no app make virtual game controllers.
        virtual_controllers: Support::NotOnThisSystem,
        hide_controllers: Support::NotOnThisSystem,
    }
}

/// The running tap thread. Dropping it stops the tap, releases any key it holds down and clears
/// macOS's key mapping. Only one can run per process.
pub struct Engine {
    shared: Shared,
    commands: mpsc::Sender<Command>,
    events: mpsc::Receiver<Event>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    /// Starts the tap thread. It runs without the Accessibility permission too, until it is
    /// granted. `wake` runs on that thread when there are events to poll.
    pub fn start(wake: impl Fn() + Send + 'static) -> io::Result<Self> {
        if RUNNING.swap(true, Relaxed) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "the engine is already running"));
        }
        let (ready_tx, ready) = mpsc::channel();
        let (commands, commands_rx) = mpsc::channel();
        let (events_tx, events) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("remapr-tap".into())
            .spawn(move || run(ready_tx, commands_rx, events_tx, Box::new(wake)));
        let thread = match thread {
            Ok(t) => t,
            Err(e) => {
                RUNNING.store(false, Relaxed);
                return Err(e);
            }
        };
        match ready.recv() {
            Ok(Ok(shared)) => Ok(Self { shared, commands, events, thread: Some(thread) }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err(io::Error::other("the tap thread stopped while starting"))
            }
        }
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
        if self.commands.send(command).is_ok() {
            self.shared.source.signal();
            self.shared.run_loop.wake_up();
        }
    }

    fn poll(&mut self) -> Vec<Event> {
        let captured = self.captured().map(Event::Captured);
        captured.into_iter().chain(self.events.try_iter()).collect()
    }

    fn capabilities(&self) -> Capabilities {
        capabilities()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shared.run_loop.stop();
        self.shared.run_loop.wake_up();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    ready: mpsc::Sender<io::Result<Shared>>,
    commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
    wake: Box<dyn Fn()>,
) {
    let Some(run_loop) = CFRunLoop::current() else {
        let _ = ready.send(Err(io::Error::other("no run loop for the tap thread")));
        RUNNING.store(false, Relaxed);
        return;
    };
    // SAFETY: a static the framework defines.
    let modes = unsafe { kCFRunLoopCommonModes };
    let mut context = CFRunLoopSourceContext {
        version: 0,
        info: null_mut(),
        retain: None,
        release: None,
        copyDescription: None,
        equal: None,
        hash: None,
        schedule: None,
        cancel: None,
        perform: Some(perform),
    };
    // SAFETY: the context is copied when the source is made, and `perform` reads no `info`.
    let source = unsafe { CFRunLoopSource::new(None, 0, &mut context) };
    let first_tick = CFAbsoluteTimeGetCurrent() + TICK_SECONDS;
    // SAFETY: `tick` has the timer callback's signature and reads no context.
    let timer = unsafe { CFRunLoopTimer::new(None, first_tick, TICK_SECONDS, 0, 0, Some(tick), null_mut()) };
    let (Some(source), Some(timer)) = (source, timer) else {
        let _ = ready.send(Err(io::Error::other("couldn't set up the tap thread's run loop")));
        RUNNING.store(false, Relaxed);
        return;
    };
    run_loop.add_source(Some(&source), modes);
    run_loop.add_timer(Some(&timer), modes);
    SOURCE.set(Some(source.clone()));
    FRONT.set(system::frontmost_app().unwrap_or_default());
    let secure_input = system::secure_input();
    THREAD.set(Some(TapThread { commands, events, wake, hid: Hid::new(), pairs: Vec::new(), secure_input, ticks: 0 }));
    if !start_tap(&run_loop) {
        log::info!("no event tap yet: waiting for the Accessibility permission");
    }
    let _ = ready.send(Ok(Shared { run_loop: run_loop.clone(), source }));

    CFRunLoop::run();

    // Stopped: let go of what the tap holds down, then of the tap and macOS's key mapping.
    let stuck: Vec<(Key, Edge)> = STATE.with_borrow(|s| s.stuck().map(|key| (key, Edge::Release)).collect());
    let mut strokes = Strokes::new();
    INJECTOR.with_borrow_mut(|i| i.strokes(&mut strokes, &stuck, 0));
    post_all(&strokes);
    if let Some(tap) = TAP.take() {
        CGEvent::tap_enable(&tap, false);
        log::info!("event tap removed");
    }
    TAP_ON.store(false, Relaxed);
    THREAD.set(None);
    SOURCE.set(None);
    CAPTURING.store(false, Relaxed);
    RUNNING.store(false, Relaxed);
}

/// Installs the tap on `run_loop`, if macOS lets Remapr change keys.
fn start_tap(run_loop: &CFRunLoop) -> bool {
    if !system::may_change_keys() {
        return false;
    }
    let mask =
        [CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged].iter().fold(0, |m, t| m | 1 << t.0);
    // SAFETY: `callback` has the tap callback's signature and reads no `user_info`.
    let tap = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            mask,
            Some(callback),
            null_mut(),
        )
    };
    let Some(tap) = tap else {
        log::warn!("macOS refused the event tap");
        return false;
    };
    let Some(source) = CFMachPort::new_run_loop_source(None, Some(&tap), 0) else { return false };
    // SAFETY: a static the framework defines.
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
    CGEvent::tap_enable(&tap, true);
    TAP.set(Some(tap));
    TAP_ON.store(true, Relaxed);
    log::info!("event tap installed");
    true
}

/// Takes the commands sent since the last call: rules, pause, capture, permission prompts.
unsafe extern "C-unwind" fn perform(_info: *mut c_void) {
    THREAD.with_borrow_mut(|thread| {
        let Some(t) = thread else { return };
        while let Ok(command) = t.commands.try_recv() {
            match command {
                Command::SetRules(rules) => {
                    let (pairs, mut tap) = output::split(&rules);
                    log::debug!(
                        "{} rules: {} to macOS's key mapping, {} to the tap",
                        rules.len(),
                        pairs.len(),
                        tap.len()
                    );
                    sort_rules(&mut tap);
                    RULES.set(tap);
                    t.pairs = pairs;
                }
                Command::SetEnabled(enabled) => ENABLED.store(enabled, Relaxed),
                Command::StartCapture => {
                    CAPTURED.store(0, Relaxed);
                    CAPTURE_GENERATION.fetch_add(1, Relaxed);
                    CAPTURING.store(true, Relaxed);
                }
                Command::CancelCapture => {
                    CAPTURING.store(false, Relaxed);
                    CAPTURED.store(0, Relaxed);
                }
                Command::AskPermission(permission) => system::ask(permission),
            }
        }
        // macOS's mapping is off while paused, and while capturing, so a capture sees the keys
        // themselves (Caps Lock, not the Esc it was swapped for).
        let on = ENABLED.load(Relaxed) && !CAPTURING.load(Relaxed);
        t.hid.set(if on { &t.pairs } else { &[] });
    });
}

/// Looks at the app in front; every fourth time, at the permission and Secure Input too.
unsafe extern "C-unwind" fn tick(_timer: *mut CFRunLoopTimer, _info: *mut c_void) {
    FRONT.set(system::frontmost_app().unwrap_or_default());
    THREAD.with_borrow_mut(|thread| {
        let Some(t) = thread else { return };
        t.ticks = t.ticks.wrapping_add(1);
        if t.ticks % 4 != 0 {
            return;
        }
        if !TAP_ON.load(Relaxed) && CFRunLoop::current().is_some_and(|run_loop| start_tap(&run_loop)) {
            t.report(Event::CapabilitiesChanged(capabilities()));
        }
        let secure_input = system::secure_input();
        if secure_input != t.secure_input {
            t.secure_input = secure_input;
            log::info!("Secure Input {}", if secure_input { "on" } else { "off" });
            t.report(Event::Blocked(secure_input));
        }
    });
}

unsafe extern "C-unwind" fn callback(
    _proxy: CGEventTapProxy,
    ty: CGEventType,
    event: NonNull<CGEvent>,
    _info: *mut c_void,
) -> *mut CGEvent {
    if ty == CGEventType::TapDisabledByTimeout || ty == CGEventType::TapDisabledByUserInput {
        // macOS turned the tap off for being slow, or for a moment of user input: back on.
        TAP.with_borrow(|tap| tap.as_ref().map(|tap| CGEvent::tap_enable(tap, true)));
        return event.as_ptr();
    }
    // SAFETY: macOS hands an event valid for the duration of the call.
    let swallow = handle(unsafe { event.as_ref() }, ty);
    if swallow { null_mut() } else { event.as_ptr() }
}

/// What to do with one event of the tap: whether to swallow it. Posts what replaces it.
fn handle(event: &CGEvent, ty: CGEventType) -> bool {
    if !(ty == CGEventType::KeyDown || ty == CGEventType::KeyUp || ty == CGEventType::FlagsChanged) {
        return false;
    }
    if CGEvent::integer_value_field(Some(event), CGEventField::EventSourceUserData) == MARKER {
        return false;
    }
    let capturing = CAPTURING.load(Relaxed);
    let keycode = CGEvent::integer_value_field(Some(event), CGEventField::KeyboardEventKeycode) as u16;
    let Some(key) = keys::key_of(keycode) else {
        // A key Remapr doesn't know (Fn) can't be mapped; while capturing, it still reaches no app.
        return capturing;
    };
    let flags = CGEvent::flags(Some(event)).0;
    let press = KeyEvent { key, up: false, injected: false };
    let release = KeyEvent { up: true, ..press };
    // Caps Lock's events say its lock changed, once per press, with no release: a tap of the key.
    let (events, count) = match ty {
        CGEventType::FlagsChanged if key == Key::CapsLock => ([press, release], 2),
        CGEventType::FlagsChanged => ([KeyEvent { up: !keys::is_down(key, flags), ..press }, release], 1),
        CGEventType::KeyUp => ([release, release], 1),
        _ => ([press, release], 1),
    };
    let (mut strokes, mut swallow) = (Strokes::new(), false);
    for &ev in &events[..count] {
        let out = STATE.with_borrow_mut(|s| {
            // A key remapped before the capture started still has to lift what it pressed.
            if capturing && !(ev.up && s.is_replacing(ev.key)) {
                capture(ev);
                return Out::swallow();
            }
            let enabled = ENABLED.load(Relaxed);
            RULES.with_borrow(|rules| FRONT.with_borrow(|app| decide(ev, enabled, rules, App::BundleId(app), s)))
        });
        INJECTOR.with_borrow_mut(|i| i.strokes(&mut strokes, out.sends(), flags & keys::ALPHA_SHIFT));
        swallow |= out.swallow;
    }
    if !swallow && key != Key::CapsLock {
        // It reaches apps: a modifier changes what they see held, and every key carries the
        // modifiers they see, Remapr's included.
        INJECTOR.with_borrow_mut(|i| {
            if ty == CGEventType::FlagsChanged {
                i.observe(key, !events[0].up);
            }
            CGEvent::set_flags(Some(event), CGEventFlags(i.pass(flags)));
        });
    }
    // One key for one key of the same kind (a plain remap, a repeat): the event itself becomes it.
    // A posted event would queue behind keys already waiting, and could reach apps after them.
    if let [stroke] = strokes.as_slice()
        && swallow
        && stroke.kind == if ty == CGEventType::KeyUp { Kind::Up } else { Kind::Down }
        && ty != CGEventType::FlagsChanged
    {
        CGEvent::set_integer_value_field(Some(event), CGEventField::KeyboardEventKeycode, i64::from(stroke.keycode));
        CGEvent::set_flags(Some(event), CGEventFlags(stroke.flags));
        return false;
    }
    post_all(&strokes);
    swallow
}

/// Feeds the capture under way; once it catches a chord, stores it for [`Engine::captured`], has
/// the commands source put macOS's key mapping back, and wakes the app.
fn capture(ev: KeyEvent) {
    CAPTURE.with_borrow_mut(|(generation, capture)| {
        let current = CAPTURE_GENERATION.load(Relaxed);
        if *generation != current {
            *generation = current;
            *capture = Capture::new();
        }
        if let Some(chord) = capture.feed(ev) {
            CAPTURED.store(u16::from(chord.mods.0) << 8 | u16::from(chord.key.index()), Relaxed);
            CAPTURING.store(false, Relaxed);
            SOURCE.with_borrow(|source| source.as_ref().map(|s| s.signal()));
            THREAD.with_borrow(|thread| thread.as_ref().map(|t| (t.wake)()));
        }
    });
}

/// Posts `strokes` behind the event the tap is on, marked as Remapr's.
fn post_all(strokes: &Strokes) {
    for stroke in strokes.as_slice() {
        let Some(event) = CGEvent::new_keyboard_event(None, stroke.keycode, stroke.kind != Kind::Up) else { continue };
        if stroke.kind == Kind::FlagsChanged {
            CGEvent::set_type(Some(&event), CGEventType::FlagsChanged);
        }
        CGEvent::set_flags(Some(&event), CGEventFlags(stroke.flags));
        CGEvent::set_integer_value_field(Some(&event), CGEventField::EventSourceUserData, MARKER);
        CGEvent::post(CGEventTapLocation::SessionEventTap, Some(&event));
    }
}
