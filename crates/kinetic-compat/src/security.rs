use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const DEFAULT_MAX_EXTRACTION_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_SUSPICIOUS_COMPRESSION_RATIO: u64 = 1000;
pub const MAX_PATH_DEPTH: usize = 64;
pub const MAX_PATH_LENGTH: usize = 1024;

const WINDOWS_RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecurityError {
    #[error("Unsafe archive entry path blocked (Zip-Slip / traversal / reserved name): {0}")]
    PathTraversalBlocked(String),
    #[error("Archive entry path is empty or invalid")]
    EmptyEntryPath,
    #[error("Integer overflow detected in archive size metadata for entry '{0}'")]
    IntegerOverflowDetected(String),
    #[error("Decompression bomb detected: uncompressed size ({uncompressed} bytes) exceeds safety limit ({limit} bytes)")]
    DecompressionBombQuotaExceeded { uncompressed: u64, limit: u64 },
    #[error("Decompression bomb ratio detected on entry '{entry}': ratio {ratio}:1 exceeds safety threshold")]
    SuspiciousCompressionRatio { entry: String, ratio: u64 },
    #[error("Target file already exists and overwrite policy is set to 'ask': {0}")]
    OverwriteBlocked(String),
}

fn is_forbidden_segment(seg: &str) -> bool {
    if seg.is_empty() || seg == "." || seg == ".." {
        return true;
    }
    if seg.ends_with('.') || seg.ends_with(' ') {
        return true;
    }
    if seg.chars().any(|c| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')
    }) {
        return true;
    }
    let stem = seg.split('.').next().unwrap_or(seg).to_ascii_uppercase();
    WINDOWS_RESERVED_NAMES.contains(&stem.as_str())
}

/// Validates and sanitizes an archive entry path against Zip-Slip, NTFS ADS, reserved device names, and deep nesting.
pub fn sanitize_relative_path(raw_path: &str) -> Result<PathBuf, SecurityError> {
    if raw_path.trim().is_empty() || raw_path.contains('\0') {
        return Err(SecurityError::EmptyEntryPath);
    }
    if raw_path.len() > MAX_PATH_LENGTH {
        return Err(SecurityError::PathTraversalBlocked(
            "Path exceeds maximum safe length".to_string(),
        ));
    }

    let normalized = raw_path.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        return Err(SecurityError::PathTraversalBlocked(raw_path.to_string()));
    }

    let path = Path::new(&normalized);
    let mut clean = PathBuf::new();
    let mut depth = 0usize;

    for component in path.components() {
        match component {
            Component::Normal(segment) => {
                let seg_str = segment.to_string_lossy();
                if is_forbidden_segment(&seg_str) {
                    return Err(SecurityError::PathTraversalBlocked(raw_path.to_string()));
                }
                depth += 1;
                if depth > MAX_PATH_DEPTH {
                    return Err(SecurityError::PathTraversalBlocked(
                        "Directory depth exceeds safety limit".to_string(),
                    ));
                }
                clean.push(segment);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SecurityError::PathTraversalBlocked(raw_path.to_string()));
            }
        }
    }

    if clean.as_os_str().is_empty() {
        return Err(SecurityError::EmptyEntryPath);
    }

    Ok(clean)
}

/// Verifies declared metadata sizes using checked arithmetic to prevent 64-bit integer overflow and decompression bombs.
pub fn verify_decompression_quota(
    entry_name: &str,
    compressed_size: u64,
    uncompressed_size: u64,
    cumulative_uncompressed: u64,
    max_quota_bytes: u64,
) -> Result<u64, SecurityError> {
    let next_total = cumulative_uncompressed
        .checked_add(uncompressed_size)
        .ok_or_else(|| SecurityError::IntegerOverflowDetected(entry_name.to_string()))?;

    if next_total > max_quota_bytes {
        return Err(SecurityError::DecompressionBombQuotaExceeded {
            uncompressed: next_total,
            limit: max_quota_bytes,
        });
    }

    if uncompressed_size > 10 * 1024 * 1024 && compressed_size > 0 {
        let ratio = uncompressed_size / compressed_size;
        if ratio > MAX_SUSPICIOUS_COMPRESSION_RATIO {
            return Err(SecurityError::SuspiciousCompressionRatio {
                entry: entry_name.to_string(),
                ratio,
            });
        }
    }

    Ok(next_total)
}

/// Streaming writer wrapper that enforces a hard byte limit during extraction to stop lying-header bombs.
pub struct BoundedExtractionWriter<W: Write> {
    inner: W,
    written_bytes: u64,
    max_allowed_bytes: u64,
}

impl<W: Write> BoundedExtractionWriter<W> {
    pub fn new(inner: W, max_allowed_bytes: u64) -> Self {
        Self {
            inner,
            written_bytes: 0,
            max_allowed_bytes,
        }
    }

    pub fn written_bytes(&self) -> u64 {
        self.written_bytes
    }
}

