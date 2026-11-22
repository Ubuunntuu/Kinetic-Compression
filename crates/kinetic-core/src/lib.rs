use serde::{Deserialize, Serialize};
use sysinfo::System;
use thiserror::Error;

pub const ENGINE_NAME: &str = "Kinetic Engine";
pub const ENGINE_VERSION: &str = "0.1.0-alpha";

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
            CompressionMode::Balanced => "Zstandard + content classification balancing speed and ratio",
            CompressionMode::Maximum => "High-level Zstandard with BCJ, Delta, and Byte-Shuffle filters",
            CompressionMode::Ultra => "Maximum density LZMA2/Brotli with FastCDC global deduplication",
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
    fn test_hardware_profile_detection() {
        let profile = HardwareProfile::detect();
        assert!(profile.logical_cores >= 1);
        assert!(profile.recommended_threads >= 1);
        assert!(profile.max_safe_memory_arena_mb >= 256);
    }
}
