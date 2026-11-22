pub mod archive;
pub mod cdc;
pub mod crypto;
pub mod dictionary;
pub mod filters;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const KINETIC_MAGIC: [u8; 8] = *b"KNT1\r\n\x1a\n";
pub const KINETIC_VERSION_MAJOR: u16 = 1;
pub const KINETIC_VERSION_MINOR: u16 = 0;
pub const GLOBAL_HEADER_SIZE: usize = 64;

pub const FLAG_ENCRYPTED: u32 = 0x01;
pub const FLAG_HAS_DICTIONARY: u32 = 0x02;
pub const FLAG_HAS_PARITY: u32 = 0x04;
pub const FLAG_SOLID_MICRO_BLOCKS: u32 = 0x08;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("Invalid .kinetic magic signature")]
    InvalidMagic,
    #[error("Unsupported .kinetic format version: {0}.{1}")]
    UnsupportedVersion(u16, u16),
    #[error("Global header CRC32 checksum mismatch (expected {expected:#010x}, found {actual:#010x})")]
    HeaderChecksumMismatch { expected: u32, actual: u32 },
    #[error("Block XXH3-64 checksum mismatch (expected {expected:#018x}, found {actual:#018x})")]
    BlockChecksumMismatch { expected: u64, actual: u64 },
    #[error("Corrupted .kinetic block or index: {0}")]
    CorruptedBlock(String),
    #[error("Cryptography / authentication error: {0}")]
    CryptoError(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalHeader {
    pub version_major: u16,
    pub version_minor: u16,
    pub flags: u32,
    pub index_offset: u64,
    pub index_length: u64,
    pub total_blocks: u64,
    pub total_files: u64,
    pub dict_offset: u64,
    pub dict_length: u32,
    pub header_crc32c: u32,
}

impl GlobalHeader {
    pub fn new(flags: u32) -> Self {
        let mut header = Self {
            version_major: KINETIC_VERSION_MAJOR,
            version_minor: KINETIC_VERSION_MINOR,
            flags,
            index_offset: 0,
            index_length: 0,
            total_blocks: 0,
            total_files: 0,
            dict_offset: 0,
            dict_length: 0,
            header_crc32c: 0,
        };
        header.header_crc32c = header.compute_checksum();
        header
    }

    pub fn to_bytes(&self) -> [u8; GLOBAL_HEADER_SIZE] {
        let mut buf = [0u8; GLOBAL_HEADER_SIZE];
        buf[0..8].copy_from_slice(&KINETIC_MAGIC);
        buf[8..10].copy_from_slice(&self.version_major.to_le_bytes());
        buf[10..12].copy_from_slice(&self.version_minor.to_le_bytes());
        buf[12..16].copy_from_slice(&self.flags.to_le_bytes());
        buf[16..24].copy_from_slice(&self.index_offset.to_le_bytes());
        buf[24..32].copy_from_slice(&self.index_length.to_le_bytes());
        buf[32..40].copy_from_slice(&self.total_blocks.to_le_bytes());
        buf[40..48].copy_from_slice(&self.total_files.to_le_bytes());
        buf[48..56].copy_from_slice(&self.dict_offset.to_le_bytes());
        buf[56..60].copy_from_slice(&self.dict_length.to_le_bytes());

        let crc = crc32fast::hash(&buf[0..60]);
        buf[60..64].copy_from_slice(&crc.to_le_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8; GLOBAL_HEADER_SIZE]) -> Result<Self, FormatError> {
        if buf[0..8] != KINETIC_MAGIC {
            return Err(FormatError::InvalidMagic);
        }

        let version_major = u16::from_le_bytes([buf[8], buf[9]]);
        let version_minor = u16::from_le_bytes([buf[10], buf[11]]);
        if version_major != KINETIC_VERSION_MAJOR {
            return Err(FormatError::UnsupportedVersion(version_major, version_minor));
        }

        let expected_crc = crc32fast::hash(&buf[0..60]);
        let actual_crc = u32::from_le_bytes([buf[60], buf[61], buf[62], buf[63]]);
        if expected_crc != actual_crc {
            return Err(FormatError::HeaderChecksumMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        Ok(Self {
            version_major,
            version_minor,
            flags: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            index_offset: u64::from_le_bytes(buf[16..24].try_into().unwrap()),
            index_length: u64::from_le_bytes(buf[24..32].try_into().unwrap()),
            total_blocks: u64::from_le_bytes(buf[32..40].try_into().unwrap()),
            total_files: u64::from_le_bytes(buf[40..48].try_into().unwrap()),
            dict_offset: u64::from_le_bytes(buf[48..56].try_into().unwrap()),
            dict_length: u32::from_le_bytes([buf[56], buf[57], buf[58], buf[59]]),
            header_crc32c: actual_crc,
        })
    }

    pub fn compute_checksum(&self) -> u32 {
        let bytes = self.to_bytes();
        u32::from_le_bytes([bytes[60], bytes[61], bytes[62], bytes[63]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_header_roundtrip() {
        let mut header = GlobalHeader::new(FLAG_SOLID_MICRO_BLOCKS | FLAG_HAS_DICTIONARY);
        header.index_offset = 1048576;
        header.index_length = 4096;
        header.total_blocks = 12;
        header.total_files = 350;
        header.dict_offset = 64;
        header.dict_length = 65536;
        header.header_crc32c = header.compute_checksum();

        let encoded = header.to_bytes();
        let decoded = GlobalHeader::from_bytes(&encoded).expect("Header should decode cleanly");
        assert_eq!(header, decoded);
    }

    #[test]
    fn test_corrupted_header_detected() {
        let header = GlobalHeader::new(0);
        let mut encoded = header.to_bytes();
        encoded[20] ^= 0xFF;
        let err = GlobalHeader::from_bytes(&encoded).unwrap_err();
        assert!(matches!(err, FormatError::HeaderChecksumMismatch { .. }));
    }
}