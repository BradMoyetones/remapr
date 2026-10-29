// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod tray;

fn main() -> eframe::Result {
    // One Remapr per session: a second launch shows the running one's window instead.
    let instance = match remapr_platform::SingleInstance::acquire() {
        Ok(None) => return Ok(()),
        Ok(instance) => instance,
        // Couldn't tell; running twice beats not running at all.
        Err(_) => None,
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Remapr")
            .with_inner_size([560.0, 640.0])
            .with_min_inner_size([460.0, 460.0])
            // No egui logo: the app sets the exe's own icon once the window exists.
            .with_icon(eframe::egui::IconData::default()),
        ..Default::default()
    };
    eframe::run_native("Remapr", options, Box::new(|cc| Ok(Box::new(app::App::new(cc, instance)))))
}
