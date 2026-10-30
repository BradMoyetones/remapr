//! The mappings and what happens to them, apart from the window: the settings, the engine backend,
//! adding a mapping by pressing keys, and the notices about what happened. The window draws a
//! `Session`, forwards clicks to it and saves its settings when they change; tests drive one with a
//! fake backend.

use std::io;

use remapr_core::{AppMatcher, Backend, Capabilities, Chord, Command, Event, Support};
use remapr_ui::Tone;

use crate::config::{Config, Mapping};
use crate::i18n::{tr, trn};
use crate::keys::chord_text;

/// Adding a mapping: first the key to change, then the key it should send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    Idle,
    From,
    To(Chord),
}

/// A message above the mappings, until the user closes it or the next one comes.
#[derive(Clone, Debug)]
pub struct Notice {
    pub text: String,
    /// A deleted mapping and where it was, so "Deshacer" can put it back.
    pub undo: Option<(usize, Mapping)>,
    /// A page to open from a "Novedades" button.
    pub link: Option<String>,
}

impl Notice {
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), undo: None, link: None }
    }
}

pub struct Session {
    /// The settings. The window reads them; changes go through the methods here, or are followed by
    /// [`Session::changed`] for what the window owns (the theme, the tray hint).
    pub config: Config,
    engine: Option<Box<dyn Backend>>,
    capabilities: Capabilities,
    /// Why keys aren't remapped, when they aren't: shown as a banner.
    status: Option<(Tone, String)>,
    capture: Capture,
    pub notice: Option<Notice>,
    /// The settings changed since the window last saved them.
    unsaved: bool,
}

impl Session {
    /// `engine` is the started backend, or why it couldn't start; `capabilities` are what this
    /// system's backend can do, for when it didn't.
    pub fn new(config: Config, engine: io::Result<Box<dyn Backend>>, capabilities: Capabilities) -> Self {
        let (engine, status) = match engine {
            Ok(engine) => (Some(engine), None),
            Err(e) if e.kind() == io::ErrorKind::Unsupported => {
                let text = tr!("Remapr can't remap keys on this system yet: for now it only works on Windows.");
                (None, Some((Tone::Info, text.to_owned())))
            }
            Err(e) => (None, Some((Tone::Danger, tr!("Couldn't start the keyboard engine: {error}", error = e)))),
        };
        let capabilities = engine.as_ref().map_or(capabilities, |e| e.capabilities());
        let mut session =
            Self { config, engine, capabilities, status, capture: Capture::Idle, notice: None, unsaved: false };
        session.sync();
        session
    }

    pub fn capture(&self) -> Capture {
        self.capture
    }

    /// Why keys aren't remapped, if they aren't.
    pub fn status(&self) -> Option<&(Tone, String)> {
        self.status.as_ref()
    }

    /// Whether a backend is running: keys are remapped, unless paused.
    pub fn is_running(&self) -> bool {
        self.engine.is_some()
    }

    /// Whether a mapping can be added by pressing keys.
    pub fn can_capture(&self) -> bool {
        self.engine.is_some() && self.capabilities.keyboard == Support::Yes
    }

    /// Whether mappings can be limited to one app.
    pub fn per_app_rules(&self) -> bool {
        self.capabilities.per_app_rules == Support::Yes
    }

    fn send(&mut self, command: Command) {
        if let Some(engine) = &mut self.engine {
            engine.send(command);
        }
    }

    /// Shows the settings' state in the engine.
    fn sync(&mut self) {
        let (enabled, rules) = (self.config.enabled, self.config.mappings.iter().map(Mapping::rule).collect());
        self.send(Command::SetEnabled(enabled));
        self.send(Command::SetRules(rules));
    }

    /// The settings changed: the engine follows them, and they need saving.
    pub fn changed(&mut self) {
        self.sync();
        self.unsaved = true;
    }

    /// Whether the settings changed since the last call, so the window saves them.
    pub fn take_unsaved(&mut self) -> bool {
        std::mem::take(&mut self.unsaved)
    }

