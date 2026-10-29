#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Until phase 2 wires the scanner, cleaner and apps modules into Tauri commands, most of them are
// only reachable from the egui UI.
#![cfg_attr(not(feature = "egui"), allow(dead_code))]

#[cfg(feature = "egui")]
mod app;
mod apps;
mod cleaner;
#[cfg(not(feature = "egui"))]
mod commands;
mod rules;
mod safety;
mod scanner;
#[cfg(feature = "egui")]
mod ui_kit;
mod util;
mod win;

#[cfg(not(feature = "egui"))]
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::init])
        .run(tauri::generate_context!())
        .expect("failed to start LogCleaner");
}

#[cfg(feature = "egui")]
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
