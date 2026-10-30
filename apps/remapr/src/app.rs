//! The main window: the list of remapped keys, adding one by pressing keys, pause and theme.
//! Closing it hides it to the tray; remapping keeps running until "Salir".
//!
//! What happens to the mappings is the [`Session`]'s; the window draws it, forwards clicks to it,
//! and saves the settings when the session changed them.

use std::sync::mpsc::{self, Receiver};

use eframe::egui::{
    self, Align, Frame, Layout, Margin, Rect, RichText, Sense, ThemePreference, ViewportCommand, pos2, vec2,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use remapr_core::{AppMatcher, Chord, Permission, Support};
use remapr_platform::{SingleInstance, TitleBarDoubleClick, startup};
use remapr_ui::{
    KeyState, Palette, Tone, banner, card, danger_icon_button, icon, icon_button, icon_label, kbd, primary_button,
    semibold, switch, title_bar,
};

use crate::config::{self, Config, Theme};
use crate::i18n::{tr, trn};
use crate::keys::{chord_name, modifier_names};
use crate::session::{Capture, Notice, Session};
use crate::tray::{self, Command, Tray};
use crate::updates::{self, Updates};

/// On macOS the window's content runs under its transparent title bar: the header leaves this much
/// room at the top for the native window buttons.
const MACOS_TITLE_BAR: i8 = 28;

pub struct App {
    session: Session,
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
        let mut notice = load_warning.map(Notice::text);
        if config.last_version != updates::VERSION {
            if !config.last_version.is_empty() {
                let text = tr!("Remapr was updated to version {version}.", version = updates::VERSION);
                notice = Some(Notice { text, undo: None, link: Some(remapr_update::release_page(updates::VERSION)) });
            }
            config.last_version = updates::VERSION.into();
            let _ = config.save();
        }
        let ctx = cc.egui_ctx.clone();
        let engine = remapr_engine::start(move || ctx.request_repaint());
        match &engine {
            // Expected where Remapr has no backend yet.
            Err(e) if e.kind() == std::io::ErrorKind::Unsupported => log::info!("no keyboard engine: {e}"),
            Err(e) => log::error!("no keyboard engine: {e}"),
            Ok(_) => {}
        }
        let mut session = Session::new(config, engine, remapr_engine::capabilities());
        session.notice = notice;

        let (sender, commands) = mpsc::channel();
        if let Some(instance) = instance {
            let (sender, ctx) = (sender.clone(), cc.egui_ctx.clone());
            instance.listen(move || {
                let _ = sender.send(Command::Show);
                ctx.request_repaint();
            });
        }
        let tray = Tray::new(session.config.enabled, sender, cc.egui_ctx.clone())
            // Expected outside Windows for now.
            .inspect_err(|e| {
                log::log!(if cfg!(windows) { log::Level::Warn } else { log::Level::Info }, "no tray icon: {e}")
            })
            .ok();
        let elevated = remapr_platform::is_elevated();
        // Starts reading the system's setting now, so it is known by the first double-click.
        let _ = remapr_platform::title_bar_double_click();

        let app = Self {
            session,
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
        app
    }

    /// Saves the settings and shows them in the tray, if the session changed them.
    fn persist(&mut self) {
        if !self.session.take_unsaved() {
            return;
        }
        if let Err(e) = self.session.config.save() {
            log::error!("couldn't save the settings: {e}");
            self.session.notice = Some(Notice::text(tr!("Couldn't save the settings: {error}", error = e)));
        }
        if let Some(tray) = &self.tray {
            tray.sync(self.session.config.enabled);
        }
    }

    fn show_window(ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }

    fn hide_to_tray(&mut self, ctx: &egui::Context) {
        self.session.cancel_capture();
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
                Command::TogglePause => self.session.set_enabled(!self.session.config.enabled),
                Command::Quit => self.quit(ctx),
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.tray.is_some() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            if self.session.config.tray_hint_seen {
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
            self.session.config.tray_hint_seen = true;
            self.session.changed();
            if quit { self.quit(&ctx) } else { self.hide_to_tray(&ctx) }
        }
    }

    /// egui also brings the native title bar along (`Options::sync_window_theme`).
    fn apply_theme(&self, ctx: &egui::Context) {
        let (preference, dark) = match self.session.config.theme {
            Theme::System => (ThemePreference::System, None),
            Theme::Light => (ThemePreference::Light, Some(false)),
            Theme::Dark => (ThemePreference::Dark, Some(true)),
        };
        ctx.set_theme(preference);
        remapr_platform::keep_title_bar_theme(dark);
    }

    fn export_mappings(&mut self) {
        let kind = remapr_platform::FileKind { name: tr!("Remapr mappings"), extension: "toml" };
        let Some(path) =
            remapr_platform::save_file_dialog(self.hwnd, tr!("Export mappings"), tr!("remapr-mappings.toml"), &kind)
        else {
            return;
        };
        let mappings = &self.session.config.mappings;
        let text = match std::fs::write(&path, config::export(mappings)) {
            Ok(()) => tr!(
                "Exported {count} to {file}.",
                count = trn!("{n} mapping|{n} mappings", mappings.len()),
                file = path.display()
            ),
            Err(e) => tr!("Couldn't export: {error}", error = e),
        };
        self.session.notice = Some(Notice::text(text));
    }

    /// Adds the mappings of an export file; one that fires on the same keys in the same place
    /// replaces the one already here.
    fn import_mappings(&mut self) {
        let kind = remapr_platform::FileKind { name: tr!("Remapr mappings"), extension: "toml" };
        let Some(path) = remapr_platform::open_file_dialog(self.hwnd, tr!("Import mappings"), &kind) else { return };
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| config::import(&t)) {
            Ok(mappings) => self.session.import(mappings),
            Err(e) => self.session.notice = Some(Notice::text(tr!("Couldn't import: {error}", error = e))),
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
                let mut enabled = self.session.config.enabled;
                if switch(ui, &mut enabled, tr!("Remapping on")).changed() {
                    self.session.set_enabled(enabled);
                }
                ui.label(RichText::new(if enabled { tr!("On") } else { tr!("Paused") }).color(p.muted));
                ui.add_space(4.0);
                let (glyph, label, next) = match self.session.config.theme {
                    Theme::System => (icon::SYSTEM, tr!("Theme: same as the system"), Theme::Light),
                    Theme::Light => (icon::SUN, tr!("Theme: light"), Theme::Dark),
                    Theme::Dark => (icon::MOON, tr!("Theme: dark"), Theme::System),
                };
                if icon_button(ui, glyph, label).clicked() {
                    self.session.config.theme = next;
                    self.apply_theme(ui.ctx());
                    self.session.changed();
                }
                let mut action = None;
                let more = icon_button(ui, icon::MORE, tr!("More options"));
                egui::Popup::menu(&more).show(|ui| {
                    let has_mappings = !self.session.config.mappings.is_empty();
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
        let waiting = matches!(self.session.capabilities().keyboard, Support::NeedsPermission(_));
        let (color, status) = match (self.session.is_running(), self.session.config.enabled) {
            (false, _) => (p.danger, tr!("Engine stopped")),
            (true, false) => (p.muted, tr!("Paused: your keys work as usual")),
            (true, true) if waiting => (p.warning, tr!("Waiting for permission")),
            (true, true) if self.session.blocked() => (p.muted, tr!("On hold while a password is typed")),
            (true, true) => (p.success, tr!("Engine running")),
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
                self.session.notice = Some(Notice::text(text));
            }
        }
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        if let Some((tone, e)) = self.session.status() {
            banner(ui, *tone, e);
            ui.add_space(4.0);
        }
        if let Support::NeedsPermission(permission) = self.session.capabilities().keyboard {
            self.permission_card(ui, permission);
        }
        if self.session.blocked() {
            banner(
                ui,
                Tone::Info,
                tr!(
                    "macOS keeps every key from Remapr while you type a password. Remapping resumes once you leave the field."
                ),
            );
            ui.add_space(4.0);
        }
        self.notice_bar(ui);
        self.updates.banner(ui);

        let capturing = self.session.capture() != Capture::Idle;
        if self.session.config.mappings.is_empty() && !capturing {
            self.empty_state(ui);
            return;
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new(tr!("Remapped keys")).font(semibold(16.0)).color(p.text));
            ui.label(RichText::new(self.session.config.mappings.len().to_string()).color(p.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let can_add = !capturing && self.session.can_capture();
                if ui.add_enabled_ui(can_add, |ui| primary_button(ui, Some(icon::ADD), tr!("Add"))).inner.clicked() {
                    self.session.start_capture();
                }
            });
        });
        ui.add_space(4.0);

        if capturing {
            self.capture_card(ui);
        }

        let (mut remove, mut set_app) = (None, None);
        let per_app = self.session.per_app_rules();
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            if !self.session.config.enabled {
                ui.multiply_opacity(0.55);
            }
            for (i, m) in self.session.config.mappings.iter().enumerate() {
                card(ui, false, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        chord_caps(ui, m.from, KeyState::Idle);
                        icon_label(ui, icon::FORWARD, 16.0, p.muted);
                        chord_caps(ui, m.to, KeyState::Idle);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if danger_icon_button(ui, icon::DELETE, tr!("Remove this mapping")).clicked() {
                                remove = Some(i);
                            }
                            if per_app && let Some(app) = app_picker(ui, m.app.as_ref()) {
                                set_app = Some((i, app));
                            }
                        });
                    });
                });
            }
        });
        if let Some(i) = remove {
            self.session.remove(i);
        } else if let Some((i, app)) = set_app {
            self.session.set_app(i, app);
        }
    }

    fn capture_card(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        let mut cancel = false;
        card(ui, true, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                match self.session.capture() {
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
            let step = match self.session.capture() {
                Capture::To(_) => tr!("Now press the key or shortcut it should send instead."),
                _ => tr!("Press the key or shortcut you want to change."),
            };
            ui.label(RichText::new(step).font(semibold(14.0)).color(p.text));
            ui.label(
                RichText::new(tr!("For a shortcut, hold a modifier ({modifiers}) and press the key. Meanwhile, the keyboard doesn't type into other apps.", modifiers = modifier_names()))
                .small()
                .color(p.muted),
            );
        });
        ui.add_space(4.0);
        if cancel {
            self.session.cancel_capture();
        }
    }

    /// What the system asks before Remapr may change keys, and a way to grant it.
    fn permission_card(&mut self, ui: &mut egui::Ui, permission: Permission) {
        let (title, text, pane) = match permission {
            Permission::Accessibility => (
                tr!("Let Remapr change keys"),
                tr!(
                    "macOS asks you to allow it once: turn Remapr on in System Settings › Privacy & Security › Accessibility. Until then, keys swapped in every app still work; shortcuts, mappings for one app, and adding a mapping by pressing keys need it."
                ),
                "Privacy_Accessibility",
            ),
            Permission::InputMonitoring => (
                tr!("Let Remapr read keys"),
                tr!(
                    "macOS asks you to allow it once: turn Remapr on in System Settings › Privacy & Security › Input Monitoring."
                ),
                "Privacy_ListenEvent",
            ),
            // Linux's device access comes with its backend (Phase 11).
            Permission::InputDevices => return,
        };
        let p = Palette::of(ui);
        card(ui, true, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).font(semibold(15.0)).color(p.text));
            ui.add(egui::Label::new(RichText::new(text).color(p.muted)).wrap());
            ui.add_space(4.0);
            if primary_button(ui, None, tr!("Open System Settings")).clicked() {
                self.session.ask_permission(permission);
                remapr_platform::open_privacy_settings(pane);
            }
            ui.label(RichText::new(tr!("Remapr notices by itself once it's allowed.")).small().color(p.muted));
        });
        ui.add_space(4.0);
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
                .add_enabled_ui(self.session.can_capture(), |ui| {
                    primary_button(ui, Some(icon::ADD), tr!("Add my first mapping"))
                })
                .inner
                .clicked()
            {
                self.session.start_capture();
            }
        });
    }

    fn notice_bar(&mut self, ui: &mut egui::Ui) {
        let Some(notice) = &self.session.notice else { return };
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
            self.session.undo(index, mapping);
        } else if close {
            self.session.notice = None;
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
        self.session.poll();
        // Capturing swallows every key system-wide; never keep doing it behind the user's back.
        // On Windows, Windows' own answer, not egui's: its focus flag can lag behind.
        let in_front = match self.hwnd {
            Some(hwnd) => remapr_platform::is_foreground(hwnd),
            None => ctx.input(|i| i.focused),
        };
        if self.session.capture() != Capture::Idle && !in_front {
            self.session.cancel_capture();
        }
        // Here too, not only after `ui`: a hidden window runs no `ui`, and the tray can pause.
        self.persist();
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
        self.persist();
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
fn app_picker(ui: &mut egui::Ui, current: Option<&AppMatcher>) -> Option<Option<AppMatcher>> {
    let p = Palette::of(ui);
    let label = current.map_or(tr!("All apps").to_owned(), |app| tr!("Only in {app}", app = app.name()));
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
        let is_current = |exe: &str| current.is_some_and(|c| c.matches(remapr_core::App::Exe(exe)));
        let missing = current.filter(|_| !apps.iter().any(|a| is_current(&a.exe)));
        if let Some(app) = missing
            && ui.selectable_label(true, app.name()).clicked()
        {
            picked = Some(Some(app.clone()));
        }
        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            for app in &apps {
                let text = RichText::new(format!("{}  ·  {}", app.exe, truncate(&app.title, 32)));
                if ui.selectable_label(is_current(&app.exe), text).clicked() {
                    picked = Some(Some(AppMatcher::Exe(app.exe.clone())));
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
