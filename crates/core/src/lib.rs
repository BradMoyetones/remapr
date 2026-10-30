//! Remapr's model and remapping logic, the same on every system: [`Key`]s, [`Chord`]s, [`Rule`]s,
//! [`decide`], which says what to do with each key event, and the contract between the app and an
//! engine [`Backend`].
//!
//! No OS code and no `unsafe`: each system's backend in `remapr-engine` translates its own key
//! codes to and from [`Key`], calls [`decide`] from its hook, and speaks [`Command`]s and
//! [`Event`]s with the app. Everything here is tested on every system.

#![forbid(unsafe_code)]

mod capture;
mod chord;
mod key;
mod protocol;
mod remap;
mod rule;

pub use capture::Capture;
pub use chord::{Chord, Mods};
pub use key::{Key, UnknownName};
pub use protocol::{Backend, Capabilities, Command, Driver, Event, Permission, Support};
pub use remap::{Edge, KeyEvent, Out, State, decide, remaps_plainly};
pub use rule::{App, AppMatcher, Rule, sort_rules};
