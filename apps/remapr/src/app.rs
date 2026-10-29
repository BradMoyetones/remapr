//! The main window: the list of remapped keys, adding one by pressing keys, pause and theme.
//! Closing it hides it to the tray; remapping keeps running until "Salir".

use std::sync::mpsc::{self, Receiver};

use eframe::egui::{
    self, Align, Frame, Layout, Margin, Rect, RichText, Sense, ThemePreference, ViewportCommand, pos2, vec2,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use remapr_engine::{Chord, Engine};
use remapr_platform::{SingleInstance, TitleBarDoubleClick, startup};
use remapr_ui::{
    KeyState, Palette, Tone, banner, card, danger_icon_button, icon, icon_button, icon_label, kbd, primary_button,
    semibold, switch, title_bar,
};

use crate::config::{self, Config, Mapping, Theme};
use crate::i18n::{tr, trn};
use crate::keys::{chord_name, chord_text};
use crate::tray::{self, Command, Tray};
use crate::updates::{self, Updates};

/// On macOS the window's content runs under its transparent title bar: the header leaves this much
/// room at the top for the native window buttons.
const MACOS_TITLE_BAR: i8 = 28;

/// Adding a mapping: first the key to change, then the key it should send.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Capture {
    Idle,
    From,
    To(Chord),
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
    /// Why there is no engine; `Info` when this system just doesn't have one yet.
    engine_error: Option<(Tone, String)>,
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
    /// Whether the title bar was last tinted for the dark theme; `None` before the first time.
    title_bar_dark: Option<bool>,
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
                let text = tr!("Remapr was updated to version {version}.", version = updates::VERSION);
                notice = Some(Notice { text, undo: None, link: Some(remapr_update::release_page(updates::VERSION)) });
            }
            config.last_version = updates::VERSION.into();
            let _ = config.save();
        }
        let ctx = cc.egui_ctx.clone();
        let (engine, engine_error) = match Engine::start(move || ctx.request_repaint()) {
            Ok(engine) => (Some(engine), None),
            Err(e) if e.kind() == std::io::ErrorKind::Unsupported => {
                let text = tr!("Remapr can't remap keys on this system yet: for now it only works on Windows.");
                (None, Some((Tone::Info, text.into())))
            }
            Err(e) => (None, Some((Tone::Danger, tr!("Couldn't start the keyboard engine: {error}", error = e)))),
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
        // Starts reading the system's setting now, so it is known by the first double-click.
        let _ = remapr_platform::title_bar_double_click();

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
            title_bar_dark: None,
            // Only the installed copy may touch the task: a dev build would point it at itself.
            starts_at_sign_in: installed.then(startup::starts_at_sign_in).flatten(),
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
            engine.set_rules(self.config.mappings.iter().map(Mapping::rule).collect());
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
                Some(Notice { text: tr!("Couldn't save the settings: {error}", error = e), undo: None, link: None });
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
            .backdrop_color(p.scrim)
            .frame(Frame::new().fill(p.surface).corner_radius(12).inner_margin(20).stroke((1.0, p.border)))
            .show(&ctx, |ui| {
                ui.set_width(360.0);
                ui.label(RichText::new(tr!("Remapr is still running")).font(semibold(17.0)).color(p.text));
                ui.add_space(2.0);
                ui.label(
                    RichText::new(tr!("Your keys stay remapped. Find Remapr in the notification area, by the clock: from there you can open it, pause it or quit."))
                    .color(p.muted),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, None, tr!("Got it")).clicked() {
                        choice = Some(false);
                    }
                    if ui.button(tr!("Quit Remapr")).clicked() {
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
        let (preference, dark) = match self.config.theme {
            Theme::System => (ThemePreference::System, None),
            Theme::Light => (ThemePreference::Light, Some(false)),
            Theme::Dark => (ThemePreference::Dark, Some(true)),
        };
        ctx.set_theme(preference);
        remapr_platform::keep_title_bar_theme(dark);
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

    fn on_captured(&mut self, chord: Chord) {
        match self.capture {
            Capture::Idle => {}
            Capture::From => {
                self.capture = Capture::To(chord);
                if let Some(engine) = &self.engine {
                    engine.start_capture();
                }
            }
            Capture::To(from) => {
                self.capture = Capture::Idle;
                self.add(Mapping::new(from, chord));
            }
        }
    }

    fn add(&mut self, mapping: Mapping) {
        let (from, to) = (chord_text(mapping.source()), chord_text(mapping.target()));
        if mapping.source() == mapping.target() {
            let text = tr!("You chose {keys} twice, so nothing changed.", keys = from);
            self.notice = Some(Notice { text, undo: None, link: None });
            return;
        }
        if let Some(existing) = self.config.mappings.iter_mut().find(|m| m.clashes_with(&mapping)) {
            (existing.to, existing.to_mods) = (mapping.to, mapping.to_mods);
            let text = tr!("{from} was already remapped. It now sends {to}.", from = from, to = to);
            self.notice = Some(Notice { text, undo: None, link: None });
        } else {
            self.config.mappings.push(mapping);
        }
        self.commit();
    }

    fn remove(&mut self, index: usize) {
        let m = self.config.mappings.remove(index);
        let text = tr!("Removed {from} → {to}.", from = chord_text(m.source()), to = chord_text(m.target()));
        self.notice = Some(Notice { text, undo: Some((index, m)), link: None });
        self.commit();
    }

    fn export_mappings(&mut self) {
        let kind = remapr_platform::FileKind { name: tr!("Remapr mappings"), extension: "toml" };
        let Some(path) =
            remapr_platform::save_file_dialog(self.hwnd, tr!("Export mappings"), tr!("remapr-mappings.toml"), &kind)
        else {
            return;
        };
        let text = match std::fs::write(&path, config::export(&self.config.mappings)) {
            Ok(()) => tr!(
                "Exported {count} to {file}.",
                count = trn!("{n} mapping|{n} mappings", self.config.mappings.len()),
                file = path.display()
            ),
            Err(e) => tr!("Couldn't export: {error}", error = e),
        };
        self.notice = Some(Notice { text, undo: None, link: None });
    }

    /// Adds the mappings of an export file; one that fires on the same keys in the same place
    /// replaces the one already here.
    fn import_mappings(&mut self) {
        let kind = remapr_platform::FileKind { name: tr!("Remapr mappings"), extension: "toml" };
        let Some(path) = remapr_platform::open_file_dialog(self.hwnd, tr!("Import mappings"), &kind) else { return };
        let imported = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| config::import(&t));
        let text = match imported {
            Ok(mappings) => {
                let (total, mut replaced) = (mappings.len(), 0);
                for mapping in mappings {
                    if let Some(existing) = self.config.mappings.iter_mut().find(|m| m.clashes_with(&mapping)) {
                        *existing = mapping;
                        replaced += 1;
                    } else {
                        self.config.mappings.push(mapping);
                    }
                }
                self.commit();
                let total = trn!("{n} mapping|{n} mappings", total);
                match replaced {
                    0 => tr!("Imported {count}.", count = total),
                    1 => tr!("Imported {count}; one replaced the mapping you had on the same keys.", count = total),
                    n => tr!(
                        "Imported {count}; {replaced} replaced the mappings you had on the same keys.",
                        count = total,
                        replaced = n
                    ),
                }
            }
            Err(e) => tr!("Couldn't import: {error}", error = e),
        };
        self.notice = Some(Notice { text, undo: None, link: None });
    }

    /// Limits mapping `index` to `app`, or lets it apply everywhere with `None`.
    fn set_app(&mut self, index: usize, app: Option<String>) {
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
        self.commit();
    }

    fn undo(&mut self, index: usize, mapping: Mapping) {
        self.notice = None;
        if self.config.mappings.iter().all(|m| !m.clashes_with(&mapping)) {
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
                ui.label(RichText::new(tr!("Remap any key on your keyboard")).color(p.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut enabled = self.config.enabled;
                if switch(ui, &mut enabled, tr!("Remapping on")).changed() {
                    self.config.enabled = enabled;
                    self.commit();
                }
                ui.label(RichText::new(if enabled { tr!("On") } else { tr!("Paused") }).color(p.muted));
                ui.add_space(4.0);
                let (glyph, label, next) = match self.config.theme {
                    Theme::System => (icon::SYSTEM, tr!("Theme: same as the system"), Theme::Light),
                    Theme::Light => (icon::SUN, tr!("Theme: light"), Theme::Dark),
                    Theme::Dark => (icon::MOON, tr!("Theme: dark"), Theme::System),
                };
                if icon_button(ui, glyph, label).clicked() {
                    self.config.theme = next;
                    self.apply_theme(ui.ctx());
                    self.commit();
                }
                let mut action = None;
                let more = icon_button(ui, icon::MORE, tr!("More options"));
                egui::Popup::menu(&more).show(|ui| {
                    let has_mappings = !self.config.mappings.is_empty();
                    if ui.add_enabled(has_mappings, egui::Button::new(tr!("Export mappings…"))).clicked() {
                        action = Some(true);
                    }
                    if ui.button(tr!("Import mappings…")).clicked() {
                        action = Some(false);
                    }
                    if action.is_some() {
                        ui.close();
                    }
                });
                match action {
                    Some(true) => self.export_mappings(),
                    Some(false) => self.import_mappings(),
                    None => {}
                }
            });
        });
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        let (color, status) = match (&self.engine, self.config.enabled) {
            (None, _) => (p.danger, tr!("Engine stopped")),
            (Some(_), true) => (p.success, tr!("Engine running")),
            (Some(_), false) => (p.muted, tr!("Paused: your keys work as usual")),
        };
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.label(RichText::new(status).small().color(p.muted));
            // Administrator rights only matter to the Windows hook.
            if cfg!(windows) && !self.elevated {
                icon_label(ui, icon::WARNING, 14.0, p.muted).on_hover_text(
                    tr!("Remapr runs without administrator rights: it doesn't remap keys inside windows that have them (Task Manager, installers). Installed, it starts with them."),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).small().color(p.muted));
                self.updates.check_button(ui);
                if let Some(mut on) = self.starts_at_sign_in {
                    ui.add_space(8.0);
                    let toggled = ui
                        .add_enabled_ui(self.elevated, |ui| switch(ui, &mut on, tr!("Start with Windows")))
                        .inner
                        .on_disabled_hover_text(tr!("Open Remapr as administrator to change this."))
                        .changed();
                    ui.label(RichText::new(tr!("Start with Windows")).small().color(p.muted));
                    if toggled {
                        self.set_starts_at_sign_in(on);
                    }
                }
            });
        });
    }

    /// Windows: the native title bar in the palette's colours, so it and the header read as one.
    fn tint_title_bar(&mut self, ui: &egui::Ui) {
        let dark = ui.visuals().dark_mode;
        let Some(hwnd) = self.hwnd.filter(|_| self.title_bar_dark != Some(dark)) else { return };
        let p = Palette::of(ui);
        let rgb = |c: egui::Color32| [c.r(), c.g(), c.b()];
        remapr_platform::tint_title_bar(hwnd, rgb(p.bg), rgb(p.text), rgb(p.border));
        self.title_bar_dark = Some(dark);
    }

    /// macOS: dragging the header's empty part moves the window; a double-click does what the
    /// system's setting says.
    fn title_bar(&self, ui: &mut egui::Ui, rect: Rect) {
        let response = title_bar::empty_area(ui, rect);
        if !response.double_clicked() || title_bar::is_fullscreen(ui) {
            return;
        }
        let command = match remapr_platform::title_bar_double_click() {
            TitleBarDoubleClick::Zoom => ViewportCommand::Maximized(!title_bar::is_maximized(ui)),
            TitleBarDoubleClick::Minimize => ViewportCommand::Minimized(true),
            TitleBarDoubleClick::Nothing => return,
        };
        ui.ctx().send_viewport_cmd(command);
    }

    fn set_starts_at_sign_in(&mut self, on: bool) {
        match std::env::current_exe().and_then(|exe| startup::register(&exe, on)) {
            Ok(()) => self.starts_at_sign_in = Some(on),
            Err(e) => {
                let text = tr!("Couldn't change starting with Windows: {error}", error = e);
                self.notice = Some(Notice { text, undo: None, link: None });
            }
        }
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        if let Some((tone, e)) = &self.engine_error {
            banner(ui, *tone, e);
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
            ui.label(RichText::new(tr!("Remapped keys")).font(semibold(16.0)).color(p.text));
            ui.label(RichText::new(self.config.mappings.len().to_string()).color(p.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let can_add = !capturing && self.engine.is_some();
                if ui.add_enabled_ui(can_add, |ui| primary_button(ui, Some(icon::ADD), tr!("Add"))).inner.clicked() {
                    self.start_capture();
                }
            });
        });
        ui.add_space(4.0);

        if capturing {
            self.capture_card(ui);
        }

        let (mut remove, mut set_app) = (None, None);
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            if !self.config.enabled {
                ui.multiply_opacity(0.55);
            }
            for (i, m) in self.config.mappings.iter().enumerate() {
                card(ui, false, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        chord_caps(ui, m.source(), KeyState::Idle);
                        icon_label(ui, icon::FORWARD, 16.0, p.muted);
                        chord_caps(ui, m.target(), KeyState::Idle);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if danger_icon_button(ui, icon::DELETE, tr!("Remove this mapping")).clicked() {
                                remove = Some(i);
                            }
                            if let Some(app) = app_picker(ui, m.app.as_deref()) {
                                set_app = Some((i, app));
                            }
                        });
                    });
                });
            }
        });
        if let Some(i) = remove {
            self.remove(i);
        } else if let Some((i, app)) = set_app {
            self.set_app(i, app);
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
                        chord_caps(ui, from, KeyState::Idle);
                        icon_label(ui, icon::FORWARD, 16.0, p.muted);
                        kbd(ui, "?", KeyState::Listening);
                    }
                    _ => {
                        kbd(ui, "?", KeyState::Listening);
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    cancel = ui.button(tr!("Cancel")).clicked();
                });
            });
            let step = match self.capture {
                Capture::To(_) => tr!("Now press the key or shortcut it should send instead."),
                _ => tr!("Press the key or shortcut you want to change."),
            };
            ui.label(RichText::new(step).font(semibold(14.0)).color(p.text));
            ui.label(
                RichText::new(tr!("For a shortcut, hold Ctrl, Shift, Alt or Win and press the key. Meanwhile, the keyboard doesn't type into other apps."))
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
            icon_label(ui, icon::KEYBOARD, 44.0, p.muted);
            ui.add_space(8.0);
            ui.label(RichText::new(tr!("No keys remapped yet")).font(semibold(17.0)).color(p.text));
            ui.label(
                RichText::new(tr!("For example, turn Caps Lock into Esc. Press the button, then the two keys."))
                    .color(p.muted),
            );
            ui.add_space(12.0);
            if ui
                .add_enabled_ui(self.engine.is_some(), |ui| {
                    primary_button(ui, Some(icon::ADD), tr!("Add my first mapping"))
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
                    close = icon_button(ui, icon::CLOSE, tr!("Close notice")).clicked();
                    if notice.undo.is_some() && ui.button(tr!("Undo")).clicked() {
                        undo = notice.undo.clone();
                    }
                    if notice.link.is_some() && ui.button(tr!("What's new")).clicked() {
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
        // winit puts the system theme back on the title bar after some Windows settings changes.
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
        self.tint_title_bar(ui);
        let mac = cfg!(target_os = "macos");
        let margin = Margin { left: 20, right: 20, top: if mac { 18 + MACOS_TITLE_BAR } else { 18 }, bottom: 10 };
        egui::Panel::top("header").frame(Frame::new().fill(bg).inner_margin(margin)).show(ui, |ui| {
            if mac {
                // The whole header, margins included, is the title bar there.
                let inner = ui.max_rect();
                let rect = Rect::from_min_max(
                    pos2(inner.left() - f32::from(margin.left), inner.top() - f32::from(margin.top)),
                    pos2(inner.right() + f32::from(margin.right), inner.bottom() + f32::from(margin.bottom)),
                );
                self.title_bar(ui, rect);
            }
            self.header(ui);
        });
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

/// A chord as keycaps joined by small plus signs.
fn chord_caps(ui: &mut egui::Ui, chord: Chord, state: KeyState) {
    let muted = Palette::of(ui).muted;
    ui.spacing_mut().item_spacing.x = 4.0;
    for (i, name) in chord_name(chord).iter().enumerate() {
        if i > 0 {
            ui.label(RichText::new("+").color(muted));
        }
        kbd(ui, name, state);
    }
    ui.spacing_mut().item_spacing.x = 8.0;
}

/// Where a mapping applies: every app, or one program picked from those with a window open.
/// Returns the new choice when the user makes one.
fn app_picker(ui: &mut egui::Ui, current: Option<&str>) -> Option<Option<String>> {
    let p = Palette::of(ui);
    let label = current.map_or(tr!("All apps").to_owned(), |exe| tr!("Only in {app}", app = exe));
    let mut picked = None;
    ui.menu_button(RichText::new(label).small().color(if current.is_some() { p.accent } else { p.muted }), |ui| {
        ui.set_max_width(320.0);
        if ui.selectable_label(current.is_none(), tr!("All apps")).clicked() {
            picked = Some(None);
        }
        ui.separator();
        ui.label(RichText::new(tr!("Only while this app is in front:")).small().color(p.muted));
        let apps = remapr_platform::running_apps();
        // Keep the current app listed even when it isn't open right now.
        let missing = current.filter(|exe| apps.iter().all(|a| a.exe != *exe));
        if let Some(exe) = missing
            && ui.selectable_label(true, exe).clicked()
        {
            picked = Some(Some(exe.to_owned()));
        }
        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            for app in &apps {
                let text = RichText::new(format!("{}  ·  {}", app.exe, truncate(&app.title, 32)));
                if ui.selectable_label(current == Some(app.exe.as_str()), text).clicked() {
                    picked = Some(Some(app.exe.clone()));
                }
            }
        });
        if picked.is_some() {
            ui.close();
        }
    });
    picked
}

fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_owned(),
    }
}
