// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Remapr")
            .with_inner_size([560.0, 640.0])
            .with_min_inner_size([460.0, 460.0]),
        ..Default::default()
    };
    eframe::run_native("Remapr", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
