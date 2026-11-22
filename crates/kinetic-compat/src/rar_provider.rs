use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::Command;

use crate::security::sanitize_relative_path;
use crate::{ArchiveEntry, CompatError, IntegrityReport};

pub const RAR4_MAGIC: [u8; 7] = *b"Rar!\x1A\x07\x00";
pub const RAR5_MAGIC: [u8; 8] = *b"Rar!\x1A\x07\x01\x00";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RarGeneration {
    Rar4,
    Rar5,
}

pub fn inspect_rar_header(archive_path: &Path) -> Result<(RarGeneration, bool), CompatError> {
    let mut f = File::open(archive_path)?;
    let mut buf = [0u8; 8];
    let n = f.read(&mut buf)?;

    let generation = if n >= 8 && buf[0..8] == RAR5_MAGIC {
        RarGeneration::Rar5
    } else if n >= 7 && buf[0..7] == RAR4_MAGIC {
        RarGeneration::Rar4
    } else {
        return Err(CompatError::Format(
            "Invalid RAR signature: file is neither a valid RAR4 nor RAR5 archive.".to_string(),
        ));
    };

    let file_name = archive_path
        .file_name()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let is_multipart = file_name.contains(".part")
        || (file_name.len() >= 4
            && file_name
                .chars()
                .rev()
                .take(2)
                .all(|c| c.is_ascii_digit())
            && file_name.contains(".r"));

    Ok((generation, is_multipart))
}

fn find_unrar_binary() -> Option<String> {
    let candidates = [
        r"C:\Program Files\WinRAR\UnRAR.exe",
        r"C:\Program Files\WinRAR\Rar.exe",
        r"C:\Program Files (x86)\WinRAR\UnRAR.exe",
        r"C:\Program Files (x86)\WinRAR\Rar.exe",
    ];
    for candidate in candidates {
        if Path::new(candidate).exists() {
            return Some(candidate.to_string());
        }
    }
    None
}

pub fn list_rar_entries(archive_path: &Path) -> Result<Vec<ArchiveEntry>, CompatError> {
    let (generation, is_multipart) = inspect_rar_header(archive_path)?;
    let codec_label = match (generation, is_multipart) {
        (RarGeneration::Rar5, true) => "RAR5 (Multipart)",
        (RarGeneration::Rar5, false) => "RAR5",
        (RarGeneration::Rar4, true) => "RAR4 (Multipart)",
        (RarGeneration::Rar4, false) => "RAR4",
    };

    let bin = find_unrar_binary().ok_or_else(|| {
        CompatError::Format(
            "RAR header validated, but UnRAR/Rar.exe helper was not found in Program Files."
                .to_string(),
        )
    })?;

    let output = Command::new(&bin)
        .args(["lb", "-c-", &archive_path.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        return Err(CompatError::Format(format!(
            "RAR listing failed with exit code {:?}",
            output.status.code()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = trimmed.replace('\\', "/");
        let _ = sanitize_relative_path(&normalized)?;

        entries.push(ArchiveEntry {
            path: normalized,
            is_dir: false,
            uncompressed_size: 0,
            compressed_size: 0,
            modified_unix_secs: 0,
            checksum_hex: None,
            codec_name: codec_label.to_string(),
        });
    }

    Ok(entries)
}

pub fn validate_rar_archive(archive_path: &Path) -> Result<IntegrityReport, CompatError> {
    let (generation, _) = inspect_rar_header(archive_path)?;
    let entries = list_rar_entries(archive_path)?;

    if let Some(bin) = find_unrar_binary() {
        let output = Command::new(&bin)
            .args(["t", "-idq", &archive_path.to_string_lossy()])
            .output()?;
        if !output.status.success() {
            return Err(CompatError::Format(
                "RAR integrity test detected corrupted archive data.".to_string(),
            ));
        }
    }

    Ok(IntegrityReport {
        valid: true,
        verified_entries: entries.len(),
        total_uncompressed_bytes: fs::metadata(archive_path)?.len(),
        message: format!(
            "Verified {:?} archive ({} entries) with zero integrity errors.",
            generation,
            entries.len()
        ),
    })
}

pub fn extract_rar_archive(
    archive_path: &Path,
    destination: &Path,
    overwrite_policy: &str,
) -> Result<u64, CompatError> {
    let entries = list_rar_entries(archive_path)?;
    for entry in &entries {
        let _ = sanitize_relative_path(&entry.path)?;
    }

    fs::create_dir_all(destination)?;
    let bin = find_unrar_binary().ok_or_else(|| {
        CompatError::Format("UnRAR/Rar.exe binary not found for RAR extraction.".to_string())
    })?;

    let overwrite_flag = match overwrite_policy {
        "overwrite" => "-o+",
        "skip" => "-o-",
        "rename" => "-or",
        _ => "-o-",
    };

    let mut dest_arg = destination.to_string_lossy().into_owned();
    if !dest_arg.ends_with('\\') {
        dest_arg.push('\\');
    }

    let output = Command::new(&bin)
        .args([
            "x",
            "-y",
            overwrite_flag,
            &archive_path.to_string_lossy(),
            &dest_arg,
        ])
        .output()?;

    if !output.status.success() {
        return Err(CompatError::Format(format!(
            "RAR extraction failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(fs::metadata(archive_path)?.len())
}