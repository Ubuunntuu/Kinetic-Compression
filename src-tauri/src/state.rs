use std::sync::Mutex;
use kinetic_core::{ArchiveFormat, CompressionMode, HardwareProfile};

pub struct AppState {
    pub default_format: Mutex<ArchiveFormat>,
    pub default_mode: Mutex<CompressionMode>,
    pub hardware: HardwareProfile,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            default_format: Mutex::new(ArchiveFormat::default()),
            default_mode: Mutex::new(CompressionMode::default()),
            hardware: HardwareProfile::detect(),
        }
    }
}
