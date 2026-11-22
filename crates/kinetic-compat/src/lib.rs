pub mod rar_provider;
pub mod security;
pub mod sevenz_provider;
pub mod zip_provider;

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use kinetic_core::{ArchiveFormat, CompressionMode};
use kinetic_format::archive::{
    create_kinetic_archive, read_and_verify_block, read_kinetic_index, validate_kinetic_archive,
    KineticWriteOptions,
};
use kinetic_format::KINETIC_MAGIC;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zip::ZipArchive;

use crate::security::{
    resolve_extraction_target, sanitize_relative_path, verify_decompression_quota, SecurityError,
    DEFAULT_MAX_EXTRACTION_BYTES,
};

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

pub fn inspect_archive_with_password(
    archive_path: &Path,
    password: Option<&str>,
) -> Result<ArchiveInspection, CompatError> {
    let format = detect_format_from_disk(archive_path)?;
    let archive_size_bytes = fs::metadata(archive_path)?.len();

    let entries = match format {
        ArchiveFormat::Zip => zip_provider::list_zip_entries(archive_path)?,
        ArchiveFormat::SevenZ => sevenz_provider::list_sevenz_entries(archive_path)?,
        ArchiveFormat::Rar => rar_provider::list_rar_entries(archive_path)?,
        ArchiveFormat::Kinetic => {
            let (_header, index, _) = read_kinetic_index(archive_path, password)
                .map_err(|e| CompatError::Format(e.to_string()))?;
            let mut list = Vec::with_capacity(index.files.len());
            for f in index.files {
                let _ = sanitize_relative_path(&f.path)?;
                let short_hash = if f.blake3_hex.len() >= 16 {
                    Some(f.blake3_hex[..16].to_string())
                } else {
                    None
                };
                list.push(ArchiveEntry {
                    path: f.path,
                    is_dir: f.is_dir,
                    uncompressed_size: f.uncompressed_size,
                    compressed_size: f.compressed_size,
                    modified_unix_secs: f.modified_unix_secs,
                    checksum_hex: short_hash,
                    codec_name: f.codec_summary,
                });
            }
            list
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

pub fn inspect_archive(archive_path: &Path) -> Result<ArchiveInspection, CompatError> {
    inspect_archive_with_password(archive_path, None)
}

pub fn validate_archive_with_password(
    archive_path: &Path,
    password: Option<&str>,
) -> Result<IntegrityReport, CompatError> {
    let format = detect_format_from_disk(archive_path)?;
    match format {
        ArchiveFormat::Zip => zip_provider::validate_zip_archive(archive_path),
        ArchiveFormat::SevenZ => sevenz_provider::validate_sevenz_archive(archive_path),
        ArchiveFormat::Rar => rar_provider::validate_rar_archive(archive_path),
        ArchiveFormat::Kinetic => {
            let (verified_entries, total_uncompressed_bytes, message) =
                validate_kinetic_archive(archive_path, password)
                    .map_err(|e| CompatError::Format(e.to_string()))?;
            Ok(IntegrityReport {
                valid: true,
                verified_entries,
                total_uncompressed_bytes,
                message,
            })
        }
    }
}

pub fn validate_archive(archive_path: &Path) -> Result<IntegrityReport, CompatError> {
    validate_archive_with_password(archive_path, None)
}

pub fn create_archive_with_options(
    source_paths: &[PathBuf],
    output_path: &Path,
    format: ArchiveFormat,
    mode: CompressionMode,
    password: Option<String>,
    enable_fastcdc_dedup: bool,
    enable_entropy_bypass: bool,
    enable_kinetic_shield_parity: bool,
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
            let opts = KineticWriteOptions {
                mode,
                password,
                enable_fastcdc_dedup,
                enable_entropy_bypass,
                enable_kinetic_shield_parity,
            };
            create_kinetic_archive(source_paths, output_path, &opts)
                .map_err(|e| CompatError::Format(e.to_string()))
        }
        ArchiveFormat::Rar => Err(CompatError::Unsupported(
            "RAR creation is not supported.".to_string(),
        )),
    }
}

pub fn create_archive(
    source_paths: &[PathBuf],
    output_path: &Path,
    format: ArchiveFormat,
    mode: CompressionMode,
) -> Result<u64, CompatError> {
    create_archive_with_options(
        source_paths,
        output_path,
        format,
        mode,
        None,
        true,
        true,
        true,
    )
}

fn entry_matches_selection(entry_path: &str, selected_set: &HashSet<String>) -> bool {
    if selected_set.is_empty() {
        return true;
    }
    let norm = entry_path.trim_end_matches('/');
    if selected_set.contains(norm) {
        return true;
    }
    for sel in selected_set {
        let prefix = format!("{}/", sel.trim_end_matches('/'));
        if norm.starts_with(&prefix) {
            return true;
        }
    }
    false
}

/// Extracts only the user-selected entries (or all entries if `selected_paths` is empty).
pub fn extract_selected_entries(
    archive_path: &Path,
    destination: &Path,
    selected_paths: &[String],
    overwrite_policy: &str,
    password: Option<&str>,
) -> Result<u64, CompatError> {
    if selected_paths.is_empty() {
        return extract_archive_with_password(archive_path, destination, overwrite_policy, password);
    }

    let selected_set: HashSet<String> = selected_paths
        .iter()
        .map(|s| s.replace('\\', "/").trim_end_matches('/').to_string())
        .collect();

    let format = detect_format_from_disk(archive_path)?;
    match format {
        ArchiveFormat::Zip => {
            fs::create_dir_all(destination)?;
            let file = File::open(archive_path)?;
            let mut archive =
                ZipArchive::new(file).map_err(|e| CompatError::Format(e.to_string()))?;
            let mut cumulative_bytes = 0u64;
            let mut copy_buf = vec![0u8; 128 * 1024];

            for i in 0..archive.len() {
                let mut entry = archive
                    .by_index(i)
                    .map_err(|e| CompatError::Format(e.to_string()))?;
                let raw_name = entry.name().replace('\\', "/");
                if !entry_matches_selection(&raw_name, &selected_set) {
                    continue;
                }

                cumulative_bytes = verify_decompression_quota(
                    &raw_name,
                    entry.compressed_size(),
                    entry.size(),
                    cumulative_bytes,
                    DEFAULT_MAX_EXTRACTION_BYTES,
                )?;

                if entry.is_dir() {
                    let safe_rel = sanitize_relative_path(&raw_name)?;
                    fs::create_dir_all(destination.join(safe_rel))?;
                    continue;
                }

                if let Some(target_path) =
                    resolve_extraction_target(destination, &raw_name, overwrite_policy)?
                {
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut out_file = File::create(&target_path)?;
                    loop {
                        let n = entry.read(&mut copy_buf)?;
                        if n == 0 {
                            break;
                        }
                        out_file.write_all(&copy_buf[..n])?;
                    }
                }
            }
            Ok(cumulative_bytes)
        }
        ArchiveFormat::Kinetic => {
            fs::create_dir_all(destination)?;
            let (_header, index, crypto_key) = read_kinetic_index(archive_path, password)
                .map_err(|e| CompatError::Format(e.to_string()))?;

            let mut needed_blocks: HashSet<u32> = HashSet::new();
            for entry in &index.files {
                if entry_matches_selection(&entry.path, &selected_set) {
                    for &bid in &entry.block_ids {
                        needed_blocks.insert(bid);
                    }
                }
            }

            let mut f = File::open(archive_path)?;
            let mut block_cache: HashMap<u32, Vec<u8>> =
                HashMap::with_capacity(needed_blocks.len());
            for desc in &index.blocks {
                if needed_blocks.contains(&desc.block_id) {
                    let raw = read_and_verify_block(
                        &mut f,
                        desc,
                        &index.blocks,
                        index.parity_offset,
                        index.parity_length,
                        crypto_key.as_ref(),
                    )
                    .map_err(|e| CompatError::Format(e.to_string()))?;
                    block_cache.insert(desc.block_id, raw);
                }
            }

            let mut cumulative_bytes = 0u64;
            for entry in &index.files {
                if !entry_matches_selection(&entry.path, &selected_set) {
                    continue;
                }
                cumulative_bytes = verify_decompression_quota(
                    &entry.path,
                    entry.compressed_size.max(1),
                    entry.uncompressed_size,
                    cumulative_bytes,
                    DEFAULT_MAX_EXTRACTION_BYTES,
                )?;

                if entry.is_dir {
                    let safe_rel = sanitize_relative_path(&entry.path)?;
                    fs::create_dir_all(destination.join(safe_rel))?;
                    continue;
                }

                if let Some(target_path) =
                    resolve_extraction_target(destination, &entry.path, overwrite_policy)?
                {
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut out_file = File::create(&target_path)?;
                    let mut file_hasher = blake3::Hasher::new();

                    for &bid in &entry.block_ids {
                        let chunk = block_cache.get(&bid).ok_or_else(|| {
                            CompatError::Format(format!("Missing block ID {}", bid))
                        })?;
                        file_hasher.update(chunk);
                        out_file.write_all(chunk)?;
                    }

                    let actual_blake3 = file_hasher.finalize().to_hex().to_string();
                    if actual_blake3 != entry.blake3_hex {
                        return Err(CompatError::Format(format!(
                            "BLAKE3 integrity verification failed for extracted file '{}'",
                            entry.path
                        )));
                    }
                }
            }
            Ok(cumulative_bytes)
        }
        _ => extract_archive_with_password(archive_path, destination, overwrite_policy, password),
    }
}

pub fn extract_archive_with_password(
    archive_path: &Path,
    destination: &Path,
    overwrite_policy: &str,
    password: Option<&str>,
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
            extract_selected_entries(archive_path, destination, &["".to_string()][..0], overwrite_policy, password)
        }
    }
}

