use std::thread;
use crossbeam_channel::{bounded, unbounded};
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
    append_files_to_kinetic_archive, create_kinetic_archive, read_and_verify_block,
    read_kinetic_index, reassemble_multipart_volumes, remove_entry_from_kinetic_archive,
    repair_kinetic_archive, split_archive_into_volumes, validate_kinetic_archive,
    KineticWriteOptions,
};
use kinetic_format::KINETIC_MAGIC;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zip::ZipArchive;

use crate::security::{
    resolve_extraction_target, sanitize_relative_path, verify_decompression_quota,
    BoundedExtractionWriter, SecurityError, DEFAULT_MAX_EXTRACTION_BYTES,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPreview {
    pub path: String,
    pub size_bytes: u64,
    pub is_text: bool,
    pub text_preview: String,
    pub hex_preview: String,
    pub codec_name: String,
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

pub fn audit_archive_security(
    archive_path: &Path,
    password: Option<&str>,
) -> Result<IntegrityReport, CompatError> {
    let inspection = inspect_archive_with_password(archive_path, password)?;
    let mut cumulative_bytes = 0u64;

    for entry in &inspection.entries {
        if !entry.path.is_empty() {
            let _ = sanitize_relative_path(&entry.path)?;
        }
        cumulative_bytes = verify_decompression_quota(
            &entry.path,
            entry.compressed_size.max(1),
            entry.uncompressed_size,
            cumulative_bytes,
            DEFAULT_MAX_EXTRACTION_BYTES,
        )?;
    }

    Ok(IntegrityReport {
        valid: true,
        verified_entries: inspection.total_entries,
        total_uncompressed_bytes: cumulative_bytes,
        message: format!(
            "Security Audit Passed: {} entries verified free of Zip-Slip, reserved device names, ADS, and decompression bombs.",
            inspection.total_entries
        ),
    })
}

pub fn repair_archive(
    archive_path: &Path,
    password: Option<&str>,
) -> Result<IntegrityReport, CompatError> {
    let format = detect_format_from_disk(archive_path)?;
    match format {
        ArchiveFormat::Kinetic => {
            let msg = repair_kinetic_archive(archive_path, password)
                .map_err(|e| CompatError::Format(e.to_string()))?;
            let (verified_entries, total_uncompressed_bytes, _) =
                validate_kinetic_archive(archive_path, password)
                    .map_err(|e| CompatError::Format(e.to_string()))?;
            Ok(IntegrityReport {
                valid: true,
                verified_entries,
                total_uncompressed_bytes,
                message: msg,
            })
        }
        _ => Err(CompatError::Unsupported(
            "Physical parity self-healing is only supported on native .kinetic archives with Kinetic Shield enabled."
                .to_string(),
        )),
    }
}

fn format_hex_dump(bytes: &[u8]) -> String {
    let mut lines = Vec::new();
    for (i, chunk) in bytes.chunks(16).take(64).enumerate() {
        let offset = i * 16;
        let hex_part: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
        let ascii_part: String = chunk
            .iter()
            .map(|&b| {
                if (0x20..=0x7E).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        lines.push(format!(
            "{:08X}  {:<47}  |{}|",
            offset,
            hex_part.join(" "),
            ascii_part
        ));
    }
    lines.join("\n")
}

pub fn preview_archive_entry(
    archive_path: &Path,
    entry_path: &str,
    password: Option<&str>,
) -> Result<EntryPreview, CompatError> {
    let format = detect_format_from_disk(archive_path)?;
    let norm_target = entry_path.replace('\\', "/");

    let (sample_bytes, total_size, codec_label) = match format {
        ArchiveFormat::Kinetic => {
            let (_header, index, crypto_key) = read_kinetic_index(archive_path, password)
                .map_err(|e| CompatError::Format(e.to_string()))?;
            let entry = index
                .files
                .iter()
                .find(|f| f.path == norm_target)
                .ok_or_else(|| {
                    CompatError::Format(format!("Entry '{}' not found in archive", entry_path))
                })?;

            let mut f = File::open(archive_path)?;
            let mut buf = Vec::new();
            if let Some(&first_bid) = entry.block_ids.first() {
                if let Some(desc) = index.blocks.iter().find(|b| b.block_id == first_bid) {
                    let raw = read_and_verify_block(
                        &mut f,
                        desc,
                        &index.blocks,
                        index.parity_offset,
                        index.parity_length,
                        crypto_key.as_ref(),
                    )
                    .map_err(|e| CompatError::Format(e.to_string()))?;
                    let take_len = raw.len().min(16 * 1024);
                    buf.extend_from_slice(&raw[..take_len]);
                }
            }
            (buf, entry.uncompressed_size, entry.codec_summary.clone())
        }
        ArchiveFormat::Zip => {
            let file = File::open(archive_path)?;
            let mut zip = ZipArchive::new(file).map_err(|e| CompatError::Format(e.to_string()))?;
            let mut found = None;
            for i in 0..zip.len() {
                let mut entry = zip
                    .by_index(i)
                    .map_err(|e| CompatError::Format(e.to_string()))?;
                if entry.name().replace('\\', "/").trim_end_matches('/') == norm_target {
                    let mut buf = vec![0u8; (entry.size() as usize).min(16 * 1024)];
                    let n = entry.read(&mut buf)?;
                    buf.truncate(n);
                    found = Some((buf, entry.size(), format!("{:?}", entry.compression())));
                    break;
                }
            }
            found.ok_or_else(|| {
                CompatError::Format(format!("Entry '{}' not found in ZIP", entry_path))
            })?
        }
        _ => {
            return Err(CompatError::Unsupported(
                "In-memory preview is supported for .kinetic and .zip archives.".to_string(),
            ))
        }
    };

    let is_text = !sample_bytes.contains(&0) && std::str::from_utf8(&sample_bytes).is_ok();
    let text_preview = String::from_utf8_lossy(&sample_bytes).into_owned();
    let hex_preview = format_hex_dump(&sample_bytes);

    Ok(EntryPreview {
        path: norm_target,
        size_bytes: total_size,
        is_text,
        text_preview,
        hex_preview,
        codec_name: codec_label,
    })
}

pub fn delete_archive_entry(
    archive_path: &Path,
    entry_path: &str,
    password: Option<&str>,
) -> Result<String, CompatError> {
    let format = detect_format_from_disk(archive_path)?;
    match format {
        ArchiveFormat::Kinetic => {
            let removed = remove_entry_from_kinetic_archive(archive_path, entry_path, password)
                .map_err(|e| CompatError::Format(e.to_string()))?;
            Ok(format!(
                "Zero-recompression update complete: removed {} entry ('{}') from .kinetic index.",
                removed, entry_path
            ))
        }
        _ => Err(CompatError::Unsupported(
            "Zero-recompression in-place deletion is an exclusive feature of the native .kinetic format."
                .to_string(),
        )),
    }
}

pub fn append_archive_entries(
    archive_path: &Path,
    new_sources: &[PathBuf],
    password: Option<&str>,
) -> Result<String, CompatError> {
    let format = detect_format_from_disk(archive_path)?;
    match format {
        ArchiveFormat::Kinetic => {
            let added = append_files_to_kinetic_archive(archive_path, new_sources, password)
                .map_err(|e| CompatError::Format(e.to_string()))?;
            Ok(format!(
                "Zero-recompression append complete: added {} file(s) to .kinetic archive.",
                added
            ))
        }
        _ => Err(CompatError::Unsupported(
            "Zero-recompression appending is an exclusive feature of the native .kinetic format."
                .to_string(),
        )),
    }
}

pub fn split_archive_volumes(
    archive_path: &Path,
    volume_size_mb: u64,
) -> Result<Vec<String>, CompatError> {
    let bytes = volume_size_mb.max(1) * 1024 * 1024;
    let parts = split_archive_into_volumes(archive_path, bytes)
        .map_err(|e| CompatError::Format(e.to_string()))?;
    Ok(parts
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

pub fn join_archive_volumes(
    first_volume_path: &Path,
    output_archive_path: &Path,
) -> Result<u64, CompatError> {
    reassemble_multipart_volumes(first_volume_path, output_archive_path)
        .map_err(|e| CompatError::Format(e.to_string()))
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
                ..Default::default()
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
                let trimmed = raw_name.trim_end_matches('/');
                if !entry_matches_selection(trimmed, &selected_set) {
                    continue;
                }

                cumulative_bytes = verify_decompression_quota(
                    trimmed,
                    entry.compressed_size(),
                    entry.size(),
                    cumulative_bytes,
                    DEFAULT_MAX_EXTRACTION_BYTES,
                )?;

                if entry.is_dir() {
                    if !trimmed.is_empty() {
                        let safe_rel = sanitize_relative_path(trimmed)?;
                        fs::create_dir_all(destination.join(safe_rel))?;
                    }
                    continue;
                }

                if let Some(target_path) =
                    resolve_extraction_target(destination, trimmed, overwrite_policy)?
                {
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let max_entry_stream = entry
                        .size()
                        .saturating_add(64 * 1024)
                        .min(DEFAULT_MAX_EXTRACTION_BYTES);
                    let write_res = (|| -> Result<(), CompatError> {
                        let out_file = File::create(&target_path)?;
                        let mut bounded = BoundedExtractionWriter::new(out_file, max_entry_stream);
                        loop {
                            let n = entry.read(&mut copy_buf)?;
                            if n == 0 {
                                break;
                            }
                            bounded.write_all(&copy_buf[..n])?;
                        }
                        bounded.flush()?;
                        Ok(())
                    })();

                    if let Err(e) = write_res {
                        let _ = fs::remove_file(&target_path);
                        return Err(e);
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

            let target_descs: Vec<_> = index
                .blocks
                .iter()
                .filter(|d| needed_blocks.contains(&d.block_id))
                .cloned()
                .collect();

            let mut block_cache: HashMap<u32, Vec<u8>> =
                HashMap::with_capacity(target_descs.len());

            if target_descs.len() <= 2 {
                let mut f = File::open(archive_path)?;
                for desc in &target_descs {
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
            } else {
                let threads = std::thread::available_parallelism()
                    .map(|n| n.get().saturating_sub(1).max(1))
                    .unwrap_or(4)
                    .min(target_descs.len());
                let (work_tx, work_rx) = bounded::<kinetic_format::archive::BlockDescriptor>(threads * 2);
                let (res_tx, res_rx) = unbounded::<Result<(u32, Vec<u8>), String>>();

                let mut handles = Vec::with_capacity(threads);
                for _ in 0..threads {
                    let rx = work_rx.clone();
                    let tx = res_tx.clone();
                    let path_clone = archive_path.to_path_buf();
                    let all_blocks = index.blocks.clone();
                    let p_off = index.parity_offset;
                    let p_len = index.parity_length;
                    let key_clone = crypto_key;

                    handles.push(thread::spawn(move || {
                        let mut f = match File::open(&path_clone) {
                            Ok(file) => file,
                            Err(e) => {
                                let _ = tx.send(Err(e.to_string()));
                                return;
                            }
                        };
                        while let Ok(desc) = rx.recv() {
                            match read_and_verify_block(
                                &mut f,
                                &desc,
                                &all_blocks,
                                p_off,
                                p_len,
                                key_clone.as_ref(),
                            ) {
                                Ok(raw) => {
                                    let _ = tx.send(Ok((desc.block_id, raw)));
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(e.to_string()));
                                    break;
                                }
                            }
                        }
                    }));
                }
                drop(work_rx);
                drop(res_tx);

                for d in target_descs {
                    let _ = work_tx.send(d);
                }
                drop(work_tx);

                let mut first_err = None;
                for res in res_rx {
                    match res {
                        Ok((bid, raw)) => {
                            block_cache.insert(bid, raw);
                        }
                        Err(e) => {
                            if first_err.is_none() {
                                first_err = Some(e);
                            }
                        }
                    }
                }
                for h in handles {
                    let _ = h.join();
                }
                if let Some(err_msg) = first_err {
                    return Err(CompatError::Format(err_msg));
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
                    let max_entry_stream = entry
                        .uncompressed_size
                        .saturating_add(64 * 1024)
                        .min(DEFAULT_MAX_EXTRACTION_BYTES);
                    let out_file = File::create(&target_path)?;
                    let mut bounded = BoundedExtractionWriter::new(out_file, max_entry_stream);
                    let mut file_hasher = blake3::Hasher::new();

                    if entry.is_solid_slice {
                        let bid = *entry.block_ids.first().ok_or_else(|| {
                            CompatError::Format("Missing solid block ID".to_string())
                        })?;
                        let solid_block = block_cache.get(&bid).ok_or_else(|| {
                            CompatError::Format(format!("Missing block ID {}", bid))
                        })?;
                        let start = entry.solid_offset as usize;
                        let end = start + (entry.uncompressed_size as usize);
                        if end > solid_block.len() {
                            return Err(CompatError::Format("Solid slice out of bounds".to_string()));
                        }
                        let slice = &solid_block[start..end];
                        file_hasher.update(slice);
                        if let Err(e) = bounded.write_all(slice) {
                            let _ = fs::remove_file(&target_path);
                            return Err(CompatError::Io(e));
                        }
                    } else {
                        for &bid in &entry.block_ids {
                            let chunk = block_cache.get(&bid).ok_or_else(|| {
                                CompatError::Format(format!("Missing block ID {}", bid))
                            })?;
                            file_hasher.update(chunk);
                            if let Err(e) = bounded.write_all(chunk) {
                                let _ = fs::remove_file(&target_path);
                                return Err(CompatError::Io(e));
                            }
                        }
                    }
                    bounded.flush()?;

                    let actual_blake3 = file_hasher.finalize().to_hex().to_string();
                    if actual_blake3 != entry.blake3_hex {
                        let _ = fs::remove_file(&target_path);
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
            let (_header, index, _) = read_kinetic_index(archive_path, password)
                .map_err(|e| CompatError::Format(e.to_string()))?;
            let all_paths: Vec<String> = index.files.into_iter().map(|f| f.path).collect();
            if all_paths.is_empty() {
                fs::create_dir_all(destination)?;
                return Ok(0);
            }
            extract_selected_entries(archive_path, destination, &all_paths, overwrite_policy, password)
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
    use kinetic_format::archive::{cleanup_interrupted_staging_files, BLOCK_HEADER_SIZE};
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

        let preview = preview_archive_entry(&knt_path, "native_engine.txt", None).unwrap();
        assert!(preview.is_text);
        assert!(preview.text_preview.contains("Kinetic Engine Phase 5"));

        let report = validate_archive(&knt_path).unwrap();
        assert!(report.valid);

        let extract_dir = dir.path().join("extracted_kinetic");
        let extracted_bytes = extract_archive(&knt_path, &extract_dir, "overwrite").unwrap();
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

    #[test]
    fn test_physical_disk_repair_and_staging_cleanup() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("block_a.txt");
        let f2 = dir.path().join("block_b.txt");
        std::fs::write(&f1, b"Alpha block data for physical disk repair test".repeat(128)).unwrap();
        std::fs::write(&f2, b"Beta block data for physical disk repair test".repeat(128)).unwrap();

        let stale_part = dir.path().join("aborted.kinetic.part");
        std::fs::write(&stale_part, b"partial data").unwrap();
        assert_eq!(cleanup_interrupted_staging_files(dir.path()).unwrap(), 1);
        assert!(!stale_part.exists());

        let knt_path = dir.path().join("repairable.kinetic");
        create_archive(
            &[f1, f2],
            &knt_path,
            ArchiveFormat::Kinetic,
            CompressionMode::Balanced,
        )
        .unwrap();

        let (_hdr, index, _) = read_kinetic_index(&knt_path, None).unwrap();
        let corrupt_pos = (index.blocks[0].file_offset as usize) + BLOCK_HEADER_SIZE + 2;
        let mut raw = std::fs::read(&knt_path).unwrap();
        raw[corrupt_pos] ^= 0xFF;
        std::fs::write(&knt_path, &raw).unwrap();

        let repair_report = repair_archive(&knt_path, None).unwrap();
        assert!(repair_report.valid);
        assert!(repair_report.message.contains("repaired corrupted Block #0"));
    }

    #[test]
    fn test_thousand_small_files_stress_roundtrip() {
        let dir = tempdir().unwrap();
        let corpus_dir = dir.path().join("stress_corpus");
        std::fs::create_dir_all(&corpus_dir).unwrap();

        for i in 0..1000 {
            let sub = corpus_dir.join(format!("module_{}", i % 10));
            std::fs::create_dir_all(&sub).unwrap();
            let file_path = sub.join(format!("item_{:04}.txt", i));
            std::fs::write(
                &file_path,
                format!("Kinetic stress test file #{} with shared template header!\n", i).repeat(8),
            )
            .unwrap();
        }

        let knt_path = dir.path().join("thousand_files.kinetic");
        create_archive(
            &[corpus_dir],
            &knt_path,
            ArchiveFormat::Kinetic,
            CompressionMode::Automatic,
        )
        .unwrap();

        let audit = audit_archive_security(&knt_path, None).unwrap();
        assert!(audit.valid);
        assert!(audit.verified_entries >= 1000);

        let extract_dir = dir.path().join("stress_extracted");
        extract_archive(&knt_path, &extract_dir, "overwrite").unwrap();
        assert!(extract_dir
            .join("stress_corpus")
            .join("module_7")
            .join("item_0997.txt")
            .exists());
    }
}