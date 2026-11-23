use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use kinetic_core::memory::MemoryArena;
use kinetic_core::progress::JobController;
use kinetic_core::{
    HardwareProfile, RecentArchive, RecentOperation, UserConfig,
};
use serde::{Deserialize, Serialize};

const STATE_FILE_NAME: &str = "kinetic_state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedState {
    config: UserConfig,
    recent_archives: Vec<RecentArchive>,
    recent_operations: Vec<RecentOperation>,
}

pub struct AppState {
    pub config: Mutex<UserConfig>,
    pub recent_archives: Mutex<Vec<RecentArchive>>,
    pub recent_operations: Mutex<Vec<RecentOperation>>,
    pub hardware: HardwareProfile,
    pub arena: Arc<MemoryArena>,
    pub job_controller: Arc<JobController>,
    pub startup_archive_path: Mutex<Option<String>>,
}

impl AppState {
    fn state_file_path() -> PathBuf {
        PathBuf::from(STATE_FILE_NAME)
    }

    pub fn save_to_disk(&self) {
        let snapshot = PersistedState {
            config: self.config.lock().unwrap().clone(),
            recent_archives: self.recent_archives.lock().unwrap().clone(),
            recent_operations: self.recent_operations.lock().unwrap().clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            let _ = fs::write(Self::state_file_path(), json);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        let hardware = HardwareProfile::detect();
        let default_config = UserConfig::from_hardware(&hardware);

        let (config, recent_archives, recent_operations) =
            if let Ok(raw) = fs::read_to_string(Self::state_file_path()) {
                if let Ok(loaded) = serde_json::from_str::<PersistedState>(&raw) {
                    (loaded.config, loaded.recent_archives, loaded.recent_operations)
                } else {
                    (default_config, Vec::new(), Vec::new())
                }
            } else {
                (default_config, Vec::new(), Vec::new())
            };

        let arena = MemoryArena::new(config.memory_arena_mb);
        let job_controller = Arc::new(JobController::new());
        Self {
            config: Mutex::new(config),
            recent_archives: Mutex::new(recent_archives),
            recent_operations: Mutex::new(recent_operations),
            hardware,
            arena,
            job_controller,
            startup_archive_path: Mutex::new(None),
        }
    }
}