pub fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    overwrite_policy: &str,
) -> Result<u64, CompatError> {
    extract_archive_with_password(archive_path, destination, overwrite_policy, None)
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

        let report = validate_archive(&zip_path).unwrap();
        assert!(report.valid);

        let extract_dir = dir.path().join("extracted_zip");
        extract_archive(&zip_path, &extract_dir, "overwrite").unwrap();
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
        create_archive(
            &[src_file.clone()],
            &sz_path,
            ArchiveFormat::SevenZ,
            CompressionMode::Maximum,
        )
        .unwrap();

        let extract_dir = dir.path().join("extracted_7z");
        extract_archive(&sz_path, &extract_dir, "overwrite").unwrap();
        let restored = std::fs::read(extract_dir.join("sample_7z.txt")).unwrap();
        assert_eq!(restored, payload);
    }

    #[test]
    fn test_native_kinetic_create_inspect_validate_and_extract_roundtrip() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("native_engine.txt");
        let payload =
            b"Kinetic Engine Phase 5 Full Native .kinetic Binary Format Roundtrip!".repeat(256);
        std::fs::write(&src_file, &payload).unwrap();

        let knt_path = dir.path().join("native_output.kinetic");
        let written = create_archive(
            &[src_file.clone()],
            &knt_path,
            ArchiveFormat::Kinetic,
            CompressionMode::Balanced,
        )
        .unwrap();
        assert_eq!(written, payload.len() as u64);

        let inspection = inspect_archive(&knt_path).unwrap();
        assert_eq!(inspection.format, ArchiveFormat::Kinetic);
        assert_eq!(inspection.total_entries, 1);

        let report = validate_archive(&knt_path).unwrap();
        assert!(report.valid);

        let extract_dir = dir.path().join("extracted_kinetic");
        let extracted_bytes =
            extract_selected_entries(&knt_path, &extract_dir, &["native_engine.txt".to_string()], "overwrite", None)
                .unwrap();
        assert_eq!(extracted_bytes, payload.len() as u64);

        let restored = std::fs::read(extract_dir.join("native_engine.txt")).unwrap();
        assert_eq!(restored, payload);
    }

    #[test]
    fn test_selective_entry_extraction() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("keep_me.txt");
        let f2 = dir.path().join("skip_me.txt");
        std::fs::write(&f1, b"Selected entry content").unwrap();
        std::fs::write(&f2, b"Unselected entry content").unwrap();

        let knt_path = dir.path().join("selective.kinetic");
        create_archive(
            &[f1, f2],
            &knt_path,
            ArchiveFormat::Kinetic,
            CompressionMode::Fast,
        )
        .unwrap();

        let extract_dir = dir.path().join("selective_out");
        extract_selected_entries(
            &knt_path,
            &extract_dir,
            &["keep_me.txt".to_string()],
            "overwrite",
            None,
        )
        .unwrap();

        assert!(extract_dir.join("keep_me.txt").exists());
        assert!(!extract_dir.join("skip_me.txt").exists());
    }
}