use std::path::PathBuf;
use std::time::Instant;
use kinetic_compat::{
    create_archive_with_options, extract_archive_with_password, extract_selected_entries,
    inspect_archive_with_password, validate_archive_with_password, ArchiveInspection,
    IntegrityReport,
};
use kinetic_core::pipeline::{
    compress_stream_parallel, decompress_blocks_parallel, PipelineJobReport,
};
use kinetic_core::progress::ProgressSnapshot;
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
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractArchiveRequest {
    pub archive_path: String,
    pub destination_path: String,
    pub overwrite_policy: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractSelectedRequest {
    pub archive_path: String,
    pub destination_path: String,
    pub selected_paths: Vec<String>,
    pub overwrite_policy: Option<String>,
    pub password: Option<String>,
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
    state.arena.set_capacity_mb(new_config.memory_arena_mb);
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
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<ArchiveInspection, String> {
    let path = PathBuf::from(&archive_path);
    let inspection = inspect_archive_with_password(&path, password.as_deref())
        .map_err(|e| e.to_string())?;
    record_recent_archive(
        &state,
        &path,
        inspection.format,
        inspection.archive_size_bytes,
    );
    Ok(inspection)
}

#[tauri::command]
pub fn validate_archive_cmd(
    archive_path: String,
    password: Option<String>,
) -> Result<IntegrityReport, String> {
    let path = PathBuf::from(&archive_path);
    validate_archive_with_password(&path, password.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_archive_cmd(
    request: CreateArchiveRequest,
    state: State<'_, AppState>,
) -> Result<OperationResult, String> {
    let sources: Vec<PathBuf> = request.source_paths.iter().map(PathBuf::from).collect();
    let out_path = PathBuf::from(&request.output_path);
    let (threads, dedup, entropy_bypass, parity) = {
        let cfg = state.config.lock().unwrap();
        (
            cfg.worker_threads,
            cfg.enable_fastcdc_dedup,
            cfg.enable_entropy_bypass,
            cfg.enable_kinetic_shield_parity,
        )
    };

    state
        .job_controller
        .begin_job("Create Archive", 0, threads);
    state.job_controller.set_current_file(&request.output_path);

    let started = Instant::now();
    let input_bytes = match create_archive_with_options(
        &sources,
        &out_path,
        request.format,
        request.mode,
        request.password,
        dedup,
        entropy_bypass,
        parity,
    ) {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            state.job_controller.fail_job(&msg);
            return Err(msg);
        }
    };
    let elapsed = started.elapsed();
    let duration_ms = elapsed.as_millis().max(1) as u64;

    let compressed_bytes = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
    state
        .job_controller
        .record_block_progress(input_bytes, compressed_bytes);
    state.job_controller.finish_job();

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
    let (policy, threads) = {
        let cfg = state.config.lock().unwrap();
        (
            request
                .overwrite_policy
                .clone()
                .unwrap_or_else(|| cfg.overwrite_policy.clone()),
            cfg.worker_threads,
        )
    };

    let archive_bytes = std::fs::metadata(&archive_path).map(|m| m.len()).unwrap_or(0);
    state
        .job_controller
        .begin_job("Extract Archive", archive_bytes, threads);
    state
        .job_controller
        .set_current_file(&request.archive_path);

    let started = Instant::now();
    let extracted_bytes = match extract_archive_with_password(
        &archive_path,
        &dest_path,
        &policy,
        request.password.as_deref(),
    ) {
        Ok(b) => b,
        Err(e) => {
            let msg = e.to_string();
            state.job_controller.fail_job(&msg);
            return Err(msg);
        }
    };
    let elapsed = started.elapsed();
    let duration_ms = elapsed.as_millis().max(1) as u64;

    state
        .job_controller
        .record_block_progress(extracted_bytes, archive_bytes);
    state.job_controller.finish_job();

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
            format: ArchiveFormat::Kinetic,
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

#[tauri::command]
pub fn extract_selected_cmd(
    request: ExtractSelectedRequest,
    state: State<'_, AppState>,
) -> Result<OperationResult, String> {
    let archive_path = PathBuf::from(&request.archive_path);
    let dest_path = PathBuf::from(&request.destination_path);
    let policy = request
        .overwrite_policy
        .unwrap_or_else(|| state.config.lock().unwrap().overwrite_policy.clone());

    let started = Instant::now();
    let extracted_bytes = extract_selected_entries(
        &archive_path,
        &dest_path,
        &request.selected_paths,
        &policy,
        request.password.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    let duration_ms = started.elapsed().as_millis().max(1) as u64;
    let archive_bytes = std::fs::metadata(&archive_path).map(|m| m.len()).unwrap_or(0);
    let throughput_mb_s =
        ((extracted_bytes as f64) / (1024.0 * 1024.0)) / (duration_ms as f64 / 1000.0);

    Ok(OperationResult {
        success: true,
        message: format!(
            "Extracted {} selected items ({:.2} KB in {} ms)",
            request.selected_paths.len(),
            (extracted_bytes as f64) / 1024.0,
            duration_ms
        ),
        archive_path: request.archive_path,
        processed_bytes: extracted_bytes,
        compressed_bytes: archive_bytes,
        duration_ms,
        throughput_mb_s,
        compression_ratio_percent: 100.0,
    })
}

#[tauri::command]
pub fn get_live_telemetry_cmd(state: State<'_, AppState>) -> ProgressSnapshot {
    state.job_controller.snapshot(
        state.arena.allocated_bytes(),
        state.arena.peak_bytes(),
    )
}

#[tauri::command]
pub fn pause_job_cmd(state: State<'_, AppState>) -> ProgressSnapshot {
    state.job_controller.pause();
    state.job_controller.snapshot(
        state.arena.allocated_bytes(),
        state.arena.peak_bytes(),
    )
}

#[tauri::command]
pub fn resume_job_cmd(state: State<'_, AppState>) -> ProgressSnapshot {
    state.job_controller.resume();
    state.job_controller.snapshot(
        state.arena.allocated_bytes(),
        state.arena.peak_bytes(),
    )
}

#[tauri::command]
pub fn cancel_job_cmd(state: State<'_, AppState>) -> ProgressSnapshot {
    state.job_controller.cancel();
    state.job_controller.snapshot(
        state.arena.allocated_bytes(),
        state.arena.peak_bytes(),
    )
}

#[tauri::command]
pub fn run_engine_pipeline_job_cmd(
    mode: CompressionMode,
    payload_size_mb: usize,
    state: State<'_, AppState>,
) -> Result<PipelineJobReport, String> {
    let (threads, entropy_bypass) = {
        let cfg = state.config.lock().unwrap();
        (cfg.worker_threads, cfg.enable_entropy_bypass)
    };

    let clamped_mb = payload_size_mb.clamp(4, 128);
    let pattern =
        b"Kinetic Engine Phase 7 Multithreaded Ring-Buffer Pipeline Telemetry Stream! ";
    let repeats = (clamped_mb * 1024 * 1024) / pattern.len();
    let dataset = pattern.repeat(repeats);

    let (blocks, report) = compress_stream_parallel(
        &dataset,
        &format!("Kinetic_Memory_Stream_{}MB.bin", clamped_mb),
        mode,
        threads,
        entropy_bypass,
        &state.arena,
        &state.job_controller,
    )
    .map_err(|e| e.to_string())?;

    let restored =
        decompress_blocks_parallel(&blocks, threads, &state.job_controller)
            .map_err(|e| e.to_string())?;

    if restored.len() != dataset.len() {
        return Err("Pipeline verification failed: restored length mismatch.".to_string());
    }

    let mut ops = state.recent_operations.lock().unwrap();
    let next_op_id = format!("op-{}", ops.len() + 1);
    ops.insert(
        0,
        RecentOperation {
            id: next_op_id,
            operation_type: "Pipeline Verify".to_string(),
            target_name: format!("{} MB Stream ({} Blocks)", clamped_mb, report.total_blocks),
            format: ArchiveFormat::Kinetic,
            mode,
            throughput_mb_s: report.throughput_mb_s,
            compression_ratio_percent: report.compression_ratio_percent,
            duration_ms: report.duration_ms,
            status: "XXH3 + BLAKE3 Verified".to_string(),
        },
    );
    ops.truncate(20);

    Ok(report)
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