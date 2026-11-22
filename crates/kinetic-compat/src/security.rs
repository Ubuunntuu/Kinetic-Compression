use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const DEFAULT_MAX_EXTRACTION_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_SUSPICIOUS_COMPRESSION_RATIO: u64 = 1000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecurityError {
    #[error("Unsafe archive entry path blocked (Zip-Slip / traversal attempt): {0}")]
    PathTraversalBlocked(String),
    #[error("Archive entry path is empty or invalid")]
    EmptyEntryPath,
    #[error("Decompression bomb detected: uncompressed size ({uncompressed} bytes) exceeds safety limit ({limit} bytes)")]
    DecompressionBombQuotaExceeded { uncompressed: u64, limit: u64 },
    #[error("Decompression bomb ratio detected on entry '{entry}': ratio {ratio}:1 exceeds safety threshold")]
    SuspiciousCompressionRatio { entry: String, ratio: u64 },
    #[error("Target file already exists and overwrite policy is set to 'ask': {0}")]
    OverwriteBlocked(String),
}

pub fn sanitize_relative_path(raw_path: &str) -> Result<PathBuf, SecurityError> {
    if raw_path.trim().is_empty() || raw_path.contains('\0') {
        return Err(SecurityError::EmptyEntryPath);
    }

    let normalized = raw_path.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        return Err(SecurityError::PathTraversalBlocked(raw_path.to_string()));
    }

    let path = Path::new(&normalized);
    let mut clean = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(segment) => {
                let seg_str = segment.to_string_lossy();
                if seg_str == ".." || seg_str.contains(':') {
                    return Err(SecurityError::PathTraversalBlocked(raw_path.to_string()));
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

pub fn verify_decompression_quota(
    entry_name: &str,
    compressed_size: u64,
    uncompressed_size: u64,
    cumulative_uncompressed: u64,
    max_quota_bytes: u64,
) -> Result<u64, SecurityError> {
    let next_total = cumulative_uncompressed.saturating_add(uncompressed_size);
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