//! Colour tokens for light and dark, fonts and the egui style built from them.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Stroke, TextStyle,
    Theme, Visuals,
};

/// Every colour a screen may use. Pick the right set with [`Palette::of`].
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub accent_soft: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub success: Color32,
    pub key_face: Color32,
    pub key_edge: Color32,
}

impl Palette {
    pub const LIGHT: Self = Self {
        bg: Color32::from_rgb(0xF6, 0xF6, 0xF8),
        surface: Color32::WHITE,
        surface_hover: Color32::from_rgb(0xF1, 0xF1, 0xF4),
        border: Color32::from_rgb(0xE4, 0xE4, 0xE9),
        text: Color32::from_rgb(0x18, 0x18, 0x1B),
        muted: Color32::from_rgb(0x6B, 0x6B, 0x76),
        accent: Color32::from_rgb(0x4F, 0x46, 0xE5),
        accent_hover: Color32::from_rgb(0x43, 0x38, 0xCA),
        on_accent: Color32::WHITE,
        accent_soft: Color32::from_rgb(0xEE, 0xED, 0xFD),
        danger: Color32::from_rgb(0xDC, 0x26, 0x26),
        danger_soft: Color32::from_rgb(0xFE, 0xF2, 0xF2),
        success: Color32::from_rgb(0x16, 0xA3, 0x4A),
        key_face: Color32::WHITE,
        key_edge: Color32::from_rgb(0xC9, 0xC9, 0xD1),
    };

    pub const DARK: Self = Self {
        bg: Color32::from_rgb(0x0E, 0x0E, 0x11),
        surface: Color32::from_rgb(0x18, 0x18, 0x1C),
        surface_hover: Color32::from_rgb(0x21, 0x21, 0x26),
        border: Color32::from_rgb(0x2A, 0x2A, 0x31),
        text: Color32::from_rgb(0xF4, 0xF4, 0xF5),
        muted: Color32::from_rgb(0xA1, 0xA1, 0xAA),
        accent: Color32::from_rgb(0x81, 0x8C, 0xF8),
        accent_hover: Color32::from_rgb(0x9B, 0xA4, 0xFA),
        on_accent: Color32::from_rgb(0x0E, 0x0E, 0x11),
        accent_soft: Color32::from_rgb(0x23, 0x23, 0x3D),
        danger: Color32::from_rgb(0xF8, 0x71, 0x71),
        danger_soft: Color32::from_rgb(0x2D, 0x16, 0x18),
        success: Color32::from_rgb(0x4A, 0xDE, 0x80),
        key_face: Color32::from_rgb(0x25, 0x25, 0x2B),
        key_edge: Color32::from_rgb(0x3A, 0x3A, 0x44),
    };

    pub fn of(ui: &egui::Ui) -> &'static Self {
        if ui.visuals().dark_mode { &Self::DARK } else { &Self::LIGHT }
    }
}

/// Segoe Fluent Icons code points (Windows 11; Windows 10's MDL2 font shares them).
pub mod icon {
    pub const ADD: char = '\u{E710}';
    pub const DELETE: char = '\u{E74D}';
    pub const CLOSE: char = '\u{E711}';
    pub const FORWARD: char = '\u{E72A}';
    pub const KEYBOARD: char = '\u{E765}';
    pub const SUN: char = '\u{E706}';
    pub const MOON: char = '\u{E708}';
    pub const SYSTEM: char = '\u{E7F4}';
    pub const WARNING: char = '\u{E7BA}';
    pub const UNDO: char = '\u{E7A7}';
    pub const DOWNLOAD: char = '\u{E896}';
    pub const REFRESH: char = '\u{E72C}';
    pub const MORE: char = '\u{E712}';
}

pub(crate) const SEMIBOLD: &str = "semibold";
pub(crate) const ICONS: &str = "icons";

/// Fonts, text sizes, spacing and both themes' visuals. Call once at startup.
pub fn install(ctx: &Context) {
    ctx.set_fonts(fonts());
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Heading, FontId::new(22.0, FontFamily::Name(SEMIBOLD.into()))),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 32.0;
        // Tooltips name icon-only buttons: show them at once, even while the pointer still moves.
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    ctx.set_visuals_of(Theme::Light, visuals(Visuals::light(), &Palette::LIGHT));
    ctx.set_visuals_of(Theme::Dark, visuals(Visuals::dark(), &Palette::DARK));
}

fn visuals(mut v: Visuals, p: &Palette) -> Visuals {
    let radius = CornerRadius::same(8);
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.extreme_bg_color = p.surface;
    v.faint_bg_color = p.surface_hover;
    v.hyperlink_color = p.accent;
    v.error_fg_color = p.danger;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    for (w, fill) in [
        (&mut v.widgets.inactive, p.surface),
        (&mut v.widgets.hovered, p.surface_hover),
        (&mut v.widgets.active, p.border),
        (&mut v.widgets.open, p.surface_hover),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, p.border);
        w.fg_stroke = Stroke::new(1.0, p.text);
        w.corner_radius = radius;
        w.expansion = 0.0;
    }
    v
}

/// Windows' own UI and icon fonts, read from the system so they add nothing to the binary.
/// egui's bundled fonts stay as the fallback.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let dir = std::path::Path::new(&std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts");
    let mut load = |name: &str, files: &[&str], y_offset_factor: f32| {
        let bytes = files.iter().find_map(|f| std::fs::read(dir.join(f)).ok())?;
        let data = FontData::from_owned(bytes).tweak(FontTweak { y_offset_factor, ..Default::default() });
        fonts.font_data.insert(name.to_owned(), Arc::new(data));
        Some(name.to_owned())
    };
    // Segoe UI's tall ascent leaves its capitals ~0.15em below the line box's centre, so text sat
    // lower than icons and keycap edges. Lifting it lines both up.
    const SEGOE_LIFT: f32 = -0.15;
    let regular = load("segoe", &["segoeui.ttf"], SEGOE_LIFT);
    let semibold = load("segoe-semibold", &["seguisb.ttf"], SEGOE_LIFT).or_else(|| regular.clone());
    let icons = load("segoe-icons", &["SegoeIcons.ttf", "segmdl2.ttf"], 0.0);

    let fallback = fonts.families[&FontFamily::Proportional].clone();
    if let Some(regular) = regular {
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, regular);
    }
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), semibold.into_iter().chain(fallback.clone()).collect());
    fonts.families.insert(FontFamily::Name(ICONS.into()), icons.into_iter().chain(fallback).collect());
    fonts
}

/// Segoe UI Semibold at `size`, for titles and emphasis.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}
