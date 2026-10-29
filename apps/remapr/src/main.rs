// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod tray;
mod updates;

use std::time::Duration;

use remapr_platform::{SingleInstance, startup};

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    // Steps the installer runs, elevated. An upgrade keeps the user's "start at sign-in" choice.
    if has("--register-task") {
        let at_sign_in = startup::starts_at_sign_in().unwrap_or(true);
        installer_step(std::env::current_exe().and_then(|exe| startup::register(&exe, at_sign_in)));
    }
    if has("--unregister-task") {
        installer_step(startup::unregister());
    }
    let background = has(startup::BACKGROUND_ARG);

    // One Remapr per session: a second launch shows the running one's window instead.
    let Some(mut instance) = acquire_instance() else { return Ok(()) };
    // An installed Remapr runs elevated through its sign-in task, so remapping also reaches admin
    // windows, with no UAC prompt. A plain launch hands over to that copy.
    let installed = is_installed();
    if !background && installed && !remapr_platform::is_elevated() {
        drop(instance.take());
        if startup::run().is_ok() && SingleInstance::request_show(Duration::from_secs(3)) {
            return Ok(());
        }
        // No task, or it didn't start: run here, without admin rights.
        let Some(again) = acquire_instance() else { return Ok(()) };
        instance = again;
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Remapr")
            .with_inner_size([560.0, 640.0])
            .with_min_inner_size([460.0, 460.0])
            // No egui logo: the app sets the exe's own icon once the window exists.
            .with_icon(eframe::egui::IconData::default()),
        ..Default::default()
    };
    eframe::run_native(
        "Remapr",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, instance, background, installed)))),
    )
}

/// `None` when another Remapr already runs in this session (it was asked to show itself).
/// `Some(None)` when that couldn't be checked: running twice beats not running at all.
fn acquire_instance() -> Option<Option<SingleInstance>> {
    match SingleInstance::acquire() {
        Ok(None) => None,
        Ok(instance) => Some(instance),
        Err(_) => Some(None),
    }
}

/// Whether this is the copy the installer put in place (wherever the user chose), not a dev build.
fn is_installed() -> bool {
    let (Ok(exe), Some(dir)) = (std::env::current_exe(), remapr_platform::install_dir()) else {
        return false;
    };
    let folder = |p: &std::path::Path| p.to_string_lossy().trim_end_matches('\\').to_lowercase();
    exe.parent().is_some_and(|parent| folder(parent) == folder(&dir))
}

/// Ends an installer step with the exit code msiexec checks.
fn installer_step(result: std::io::Result<()>) -> ! {
    if let Err(e) = &result {
        eprintln!("remapr: {e}");
    }
    std::process::exit(if result.is_ok() { 0 } else { 1 })
}
