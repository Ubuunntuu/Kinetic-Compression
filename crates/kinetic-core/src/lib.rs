pub mod classifier;
pub mod memory;
pub mod pipeline;
pub mod progress;

use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};
use thiserror::Error;

pub const ENGINE_NAME: &str = "Kinetic Engine";
pub const ENGINE_VERSION: &str = "1.0.0";

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Unsupported archive operation: {0}")]
    UnsupportedOperation(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveFormat {
    Zip,
    Kinetic,
    SevenZ,
    Rar,
}

impl Default for ArchiveFormat {
    fn default() -> Self {
        ArchiveFormat::Zip
    }
}

impl ArchiveFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ArchiveFormat::Zip => ".zip",
            ArchiveFormat::Kinetic => ".kinetic",
            ArchiveFormat::SevenZ => ".7z",
            ArchiveFormat::Rar => ".rar",
        }
    }

    pub fn can_create(&self) -> bool {
        match self {
            ArchiveFormat::Zip => true,
            ArchiveFormat::Kinetic => true,
            ArchiveFormat::SevenZ => true,
            ArchiveFormat::Rar => false,
        }
    }

    pub fn can_extract(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionMode {
    Store,
    Fast,
    Balanced,
    Maximum,
    Ultra,
    Automatic,
}

impl Default for CompressionMode {
    fn default() -> Self {
        CompressionMode::Balanced
    }
}

impl CompressionMode {
    pub fn target_block_size_bytes(&self) -> usize {
        match self {
            CompressionMode::Store => 4 * 1024 * 1024,
            CompressionMode::Fast => 4 * 1024 * 1024,
            CompressionMode::Balanced => 8 * 1024 * 1024,
            CompressionMode::Maximum => 16 * 1024 * 1024,
            CompressionMode::Ultra => 32 * 1024 * 1024,
            CompressionMode::Automatic => 8 * 1024 * 1024,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            CompressionMode::Store => "No compression; instant archiving and indexing",
            CompressionMode::Fast => "LZ4 hardware-accelerated pipeline prioritizing speed",
            CompressionMode::Balanced => "Adaptive LZ4/Zlib + content classification balancing speed and ratio",
            CompressionMode::Maximum => "High-level Deflate/Zlib with BCJ, Delta, and Byte-Shuffle filters",
            CompressionMode::Ultra => "Maximum density Brotli-Ultra with Seed Dictionary & FastCDC deduplication",
            CompressionMode::Automatic => "Per-block dynamic routing via Shannon entropy and magic bytes",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub logical_cores: usize,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub recommended_threads: usize,
    pub max_safe_memory_arena_mb: u64,
}

impl HardwareProfile {
    pub fn detect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_memory();

        let logical_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        let total_memory_mb = sys.total_memory() / (1024 * 1024);
        let available_memory_mb = sys.available_memory() / (1024 * 1024);
        let recommended_threads = logical_cores.saturating_sub(1).max(1);
        let max_safe_memory_arena_mb = (available_memory_mb / 2).clamp(256, 8192);

        Self {
            logical_cores,
            total_memory_mb,
            available_memory_mb,
            recommended_threads,
            max_safe_memory_arena_mb,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskTelemetry {
    pub mount_point: String,
    pub name: String,
    pub file_system: String,
    pub total_gb: f64,
    pub available_gb: f64,
    pub used_percent: f64,
}

impl DiskTelemetry {
    pub fn detect_all() -> Vec<Self> {
        let disks = Disks::new_with_refreshed_list();
        let mut result = Vec::new();

        for disk in disks.list() {
            let total = disk.total_space();
            if total == 0 {
                continue;
            }
            let avail = disk.available_space();
            let used = total.saturating_sub(avail);
            let total_gb = (total as f64) / 1_073_741_824.0;
            let available_gb = (avail as f64) / 1_073_741_824.0;
            let used_percent = ((used as f64) / (total as f64)) * 100.0;

            result.push(Self {
                mount_point: disk.mount_point().to_string_lossy().into_owned(),
                name: disk.name().to_string_lossy().into_owned(),
                file_system: disk.file_system().to_string_lossy().into_owned(),
                total_gb: (total_gb * 10.0).round() / 10.0,
                available_gb: (available_gb * 10.0).round() / 10.0,
                used_percent: (used_percent * 10.0).round() / 10.0,
            });
        }
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentArchive {
    pub name: String,
    pub path: String,
    pub format: ArchiveFormat,
    pub size_bytes: u64,
    pub last_opened_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentOperation {
    pub id: String,
    pub operation_type: String,
    pub target_name: String,
    pub format: ArchiveFormat,
    pub mode: CompressionMode,
    pub throughput_mb_s: f64,
    pub compression_ratio_percent: f64,
    pub duration_ms: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub theme: String,
    pub default_format: ArchiveFormat,
    pub default_mode: CompressionMode,
    pub worker_threads: usize,
    pub memory_arena_mb: u64,
    pub enable_fastcdc_dedup: bool,
    pub enable_entropy_bypass: bool,
    pub enable_kinetic_shield_parity: bool,
    pub verify_integrity_on_complete: bool,
    pub overwrite_policy: String,
}

impl UserConfig {
    pub fn from_hardware(hw: &HardwareProfile) -> Self {
        Self {
            theme: "dark".to_string(),
            default_format: ArchiveFormat::Zip,
            default_mode: CompressionMode::Balanced,
            worker_threads: hw.recommended_threads,
            memory_arena_mb: hw.max_safe_memory_arena_mb.min(2048),
            enable_fastcdc_dedup: true,
            enable_entropy_bypass: true,
            enable_kinetic_shield_parity: false,
            verify_integrity_on_complete: true,
            overwrite_policy: "ask".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_format_is_zip() {
        assert_eq!(ArchiveFormat::default(), ArchiveFormat::Zip);
        assert_eq!(ArchiveFormat::Zip.extension(), ".zip");
    }

    #[test]
    fn test_rar_creation_is_disabled_for_legal_compliance() {
        assert!(!ArchiveFormat::Rar.can_create());
        assert!(ArchiveFormat::Rar.can_extract());
        assert!(ArchiveFormat::Kinetic.can_create());
    }

    #[test]
    fn test_hardware_and_config_initialization() {
        let profile = HardwareProfile::detect();
        let cfg = UserConfig::from_hardware(&profile);
        assert_eq!(cfg.theme, "dark");
        assert_eq!(cfg.default_format, ArchiveFormat::Zip);
        assert!(cfg.worker_threads >= 1);
    }
}