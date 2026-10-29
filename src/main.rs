#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod apps;
mod cleaner;
mod commands;
mod rules;
mod safety;
mod scanner;
mod state;
mod util;
mod win;

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
        .expect("failed to start Freespace");
}
