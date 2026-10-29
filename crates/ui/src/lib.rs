//! Remapr's design system for egui. Apps call [`install`] once, then build screens from these
//! widgets and read colours from [`Palette::of`], never from hard-coded values.

mod theme;
mod widgets;

pub use theme::{Palette, icon, install, semibold};
pub use widgets::{
    KeyState, card, danger_banner, danger_icon_button, icon_button, icon_text, kbd, primary_button, switch,
};
