pub mod commands;
pub mod state;

use commands::{
    clear_recent_history, create_archive_cmd, extract_archive_cmd, get_engine_status,
    inspect_archive_cmd, update_user_config, validate_archive_cmd,
};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_engine_status,
            update_user_config,
            clear_recent_history,
            inspect_archive_cmd,
            validate_archive_cmd,
            create_archive_cmd,
            extract_archive_cmd
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kinetic desktop application");
}