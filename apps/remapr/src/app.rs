//! The main window: the list of remapped keys, adding one by pressing keys, pause and theme.

use eframe::egui::{
    self, Align, Frame, Layout, Margin, RichText, Sense, SystemTheme, ThemePreference, ViewportCommand, vec2,
};
use remapr_engine::{Engine, Vk, key_name};
use remapr_ui::{
    KeyState, Palette, card, danger_banner, icon, icon_button, icon_text, kbd, primary_button, semibold, switch,
};

use crate::config::{Config, Mapping, Theme};

/// Adding a mapping: first the key to change, then the key it should send.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Capture {
    Idle,
    From,
    To(Vk),
}

struct Notice {
    text: String,
    /// A deleted mapping and where it was, so "Deshacer" can put it back.
    undo: Option<(usize, Mapping)>,
}

pub struct App {
    config: Config,
    engine: Option<Engine>,
    engine_error: Option<String>,
    capture: Capture,
    notice: Option<Notice>,
    /// Applied on the next frame: viewport commands sent before the first one are dropped.
    theme_dirty: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        remapr_ui::install(&cc.egui_ctx);
        let (config, load_warning) = Config::load();
        let ctx = cc.egui_ctx.clone();
        let (engine, engine_error) = match Engine::start(move || ctx.request_repaint()) {
            Ok(engine) => (Some(engine), None),
            Err(e) => (None, Some(format!("No se pudo iniciar el motor de teclado: {e}"))),
        };
        let app = Self {
            config,
            engine,
            engine_error,
            capture: Capture::Idle,
            notice: load_warning.map(|text| Notice { text, undo: None }),
            theme_dirty: true,
        };
        app.sync_engine();
        app
    }

    fn sync_engine(&self) {
        if let Some(engine) = &self.engine {
            engine.set_enabled(self.config.enabled);
            engine.set_mappings(self.config.mappings.iter().map(|m| (m.from, m.to)));
        }
    }

    /// Applies the config to the engine and saves it.
    fn commit(&mut self) {
        self.sync_engine();
        if let Err(e) = self.config.save() {
            self.notice = Some(Notice { text: format!("No se pudo guardar la configuración: {e}"), undo: None });
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let (preference, window) = match self.config.theme {
            Theme::System => (ThemePreference::System, SystemTheme::SystemDefault),
            Theme::Light => (ThemePreference::Light, SystemTheme::Light),
            Theme::Dark => (ThemePreference::Dark, SystemTheme::Dark),
        };
        ctx.set_theme(preference);
        // Keeps the native title bar in step with the app.
        ctx.send_viewport_cmd(ViewportCommand::SetTheme(window));
    }

    fn start_capture(&mut self) {
        if let Some(engine) = &self.engine {
            engine.start_capture();
            self.capture = Capture::From;
            self.notice = None;
        }
    }

    fn cancel_capture(&mut self) {
        if let Some(engine) = &self.engine {
            engine.cancel_capture();
        }
        self.capture = Capture::Idle;
    }

    fn on_captured(&mut self, vk: Vk) {
        match self.capture {
            Capture::Idle => {}
            Capture::From => {
                self.capture = Capture::To(vk);
                if let Some(engine) = &self.engine {
                    engine.start_capture();
                }
            }
            Capture::To(from) => {
                self.capture = Capture::Idle;
                self.add(Mapping { from, to: vk });
            }
        }
    }

    fn add(&mut self, mapping: Mapping) {
        let (from, to) = (key_name(mapping.from), key_name(mapping.to));
        if mapping.from == mapping.to {
            let text = format!("Elegiste {from} dos veces, así que no se cambió nada.");
            self.notice = Some(Notice { text, undo: None });
            return;
        }
        if let Some(existing) = self.config.mappings.iter_mut().find(|m| m.from == mapping.from) {
            existing.to = mapping.to;
            let text = format!("{from} ya estaba reasignada. Ahora envía {to}.");
            self.notice = Some(Notice { text, undo: None });
        } else {
            self.config.mappings.push(mapping);
        }
        self.commit();
    }

    fn remove(&mut self, index: usize) {
        let m = self.config.mappings.remove(index);
        let text = format!("Se quitó {} → {}.", key_name(m.from), key_name(m.to));
        self.notice = Some(Notice { text, undo: Some((index, m)) });
        self.commit();
    }

    fn undo(&mut self, index: usize, mapping: Mapping) {
        self.notice = None;
        if self.config.mappings.iter().all(|m| m.from != mapping.from) {
            let index = index.min(self.config.mappings.len());
            self.config.mappings.insert(index, mapping);
            self.commit();
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        ui.horizontal(|ui| {
            kbd(ui, "R", KeyState::Idle);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.heading("Remapr");
                ui.label(RichText::new("Reasigna cualquier tecla de tu teclado").color(p.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut enabled = self.config.enabled;
                if switch(ui, &mut enabled, "Remapeo activo").changed() {
                    self.config.enabled = enabled;
                    self.commit();
                }
                ui.label(RichText::new(if enabled { "Activo" } else { "En pausa" }).color(p.muted));
                ui.add_space(4.0);
                let (glyph, label, next) = match self.config.theme {
                    Theme::System => (icon::SYSTEM, "Tema: igual que Windows", Theme::Light),
                    Theme::Light => (icon::SUN, "Tema: claro", Theme::Dark),
                    Theme::Dark => (icon::MOON, "Tema: oscuro", Theme::System),
                };
                if icon_button(ui, glyph, label).clicked() {
                    self.config.theme = next;
                    self.theme_dirty = true;
                    self.commit();
                }
            });
        });
    }

    fn footer(&self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        let (color, status) = match (&self.engine, self.config.enabled) {
            (None, _) => (p.danger, "Motor detenido"),
            (Some(_), true) => (p.success, "Motor activo"),
            (Some(_), false) => (p.muted, "En pausa: tus teclas funcionan como siempre"),
        };
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.label(RichText::new(status).small().color(p.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).small().color(p.muted));
            });
        });
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        if let Some(e) = &self.engine_error {
            danger_banner(ui, e);
            ui.add_space(4.0);
        }
        self.notice_bar(ui);

        let capturing = self.capture != Capture::Idle;
        if self.config.mappings.is_empty() && !capturing {
            self.empty_state(ui);
            return;
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new("Teclas reasignadas").font(semibold(16.0)).color(p.text));
            ui.label(RichText::new(self.config.mappings.len().to_string()).color(p.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let can_add = !capturing && self.engine.is_some();
                if ui.add_enabled_ui(can_add, |ui| primary_button(ui, Some(icon::ADD), "Agregar")).inner.clicked() {
                    self.start_capture();
                }
            });
        });
        ui.add_space(4.0);

        if capturing {
            self.capture_card(ui);
        }

        let mut remove = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            if !self.config.enabled {
                ui.multiply_opacity(0.55);
            }
            for (i, m) in self.config.mappings.iter().enumerate() {
                card(ui, false, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        kbd(ui, &key_name(m.from), KeyState::Idle);
                        ui.label(icon_text(icon::FORWARD, 14.0).color(p.muted));
                        kbd(ui, &key_name(m.to), KeyState::Idle);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icon_button(ui, icon::DELETE, "Quitar este mapeo").clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                });
            }
        });
        if let Some(i) = remove {
            self.remove(i);
        }
    }

    fn capture_card(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        let mut cancel = false;
        card(ui, true, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                match self.capture {
                    Capture::To(from) => {
                        kbd(ui, &key_name(from), KeyState::Idle);
                        ui.label(icon_text(icon::FORWARD, 14.0).color(p.muted));
                        kbd(ui, "?", KeyState::Listening);
                    }
                    _ => {
                        kbd(ui, "?", KeyState::Listening);
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    cancel = ui.button("Cancelar").clicked();
                });
            });
            let step = match self.capture {
                Capture::To(_) => "Ahora presiona la tecla que debe enviar en su lugar.",
                _ => "Presiona la tecla que quieres cambiar.",
            };
            ui.label(RichText::new(step).font(semibold(14.0)).color(p.text));
            ui.label(
                RichText::new(
                    "Cualquier tecla cuenta, incluso Esc o Win. Mientras tanto, el teclado no escribe en otras apps.",
                )
                .small()
                .color(p.muted),
            );
        });
        ui.add_space(4.0);
        if cancel {
            self.cancel_capture();
        }
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        ui.add_space(56.0);
        ui.vertical_centered(|ui| {
            ui.label(icon_text(icon::KEYBOARD, 44.0).color(p.muted));
            ui.add_space(8.0);
            ui.label(RichText::new("Todavía no reasignaste ninguna tecla").font(semibold(17.0)).color(p.text));
            ui.label(
                RichText::new("Por ejemplo, convierte Bloq Mayús en Esc. Presiona el botón y luego las dos teclas.")
                    .color(p.muted),
            );
            ui.add_space(12.0);
            if ui
                .add_enabled_ui(self.engine.is_some(), |ui| {
                    primary_button(ui, Some(icon::ADD), "Agregar mi primer mapeo")
                })
                .inner
                .clicked()
            {
                self.start_capture();
            }
        });
    }

    fn notice_bar(&mut self, ui: &mut egui::Ui) {
        let Some(notice) = &self.notice else { return };
        let p = Palette::of(ui);
        let (mut close, mut undo) = (false, None);
        card(ui, false, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(RichText::new(&notice.text).color(p.text)).wrap());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close = icon_button(ui, icon::CLOSE, "Cerrar aviso").clicked();
                    if notice.undo.is_some() && ui.button("Deshacer").clicked() {
                        undo = notice.undo;
                    }
                });
            });
        });
        ui.add_space(4.0);
        if let Some((index, mapping)) = undo {
            self.undo(index, mapping);
        } else if close {
            self.notice = None;
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if std::mem::take(&mut self.theme_dirty) {
            self.apply_theme(ctx);
        }
        if self.capture == Capture::Idle {
            return;
        }
        // Capturing swallows every key system-wide; never keep doing it behind the user's back.
        if !ctx.input(|i| i.focused) {
            self.cancel_capture();
        } else if let Some(vk) = self.engine.as_ref().and_then(Engine::captured) {
            self.on_captured(vk);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let bg = Palette::of(ui).bg;
        egui::Panel::top("header")
            .frame(Frame::new().fill(bg).inner_margin(Margin { left: 20, right: 20, top: 18, bottom: 10 }))
            .show(ui, |ui| self.header(ui));
        egui::Panel::bottom("footer")
            .frame(Frame::new().fill(bg).inner_margin(Margin::symmetric(20, 10)))
            .show(ui, |ui| self.footer(ui));
        egui::CentralPanel::default()
            .frame(Frame::new().fill(bg).inner_margin(Margin::symmetric(20, 6)))
            .show(ui, |ui| self.body(ui));
    }
}
