#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::AppState;
use ghost_core::{history::HistoryStore, logging::Logger, tools::Tools};
use std::sync::Arc;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().app_data_dir().expect("dossier de données");
            std::fs::create_dir_all(&data).ok();
            let mut tool_dirs = vec![];
            if let Ok(r) = app.path().resource_dir() {
                tool_dirs.push(r.join("tools"));
                tool_dirs.push(r.join("resources").join("tools"));
            }
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    tool_dirs.push(dir.join("tools"));
                }
            }
            let state = AppState::new(
                data.clone(),
                Tools::new(tool_dirs),
                HistoryStore::new(data.join("history.jsonl")),
                Logger::new(data.join("logs").join("ghost.log")),
            );
            state.logger.info("démarrage de l'application");
            app.manage(Arc::new(state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_status,
            commands::set_simulation,
            commands::get_settings,
            commands::save_settings,
            commands::list_devices,
            commands::check_compat,
            commands::analyze_folder,
            commands::analyze_backup,
            commands::backup_create,
            commands::backup_list,
            commands::backup_verify,
            commands::backup_delete,
            commands::backup_export,
            commands::backup_import,
            commands::restore_backup,
            commands::clone_start,
            commands::smart_plan,
            commands::smart_transfer,
            commands::cancel_operation,
            commands::history_list,
            commands::history_clear,
            commands::diagnostics_run,
            commands::security_selftest,
            commands::diagnostics_export,
            commands::verify_update,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Ghost Transfert Apple");
}
