//! Screenshots of the design system, light and dark: every widget, tone and icon, as screens use
//! them. They catch a change of look in review, and show the UI on systems where Remapr can't
//! remap yet.
//!
//! `cargo test -p remapr-ui --features snapshots` compares with `tests/snapshots/`;
//! `UPDATE_SNAPSHOTS=1` in front writes new ones.

#![cfg(feature = "snapshots")]

use eframe::egui::{self, Layout, RichText, Theme};
use egui_kittest::Harness;
use remapr_ui::{
    KeyState, Palette, Tone, banner, card, danger_icon_button, icon, icon_button, icon_label, kbd, primary_button,
    semibold, switch,
};

fn chord(ui: &mut egui::Ui, keys: &[&str]) {
    let muted = Palette::of(ui).muted;
    for (i, key) in keys.iter().enumerate() {
        if i > 0 {
            ui.label(RichText::new("+").color(muted));
        }
        kbd(ui, key, KeyState::Idle);
    }
}

fn gallery(ui: &mut egui::Ui) {
    let p = *Palette::of(ui);
    egui::Frame::new().fill(p.bg).inner_margin(20).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            kbd(ui, "R", KeyState::Idle);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.heading("Remapr");
                ui.label(RichText::new("Remap any key on your keyboard").color(p.muted));
            });
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                switch(ui, &mut true, "Remapping on");
                ui.label(RichText::new("On").color(p.muted));
                icon_button(ui, icon::SYSTEM, "Theme");
                icon_button(ui, icon::MORE, "More options");
            });
        });
        ui.add_space(8.0);
        for (tone, text) in [
            (Tone::Info, "Remapr can't remap keys on this system yet."),
            (Tone::Success, "Exported 3 mappings."),
            (Tone::Warning, "Remapr runs without administrator rights."),
            (Tone::Danger, "Couldn't start the keyboard engine."),
        ] {
            banner(ui, tone, text);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Remapped keys").font(semibold(16.0)).color(p.text));
            ui.label(RichText::new("2").color(p.muted));
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                primary_button(ui, Some(icon::ADD), "Add");
            });
        });
        for (from, to, app) in
            [(&["Caps Lock"][..], &["Esc"][..], "All apps"), (&["Ctrl", "Shift", "T"], &["F5"], "Only in code.exe")]
        {
            card(ui, false, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    chord(ui, from);
                    icon_label(ui, icon::FORWARD, 16.0, p.muted);
                    chord(ui, to);
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        danger_icon_button(ui, icon::DELETE, "Remove this mapping");
                        ui.label(RichText::new(app).small().color(p.accent));
                    });
                });
            });
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            switch(ui, &mut false, "Off");
            ui.add_enabled_ui(false, |ui| primary_button(ui, None, "Disabled"));
            ui.label(RichText::new("Áé ñ ¿¡ · ⌘ ⌥ ⇧ ⌃ ← → −").color(p.text));
        });
        ui.horizontal_wrapped(|ui| {
            for i in [
                icon::ADD,
                icon::CLOSE,
                icon::DELETE,
                icon::DOWNLOAD,
                icon::ERROR,
                icon::FORWARD,
                icon::INFO,
                icon::KEYBOARD,
                icon::MOON,
                icon::MORE,
                icon::REFRESH,
                icon::SUCCESS,
                icon::SUN,
                icon::SYSTEM,
                icon::UNDO,
                icon::WARNING,
            ] {
                icon_label(ui, i, 20.0, p.text);
            }
        });
    });
}

fn snapshot(theme: Theme, name: &str) {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(560.0, 620.0))
        .with_pixels_per_point(2.0)
        .with_theme(theme)
        .with_context_setup(remapr_ui::install)
        .build_ui(gallery);
    harness.run();
    harness.snapshot(name);
}

#[test]
fn gallery_light() {
    snapshot(Theme::Light, "gallery_light");
}

#[test]
fn gallery_dark() {
    snapshot(Theme::Dark, "gallery_dark");
}
