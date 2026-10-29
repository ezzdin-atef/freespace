#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod apps;
mod cleaner;
mod rules;
mod safety;
mod scanner;
mod ui_kit;
mod util;
mod win;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("LogCleaner")
            .with_inner_size([1400.0, 860.0])
            .with_min_inner_size([1150.0, 680.0])
            // Dev aid: open without stealing keyboard focus (used for screenshot checks).
            .with_active(std::env::var_os("LOGCLEANER_BACKGROUND").is_none()),
        ..Default::default()
    };
    eframe::run_native(
        "LogCleaner",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
