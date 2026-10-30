//! Remapr's model and remapping logic, the same on every system: [`Key`]s, [`Chord`]s, [`Rule`]s,
//! and [`decide`], which says what to do with each key event.
//!
//! No OS code and no `unsafe`: each system's backend in `remapr-engine` translates its own key
//! codes to and from [`Key`] and calls [`decide`] from its hook. Everything here is tested on every
//! system.

#![forbid(unsafe_code)]

mod chord;
mod key;
mod remap;
mod rule;

pub use chord::{Chord, Mods};
pub use key::{Key, UnknownName};
pub use remap::{Edge, Event, Out, State, decide, remaps_plainly};
pub use rule::{App, AppMatcher, Rule, sort_rules};
