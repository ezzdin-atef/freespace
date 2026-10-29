#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(feature = "egui")]
mod app;
mod apps;
mod cleaner;
#[cfg(not(feature = "egui"))]
mod commands;
mod rules;
mod safety;
mod scanner;
#[cfg(not(feature = "egui"))]
mod state;
#[cfg(feature = "egui")]
mod ui_kit;
mod util;
mod win;

#[cfg(not(feature = "egui"))]
fn main() {
    use commands::*;
    tauri::Builder::default()
        .manage(Shared::default())
        .setup(|app| {
            start_apps_load(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            init,
            status,
            start_scan,
            stop_scan,
            groups,
            findings,
            full_clean_ids,
            usage,
            plan_clean,
            clean,
            stop_clean,
            report,
            apps,
            scan_leftovers,
            leftovers,
            plan_leftovers,
            delete_leftovers,
            uninstall,
            stop_uninstall,
            remove_entry,
            reveal,
            relaunch_admin,
            reset_pc,
        ])
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
