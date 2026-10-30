//! How the app talks to an engine backend, whatever the system: it sends [`Command`]s and drains
//! [`Event`]s once per frame, so a backend does its work on its own thread and never blocks a
//! frame. [`Capabilities`] say what this system's backend can do, so the interface asks them
//! instead of asking which system it runs on.

use crate::{Chord, Rule};

/// Asks the backend to do something. What comes of it arrives later as an [`Event`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Replaces every rule.
    SetRules(Vec<Rule>),
    /// Pauses or resumes remapping. Keys held down when pausing are still released correctly.
    SetEnabled(bool),
    /// Swallows every key until a key or chord is pressed and all keys are up again, then reports
    /// it as [`Event::Captured`].
    StartCapture,
    CancelCapture,
}

/// What a backend reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A capture finished with this chord.
    Captured(Chord),
    /// What the backend can do changed: a permission was granted or taken back.
    CapabilitiesChanged(Capabilities),
    /// The backend stopped on its own and remaps nothing more; the system's words.
    Stopped(String),
}

/// How far one function is from working on this system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    Yes,
    /// Works once the user grants it.
    NeedsPermission(Permission),
    /// Works once a driver is installed.
    NeedsDriver(Driver),
    /// Remapr doesn't do it on this system yet; a later phase will.
    NotYet,
    /// This system doesn't let any app do it.
    NotOnThisSystem,
}

/// A permission a system asks the user for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// macOS: Privacy & Security › Accessibility, to change keys.
    Accessibility,
    /// macOS: Privacy & Security › Input Monitoring, to read keys.
    InputMonitoring,
    /// Linux: access to `/dev/input` and `/dev/uinput`.
    InputDevices,
}

/// A driver a function needs (Windows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driver {
    /// Virtual game controllers.
    ViGEmBus,
    /// Hiding physical game controllers from games.
    HidHide,
}

/// What this system's backend can do, function by function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// Remapping keys and shortcuts.
    pub keyboard: Support,
    /// Rules that apply only while one app is in front.
    pub per_app_rules: Support,
    pub mouse: Support,
    /// Reading physical game controllers.
    pub controllers: Support,
    /// Making virtual game controllers.
    pub virtual_controllers: Support,
    /// Hiding physical game controllers, so games see only the virtual one.
    pub hide_controllers: Support,
}

impl Capabilities {
    /// A system without a backend yet: nothing works, and what the system forbids is said so.
    pub const fn none(virtual_controllers: Support) -> Self {
        Self {
            keyboard: Support::NotYet,
            per_app_rules: Support::NotYet,
            mouse: Support::NotYet,
            controllers: Support::NotYet,
            virtual_controllers,
            hide_controllers: virtual_controllers,
        }
    }
}

/// The app's view of an engine backend.
pub trait Backend {
    fn send(&mut self, command: Command);
    /// Events since the last call, oldest first.
    fn poll(&mut self) -> Vec<Event>;
    fn capabilities(&self) -> Capabilities;
}