    /// Takes in what the engine reports.
    pub fn poll(&mut self) {
        let events = self.engine.as_mut().map(|e| e.poll()).unwrap_or_default();
        for event in events {
            match event {
                Event::Captured(chord) => self.on_captured(chord),
                Event::CapabilitiesChanged(capabilities) => self.capabilities = capabilities,
                Event::Stopped(why) => {
                    log::error!("the engine stopped: {why}");
                    self.engine = None;
                    self.capture = Capture::Idle;
                    self.status = Some((Tone::Danger, tr!("The keyboard engine stopped: {error}", error = why)));
                }
            }
        }
    }

    pub fn start_capture(&mut self) {
        if self.can_capture() {
            self.send(Command::StartCapture);
            self.capture = Capture::From;
            self.notice = None;
        }
    }

    pub fn cancel_capture(&mut self) {
        if self.capture != Capture::Idle {
            self.send(Command::CancelCapture);
            self.capture = Capture::Idle;
        }
    }

    fn on_captured(&mut self, chord: Chord) {
        match self.capture {
            Capture::Idle => {}
            Capture::From => {
                self.capture = Capture::To(chord);
                self.send(Command::StartCapture);
            }
            Capture::To(from) => {
                self.capture = Capture::Idle;
                self.add(Mapping::new(from, chord));
            }
        }
    }

    /// Adds `mapping`; one on the same keys in the same place gets its target instead.
    pub fn add(&mut self, mapping: Mapping) {
        let (from, to) = (chord_text(mapping.from), chord_text(mapping.to));
        if mapping.from == mapping.to {
            self.notice = Some(Notice::text(tr!("You chose {keys} twice, so nothing changed.", keys = from)));
            return;
        }
        if let Some(existing) = self.config.mappings.iter_mut().find(|m| m.clashes_with(&mapping)) {
            existing.to = mapping.to;
            let text = tr!("{from} was already remapped. It now sends {to}.", from = from, to = to);
            self.notice = Some(Notice::text(text));
        } else {
            self.config.mappings.push(mapping);
        }
        self.changed();
    }

    pub fn remove(&mut self, index: usize) {
        let m = self.config.mappings.remove(index);
        let text = tr!("Removed {from} → {to}.", from = chord_text(m.from), to = chord_text(m.to));
        self.notice = Some(Notice { text, undo: Some((index, m)), link: None });
        self.changed();
    }

    /// Puts back a removed mapping where it was, unless one on the same keys took its place.
    pub fn undo(&mut self, index: usize, mapping: Mapping) {
        self.notice = None;
        if self.config.mappings.iter().all(|m| !m.clashes_with(&mapping)) {
            let index = index.min(self.config.mappings.len());
            self.config.mappings.insert(index, mapping);
            self.changed();
        }
    }

    /// Limits mapping `index` to `app`, or lets it apply everywhere with `None`.
    pub fn set_app(&mut self, index: usize, app: Option<AppMatcher>) {
        let mut moved = self.config.mappings[index].clone();
        moved.app = app;
        // Another mapping of the same keys already there: this one replaces it.
        if let Some(other) =
            (0..self.config.mappings.len()).find(|&i| i != index && self.config.mappings[i].clashes_with(&moved))
        {
            self.config.mappings.remove(other);
            let index = if other < index { index - 1 } else { index };
            self.config.mappings[index] = moved;
        } else {
            self.config.mappings[index] = moved;
        }
        self.changed();
    }

