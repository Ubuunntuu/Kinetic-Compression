use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use kinetic_core::CompressionMode;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::security::{
    resolve_extraction_target, sanitize_relative_path, verify_decompression_quota,
    DEFAULT_MAX_EXTRACTION_BYTES,
};
use crate::{ArchiveEntry, CompatError, IntegrityReport};

fn map_zip_options(mode: CompressionMode) -> SimpleFileOptions {
    let (method, level) = match mode {
        CompressionMode::Store => (CompressionMethod::Stored, None),
        CompressionMode::Fast => (CompressionMethod::Deflated, Some(1)),
        CompressionMode::Balanced | CompressionMode::Automatic => {
            (CompressionMethod::Deflated, Some(6))
        }
        CompressionMode::Maximum | CompressionMode::Ultra => {
            (CompressionMethod::Deflated, Some(9))
        }
    };

    SimpleFileOptions::default()
        .compression_method(method)
        .compression_level(level)
        .large_file(true)
}

pub fn list_zip_entries(archive_path: &Path) -> Result<Vec<ArchiveEntry>, CompatError> {
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| CompatError::Format(e.to_string()))?;
    let mut entries = Vec::with_capacity(archive.len());

    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|e| CompatError::Format(e.to_string()))?;

        let raw_name = entry.name().replace('\\', "/");
        let _ = sanitize_relative_path(&raw_name)?;

        entries.push(ArchiveEntry {
            path: raw_name.trim_end_matches('/').to_string(),
            is_dir: entry.is_dir(),
            uncompressed_size: entry.size(),
            compressed_size: entry.compressed_size(),
            modified_unix_secs: 0,
            checksum_hex: Some(format!("{:08x}", entry.crc32())),
            codec_name: format!("{:?}", entry.compression()),
        });
    }

    Ok(entries)
}

pub fn validate_zip_archive(archive_path: &Path) -> Result<IntegrityReport, CompatError> {
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| CompatError::Format(e.to_string()))?;
    let mut verified_files = 0usize;
    let mut total_uncompressed_bytes = 0u64;
    let mut sink_buf = vec![0u8; 64 * 1024];

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| CompatError::Format(e.to_string()))?;
        let raw_name = entry.name().to_string();
        let _ = sanitize_relative_path(&raw_name)?;

        total_uncompressed_bytes = verify_decompression_quota(
            &raw_name,
            entry.compressed_size(),
            entry.size(),
            total_uncompressed_bytes,
            DEFAULT_MAX_EXTRACTION_BYTES,
        )?;

        if !entry.is_dir() {
            while entry.read(&mut sink_buf)? > 0 {}
            verified_files += 1;
        }
    }

    Ok(IntegrityReport {
        valid: true,
        verified_entries: verified_files,
        total_uncompressed_bytes,
        message: format!(
            "Verified {} ZIP entries ({} bytes) with zero CRC32 or path errors.",
            verified_files, total_uncompressed_bytes
        ),
    })
}

pub fn create_zip_archive(
    source_paths: &[PathBuf],
    output_path: &Path,
    mode: CompressionMode,
) -> Result<u64, CompatError> {
    if source_paths.is_empty() {
        return Err(CompatError::Format(
            "At least one input file or folder is required.".to_string(),
        ));
    }

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let staging_path = output_path.with_extension("zip.part");
    let result = (|| -> Result<u64, CompatError> {
        let out_file = File::create(&staging_path)?;
        let mut writer = ZipWriter::new(out_file);
        let options = map_zip_options(mode);
        let mut total_input_bytes = 0u64;
        let mut copy_buf = vec![0u8; 128 * 1024];

        for src in source_paths {
            if !src.exists() {
                return Err(CompatError::Format(format!(
                    "Source path does not exist: {}",
                    src.display()
                )));
            }

            if src.is_file() {
                let file_name = src
                    .file_name()
                    .ok_or_else(|| CompatError::Format("Invalid source file name".to_string()))?
                    .to_string_lossy()
                    .replace('\\', "/");

                writer
                    .start_file(&file_name, options)
                    .map_err(|e| CompatError::Format(e.to_string()))?;
                let mut f = File::open(src)?;
                loop {
                    let read_len = f.read(&mut copy_buf)?;
                    if read_len == 0 {
                        break;
                    }
                    writer.write_all(&copy_buf[..read_len])?;
                    total_input_bytes += read_len as u64;
                }
            } else if src.is_dir() {
                let base_parent = src.parent().unwrap_or(src);
                for entry in WalkDir::new(src) {
                    let entry = entry.map_err(|e| CompatError::Format(e.to_string()))?;
                    let path = entry.path();
                    let rel = path
                        .strip_prefix(base_parent)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .replace('\\', "/");

                    if rel.is_empty() {
                        continue;
                    }

                    if path.is_dir() {
                        let dir_name = if rel.ends_with('/') {
                            rel
                        } else {
                            format!("{}/", rel)
                        };
                        writer
                            .add_directory(&dir_name, options)
                            .map_err(|e| CompatError::Format(e.to_string()))?;
                    } else if path.is_file() {
                        writer
                            .start_file(&rel, options)
                            .map_err(|e| CompatError::Format(e.to_string()))?;
                        let mut f = File::open(path)?;
                        loop {
                            let read_len = f.read(&mut copy_buf)?;
                            if read_len == 0 {
                                break;
                            }
                            writer.write_all(&copy_buf[..read_len])?;
                            total_input_bytes += read_len as u64;
                        }
                    }
                }
            }
        }

        writer
            .finish()
            .map_err(|e| CompatError::Format(e.to_string()))?;
        Ok(total_input_bytes)
    })();

    match result {
        Ok(total_bytes) => {
            if output_path.exists() {
                fs::remove_file(output_path)?;
            }
            fs::rename(&staging_path, output_path)?;
            Ok(total_bytes)
        }
        Err(err) => {
            let _ = fs::remove_file(&staging_path);
            Err(err)
        }
    }
}

pub fn extract_zip_archive(
    archive_path: &Path,
    destination: &Path,
    overwrite_policy: &str,
) -> Result<u64, CompatError> {
    fs::create_dir_all(destination)?;
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| CompatError::Format(e.to_string()))?;
    let mut cumulative_bytes = 0u64;
    let mut copy_buf = vec![0u8; 128 * 1024];

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| CompatError::Format(e.to_string()))?;
        let raw_name = entry.name().to_string();

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