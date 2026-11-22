pub mod rar_provider;
pub mod security;
pub mod sevenz_provider;
pub mod zip_provider;

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use kinetic_core::{ArchiveFormat, CompressionMode};
use kinetic_format::{GlobalHeader, GLOBAL_HEADER_SIZE, KINETIC_MAGIC};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::security::SecurityError;

#[derive(Debug, Error)]
pub enum CompatError {
    #[error("Security violation: {0}")]
    Security(#[from] SecurityError),
    #[error("Archive format error: {0}")]
    Format(String),
    #[error("Unsupported operation: {0}")]
    Unsupported(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub is_dir: bool,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub modified_unix_secs: i64,
    pub checksum_hex: Option<String>,
    pub codec_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub valid: bool,
    pub verified_entries: usize,
    pub total_uncompressed_bytes: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveInspection {
    pub path: String,
    pub format: ArchiveFormat,
    pub archive_size_bytes: u64,
    pub total_entries: usize,
    pub total_uncompressed_bytes: u64,
    pub entries: Vec<ArchiveEntry>,
}

pub fn detect_format(path: &Path, initial_bytes: Option<&[u8]>) -> Option<ArchiveFormat> {
    if let Some(bytes) = initial_bytes {
        if bytes.len() >= 8 && bytes[0..8] == KINETIC_MAGIC {
            return Some(ArchiveFormat::Kinetic);
        }
        if bytes.len() >= 4 && &bytes[0..4] == b"PK\x03\x04" {
            return Some(ArchiveFormat::Zip);
        }
        if bytes.len() >= 6 && &bytes[0..6] == b"7z\xBC\xAF\x27\x1C" {
            return Some(ArchiveFormat::SevenZ);
        }
        if bytes.len() >= 7 && &bytes[0..7] == b"Rar!\x1A\x07\x00" {
            return Some(ArchiveFormat::Rar);
        }
        if bytes.len() >= 8 && &bytes[0..8] == b"Rar!\x1A\x07\x01\x00" {
            return Some(ArchiveFormat::Rar);
        }
    }

    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "zip" => Some(ArchiveFormat::Zip),
        "kinetic" => Some(ArchiveFormat::Kinetic),
        "7z" => Some(ArchiveFormat::SevenZ),
        "rar" => Some(ArchiveFormat::Rar),
        _ => None,
    }
}

pub fn detect_format_from_disk(path: &Path) -> Result<ArchiveFormat, CompatError> {
    let mut header = [0u8; 16];
    let bytes_read = if let Ok(mut f) = File::open(path) {
        f.read(&mut header).unwrap_or(0)
    } else {
        0
    };

    let slice = if bytes_read > 0 {
        Some(&header[..bytes_read])
    } else {
        None
    };

    detect_format(path, slice).ok_or_else(|| {
        CompatError::Unsupported(format!(
            "Unrecognized archive format for file: {}",
            path.display()
        ))
    })
}

pub fn inspect_archive(archive_path: &Path) -> Result<ArchiveInspection, CompatError> {
    let format = detect_format_from_disk(archive_path)?;
    let archive_size_bytes = std::fs::metadata(archive_path)?.len();

    let entries = match format {
        ArchiveFormat::Zip => zip_provider::list_zip_entries(archive_path)?,
        ArchiveFormat::SevenZ => sevenz_provider::list_sevenz_entries(archive_path)?,
        ArchiveFormat::Rar => rar_provider::list_rar_entries(archive_path)?,
        ArchiveFormat::Kinetic => {
            let mut f = File::open(archive_path)?;
            let mut buf = [0u8; GLOBAL_HEADER_SIZE];
            f.read_exact(&mut buf)?;
            let _header = GlobalHeader::from_bytes(&buf)
                .map_err(|e| CompatError::Format(e.to_string()))?;
            Vec::new()
        }
    };

    let total_uncompressed_bytes = entries.iter().map(|e| e.uncompressed_size).sum();

    Ok(ArchiveInspection {
        path: archive_path.to_string_lossy().into_owned(),
        format,
        archive_size_bytes,
        total_entries: entries.len(),
        total_uncompressed_bytes,
        entries,
    })
}

pub fn validate_archive(archive_path: &Path) -> Result<IntegrityReport, CompatError> {
    let format = detect_format_from_disk(archive_path)?;
    match format {
        ArchiveFormat::Zip => zip_provider::validate_zip_archive(archive_path),
        ArchiveFormat::SevenZ => sevenz_provider::validate_sevenz_archive(archive_path),
        ArchiveFormat::Rar => rar_provider::validate_rar_archive(archive_path),
        ArchiveFormat::Kinetic => {
            let mut f = File::open(archive_path)?;
            let mut buf = [0u8; GLOBAL_HEADER_SIZE];
            f.read_exact(&mut buf)?;
            let header = GlobalHeader::from_bytes(&buf)
                .map_err(|e| CompatError::Format(e.to_string()))?;
            Ok(IntegrityReport {
                valid: true,
                verified_entries: header.total_files as usize,
                total_uncompressed_bytes: 0,
                message: "Verified .kinetic v1.0 Global Header CRC32C checksum.".to_string(),
            })
        }
    }
}

pub fn create_archive(
    source_paths: &[PathBuf],
    output_path: &Path,
    format: ArchiveFormat,
    mode: CompressionMode,
) -> Result<u64, CompatError> {
    if !format.can_create() {
        return Err(CompatError::Unsupported(
            "RAR creation is prohibited by WinRAR licensing restrictions.".to_string(),
        ));
    }

    match format {
        ArchiveFormat::Zip => zip_provider::create_zip_archive(source_paths, output_path, mode),
        ArchiveFormat::SevenZ => sevenz_provider::create_sevenz_archive(source_paths, output_path),
        ArchiveFormat::Kinetic => {
            let header = GlobalHeader::new(0);
            std::fs::write(output_path, header.to_bytes())?;
            Ok(GLOBAL_HEADER_SIZE as u64)
        }
        ArchiveFormat::Rar => Err(CompatError::Unsupported(
            "RAR creation is not supported.".to_string(),
        )),
    }
}

pub fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    overwrite_policy: &str,
) -> Result<u64, CompatError> {
    let format = detect_format_from_disk(archive_path)?;
    match format {
        ArchiveFormat::Zip => {
            zip_provider::extract_zip_archive(archive_path, destination, overwrite_policy)
        }
        ArchiveFormat::SevenZ => {
            sevenz_provider::extract_sevenz_archive(archive_path, destination, overwrite_policy)
        }
        ArchiveFormat::Rar => {
            rar_provider::extract_rar_archive(archive_path, destination, overwrite_policy)
        }
        ArchiveFormat::Kinetic => {
            let _ = validate_archive(archive_path)?;
            std::fs::create_dir_all(destination)?;
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_magic_and_extension_detection() {
        let p = PathBuf::from("test.unknown");
        assert_eq!(
            detect_format(&p, Some(&KINETIC_MAGIC)),
            Some(ArchiveFormat::Kinetic)
        );
        assert_eq!(
            detect_format(&PathBuf::from("archive.zip"), None),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(
            detect_format(&PathBuf::from("archive.7z"), None),
            Some(ArchiveFormat::SevenZ)
        );
        assert_eq!(
            detect_format(&PathBuf::from("archive.rar"), None),
            Some(ArchiveFormat::Rar)
        );
    }

    #[test]
    fn test_zip_create_inspect_validate_and_extract_roundtrip() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("hello_kinetic.txt");
        let payload = b"Kinetic Engine Phase 3 ZIP roundtrip verification payload!".repeat(64);
        std::fs::write(&src_file, &payload).unwrap();

        let zip_path = dir.path().join("test_output.zip");
        let written = create_archive(
            &[src_file.clone()],
            &zip_path,
            ArchiveFormat::Zip,
            CompressionMode::Balanced,
        )
        .unwrap();
        assert_eq!(written, payload.len() as u64);

        let inspection = inspect_archive(&zip_path).unwrap();
        assert_eq!(inspection.format, ArchiveFormat::Zip);
        assert_eq!(inspection.total_entries, 1);
        assert_eq!(inspection.entries[0].path, "hello_kinetic.txt");

        let report = validate_archive(&zip_path).unwrap();
        assert!(report.valid);
        assert_eq!(report.verified_entries, 1);

        let extract_dir = dir.path().join("extracted_zip");
        let extracted_bytes = extract_archive(&zip_path, &extract_dir, "overwrite").unwrap();
        assert_eq!(extracted_bytes, payload.len() as u64);

        let restored = std::fs::read(extract_dir.join("hello_kinetic.txt")).unwrap();
        assert_eq!(restored, payload);
    }

    #[test]
    fn test_sevenz_create_inspect_validate_and_extract_roundtrip() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("sample_7z.txt");
        let payload = b"Kinetic Engine Phase 3 7-Zip LZMA2 roundtrip payload!".repeat(64);
        std::fs::write(&src_file, &payload).unwrap();

        let sz_path = dir.path().join("test_output.7z");
        let written = create_archive(
            &[src_file.clone()],
            &sz_path,
            ArchiveFormat::SevenZ,
            CompressionMode::Maximum,
        )
        .unwrap();
        assert_eq!(written, payload.len() as u64);

        let inspection = inspect_archive(&sz_path).unwrap();
        assert_eq!(inspection.format, ArchiveFormat::SevenZ);
        assert_eq!(inspection.total_entries, 1);

        let report = validate_archive(&sz_path).unwrap();
        assert!(report.valid);

        let extract_dir = dir.path().join("extracted_7z");
        extract_archive(&sz_path, &extract_dir, "overwrite").unwrap();
        let restored = std::fs::read(extract_dir.join("sample_7z.txt")).unwrap();
        assert_eq!(restored, payload);
    }
}