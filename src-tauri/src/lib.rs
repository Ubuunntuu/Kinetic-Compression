pub mod commands;
pub mod shell;
pub mod state;

use commands::{
    append_entries_cmd, audit_archive_cmd, browse_filesystem_cmd, cancel_job_cmd,
    clear_recent_history, create_archive_cmd, delete_entry_cmd, extract_archive_cmd,
    extract_selected_cmd, get_engine_status, get_live_telemetry_cmd,
    get_shell_integration_status_cmd, inspect_archive_cmd, join_archive_cmd, pause_job_cmd,
    preview_entry_cmd, register_shell_context_menu_cmd, repair_archive_cmd, resume_job_cmd,
    run_benchmark_suite_cmd, run_engine_pipeline_job_cmd, split_archive_cmd,
    unregister_shell_context_menu_cmd, update_user_config, validate_archive_cmd,
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
            audit_archive_cmd,
            repair_archive_cmd,
            preview_entry_cmd,
            delete_entry_cmd,
            append_entries_cmd,
            split_archive_cmd,
            join_archive_cmd,
            browse_filesystem_cmd,
            create_archive_cmd,
            extract_archive_cmd,
            extract_selected_cmd,
            get_live_telemetry_cmd,
            pause_job_cmd,
            resume_job_cmd,
            cancel_job_cmd,
            run_engine_pipeline_job_cmd,
            run_benchmark_suite_cmd,
            get_shell_integration_status_cmd,
            register_shell_context_menu_cmd,
            unregister_shell_context_menu_cmd
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kinetic desktop application");
}