//! The "new version" banner: checks for releases in the background, then downloads, verifies and
//! installs one when the user asks. The installer ends this Remapr and starts the new one.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui::{self, Align, Layout, RichText};
use remapr_ui::{Palette, card, icon, icon_text, primary_button, semibold};
use remapr_update::{Release, Source};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Between background checks. The first one comes a few seconds after start.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

enum Event {
    Found(Release),
    Progress(f32),
    Installing,
    /// The installer runs elevated and can't be stopped by a UAC prompt: quit so it needn't force us.
    QuitForInstaller,
    Failed(String),
}

enum State {
    Idle,
    Available(Release),
    Downloading(Release, f32),
    Installing(Release),
    Failed(Release, String),
}

enum Action {
    Update,
    Later,
    Close,
    Notes(String),
}

pub struct Updates {
    state: State,
    /// "Más tarde" on this version: not offered again until Remapr restarts.
    dismissed: Option<String>,
    events: Receiver<Event>,
    sender: Sender<Event>,
    elevated: bool,
}

impl Updates {
    /// Checks in the background when there is a `source`: the installed app, or a debug build
    /// pointed at a test folder.
    pub fn new(ctx: &egui::Context, source: Option<Source>, elevated: bool) -> Self {
        let (sender, events) = mpsc::channel();
        if let Some(source) = source {
            let (sender, ctx) = (sender.clone(), ctx.clone());
            let _ = std::thread::Builder::new().name("remapr-updates".into()).spawn(move || {
                std::thread::sleep(Duration::from_secs(5));
                loop {
                    // A failed background check isn't worth bothering the user: the next one comes later.
                    if let Ok(Some(release)) = source.check(VERSION) {
                        let _ = sender.send(Event::Found(release));
                        ctx.request_repaint();
                    }
                    std::thread::sleep(CHECK_EVERY);
                }
            });
        }
        Self { state: State::Idle, dismissed: None, events, sender, elevated }
    }

    /// Takes in what the background threads report. `true` when Remapr must quit for the installer.
    pub fn poll(&mut self) -> bool {
        let mut quit = false;
        while let Ok(event) = self.events.try_recv() {
            let state = std::mem::replace(&mut self.state, State::Idle);
            self.state = match (state, event) {
                (State::Idle | State::Available(_), Event::Found(r)) if self.dismissed.as_ref() != Some(&r.version) => {
                    State::Available(r)
                }
                (State::Downloading(r, _), Event::Progress(p)) => State::Downloading(r, p),
                (State::Downloading(r, _), Event::Installing) => State::Installing(r),
                (State::Downloading(r, _) | State::Installing(r), Event::Failed(e)) => State::Failed(r, e),
                (state, Event::QuitForInstaller) => {
                    quit = true;
                    state
                }
                (state, _) => state,
            };
        }
        quit
    }

    fn start(&mut self, ctx: &egui::Context, release: Release) {
        self.state = State::Downloading(release.clone(), 0.0);
        let (sender, ctx, elevated) = (self.sender.clone(), ctx.clone(), self.elevated);
        let _ = std::thread::Builder::new().name("remapr-update".into()).spawn(move || {
            let send = |event| {
                let _ = sender.send(event);
                ctx.request_repaint();
            };
            let dir = std::env::temp_dir().join("Remapr").join("update");
            let result = remapr_update::download(&release, &dir, |p| send(Event::Progress(p))).and_then(|msi| {
                send(Event::Installing);
                let mut installer = remapr_update::install(&msi)?;
                if elevated {
                    send(Event::QuitForInstaller);
                    return Ok(());
                }
                // A UAC prompt comes first; once accepted, the installer ends this Remapr itself.
                remapr_update::install_result(installer.wait().ok().and_then(|s| s.code()))
            });
            if let Err(e) = result {
                send(Event::Failed(e));
            }
        });
    }

    /// The banner, when there is something to say.
    pub fn banner(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        let mut action = None;
        match &self.state {
            State::Idle => return,
            State::Available(release) => {
                card(ui, true, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(icon_text(icon::DOWNLOAD, 18.0).color(p.accent));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let title = format!("Nueva versión disponible: v{}", release.version);
                            ui.label(RichText::new(title).font(semibold(14.0)).color(p.text));
                            let detail = format!("Tienes la v{VERSION}. Se instala sola y Remapr se reinicia.");
                            ui.label(RichText::new(detail).small().color(p.muted));
                        });
                    });
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        if ui.link(RichText::new("Novedades").color(p.accent)).clicked() {
                            action = Some(Action::Notes(release.notes_url.clone()));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if primary_button(ui, None, "Actualizar y reiniciar").clicked() {
                                action = Some(Action::Update);
                            }
                            if ui.button("Más tarde").clicked() {
                                action = Some(Action::Later);
                            }
                        });
                    });
                });
            }
            State::Downloading(release, progress) => {
                card(ui, true, |ui| {
                    ui.set_width(ui.available_width());
                    let text = format!("Descargando la v{}…", release.version);
                    ui.label(RichText::new(text).font(semibold(14.0)).color(p.text));
                    ui.add(egui::ProgressBar::new(*progress).show_percentage().fill(p.accent));
                });
            }
            State::Installing(release) => {
                card(ui, true, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spinner();
                        let text = format!("Instalando la v{}. Remapr se reiniciará solo.", release.version);
                        ui.label(RichText::new(text).color(p.text));
                    });
                });
            }
            State::Failed(_, error) => {
                card(ui, false, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(icon_text(icon::WARNING, 16.0).color(p.danger));
                        ui.add(
                            egui::Label::new(RichText::new(format!("No se pudo actualizar. {error}")).color(p.danger))
                                .wrap(),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if primary_button(ui, None, "Reintentar").clicked() {
                                action = Some(Action::Update);
                            }
                            if ui.button("Cerrar").clicked() {
                                action = Some(Action::Close);
                            }
                        });
                    });
                });
            }
        }
        ui.add_space(4.0);

        match action {
            Some(Action::Notes(url)) => remapr_platform::open_in_browser(&url),
            Some(Action::Update) => {
                if let State::Available(release) | State::Failed(release, _) =
                    std::mem::replace(&mut self.state, State::Idle)
                {
                    self.start(ui.ctx(), release);
                }
            }
            Some(Action::Later) => {
                if let State::Available(release) = std::mem::replace(&mut self.state, State::Idle) {
                    self.dismissed = Some(release.version);
                }
            }
            Some(Action::Close) => self.state = State::Idle,
            None => {}
        }
    }
}