impl<W: Write> Write for BoundedExtractionWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next = self
            .written_bytes
            .checked_add(buf.len() as u64)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Integer overflow in decompressed byte stream",
                )
            })?;

        if next > self.max_allowed_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Decompression bomb aborted: stream exceeded allowed quota of {} bytes",
                    self.max_allowed_bytes
                ),
            ));
        }

        let n = self.inner.write(buf)?;
        self.written_bytes += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub fn resolve_extraction_target(
    destination_root: &Path,
    raw_entry_path: &str,
    overwrite_policy: &str,
) -> Result<Option<PathBuf>, SecurityError> {
    let safe_rel = sanitize_relative_path(raw_entry_path)?;
    let candidate = destination_root.join(&safe_rel);

    if !candidate.exists() {
        return Ok(Some(candidate));
    }

    match overwrite_policy {
        "overwrite" => Ok(Some(candidate)),
        "skip" => Ok(None),
        "rename" => {
            let stem = candidate
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "extracted".to_string());
            let ext = candidate
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let parent = candidate.parent().unwrap_or(destination_root);

            for counter in 1..10_000 {
                let renamed = parent.join(format!("{}_{}{}", stem, counter, ext));
                if !renamed.exists() {
                    return Ok(Some(renamed));
                }
            }
            Err(SecurityError::OverwriteBlocked(
                candidate.to_string_lossy().into_owned(),
            ))
        }
        _ => Err(SecurityError::OverwriteBlocked(
            candidate.to_string_lossy().into_owned(),
        )),
    }
}


/// Pre-flight check ensuring the destination drive has enough free space before writing or extracting.
pub fn verify_destination_disk_space(target_path: &Path, required_bytes: u64) -> Result<(), SecurityError> {
    if required_bytes == 0 {
        return Ok(());
    }
    let abs_target = std::fs::canonicalize(target_path).unwrap_or_else(|_| target_path.to_path_buf());
    let target_str = abs_target.to_string_lossy().to_uppercase();

    let disks = sysinfo::Disks::new_with_refreshed_list();
    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy().to_uppercase();
        if !mount.is_empty() && target_str.starts_with(&mount) {
            let avail = disk.available_space();
            // Require a 16 MB safety reserve above the required extraction/archive size
            let needed = required_bytes.saturating_add(16 * 1024 * 1024);
            if avail < needed {
                return Err(SecurityError::DecompressionBombQuotaExceeded {
                    uncompressed: needed,
                    limit: avail,
                });
            }
            break;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_slip_traversal_blocked() {
        assert!(sanitize_relative_path("../secret.txt").is_err());
        assert!(sanitize_relative_path("folder/../../Windows/System32/cmd.exe").is_err());
        assert!(sanitize_relative_path("C:\\Windows\\win.ini").is_err());
        assert!(sanitize_relative_path("/etc/passwd").is_err());
        assert!(sanitize_relative_path("\\\\server\\share\\file.txt").is_err());
    }

    #[test]
    fn test_windows_reserved_device_names_and_ads_blocked() {
        assert!(sanitize_relative_path("CON").is_err());
        assert!(sanitize_relative_path("src/NUL.txt").is_err());
        assert!(sanitize_relative_path("LPT1.doc").is_err());
        assert!(sanitize_relative_path("COM1").is_err());
        assert!(sanitize_relative_path("report.txt:Zone.Identifier").is_err());
        assert!(sanitize_relative_path("trailing_dot./file.txt").is_err());
    }

    #[test]
    fn test_integer_overflow_metadata_blocked() {
        let res = verify_decompression_quota("overflow.bin", 100, u64::MAX, 10, DEFAULT_MAX_EXTRACTION_BYTES);
        assert!(matches!(res, Err(SecurityError::IntegerOverflowDetected(_))));
    }

    #[test]
    fn test_bounded_extraction_writer_stops_lying_header_bomb() {
        let sink: Vec<u8> = Vec::new();
        let mut guard = BoundedExtractionWriter::new(sink, 1024);
        assert!(guard.write_all(&[0u8; 512]).is_ok());
        assert!(guard.write_all(&[0u8; 512]).is_ok());
        // Writing 1 byte beyond the 1024-byte quota must immediately abort
        assert!(guard.write_all(&[0u8; 1]).is_err());
    }

    #[test]
    fn test_valid_relative_paths_allowed() {
        let p = sanitize_relative_path("src/components/Dashboard.tsx").unwrap();
        assert_eq!(p, PathBuf::from("src").join("components").join("Dashboard.tsx"));
    }

    #[test]
    fn test_decompression_bomb_detection() {
        let res = verify_decompression_quota(
            "bomb.txt",
            10 * 1024,
            50 * 1024 * 1024,
            0,
            DEFAULT_MAX_EXTRACTION_BYTES,
        );
        assert!(matches!(res, Err(SecurityError::SuspiciousCompressionRatio { .. })));
    }
}