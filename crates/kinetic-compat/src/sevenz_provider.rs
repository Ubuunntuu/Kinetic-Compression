use std::fs::{self, File};
use std::path::{Path, PathBuf};
use sevenz_rust::{Password, SevenZArchiveEntry, SevenZReader, SevenZWriter};
use walkdir::WalkDir;

use crate::security::{
    resolve_extraction_target, sanitize_relative_path, verify_decompression_quota,
    DEFAULT_MAX_EXTRACTION_BYTES,
};
use crate::{ArchiveEntry, CompatError, IntegrityReport};

pub fn list_sevenz_entries(archive_path: &Path) -> Result<Vec<ArchiveEntry>, CompatError> {
    let reader = SevenZReader::open(archive_path, Password::empty())
        .map_err(|e| CompatError::Format(e.to_string()))?;

    let mut entries = Vec::new();
    for file in &reader.archive().files {
        let raw_name = file.name().replace('\\', "/");
        let _ = sanitize_relative_path(&raw_name)?;

        let crc_hex = if file.has_crc {
            Some(format!("{:08x}", file.crc as u32))
        } else {
            None
        };

        entries.push(ArchiveEntry {
            path: raw_name.trim_end_matches('/').to_string(),
            is_dir: file.is_directory(),
            uncompressed_size: file.size(),
            compressed_size: file.compressed_size,
            modified_unix_secs: 0,
            checksum_hex: crc_hex,
            codec_name: "LZMA2".to_string(),
        });
    }

    Ok(entries)
}

pub fn validate_sevenz_archive(archive_path: &Path) -> Result<IntegrityReport, CompatError> {
    let mut reader = SevenZReader::open(archive_path, Password::empty())
        .map_err(|e| CompatError::Format(e.to_string()))?;

    let mut verified_entries = 0usize;
    let mut total_uncompressed_bytes = 0u64;
    let mut sink = vec![0u8; 64 * 1024];

    reader
        .for_each_entries(|entry, entry_reader| {
            let raw_name = entry.name().to_string();
            sanitize_relative_path(&raw_name)
                .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;

            total_uncompressed_bytes = verify_decompression_quota(
                &raw_name,
                entry.compressed_size,
                entry.size(),
                total_uncompressed_bytes,
                DEFAULT_MAX_EXTRACTION_BYTES,
            )
            .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;

            if !entry.is_directory() {
                while entry_reader.read(&mut sink)? > 0 {}
                verified_entries += 1;
            }
            Ok(true)
        })
        .map_err(|e| CompatError::Format(e.to_string()))?;

    Ok(IntegrityReport {
        valid: true,
        verified_entries,
        total_uncompressed_bytes,
        message: format!(
            "Verified {} 7Z entries ({} bytes) with LZMA2 stream integrity.",
            verified_entries, total_uncompressed_bytes
        ),
    })
}

pub fn create_sevenz_archive(
    source_paths: &[PathBuf],
    output_path: &Path,
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

    let staging_path = output_path.with_extension("7z.part");
    let result = (|| -> Result<u64, CompatError> {
        let mut writer = SevenZWriter::create(&staging_path)
            .map_err(|e| CompatError::Format(e.to_string()))?;
        let mut total_input_bytes = 0u64;

        for src in source_paths {
            if !src.exists() {
                return Err(CompatError::Format(format!(
                    "Source path does not exist: {}",
                    src.display()
                )));
            }

            if src.is_file() {
                let entry_name = src
                    .file_name()
                    .ok_or_else(|| CompatError::Format("Invalid source file name".to_string()))?
                    .to_string_lossy()
                    .replace('\\', "/");
                let len = src.metadata()?.len();
                let entry = SevenZArchiveEntry::from_path(src, entry_name);
                writer
                    .push_archive_entry(entry, Some(File::open(src)?))
                    .map_err(|e| CompatError::Format(e.to_string()))?;
                total_input_bytes += len;
            } else if src.is_dir() {
                let base_parent = src.parent().unwrap_or(src);
                for walk_entry in WalkDir::new(src) {
                    let walk_entry =
                        walk_entry.map_err(|e| CompatError::Format(e.to_string()))?;
                    let path = walk_entry.path();
                    let rel = path
                        .strip_prefix(base_parent)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .replace('\\', "/");

                    if rel.is_empty() {
                        continue;
                    }

                    let entry = SevenZArchiveEntry::from_path(path, rel);
                    if path.is_file() {
                        let len = path.metadata()?.len();
                        writer
                            .push_archive_entry(entry, Some(File::open(path)?))
                            .map_err(|e| CompatError::Format(e.to_string()))?;
                        total_input_bytes += len;
                    } else if path.is_dir() {
                        writer
                            .push_archive_entry::<File>(entry, None)
                            .map_err(|e| CompatError::Format(e.to_string()))?;
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
        Ok(bytes) => {
            if output_path.exists() {
                fs::remove_file(output_path)?;
            }
            fs::rename(&staging_path, output_path)?;
            Ok(bytes)
        }
        Err(err) => {
            let _ = fs::remove_file(&staging_path);
            Err(err)
        }
    }
}

pub fn extract_sevenz_archive(
    archive_path: &Path,
    destination: &Path,
    overwrite_policy: &str,
) -> Result<u64, CompatError> {
    fs::create_dir_all(destination)?;
    let mut reader = SevenZReader::open(archive_path, Password::empty())
        .map_err(|e| CompatError::Format(e.to_string()))?;
    let mut cumulative_bytes = 0u64;

    reader
        .for_each_entries(|entry, entry_reader| {
            let raw_name = entry.name().to_string();
            cumulative_bytes = verify_decompression_quota(
                &raw_name,
                entry.compressed_size,
                entry.size(),
                cumulative_bytes,
                DEFAULT_MAX_EXTRACTION_BYTES,
            )
            .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;

            if entry.is_directory() {
                let safe_rel = sanitize_relative_path(&raw_name)
                    .map_err(|e| sevenz_rust::Error::other(e.to_string()))?;
                fs::create_dir_all(destination.join(safe_rel))?;
                return Ok(true);
            }

            if let Some(target_path) =
                resolve_extraction_target(destination, &raw_name, overwrite_policy)
                    .map_err(|e| sevenz_rust::Error::other(e.to_string()))?
            {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out_file = File::create(&target_path)?;
                std::io::copy(entry_reader, &mut out_file)?;
            }
            Ok(true)
        })
        .map_err(|e| CompatError::Format(e.to_string()))?;

    Ok(cumulative_bytes)
}