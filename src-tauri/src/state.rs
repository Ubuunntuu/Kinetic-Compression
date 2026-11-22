use std::sync::{Arc, Mutex};
use kinetic_core::memory::MemoryArena;
use kinetic_core::progress::JobController;
use kinetic_core::{
    HardwareProfile, RecentArchive, RecentOperation, UserConfig,
};

pub struct AppState {
    pub config: Mutex<UserConfig>,
    pub recent_archives: Mutex<Vec<RecentArchive>>,
    pub recent_operations: Mutex<Vec<RecentOperation>>,
    pub hardware: HardwareProfile,
    pub arena: Arc<MemoryArena>,
    pub job_controller: Arc<JobController>,
}

impl Default for AppState {
    fn default() -> Self {
        let hardware = HardwareProfile::detect();
        let config = UserConfig::from_hardware(&hardware);
        let arena = MemoryArena::new(config.memory_arena_mb);
        let job_controller = Arc::new(JobController::new());
        Self {
            config: Mutex::new(config),
            recent_archives: Mutex::new(Vec::new()),
            recent_operations: Mutex::new(Vec::new()),
            hardware,
            arena,
            job_controller,
        }
    }
}