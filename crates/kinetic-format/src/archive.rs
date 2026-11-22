use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use kinetic_core::classifier::classify_content;
use kinetic_core::pipeline::BlockCodec;
use kinetic_core::CompressionMode;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use xxhash_rust::xxh3::xxh3_64;

use crate::cdc::chunk_slice_fastcdc;
use crate::crypto::{
    decrypt_block_aes256_gcm, derive_key_argon2id, encrypt_block_aes256_gcm,
    generate_salt_and_nonce, NONCE_LEN, SALT_LEN,
};
use crate::dictionary::{
    apply_dict_transform, reverse_dict_transform, train_seed_dictionary, FILTER_SHARED_DICT,
};
use crate::filters::{apply_filter, reverse_filter, select_optimal_filter};
use crate::{
    FormatError, GlobalHeader, FLAG_ENCRYPTED, FLAG_HAS_DICTIONARY, FLAG_HAS_PARITY,
    FLAG_SOLID_MICRO_BLOCKS, GLOBAL_HEADER_SIZE,
};

pub const BLOCK_HEADER_SIZE: usize = 32;
pub const CODEC_STORE: u8 = 0x00;
pub const CODEC_LZ4: u8 = 0x01;
pub const CODEC_DEFLATE_HIGH: u8 = 0x02;
pub const CODEC_BROTLI: u8 = 0x04;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockFrameHeader {
    pub block_id: u32,
    pub codec_id: u8,
    pub filter_mask: u8,
    pub reserved: u16,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub block_xxh3_64: u64,
}

