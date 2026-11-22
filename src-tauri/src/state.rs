use std::sync::Mutex;
use kinetic_core::{
    HardwareProfile, RecentArchive, RecentOperation, UserConfig,
};

pub struct AppState {
    pub config: Mutex<UserConfig>,
    pub recent_archives: Mutex<Vec<RecentArchive>>,
    pub recent_operations: Mutex<Vec<RecentOperation>>,
    pub hardware: HardwareProfile,
}

impl Default for AppState {
    fn default() -> Self {
        let hardware = HardwareProfile::detect();
        let config = UserConfig::from_hardware(&hardware);
        Self {
            config: Mutex::new(config),
            recent_archives: Mutex::new(Vec::new()),
            recent_operations: Mutex::new(Vec::new()),
            hardware,
        }
    }
}