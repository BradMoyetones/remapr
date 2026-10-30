//! macOS's own key mapping (`hidutil property --set '{"UserKeyMapping":…}'`), set on a thread of
//! its own so the tap never waits for a process. Remapr owns `UserKeyMapping` while it runs, and
//! clears it when it stops, so no key stays swapped after Remapr is gone:
//! - on a normal stop, when [`Hid`] is dropped;
//! - when a signal ends it (`kill`, Ctrl+C, a logout), from a signal handler, since destructors
//!   don't run then;
//! - after a crash, on the next start: the first mapping set is always applied.

use std::ffi::c_int;
use std::process::Command;
use std::ptr::null;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use remapr_core::Key;

use super::output::user_key_mapping;

pub struct Hid {
    /// The mapping to apply, as `hidutil`'s argument; `None` once stopping.
    sender: Option<mpsc::Sender<String>>,
    thread: Option<JoinHandle<()>>,
    /// The last mapping sent; `None` before the first, so a mapping left by a crashed Remapr goes.
    last: Option<String>,
}

impl Hid {
    pub fn new() -> Self {
        static HANDLERS: Once = Once::new();
        HANDLERS.call_once(clear_on_signals);
        let (sender, receiver) = mpsc::channel::<String>();
        let thread = thread::Builder::new()
            .name("remapr-hid".into())
            .spawn(move || {
                // Only the newest mapping matters when several wait.
                while let Ok(mut mapping) = receiver.recv() {
                    while let Ok(newer) = receiver.try_recv() {
                        mapping = newer;
                    }
                    apply(&mapping);
                }
            })
            .inspect_err(|e| log::error!("no thread for the key mapping: {e}"))
            .ok();
        Self { sender: Some(sender), thread, last: None }
    }

    /// Makes macOS swap the keys of `pairs`, and no others.
    pub fn set(&mut self, pairs: &[(Key, Key)]) {
        let mapping = user_key_mapping(pairs);
        if self.last.as_ref() == Some(&mapping) {
            return;
        }
        if let Some(sender) = &self.sender {
            let _ = sender.send(mapping.clone());
        }
        SWAPPING.store(!pairs.is_empty(), Relaxed);
        self.last = Some(mapping);
    }
}

impl Drop for Hid {
    /// Clears the mapping before returning, so no key stays swapped after Remapr stops.
    fn drop(&mut self) {
        drop(self.sender.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        apply(&user_key_mapping(&[]));
        SWAPPING.store(false, Relaxed);
    }
}

/// Whether a mapping with keys in it is set, for the signal handler.
static SWAPPING: AtomicBool = AtomicBool::new(false);

/// Has the signals that end a process clear the mapping first.
fn clear_on_signals() {
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        // SAFETY: `on_signal` is a valid handler, and does only what a handler may.
        unsafe { libc::signal(signal, on_signal as extern "C" fn(c_int) as libc::sighandler_t) };
    }
}

/// Clears the mapping, then ends the process as `signal` would have. Only async-signal-safe calls:
/// `fork`, `execve`, `waitpid`, `signal` and `raise`, with arguments made beforehand.
extern "C" fn on_signal(signal: c_int) {
    if SWAPPING.load(Relaxed) {
        let argv =
            [c"hidutil".as_ptr(), c"property".as_ptr(), c"--set".as_ptr(), c"{\"UserKeyMapping\":[]}".as_ptr(), null()];
        let envp = [null()];
        // SAFETY: the child only calls execve or _exit; every pointer is to a static string or to
        // these arrays, which outlive the calls.
        unsafe {
            let pid = libc::fork();
            if pid == 0 {
                libc::execve(c"/usr/bin/hidutil".as_ptr(), argv.as_ptr(), envp.as_ptr());
                libc::_exit(127);
            }
            if pid > 0 {
                let mut status = 0;
                libc::waitpid(pid, &mut status, 0);
            }
        }
    }
    // SAFETY: restoring the default action and raising the same signal again.
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
        libc::raise(signal);
    }
}

fn apply(mapping: &str) {
    match Command::new("/usr/bin/hidutil").args(["property", "--set", mapping]).output() {
        Ok(out) if out.status.success() => log::debug!("key mapping set: {mapping}"),
        Ok(out) => log::warn!("hidutil refused the key mapping: {}", String::from_utf8_lossy(&out.stderr).trim()),
        Err(e) => log::warn!("couldn't run hidutil: {e}"),
    }
}
