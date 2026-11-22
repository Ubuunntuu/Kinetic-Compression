use std::path::PathBuf;
use std::time::Instant;
use kinetic_compat::{
    create_archive, extract_archive, inspect_archive, validate_archive, ArchiveInspection,
    IntegrityReport,
};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateArchiveRequest {
    pub source_paths: Vec<String>,
    pub output_path: String,
    pub format: ArchiveFormat,
    pub mode: CompressionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractArchiveRequest {
    pub archive_path: String,
    pub destination_path: String,
    pub overwrite_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
    pub archive_path: String,
    pub processed_bytes: u64,
    pub compressed_bytes: u64,
    pub duration_ms: u64,
    pub throughput_mb_s: f64,
    pub compression_ratio_percent: f64,
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

fn record_recent_archive(state: &AppState, path: &PathBuf, format: ArchiveFormat, size_bytes: u64) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let path_str = path.to_string_lossy().into_owned();

    let mut list = state.recent_archives.lock().unwrap();
    list.retain(|item| item.path != path_str);
    list.insert(
        0,
        RecentArchive {
            name,
            path: path_str,
            format,
            size_bytes,
            last_opened_utc: "Active Session".to_string(),
        },
    );
    list.truncate(15);
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
        return Err(
            "RAR cannot be set as the default creation format due to licensing restrictions."
                .to_string(),
        );
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

#[tauri::command]
pub fn inspect_archive_cmd(
    archive_path: String,
    state: State<'_, AppState>,
) -> Result<ArchiveInspection, String> {
    let path = PathBuf::from(&archive_path);
    let inspection = inspect_archive(&path).map_err(|e| e.to_string())?;
    record_recent_archive(
        &state,
        &path,
        inspection.format,
        inspection.archive_size_bytes,
    );
    Ok(inspection)
}

#[tauri::command]
pub fn validate_archive_cmd(archive_path: String) -> Result<IntegrityReport, String> {
    let path = PathBuf::from(&archive_path);
    validate_archive(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_archive_cmd(
    request: CreateArchiveRequest,
    state: State<'_, AppState>,
) -> Result<OperationResult, String> {
    let sources: Vec<PathBuf> = request.source_paths.iter().map(PathBuf::from).collect();
    let out_path = PathBuf::from(&request.output_path);

    let started = Instant::now();
    let input_bytes = create_archive(&sources, &out_path, request.format, request.mode)
        .map_err(|e| e.to_string())?;
    let elapsed = started.elapsed();
    let duration_ms = elapsed.as_millis().max(1) as u64;

    let compressed_bytes = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
    let throughput_mb_s =
        ((input_bytes as f64) / (1024.0 * 1024.0)) / (duration_ms as f64 / 1000.0);
    let ratio_percent = if input_bytes > 0 {
        ((compressed_bytes as f64) / (input_bytes as f64)) * 100.0
    } else {
        100.0
    };

    record_recent_archive(&state, &out_path, request.format, compressed_bytes);

    let target_name = out_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| request.output_path.clone());

    let mut ops = state.recent_operations.lock().unwrap();
    let next_op_id = format!("op-{}", ops.len() + 1);
    ops.insert(
        0,
        RecentOperation {
            id: next_op_id,
            operation_type: "Compress".to_string(),
            target_name: target_name.clone(),
            format: request.format,
            mode: request.mode,
            throughput_mb_s,
            compression_ratio_percent: ratio_percent,
            duration_ms,
            status: "Completed".to_string(),
        },
    );
    ops.truncate(20);

    Ok(OperationResult {
        success: true,
        message: format!(
            "Created {} ({:.2} MB -> {:.2} MB in {} ms)",
            target_name,
            (input_bytes as f64) / (1024.0 * 1024.0),
            (compressed_bytes as f64) / (1024.0 * 1024.0),
            duration_ms
        ),
        archive_path: request.output_path,
        processed_bytes: input_bytes,
        compressed_bytes,
        duration_ms,
        throughput_mb_s,
        compression_ratio_percent: ratio_percent,
    })
}

#[tauri::command]
pub fn extract_archive_cmd(
    request: ExtractArchiveRequest,
    state: State<'_, AppState>,
) -> Result<OperationResult, String> {
    let archive_path = PathBuf::from(&request.archive_path);
    let dest_path = PathBuf::from(&request.destination_path);
    let policy = request.overwrite_policy.unwrap_or_else(|| {
        state.config.lock().unwrap().overwrite_policy.clone()
    });

    let started = Instant::now();
    let extracted_bytes =
        extract_archive(&archive_path, &dest_path, &policy).map_err(|e| e.to_string())?;
    let elapsed = started.elapsed();
    let duration_ms = elapsed.as_millis().max(1) as u64;

    let archive_bytes = std::fs::metadata(&archive_path).map(|m| m.len()).unwrap_or(0);
    let throughput_mb_s =
        ((extracted_bytes as f64) / (1024.0 * 1024.0)) / (duration_ms as f64 / 1000.0);

    let target_name = archive_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| request.archive_path.clone());

    let mut ops = state.recent_operations.lock().unwrap();
    let next_op_id = format!("op-{}", ops.len() + 1);
    ops.insert(
        0,
        RecentOperation {
            id: next_op_id,
            operation_type: "Extract".to_string(),
            target_name: target_name.clone(),
            format: ArchiveFormat::Zip,
            mode: CompressionMode::Balanced,
            throughput_mb_s,
            compression_ratio_percent: 100.0,
            duration_ms,
            status: "Verified & Extracted".to_string(),
        },
    );
    ops.truncate(20);

    Ok(OperationResult {
        success: true,
        message: format!(
            "Extracted {} ({:.2} MB in {} ms at {:.1} MB/s)",
            target_name,
            (extracted_bytes as f64) / (1024.0 * 1024.0),
            duration_ms,
            throughput_mb_s
        ),
        archive_path: request.archive_path,
        processed_bytes: extracted_bytes,
        compressed_bytes: archive_bytes,
        duration_ms,
        throughput_mb_s,
        compression_ratio_percent: 100.0,
    })
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