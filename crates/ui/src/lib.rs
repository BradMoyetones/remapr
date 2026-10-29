//! Remapr's design system for egui. Apps call [`install`] once, then build screens from these
//! widgets and read colours from [`Palette::of`], never from hard-coded values.
//!
//! The same look on every system: Inter for text, Lucide for icons, one palette with checked
//! contrast. [`title_bar`] makes a strip of the app act as the window's title bar where the app
//! draws over it (macOS).

mod theme;
pub mod title_bar;
mod widgets;

pub use theme::{Icon, Palette, Tone, icon, install, semibold};
pub use widgets::{KeyState, banner, card, danger_icon_button, icon_button, icon_label, kbd, primary_button, switch};
