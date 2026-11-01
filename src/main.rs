#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod apps;
mod cleaner;
mod commands;
mod icons;
mod projects;
mod rules;
mod safety;
mod scanner;
mod settings;
mod state;
mod tray;
mod util;
mod win;

use tauri::Manager;

/// Handles launch before any window exists: hands over to a copy already running, or restarts elevated. Declining
/// the UAC prompt still opens the app, as a normal user. Debug builds skip this: `tauri dev` must keep its process.
fn hand_over_launch() -> bool {
    if cfg!(debug_assertions) || std::env::args().any(|a| a == win::RELAUNCHED) {
        return false;
    }
    win::signal_running_copy() || (!win::is_admin() && win::relaunch_as_admin())
}

fn main() {
    use commands::*;
    if hand_over_launch() {
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Shared::default())
        .setup(|app| {
            let handle = app.handle();
            let hidden = load_settings(handle);
            // No tray icon, no way back to a hidden window: then the window behaves as usual.
            let has_tray = tray::setup(handle).is_ok();
            lock_app(handle).has_tray = has_tray;
            if !(hidden && has_tray) {
                tray::show_main(handle);
            }
            let shown = handle.clone();
            win::on_show_request(move || tray::show_main(&shown));
            start_rescan_timer(handle);
            start_apps_load(handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if lock_app(window.app_handle()).has_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            init,
            status,
            start_scan,
            stop_scan,
            groups,
            findings,
            full_clean_ids,
            projects,
            usage,
            plan_clean,
            clean,
            stop_clean,
            report,
            apps,
            app_logos,
            scan_leftovers,
            leftovers,
            plan_leftovers,
            delete_leftovers,
            plan_reset,
            reset_apps,
            uninstall,
            stop_uninstall,
            remove_entry,
            reveal,
            get_settings,
            set_settings,
            relaunch_admin,
            reset_pc,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Freespace");
}