    /// Pauses or resumes remapping.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
        self.changed();
    }

    /// Adds the mappings of an export file; one that fires on the same keys in the same place
    /// replaces the one already here.
    pub fn import(&mut self, mappings: Vec<Mapping>) {
        let (total, mut replaced) = (mappings.len(), 0);
        for mapping in mappings {
            if let Some(existing) = self.config.mappings.iter_mut().find(|m| m.clashes_with(&mapping)) {
                *existing = mapping;
                replaced += 1;
            } else {
                self.config.mappings.push(mapping);
            }
        }
        self.changed();
        let total = trn!("{n} mapping|{n} mappings", total);
        let text = match replaced {
            0 => tr!("Imported {count}.", count = total),
            1 => tr!("Imported {count}; one replaced the mapping you had on the same keys.", count = total),
            n => tr!(
                "Imported {count}; {replaced} replaced the mappings you had on the same keys.",
                count = total,
                replaced = n
            ),
        };
        self.notice = Some(Notice::text(text));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use remapr_core::{Key, Mods};

    use super::*;

    const ALL: Capabilities = Capabilities {
        keyboard: Support::Yes,
        per_app_rules: Support::Yes,
        mouse: Support::NotYet,
        controllers: Support::NotYet,
        virtual_controllers: Support::NotYet,
        hide_controllers: Support::NotYet,
    };

    /// A backend that remembers what it was told and reports what a test hands it.
    #[derive(Clone, Default)]
    struct Fake {
        sent: Rc<RefCell<Vec<Command>>>,
        events: Rc<RefCell<Vec<Event>>>,
    }

    impl Fake {
        /// What it was told since the last call, without the rules and pause syncs.
        fn told(&self) -> Vec<Command> {
            let sent = std::mem::take(&mut *self.sent.borrow_mut());
            sent.into_iter().filter(|c| !matches!(c, Command::SetRules(_) | Command::SetEnabled(_))).collect()
        }

        /// The rules it holds now.
        fn rules(&self) -> Vec<remapr_core::Rule> {
            let sent = self.sent.borrow();
            let last =
                sent.iter().rev().find_map(|c| if let Command::SetRules(r) = c { Some(r.clone()) } else { None });
            last.unwrap_or_default()
        }

        fn report(&self, event: Event) {
            self.events.borrow_mut().push(event);
        }
    }

    impl Backend for Fake {
        fn send(&mut self, command: Command) {
            self.sent.borrow_mut().push(command);
        }

        fn poll(&mut self) -> Vec<Event> {
            std::mem::take(&mut *self.events.borrow_mut())
        }

        fn capabilities(&self) -> Capabilities {
            ALL
        }
    }

    fn session(mappings: Vec<Mapping>) -> (Session, Fake) {
        let fake = Fake::default();
        let config = Config { mappings, ..Config::default() };
        let session = Session::new(config, Ok(Box::new(fake.clone())), ALL);
        (session, fake)
    }

    fn key(key: Key) -> Chord {
        Chord::key(key)
    }

    fn caps_to_esc() -> Mapping {
        Mapping::new(key(Key::CapsLock), key(Key::Escape))
    }

    #[test]
    fn pressing_two_keys_adds_a_mapping() {
        let (mut s, fake) = session(vec![]);
        s.start_capture();
        assert_eq!((s.capture(), fake.told()), (Capture::From, vec![Command::StartCapture]));
        fake.report(Event::Captured(key(Key::CapsLock)));
        s.poll();
        assert_eq!((s.capture(), fake.told()), (Capture::To(key(Key::CapsLock)), vec![Command::StartCapture]));
        fake.report(Event::Captured(key(Key::Escape)));
        s.poll();
        assert_eq!(s.capture(), Capture::Idle);
        assert_eq!(s.config.mappings, [caps_to_esc()]);
        assert_eq!(fake.rules(), [caps_to_esc().rule()]);
        assert!(s.take_unsaved());
        assert!(!s.take_unsaved());
    }

    #[test]
    fn the_same_keys_twice_change_nothing() {
        let (mut s, fake) = session(vec![]);
        s.start_capture();
        fake.report(Event::Captured(key(Key::F1)));
        fake.report(Event::Captured(key(Key::F1)));
        s.poll();
        assert!(s.config.mappings.is_empty());
        assert!(s.notice.is_some());
        assert!(!s.take_unsaved());
    }

    #[test]
    fn a_mapping_on_the_same_keys_takes_the_new_target() {
        let (mut s, fake) = session(vec![caps_to_esc()]);
        s.add(Mapping::new(key(Key::CapsLock), Chord { mods: Mods::CTRL, key: Key::KeyZ }));
        assert_eq!(s.config.mappings.len(), 1);
        assert_eq!(s.config.mappings[0].to, Chord { mods: Mods::CTRL, key: Key::KeyZ });
        assert!(s.notice.is_some());
        assert_eq!(fake.rules(), [s.config.mappings[0].rule()]);
    }

    #[test]
    fn removing_can_be_undone() {
        let f1 = Mapping::new(key(Key::F1), key(Key::F2));
        let (mut s, fake) = session(vec![caps_to_esc(), f1.clone()]);
        s.remove(0);
        assert_eq!(s.config.mappings, std::slice::from_ref(&f1));
        assert_eq!(fake.rules(), [f1.rule()]);
        let (index, removed) = s.notice.as_ref().and_then(|n| n.undo.clone()).expect("an undo");
        s.undo(index, removed);
        assert_eq!(s.config.mappings, [caps_to_esc(), f1]);
        assert!(s.notice.is_none());
    }

    #[test]
    fn limiting_to_an_app_replaces_its_twin() {
        let chrome = Some(AppMatcher::Exe("chrome.exe".into()));
        let in_chrome = Mapping { app: chrome.clone(), ..Mapping::new(key(Key::CapsLock), key(Key::F5)) };
        let (mut s, _) = session(vec![in_chrome, caps_to_esc()]);
        s.set_app(1, chrome.clone());
        assert_eq!(s.config.mappings, [Mapping { app: chrome, ..caps_to_esc() }]);
    }

    #[test]
    fn pausing_tells_the_engine() {
        let (mut s, fake) = session(vec![]);
        s.set_enabled(false);
        assert!(fake.sent.borrow().contains(&Command::SetEnabled(false)));
        assert!(!s.config.enabled);
    }

    #[test]
    fn import_adds_and_replaces() {
        let (mut s, _) = session(vec![caps_to_esc()]);
        let new_target = Mapping::new(key(Key::CapsLock), key(Key::Backspace));
        let f1 = Mapping::new(key(Key::F1), key(Key::F2));
        s.import(vec![new_target.clone(), f1.clone()]);
        assert_eq!(s.config.mappings, [new_target, f1]);
        assert!(s.take_unsaved());
    }

    #[test]
    fn without_an_engine_nothing_is_captured() {
        let unsupported = Err(io::Error::new(io::ErrorKind::Unsupported, "no"));
        let mut s = Session::new(Config::default(), unsupported, Capabilities::none(Support::NotYet));
        assert!(!s.can_capture() && !s.is_running() && !s.per_app_rules());
        assert_eq!(s.status().map(|(tone, _)| *tone), Some(Tone::Info));
        s.start_capture();
        assert_eq!(s.capture(), Capture::Idle);

        let failed = Err(io::Error::other("access denied"));
        let s = Session::new(Config::default(), failed, ALL);
        assert_eq!(s.status().map(|(tone, _)| *tone), Some(Tone::Danger));
    }

    #[test]
    fn an_engine_that_stops_ends_the_capture_and_says_so() {
        let (mut s, fake) = session(vec![]);
        s.start_capture();
        fake.report(Event::Stopped("the hook went away".into()));
        s.poll();
        assert_eq!(s.capture(), Capture::Idle);
        assert!(!s.is_running() && !s.can_capture());
        assert_eq!(s.status().map(|(tone, _)| *tone), Some(Tone::Danger));
    }

    #[test]
    fn capabilities_can_change() {
        let (mut s, fake) = session(vec![]);
        let per_app_gone = Capabilities { per_app_rules: Support::NotYet, ..ALL };
        fake.report(Event::CapabilitiesChanged(per_app_gone));
        s.poll();
        assert!(!s.per_app_rules());
        assert!(s.can_capture());
    }
}