impl BlockFrameHeader {
    pub fn to_bytes(&self) -> [u8; BLOCK_HEADER_SIZE] {
        let mut buf = [0u8; BLOCK_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.block_id.to_le_bytes());
        buf[4] = self.codec_id;
        buf[5] = self.filter_mask;
        buf[6..8].copy_from_slice(&self.reserved.to_le_bytes());
        buf[8..16].copy_from_slice(&self.compressed_size.to_le_bytes());
        buf[16..24].copy_from_slice(&self.uncompressed_size.to_le_bytes());
        buf[24..32].copy_from_slice(&self.block_xxh3_64.to_le_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8; BLOCK_HEADER_SIZE]) -> Self {
        Self {
            block_id: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            codec_id: buf[4],
            filter_mask: buf[5],
            reserved: u16::from_le_bytes([buf[6], buf[7]]),
            compressed_size: u64::from_le_bytes(buf[8..16].try_into().unwrap()),
            uncompressed_size: u64::from_le_bytes(buf[16..24].try_into().unwrap()),
            block_xxh3_64: u64::from_le_bytes(buf[24..32].try_into().unwrap()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDescriptor {
    pub block_id: u32,
    pub file_offset: u64,
    pub codec_id: u8,
    pub filter_mask: u8,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub block_xxh3_64: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KineticFileEntry {
    pub path: String,
    pub is_dir: bool,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub modified_unix_secs: i64,
    pub blake3_hex: String,
    pub codec_summary: String,
    pub block_ids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CentralDirectoryIndex {
    pub files: Vec<KineticFileEntry>,
    pub blocks: Vec<BlockDescriptor>,
    pub deduplicated_chunks_saved: usize,
    pub deduplicated_bytes_saved: u64,
    pub parity_offset: u64,
    pub parity_length: u64,
    pub archive_blake3_hex: String,
}

#[derive(Debug, Clone)]
pub struct KineticWriteOptions {
    pub mode: CompressionMode,
    pub password: Option<String>,
    pub enable_fastcdc_dedup: bool,
    pub enable_entropy_bypass: bool,
    pub enable_kinetic_shield_parity: bool,
}

impl Default for KineticWriteOptions {
    fn default() -> Self {
        Self {
            mode: CompressionMode::Balanced,
            password: None,
            enable_fastcdc_dedup: true,
            enable_entropy_bypass: true,
            enable_kinetic_shield_parity: true,
        }
    }
}

fn codec_name(codec_id: u8, filter_mask: u8) -> String {
    let base = match codec_id {
        CODEC_STORE => "Store",
        CODEC_LZ4 => "LZ4",
        CODEC_DEFLATE_HIGH => "Zlib-High",
        CODEC_BROTLI => "Brotli-Ultra",
        _ => "Kinetic",
    };
    if filter_mask != 0 {
        format!("{}+Filter({:#04x})", base, filter_mask)
    } else {
        base.to_string()
    }
}

fn compress_chunk_payload(
    raw_chunk: &[u8],
    file_path: &str,
    mode: CompressionMode,
    enable_entropy_bypass: bool,
    seed_dict: &[u8],
) -> Result<(u8, u8, Vec<u8>), FormatError> {
    let profile = classify_content(file_path, raw_chunk);
    let entropy = profile.shannon_entropy;

    if mode == CompressionMode::Store || (enable_entropy_bypass && entropy > 7.85) {
        return Ok((CODEC_STORE, 0, raw_chunk.to_vec()));
    }

    let base_filter = select_optimal_filter(file_path, raw_chunk);
    let mut working = apply_filter(base_filter, raw_chunk);
    let mut filter_mask = base_filter;

    if !seed_dict.is_empty() && base_filter == 0 {
        let dict_preconditioned = apply_dict_transform(&working, seed_dict);
        if dict_preconditioned.len() + 8 < working.len() {
            working = dict_preconditioned;
            filter_mask |= FILTER_SHARED_DICT;
        }
    }

    let target_codec = match mode {
        CompressionMode::Store => CODEC_STORE,
        CompressionMode::Fast => CODEC_LZ4,
        CompressionMode::Balanced => {
            if entropy <= 5.0 {
                CODEC_DEFLATE_HIGH
            } else {
                CODEC_LZ4
            }
        }
        CompressionMode::Maximum => CODEC_DEFLATE_HIGH,
        CompressionMode::Ultra => CODEC_BROTLI,
        CompressionMode::Automatic => match profile.recommended_codec {
            BlockCodec::Store => CODEC_STORE,
            BlockCodec::Lz4 => CODEC_LZ4,
            BlockCodec::DeflateHigh => CODEC_DEFLATE_HIGH,
            BlockCodec::BrotliUltra => CODEC_BROTLI,
        },
    };

    match target_codec {
        CODEC_STORE => Ok((CODEC_STORE, 0, raw_chunk.to_vec())),
        CODEC_LZ4 => {
            let comp = lz4_flex::compress_prepend_size(&working);
            if comp.len() < raw_chunk.len() {
                Ok((CODEC_LZ4, filter_mask, comp))
            } else {
                Ok((CODEC_STORE, 0, raw_chunk.to_vec()))
            }
        }
        CODEC_DEFLATE_HIGH => {
            let level = if mode == CompressionMode::Maximum {
                Compression::best()
            } else {
                Compression::new(6)
            };
            let mut enc = ZlibEncoder::new(Vec::with_capacity(working.len() / 2), level);
            enc.write_all(&working)?;
            let comp = enc.finish()?;
            if comp.len() < raw_chunk.len() {
                Ok((CODEC_DEFLATE_HIGH, filter_mask, comp))
            } else {
                Ok((CODEC_STORE, 0, raw_chunk.to_vec()))
            }
        }
        CODEC_BROTLI => {
            let mut comp = Vec::with_capacity(working.len() / 2);
            {
                let mut writer = brotli::CompressorWriter::new(&mut comp, 4096, 9, 20);
                writer.write_all(&working)?;
                writer.flush()?;
            }
            if comp.len() < raw_chunk.len() {
                Ok((CODEC_BROTLI, filter_mask, comp))
            } else {
                Ok((CODEC_STORE, 0, raw_chunk.to_vec()))
            }
        }
        _ => Ok((CODEC_STORE, 0, raw_chunk.to_vec())),
    }
}

pub fn decode_block_payload(
    codec_id: u8,
    filter_mask: u8,
    uncompressed_size: u64,
    expected_xxh3: u64,
    payload: &[u8],
    seed_dict: &[u8],
) -> Result<Vec<u8>, FormatError> {
    let mut working = match codec_id {
        CODEC_STORE => payload.to_vec(),
        CODEC_LZ4 => lz4_flex::decompress_size_prepended(payload)
            .map_err(|e| FormatError::CorruptedBlock(format!("LZ4 decode failed: {}", e)))?,
        CODEC_DEFLATE_HIGH => {
            let mut dec = ZlibDecoder::new(payload);
            let mut out = Vec::with_capacity(uncompressed_size as usize);
            dec.read_to_end(&mut out)?;
            out
        }
        CODEC_BROTLI => {
            let mut dec = brotli::Decompressor::new(payload, 4096);
            let mut out = Vec::with_capacity(uncompressed_size as usize);
            dec.read_to_end(&mut out)?;
            out
        }
        other => {
            return Err(FormatError::CorruptedBlock(format!(
                "Unknown block codec ID: {:#04x}",
                other
            )))
        }
    };

    if (filter_mask & FILTER_SHARED_DICT) != 0 {
        working = reverse_dict_transform(&working, seed_dict)?;
    }

    let base_filter = filter_mask & !FILTER_SHARED_DICT;
    let restored = reverse_filter(base_filter, &working);
    let actual_xxh3 = xxh3_64(&restored);
    if actual_xxh3 != expected_xxh3 {
        return Err(FormatError::BlockChecksumMismatch {
            expected: expected_xxh3,
            actual: actual_xxh3,
        });
    }
    Ok(restored)
}

pub fn create_kinetic_archive(
    source_paths: &[PathBuf],
    output_path: &Path,
    options: &KineticWriteOptions,
) -> Result<u64, FormatError> {
    if source_paths.is_empty() {
        return Err(FormatError::CorruptedBlock(
            "At least one input file or directory is required.".to_string(),
        ));
    }

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let staging_path = output_path.with_extension("kinetic.part");
    let result = (|| -> Result<u64, FormatError> {
        let mut out = File::create(&staging_path)?;

        let mut flags = FLAG_SOLID_MICRO_BLOCKS;
        let crypto_ctx = if let Some(ref pwd) = options.password {
            if !pwd.is_empty() {
                flags |= FLAG_ENCRYPTED;
                let (salt, nonce) =
                    generate_salt_and_nonce(output_path.to_string_lossy().as_bytes());
                let key = derive_key_argon2id(pwd, &salt)?;
                Some((salt, nonce, key))
            } else {
                None
            }
        } else {
            None
        };

        if options.enable_kinetic_shield_parity {
            flags |= FLAG_HAS_PARITY;
        }

        let placeholder_header = GlobalHeader::new(flags);
        out.write_all(&placeholder_header.to_bytes())?;

        if let Some((ref salt, ref nonce, _)) = crypto_ctx {
            out.write_all(salt)?;
            out.write_all(nonce)?;
        }

        let mut files_to_pack: Vec<(PathBuf, String, bool)> = Vec::new();
        for src in source_paths {
            if !src.exists() {
                return Err(FormatError::CorruptedBlock(format!(
                    "Source path does not exist: {}",
                    src.display()
                )));
            }
            if src.is_file() {
                let name = src
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .replace('\\', "/");
                files_to_pack.push((src.clone(), name, false));
            } else if src.is_dir() {
                let base_parent = src.parent().unwrap_or(src);
                for entry in WalkDir::new(src) {
                    let entry =
                        entry.map_err(|e| FormatError::CorruptedBlock(e.to_string()))?;
                    let p = entry.path();
                    let rel = p
                        .strip_prefix(base_parent)
                        .unwrap_or(p)
                        .to_string_lossy()
                        .replace('\\', "/");
                    if !rel.is_empty() {
                        files_to_pack.push((p.to_path_buf(), rel, p.is_dir()));
                    }
                }
            }
        }

        files_to_pack.sort_by(|a, b| {
            let ext_a = Path::new(&a.1).extension().and_then(|e| e.to_str()).unwrap_or("");
            let ext_b = Path::new(&b.1).extension().and_then(|e| e.to_str()).unwrap_or("");
            ext_a.cmp(ext_b).then_with(|| a.1.cmp(&b.1))
        });

        let mut dict_samples: Vec<Vec<u8>> = Vec::new();
        for (disk_path, _, is_dir) in files_to_pack.iter().take(32) {
            if !*is_dir {
                if let Ok(bytes) = fs::read(disk_path) {
                    if !bytes.is_empty() && bytes.len() <= 256 * 1024 {
                        dict_samples.push(bytes);
                    }
                }
            }
        }

        let seed_dict = if options.mode != CompressionMode::Store && !dict_samples.is_empty() {
            train_seed_dictionary(&dict_samples)
        } else {
            Vec::new()
        };

        let (dict_offset, dict_length) = if !seed_dict.is_empty() {
            flags |= FLAG_HAS_DICTIONARY;
            let d_off = out.stream_position()?;
            let stored_dict = if let Some((_, ref base_nonce, ref key)) = crypto_ctx {
                encrypt_block_aes256_gcm(key, base_nonce, u32::MAX - 1, &seed_dict)?
            } else {
                seed_dict.clone()
            };
            out.write_all(&stored_dict)?;
            (d_off, stored_dict.len() as u32)
        } else {
            (0, 0)
        };

        let mut chunk_dedup_table: HashMap<[u8; 32], u32> = HashMap::new();
        let mut block_descriptors: Vec<BlockDescriptor> = Vec::new();
        let mut file_entries: Vec<KineticFileEntry> = Vec::new();
        let mut parity_stripe: Vec<u8> = Vec::new();
        let mut deduplicated_chunks_saved = 0usize;
        let mut deduplicated_bytes_saved = 0u64;
        let mut total_input_bytes = 0u64;
        let mut archive_hasher = blake3::Hasher::new();

        for (disk_path, rel_path, is_dir) in files_to_pack {
            if is_dir {
                file_entries.push(KineticFileEntry {
                    path: rel_path,
                    is_dir: true,
                    uncompressed_size: 0,
                    compressed_size: 0,
                    modified_unix_secs: 0,
                    blake3_hex: String::new(),
                    codec_summary: "Directory".to_string(),
                    block_ids: Vec::new(),
                });
                continue;
            }

            let raw_data = fs::read(&disk_path)?;
            let file_len = raw_data.len() as u64;
            total_input_bytes += file_len;
            archive_hasher.update(&raw_data);

            let file_blake3 = blake3::hash(&raw_data).to_hex().to_string();
            let chunks = chunk_slice_fastcdc(&raw_data, options.enable_fastcdc_dedup);

            let mut file_block_ids = Vec::with_capacity(chunks.len());
            let mut file_compressed_bytes = 0u64;
            let mut last_codec_label = "Store".to_string();

            for (offset, length) in chunks {
                let slice = &raw_data[offset..offset + length];
                let chunk_hash = *blake3::hash(slice).as_bytes();

                if options.enable_fastcdc_dedup {
                    if let Some(&existing_block_id) = chunk_dedup_table.get(&chunk_hash) {
                        file_block_ids.push(existing_block_id);
                        deduplicated_chunks_saved += 1;
                        deduplicated_bytes_saved += length as u64;
                        last_codec_label = "FastCDC-Dedup".to_string();
                        continue;
                    }
                }

                let block_id = block_descriptors.len() as u32;
                let block_xxh3 = xxh3_64(slice);
                let (codec_id, filter_mask, mut payload) = compress_chunk_payload(
                    slice,
                    &rel_path,
                    options.mode,
                    options.enable_entropy_bypass,
                    &seed_dict,
                )?;

                last_codec_label = codec_name(codec_id, filter_mask);

                if let Some((_, ref base_nonce, ref key)) = crypto_ctx {
                    payload = encrypt_block_aes256_gcm(key, base_nonce, block_id, &payload)?;
                }

                if options.enable_kinetic_shield_parity {
                    if parity_stripe.len() < payload.len() {
                        parity_stripe.resize(payload.len(), 0u8);
                    }
                    for (p, &b) in parity_stripe.iter_mut().zip(payload.iter()) {
                        *p ^= b;
                    }
                }

                let file_offset = out.stream_position()?;
                let frame_header = BlockFrameHeader {
                    block_id,
                    codec_id,
                    filter_mask,
                    reserved: 0,
                    compressed_size: payload.len() as u64,
                    uncompressed_size: length as u64,
                    block_xxh3_64: block_xxh3,
                };

                out.write_all(&frame_header.to_bytes())?;
                out.write_all(&payload)?;

                file_compressed_bytes += payload.len() as u64;
                file_block_ids.push(block_id);
                chunk_dedup_table.insert(chunk_hash, block_id);

                block_descriptors.push(BlockDescriptor {
                    block_id,
                    file_offset,
                    codec_id,
                    filter_mask,
                    compressed_size: payload.len() as u64,
                    uncompressed_size: length as u64,
                    block_xxh3_64: block_xxh3,
                });
            }

            file_entries.push(KineticFileEntry {
                path: rel_path,
                is_dir: false,
                uncompressed_size: file_len,
                compressed_size: file_compressed_bytes,
                modified_unix_secs: 0,
                blake3_hex: file_blake3,
                codec_summary: last_codec_label,
                block_ids: file_block_ids,
            });
        }

        let (parity_offset, parity_length) =
            if options.enable_kinetic_shield_parity && !parity_stripe.is_empty() {
                let p_off = out.stream_position()?;
                let p_len = parity_stripe.len() as u64;
                out.write_all(&parity_stripe)?;
                (p_off, p_len)
            } else {
                (0, 0)
            };

        let index = CentralDirectoryIndex {
            files: file_entries,
            blocks: block_descriptors,
            deduplicated_chunks_saved,
            deduplicated_bytes_saved,
            parity_offset,
            parity_length,
            archive_blake3_hex: archive_hasher.finalize().to_hex().to_string(),
        };

        let index_json = serde_json::to_vec(&index)
            .map_err(|e| FormatError::CorruptedBlock(e.to_string()))?;
        let mut index_payload = lz4_flex::compress_prepend_size(&index_json);

        if let Some((_, ref base_nonce, ref key)) = crypto_ctx {
            index_payload =
                encrypt_block_aes256_gcm(key, base_nonce, u32::MAX, &index_payload)?;
        }

        let index_offset = out.stream_position()?;
        let index_length = index_payload.len() as u64;
        out.write_all(&index_payload)?;

        let mut final_header = GlobalHeader::new(flags);
        final_header.index_offset = index_offset;
        final_header.index_length = index_length;
        final_header.total_blocks = index.blocks.len() as u64;
        final_header.total_files = index.files.len() as u64;
        final_header.dict_offset = dict_offset;
        final_header.dict_length = dict_length;
        final_header.header_crc32c = final_header.compute_checksum();

        out.seek(SeekFrom::Start(0))?;
        out.write_all(&final_header.to_bytes())?;
        out.flush()?;

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
        Err(e) => {
            let _ = fs::remove_file(&staging_path);
            Err(e)
        }
    }
}

pub fn load_seed_dictionary(
    file: &mut File,
    header: &GlobalHeader,
    crypto_key: Option<&([u8; NONCE_LEN], [u8; 32])>,
) -> Result<Vec<u8>, FormatError> {
    if (header.flags & FLAG_HAS_DICTIONARY) == 0 || header.dict_offset == 0 || header.dict_length == 0 {
        return Ok(Vec::new());
    }
    file.seek(SeekFrom::Start(header.dict_offset))?;
    let mut buf = vec![0u8; header.dict_length as usize];
    file.read_exact(&mut buf)?;

    if let Some((nonce, key)) = crypto_key {
        buf = decrypt_block_aes256_gcm(key, nonce, u32::MAX - 1, &buf)?;
    }
    Ok(buf)
}

pub fn read_kinetic_index(
    archive_path: &Path,
    password: Option<&str>,
) -> Result<(GlobalHeader, CentralDirectoryIndex, Option<([u8; NONCE_LEN], [u8; 32])>), FormatError>
{
    let mut f = File::open(archive_path)?;
    let mut header_buf = [0u8; GLOBAL_HEADER_SIZE];
    f.read_exact(&mut header_buf)?;
    let header = GlobalHeader::from_bytes(&header_buf)?;

    let crypto_key = if (header.flags & FLAG_ENCRYPTED) != 0 {
        let pwd = password.filter(|p| !p.is_empty()).ok_or_else(|| {
            FormatError::CryptoError(
                "Archive is encrypted with AES-256-GCM; a password is required.".to_string(),
            )
        })?;
        let mut salt = [0u8; SALT_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        f.read_exact(&mut salt)?;
        f.read_exact(&mut nonce)?;
        let key = derive_key_argon2id(pwd, &salt)?;
        Some((nonce, key))
    } else {
        None
    };

    if header.index_offset == 0 || header.index_length == 0 {
        return Ok((
            header,
            CentralDirectoryIndex {
                files: Vec::new(),
                blocks: Vec::new(),
                deduplicated_chunks_saved: 0,
                deduplicated_bytes_saved: 0,
                parity_offset: 0,
                parity_length: 0,
                archive_blake3_hex: String::new(),
            },
            crypto_key,
        ));
    }

    f.seek(SeekFrom::Start(header.index_offset))?;
    let mut index_buf = vec![0u8; header.index_length as usize];
    f.read_exact(&mut index_buf)?;

    if let Some((ref nonce, ref key)) = crypto_key {
        index_buf = decrypt_block_aes256_gcm(key, nonce, u32::MAX, &index_buf)?;
    }

    let decompressed_json = lz4_flex::decompress_size_prepended(&index_buf)
        .map_err(|e| FormatError::CorruptedBlock(format!("Index LZ4 decode error: {}", e)))?;

    let index: CentralDirectoryIndex = serde_json::from_slice(&decompressed_json)
        .map_err(|e| FormatError::CorruptedBlock(format!("Index JSON parse error: {}", e)))?;

    Ok((header, index, crypto_key))
}

pub fn read_and_verify_block(
    file: &mut File,
    desc: &BlockDescriptor,
    all_blocks: &[BlockDescriptor],
    parity_offset: u64,
    parity_length: u64,
    crypto_key: Option<&([u8; NONCE_LEN], [u8; 32])>,
) -> Result<Vec<u8>, FormatError> {
    let mut header_buf = [0u8; GLOBAL_HEADER_SIZE];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header_buf)?;
    let header = GlobalHeader::from_bytes(&header_buf)?;
    let seed_dict = load_seed_dictionary(file, &header, crypto_key)?;

    file.seek(SeekFrom::Start(desc.file_offset))?;
    let mut frame_buf = [0u8; BLOCK_HEADER_SIZE];
    file.read_exact(&mut frame_buf)?;
    let _frame = BlockFrameHeader::from_bytes(&frame_buf);

    let mut raw_payload = vec![0u8; desc.compressed_size as usize];
    file.read_exact(&mut raw_payload)?;

    let attempt = (|| -> Result<Vec<u8>, FormatError> {
        let decrypted = if let Some((nonce, key)) = crypto_key {
            decrypt_block_aes256_gcm(key, nonce, desc.block_id, &raw_payload)?
        } else {
            raw_payload.clone()
        };
        decode_block_payload(
            desc.codec_id,
            desc.filter_mask,
            desc.uncompressed_size,
            desc.block_xxh3_64,
            &decrypted,
            &seed_dict,
        )
    })();

    if attempt.is_ok() {
        return attempt;
    }

    if parity_offset > 0 && parity_length >= desc.compressed_size {
        file.seek(SeekFrom::Start(parity_offset))?;
        let mut recovered = vec![0u8; parity_length as usize];
        file.read_exact(&mut recovered)?;

        for other in all_blocks {
            if other.block_id == desc.block_id {
                continue;
            }
            file.seek(SeekFrom::Start(
                other.file_offset + (BLOCK_HEADER_SIZE as u64),
            ))?;
            let mut other_payload = vec![0u8; other.compressed_size as usize];
            file.read_exact(&mut other_payload)?;
            for (r, &b) in recovered.iter_mut().zip(other_payload.iter()) {
                *r ^= b;
            }
        }

        recovered.truncate(desc.compressed_size as usize);
        let decrypted = if let Some((nonce, key)) = crypto_key {
            decrypt_block_aes256_gcm(key, nonce, desc.block_id, &recovered)?
        } else {
            recovered
        };
        return decode_block_payload(
            desc.codec_id,
            desc.filter_mask,
            desc.uncompressed_size,
            desc.block_xxh3_64,
            &decrypted,
            &seed_dict,
        );
    }

    attempt
}

pub fn validate_kinetic_archive(
    archive_path: &Path,
    password: Option<&str>,
) -> Result<(usize, u64, String), FormatError> {
    let (_header, index, crypto_key) = read_kinetic_index(archive_path, password)?;
    let mut f = File::open(archive_path)?;

    let mut decoded_blocks: HashMap<u32, Vec<u8>> = HashMap::with_capacity(index.blocks.len());
    for desc in &index.blocks {
        let raw = read_and_verify_block(
            &mut f,
            desc,
            &index.blocks,
            index.parity_offset,
            index.parity_length,
            crypto_key.as_ref(),
        )?;
        decoded_blocks.insert(desc.block_id, raw);
    }

    let mut verified_files = 0usize;
    let mut total_uncompressed = 0u64;

    for entry in &index.files {
        if entry.is_dir {
            continue;
        }
        let mut file_hasher = blake3::Hasher::new();
        for &bid in &entry.block_ids {
            let chunk = decoded_blocks.get(&bid).ok_or_else(|| {
                FormatError::CorruptedBlock(format!("Missing block ID {} in archive", bid))
            })?;
            file_hasher.update(chunk);
            total_uncompressed += chunk.len() as u64;
        }
        let actual_hex = file_hasher.finalize().to_hex().to_string();
        if actual_hex != entry.blake3_hex {
            return Err(FormatError::CorruptedBlock(format!(
                "BLAKE3 mismatch on file '{}': expected {}, found {}",
                entry.path, entry.blake3_hex, actual_hex
            )));
        }
        verified_files += 1;
    }

    let msg = format!(
        "Verified {} .kinetic files ({} unique blocks, {} duplicate chunks saved via FastCDC) with XXH3-64 & BLAKE3 integrity.",
        verified_files,
        index.blocks.len(),
        index.deduplicated_chunks_saved
    );
    Ok((verified_files, total_uncompressed, msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_kinetic_archive_roundtrip_and_fastcdc_deduplication() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("alpha.txt");
        let file2 = dir.path().join("beta_duplicate.txt");
        let payload = b"Kinetic Phase 6 Native Format + FastCDC Deduplication Payload! ".repeat(2048);
        fs::write(&file1, &payload).unwrap();
        fs::write(&file2, &payload).unwrap();

        let archive_path = dir.path().join("bundle.kinetic");
        let opts = KineticWriteOptions {
            mode: CompressionMode::Ultra,
            ..Default::default()
        };

        let total_in =
            create_kinetic_archive(&[file1, file2], &archive_path, &opts).unwrap();
        assert_eq!(total_in, (payload.len() * 2) as u64);

        let (hdr, index, _) = read_kinetic_index(&archive_path, None).unwrap();
        assert_eq!(index.files.len(), 2);
        assert!(index.deduplicated_chunks_saved >= 1);
        assert!((hdr.flags & FLAG_HAS_DICTIONARY) != 0);

        let (verified_count, verified_bytes, _) =
            validate_kinetic_archive(&archive_path, None).unwrap();
        assert_eq!(verified_count, 2);
        assert_eq!(verified_bytes, (payload.len() * 2) as u64);
    }

    #[test]
    fn test_kinetic_encrypted_archive_aes256_gcm() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("secret.bin");
        let payload = b"Encrypted AES-256-GCM + Argon2id native .kinetic test data".repeat(512);
        fs::write(&src, &payload).unwrap();

        let archive_path = dir.path().join("encrypted.kinetic");
        let opts = KineticWriteOptions {
            mode: CompressionMode::Automatic,
            password: Some("StrongVaultKey#2026".to_string()),
            ..Default::default()
        };

        create_kinetic_archive(&[src], &archive_path, &opts).unwrap();

        assert!(read_kinetic_index(&archive_path, None).is_err());
        assert!(read_kinetic_index(&archive_path, Some("WrongKey")).is_err());

        let (files, bytes, _) =
            validate_kinetic_archive(&archive_path, Some("StrongVaultKey#2026")).unwrap();
        assert_eq!(files, 1);
        assert_eq!(bytes, payload.len() as u64);
    }

    #[test]
    fn test_kinetic_shield_parity_self_healing_recovery() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("part1.txt");
        let f2 = dir.path().join("part2.txt");
        fs::write(&f1, b"First independent block content for parity test".repeat(300)).unwrap();
        fs::write(&f2, b"Second independent block content for parity test".repeat(300)).unwrap();

        let archive_path = dir.path().join("shield.kinetic");
        let opts = KineticWriteOptions {
            enable_kinetic_shield_parity: true,
            ..Default::default()
        };
        create_kinetic_archive(&[f1, f2], &archive_path, &opts).unwrap();

        let (_hdr, index, _) = read_kinetic_index(&archive_path, None).unwrap();
        assert_eq!(index.blocks.len(), 2);

        let corrupt_offset = index.blocks[0].file_offset + (BLOCK_HEADER_SIZE as u64) + 4;
        let mut raw_archive = fs::read(&archive_path).unwrap();
        raw_archive[corrupt_offset as usize] ^= 0xFF;
        fs::write(&archive_path, &raw_archive).unwrap();

        let (verified_files, _, _) = validate_kinetic_archive(&archive_path, None).unwrap();
        assert_eq!(verified_files, 2);
    }
}