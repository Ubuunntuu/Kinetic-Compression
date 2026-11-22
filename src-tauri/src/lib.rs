pub mod commands;
pub mod state;

use commands::get_engine_status;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![get_engine_status])
        .run(tauri::generate_context!())
        .expect("error while running Kinetic desktop application");
}
