use kinetic_core::{
    ArchiveFormat, CompressionMode, DiskTelemetry, HardwareProfile, RecentArchive,
    RecentOperation, UserConfig, ENGINE_NAME, ENGINE_VERSION,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatCapability {
    pub format: ArchiveFormat,
    pub extension: &'static str,
    pub can_create: bool,
    pub can_extract: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeCapability {
    pub mode: CompressionMode,
    pub target_block_size_bytes: usize,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatusPayload {
    pub engine_name: &'static str,
    pub engine_version: &'static str,
    pub default_format: ArchiveFormat,
    pub default_mode: CompressionMode,
    pub config: UserConfig,
    pub hardware: HardwareProfile,
    pub disks: Vec<DiskTelemetry>,
    pub recent_archives: Vec<RecentArchive>,
    pub recent_operations: Vec<RecentOperation>,
    pub supported_formats: Vec<FormatCapability>,
    pub compression_modes: Vec<ModeCapability>,
}

pub fn build_engine_status(state: &AppState) -> EngineStatusPayload {
    let formats = [
        ArchiveFormat::Zip,
        ArchiveFormat::Kinetic,
        ArchiveFormat::SevenZ,
        ArchiveFormat::Rar,
    ]
    .into_iter()
    .map(|f| FormatCapability {
        format: f,
        extension: f.extension(),
        can_create: f.can_create(),
        can_extract: f.can_extract(),
    })
    .collect();

    let modes = [
        CompressionMode::Store,
        CompressionMode::Fast,
        CompressionMode::Balanced,
        CompressionMode::Maximum,
        CompressionMode::Ultra,
        CompressionMode::Automatic,
    ]
    .into_iter()
    .map(|m| ModeCapability {
        mode: m,
        target_block_size_bytes: m.target_block_size_bytes(),
        description: m.description(),
    })
    .collect();

    let config = state.config.lock().unwrap().clone();
    let recent_archives = state.recent_archives.lock().unwrap().clone();
    let recent_operations = state.recent_operations.lock().unwrap().clone();

    EngineStatusPayload {
        engine_name: ENGINE_NAME,
        engine_version: ENGINE_VERSION,
        default_format: config.default_format,
        default_mode: config.default_mode,
        config,
        hardware: state.hardware.clone(),
        disks: DiskTelemetry::detect_all(),
        recent_archives,
        recent_operations,
        supported_formats: formats,
        compression_modes: modes,
    }
}

#[tauri::command]
pub fn get_engine_status(state: State<'_, AppState>) -> EngineStatusPayload {
    build_engine_status(&state)
}

#[tauri::command]
pub fn update_user_config(
    new_config: UserConfig,
    state: State<'_, AppState>,
) -> Result<EngineStatusPayload, String> {
    if !new_config.default_format.can_create() {
        return Err("RAR cannot be set as the default creation format due to licensing restrictions.".to_string());
    }
    let mut cfg = state.config.lock().unwrap();
    *cfg = new_config;
    drop(cfg);
    Ok(build_engine_status(&state))
}

#[tauri::command]
pub fn clear_recent_history(state: State<'_, AppState>) -> EngineStatusPayload {
    state.recent_archives.lock().unwrap().clear();
    state.recent_operations.lock().unwrap().clear();
    build_engine_status(&state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_status_payload_integrity() {
        let state = AppState::default();
        let status = build_engine_status(&state);
        assert_eq!(status.default_format, ArchiveFormat::Zip);
        assert_eq!(status.default_mode, CompressionMode::Balanced);
        assert_eq!(status.supported_formats.len(), 4);
        assert_eq!(status.compression_modes.len(), 6);
        assert_eq!(status.config.theme, "dark");
    }
}