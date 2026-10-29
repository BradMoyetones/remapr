//! The main window: the list of remapped keys, adding one by pressing keys, pause and theme.
//! Closing it hides it to the tray; remapping keeps running until "Salir".

use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Align, Frame, Layout, Margin, RichText, Sense, ThemePreference, ViewportCommand, vec2};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use remapr_engine::{Engine, Vk, key_name};
use remapr_platform::{SingleInstance, startup};
use remapr_ui::{
    KeyState, Palette, card, danger_banner, danger_icon_button, icon, icon_button, icon_text, kbd, primary_button,
    semibold, switch,
};

use crate::config::{Config, Mapping, Theme};
use crate::tray::{self, Command, Tray};
use crate::updates::{self, Updates};

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
    /// A page to open from a "Novedades" button.
    link: Option<String>,
}

pub struct App {
    config: Config,
    engine: Option<Engine>,
    engine_error: Option<String>,
    capture: Capture,
    notice: Option<Notice>,
    /// `None` if Windows refused the tray icon; closing the window then quits.
    tray: Option<Tray>,
    commands: Receiver<Command>,
    /// The first close explains where Remapr went before hiding.
    tray_hint_open: bool,
    quitting: bool,
    /// The native window, for asking Windows whether it is in front.
    hwnd: Option<isize>,
    /// Started at sign-in: the window stays in the tray until asked for.
    start_hidden: bool,
    elevated: bool,
    /// `None` when Remapr isn't installed (no sign-in task to switch).
    starts_at_sign_in: Option<bool>,
    updates: Updates,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        instance: Option<SingleInstance>,
        background: bool,
        installed: bool,
    ) -> Self {
        remapr_ui::install(&cc.egui_ctx);
        let hwnd = match cc.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(handle)) => Some(handle.hwnd.get()),
            _ => None,
        };
        if let Some(hwnd) = hwnd {
            remapr_platform::use_exe_icon(hwnd, tray::ICON_ACTIVE);
            remapr_platform::repaint_title_bar_when_shown(hwnd);
            if background {
                remapr_platform::hide_first_show(hwnd);
            }
        }
        // Otherwise our own hook goes blind while this window is in front (see the function).
        remapr_platform::stop_raw_keyboard_input();
        let (mut config, load_warning) = Config::load();
        // First start after an update: say so, with a way to the release notes.
        let mut notice = load_warning.map(|text| Notice { text, undo: None, link: None });
        if config.last_version != updates::VERSION {
            if !config.last_version.is_empty() {
                let text = format!("Remapr se actualizó a la versión {}.", updates::VERSION);
                notice = Some(Notice { text, undo: None, link: Some(remapr_update::release_page(updates::VERSION)) });
            }
            config.last_version = updates::VERSION.into();
            let _ = config.save();
        }
        let ctx = cc.egui_ctx.clone();
        let (engine, engine_error) = match Engine::start(move || ctx.request_repaint()) {
            Ok(engine) => (Some(engine), None),
            Err(e) => (None, Some(format!("No se pudo iniciar el motor de teclado: {e}"))),
        };

        let (sender, commands) = mpsc::channel();
        if let Some(instance) = instance {
            let (sender, ctx) = (sender.clone(), cc.egui_ctx.clone());
            instance.listen(move || {
                let _ = sender.send(Command::Show);
                ctx.request_repaint();
            });
        }
        let tray = Tray::new(config.enabled, sender, cc.egui_ctx.clone()).ok();
        let elevated = remapr_platform::is_elevated();

        let app = Self {
            config,
            engine,
            engine_error,
            capture: Capture::Idle,
            notice,
            tray,
            commands,
            tray_hint_open: false,
            quitting: false,
            hwnd,
            start_hidden: background,
            elevated,
            starts_at_sign_in: startup::starts_at_sign_in(),
            updates: Updates::new(&cc.egui_ctx, update_source(installed), elevated),
        };
        app.apply_theme(&cc.egui_ctx);
        app.sync();
        app
    }

    /// Shows the config's state in the engine and the tray.
    fn sync(&self) {
        if let Some(engine) = &self.engine {
            engine.set_enabled(self.config.enabled);
            engine.set_mappings(self.config.mappings.iter().map(|m| (m.from, m.to)));
        }
        if let Some(tray) = &self.tray {
            tray.sync(self.config.enabled);
        }
    }

    /// Applies the config everywhere and saves it.
    fn commit(&mut self) {
        self.sync();
        if let Err(e) = self.config.save() {
            self.notice =
                Some(Notice { text: format!("No se pudo guardar la configuración: {e}"), undo: None, link: None });
        }
    }

    fn show_window(ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }

    fn hide_to_tray(&mut self, ctx: &egui::Context) {
        self.cancel_capture();
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn handle_commands(&mut self, ctx: &egui::Context) {
        while let Ok(command) = self.commands.try_recv() {
            match command {
                Command::Show => Self::show_window(ctx),
                Command::TogglePause => {
                    self.config.enabled = !self.config.enabled;
                    self.commit();
                }
                Command::Quit => self.quit(ctx),
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.tray.is_some() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            if self.config.tray_hint_seen {
                self.hide_to_tray(ctx);
            } else {
                self.tray_hint_open = true;
            }
        }
    }

    fn tray_hint(&mut self, ui: &egui::Ui) {
        let (ctx, p) = (ui.ctx().clone(), Palette::of(ui));
        let mut choice = None;
        egui::Modal::new(egui::Id::unique("tray_hint"))
            .frame(Frame::new().fill(p.surface).corner_radius(12).inner_margin(20).stroke((1.0, p.border)))
            .show(&ctx, |ui| {
                ui.set_width(360.0);
                ui.label(RichText::new("Remapr sigue funcionando").font(semibold(17.0)).color(p.text));
                ui.add_space(2.0);
                ui.label(
                    RichText::new(
                        "Tus teclas siguen reasignadas. Encuéntralo en la bandeja del sistema, junto al reloj: \
                         desde ahí puedes abrirlo, pausarlo o salir.",
                    )
                    .color(p.muted),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, None, "Entendido").clicked() {
                        choice = Some(false);
                    }
                    if ui.button("Salir de Remapr").clicked() {
                        choice = Some(true);
                    }
                });
            });
        if let Some(quit) = choice {
            self.tray_hint_open = false;
            self.config.tray_hint_seen = true;
            self.commit();
            if quit { self.quit(&ctx) } else { self.hide_to_tray(&ctx) }
        }
    }

    /// egui also brings the native title bar along (`Options::sync_window_theme`).
    fn apply_theme(&self, ctx: &egui::Context) {
        ctx.set_theme(match self.config.theme {
            Theme::System => ThemePreference::System,
            Theme::Light => ThemePreference::Light,
            Theme::Dark => ThemePreference::Dark,
        });
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
            self.notice = Some(Notice { text, undo: None, link: None });
            return;
        }
        if let Some(existing) = self.config.mappings.iter_mut().find(|m| m.from == mapping.from) {
            existing.to = mapping.to;
            let text = format!("{from} ya estaba reasignada. Ahora envía {to}.");
            self.notice = Some(Notice { text, undo: None, link: None });
        } else {
            self.config.mappings.push(mapping);
        }
        self.commit();
    }

    fn remove(&mut self, index: usize) {
        let m = self.config.mappings.remove(index);
        let text = format!("Se quitó {} → {}.", key_name(m.from), key_name(m.to));
        self.notice = Some(Notice { text, undo: Some((index, m)), link: None });
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
                    self.apply_theme(ui.ctx());
                    self.commit();
                }
            });
        });
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
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
            if !self.elevated {
                ui.label(icon_text(icon::WARNING, 13.0).color(p.muted)).on_hover_text(
                    "Remapr corre sin permisos de administrador: no reasigna teclas dentro de ventanas \
                     que sí los tienen (Administrador de tareas, instaladores). Instalado, inicia con ellos.",
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).small().color(p.muted));
                if let Some(mut on) = self.starts_at_sign_in {
                    ui.add_space(8.0);
                    let toggled = ui
                        .add_enabled_ui(self.elevated, |ui| switch(ui, &mut on, "Iniciar con Windows"))
                        .inner
                        .on_disabled_hover_text("Abre Remapr como administrador para cambiar esto.")
                        .changed();
                    ui.label(RichText::new("Iniciar con Windows").small().color(p.muted));
                    if toggled {
                        self.set_starts_at_sign_in(on);
                    }
                }
            });
        });
    }

    fn set_starts_at_sign_in(&mut self, on: bool) {
        match std::env::current_exe().and_then(|exe| startup::register(&exe, on)) {
            Ok(()) => self.starts_at_sign_in = Some(on),
            Err(e) => {
                let text = format!("No se pudo cambiar el inicio con Windows: {e}");
                self.notice = Some(Notice { text, undo: None, link: None });
            }
        }
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        if let Some(e) = &self.engine_error {
            danger_banner(ui, e);
            ui.add_space(4.0);
        }
        self.notice_bar(ui);
        self.updates.banner(ui);

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
                            if danger_icon_button(ui, icon::DELETE, "Quitar este mapeo").clicked() {
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
        let (mut close, mut undo, mut open) = (false, None, None);
        card(ui, false, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(RichText::new(&notice.text).color(p.text)).wrap());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close = icon_button(ui, icon::CLOSE, "Cerrar aviso").clicked();
                    if notice.undo.is_some() && ui.button("Deshacer").clicked() {
                        undo = notice.undo;
                    }
                    if notice.link.is_some() && ui.button("Novedades").clicked() {
                        open = notice.link.clone();
                    }
                });
            });
        });
        ui.add_space(4.0);
        if let Some(url) = open {
            remapr_platform::open_in_browser(&url);
        }
        if let Some((index, mapping)) = undo {
            self.undo(index, mapping);
        } else if close {
            self.notice = None;
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if std::mem::take(&mut self.start_hidden) {
            // Tells winit the window is hidden; `hide_first_show` already kept it off screen.
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }
        self.handle_commands(ctx);
        if self.updates.poll() {
            self.quit(ctx);
        }
        if self.capture == Capture::Idle {
            return;
        }
        // Capturing swallows every key system-wide; never keep doing it behind the user's back.
        // Windows' own answer, not egui's: its focus flag can lag behind.
        if !self.hwnd.is_some_and(remapr_platform::is_foreground) {
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
        if self.tray_hint_open {
            self.tray_hint(ui);
        }
    }
}

/// Releases come from GitHub for the installed app. A debug build can be pointed at a test folder
/// with `REMAPR_UPDATES_DIR` (see `remapr_update::Source::local`); other dev builds never update.
fn update_source(installed: bool) -> Option<remapr_update::Source> {
    if installed {
        return Some(remapr_update::Source::github());
    }
    let dir = std::env::var_os("REMAPR_UPDATES_DIR").filter(|_| cfg!(debug_assertions))?;
    Some(remapr_update::Source::local(std::path::Path::new(&dir)))
}
