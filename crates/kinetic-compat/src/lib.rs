use std::path::Path;
use kinetic_core::ArchiveFormat;
use kinetic_format::KINETIC_MAGIC;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_magic_and_extension_detection() {
        let p = PathBuf::from("test.unknown");
        assert_eq!(detect_format(&p, Some(&KINETIC_MAGIC)), Some(ArchiveFormat::Kinetic));
        assert_eq!(detect_format(&PathBuf::from("archive.zip"), None), Some(ArchiveFormat::Zip));
        assert_eq!(detect_format(&PathBuf::from("archive.7z"), None), Some(ArchiveFormat::SevenZ));
        assert_eq!(detect_format(&PathBuf::from("archive.rar"), None), Some(ArchiveFormat::Rar));
    }
}
