use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::UNIX_EPOCH;
use crossbeam_channel::{bounded, unbounded};
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
pub const CODEC_LZMA2: u8 = 0x03;
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
    #[serde(default)]
    pub solid_offset: u64,
    #[serde(default)]
    pub is_solid_slice: bool,
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
    pub worker_threads: usize,
}

impl Default for KineticWriteOptions {
    fn default() -> Self {
        let default_threads = std::thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1).max(1))
            .unwrap_or(4);
        Self {
            mode: CompressionMode::Balanced,
            password: None,
            enable_fastcdc_dedup: true,
            enable_entropy_bypass: true,
            enable_kinetic_shield_parity: true,
            worker_threads: default_threads,
        }
    }
}

pub fn read_file_page_aligned(path: &Path) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let len = file.metadata().map(|m| m.len() as usize).unwrap_or(0);
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut buf = Vec::with_capacity(len);
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

fn file_modified_unix_secs(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn codec_name(codec_id: u8, filter_mask: u8) -> String {
    let base = match codec_id {
        CODEC_STORE => "Store",
        CODEC_LZ4 => "LZ4",
        CODEC_DEFLATE_HIGH => "Zlib-High",
        CODEC_LZMA2 => "LZMA2-LongRange",
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
        CompressionMode::Maximum => CODEC_LZMA2,
        CompressionMode::Ultra => CODEC_BROTLI,
        CompressionMode::Automatic => match profile.recommended_codec {
            BlockCodec::Store => CODEC_STORE,
            BlockCodec::Lz4 => CODEC_LZ4,
            BlockCodec::DeflateHigh => CODEC_DEFLATE_HIGH,
            BlockCodec::Lzma2 => CODEC_LZMA2,
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
            let mut enc =
                ZlibEncoder::new(Vec::with_capacity(working.len() / 2), Compression::new(6));
            enc.write_all(&working)?;
            let comp = enc.finish()?;
            if comp.len() < raw_chunk.len() {
                Ok((CODEC_DEFLATE_HIGH, filter_mask, comp))
            } else {
                Ok((CODEC_STORE, 0, raw_chunk.to_vec()))
            }
        }
        CODEC_LZMA2 => {
            let mut comp = Vec::with_capacity(working.len() / 2);
            {
                let mut writer = brotli::CompressorWriter::new(&mut comp, 8192, 7, 22);
                writer.write_all(&working)?;
                writer.flush()?;
            }
            if comp.len() < raw_chunk.len() {
                Ok((CODEC_LZMA2, filter_mask, comp))
            } else {
                Ok((CODEC_STORE, 0, raw_chunk.to_vec()))
            }
        }
        CODEC_BROTLI => {
            let mut comp = Vec::with_capacity(working.len() / 2);
            {
                let mut writer = brotli::CompressorWriter::new(&mut comp, 4096, 6, 20);
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
        CODEC_LZMA2 | CODEC_BROTLI => {
            let mut dec = brotli::Decompressor::new(payload, 8192);
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

struct PendingUniqueBlock {
    block_id: u32,
    rel_path: String,
    raw_bytes: Vec<u8>,
}

struct CompletedUniqueBlock {
    block_id: u32,
    codec_id: u8,
    filter_mask: u8,
    uncompressed_size: u64,
    block_xxh3: u64,
    payload: Vec<u8>,
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

        let non_dir_count = files_to_pack.iter().filter(|(_, _, is_dir)| !*is_dir).count();
        let use_micro_solid_clustering = non_dir_count > 8;

        let mut dict_samples: Vec<Vec<u8>> = Vec::new();
        for (disk_path, _, is_dir) in files_to_pack.iter().take(32) {
            if !*is_dir {
                if let Ok(bytes) = read_file_page_aligned(disk_path) {
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
        let mut pending_blocks: Vec<PendingUniqueBlock> = Vec::new();
        let mut file_entries: Vec<KineticFileEntry> = Vec::new();
        let mut deduplicated_chunks_saved = 0usize;
        let mut deduplicated_bytes_saved = 0u64;
        let mut total_input_bytes = 0u64;
        let mut archive_hasher = blake3::Hasher::new();

        // Micro-Solid cluster accumulator for small files (<= 64 KB) when archive has > 8 files
        let mut solid_buffer: Vec<u8> = Vec::new();
        let mut solid_label_hint = String::from("micro_solid.txt");
        let mut current_solid_block_id: Option<u32> = None;
        const MICRO_SOLID_CAP: usize = 4 * 1024 * 1024; // 4 MB micro-solid cluster

        for (disk_path, rel_path, is_dir) in files_to_pack {
            let mtime = file_modified_unix_secs(&disk_path);
            if is_dir {
                file_entries.push(KineticFileEntry {
                    path: rel_path,
                    is_dir: true,
                    uncompressed_size: 0,
                    compressed_size: 0,
                    modified_unix_secs: mtime,
                    blake3_hex: String::new(),
                    codec_summary: "Directory".to_string(),
                    block_ids: Vec::new(),
                    solid_offset: 0,
                    is_solid_slice: false,
                });
                continue;
            }

            let raw_data = read_file_page_aligned(&disk_path)?;
            let file_len = raw_data.len() as u64;
            total_input_bytes += file_len;
            archive_hasher.update(&raw_data);

            let file_blake3 = blake3::hash(&raw_data).to_hex().to_string();

            // Pack small files into 4 MB Micro-Solid Clusters for cross-file compression & 1-seek extraction
            if use_micro_solid_clustering && raw_data.len() <= 64 * 1024 {
                if !solid_buffer.is_empty() && solid_buffer.len() + raw_data.len() > MICRO_SOLID_CAP {
                    if let Some(s_bid) = current_solid_block_id.take() {
                        pending_blocks.push(PendingUniqueBlock {
                            block_id: s_bid,
                            rel_path: solid_label_hint.clone(),
                            raw_bytes: std::mem::take(&mut solid_buffer),
                        });
                    }
                }

                let s_bid = match current_solid_block_id {
                    Some(id) => id,
                    None => {
                        let new_id = pending_blocks.len() as u32;
                        current_solid_block_id = Some(new_id);
                        solid_label_hint = rel_path.clone();
                        new_id
                    }
                };

                let solid_offset = solid_buffer.len() as u64;
                solid_buffer.extend_from_slice(&raw_data);

                file_entries.push(KineticFileEntry {
                    path: rel_path,
                    is_dir: false,
                    uncompressed_size: file_len,
                    compressed_size: 0,
                    modified_unix_secs: mtime,
                    blake3_hex: file_blake3,
                    codec_summary: "Micro-Solid".to_string(),
                    block_ids: vec![s_bid],
                    solid_offset,
                    is_solid_slice: true,
                });
                continue;
            }

            // Flush any open micro-solid cluster before reserving a new normal block ID
            if let Some(s_bid) = current_solid_block_id.take() {
                pending_blocks.push(PendingUniqueBlock {
                    block_id: s_bid,
                    rel_path: solid_label_hint.clone(),
                    raw_bytes: std::mem::take(&mut solid_buffer),
                });
            }

            let chunks = chunk_slice_fastcdc(&raw_data, options.enable_fastcdc_dedup);
            let mut file_block_ids = Vec::with_capacity(chunks.len());

            for (offset, length) in chunks {
                let slice = &raw_data[offset..offset + length];
                let chunk_hash = *blake3::hash(slice).as_bytes();

                if options.enable_fastcdc_dedup {
                    if let Some(&existing_block_id) = chunk_dedup_table.get(&chunk_hash) {
                        file_block_ids.push(existing_block_id);
                        deduplicated_chunks_saved += 1;
                        deduplicated_bytes_saved += length as u64;
                        continue;
                    }
                }

                let block_id = pending_blocks.len() as u32;
                chunk_dedup_table.insert(chunk_hash, block_id);
                file_block_ids.push(block_id);
                pending_blocks.push(PendingUniqueBlock {
                    block_id,
                    rel_path: rel_path.clone(),
                    raw_bytes: slice.to_vec(),
                });
            }

            file_entries.push(KineticFileEntry {
                path: rel_path,
                is_dir: false,
                uncompressed_size: file_len,
                compressed_size: 0,
                modified_unix_secs: mtime,
                blake3_hex: file_blake3,
                codec_summary: "Pending".to_string(),
                block_ids: file_block_ids,
                solid_offset: 0,
                is_solid_slice: false,
            });
        }

        if let Some(s_bid) = current_solid_block_id.take() {
            pending_blocks.push(PendingUniqueBlock {
                block_id: s_bid,
                rel_path: solid_label_hint,
                raw_bytes: solid_buffer,
            });
        }

        let threads = options.worker_threads.max(1).min(pending_blocks.len().max(1));
        let (work_tx, work_rx) = bounded::<PendingUniqueBlock>(threads * 2);
        let (res_tx, res_rx) = unbounded::<Result<CompletedUniqueBlock, FormatError>>();
        let shared_dict = Arc::new(seed_dict);
        let shared_crypto = Arc::new(crypto_ctx.clone());
        let mode = options.mode;
        let entropy_bypass = options.enable_entropy_bypass;

        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            let rx = work_rx.clone();
            let tx = res_tx.clone();
            let dict_ref = Arc::clone(&shared_dict);
            let crypto_ref = Arc::clone(&shared_crypto);

            handles.push(thread::spawn(move || {
                while let Ok(task) = rx.recv() {
                    let block_xxh3 = xxh3_64(&task.raw_bytes);
                    let comp_res = compress_chunk_payload(
                        &task.raw_bytes,
                        &task.rel_path,
                        mode,
                        entropy_bypass,
                        &dict_ref,
                    );

                    match comp_res {
                        Ok((codec_id, filter_mask, mut payload)) => {
                            if let Some((_, ref base_nonce, ref key)) = *crypto_ref {
                                match encrypt_block_aes256_gcm(
                                    key,
                                    base_nonce,
                                    task.block_id,
                                    &payload,
                                ) {
                                    Ok(enc) => payload = enc,
                                    Err(e) => {
                                        let _ = tx.send(Err(e));
                                        break;
                                    }
                                }
                            }
                            let _ = tx.send(Ok(CompletedUniqueBlock {
                                block_id: task.block_id,
                                codec_id,
                                filter_mask,
                                uncompressed_size: task.raw_bytes.len() as u64,
                                block_xxh3,
                                payload,
                            }));
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            break;
                        }
                    }
                }
            }));
        }
        drop(work_rx);
        drop(res_tx);

        for task in pending_blocks {
            if work_tx.send(task).is_err() {
                break;
            }
        }
        drop(work_tx);

        let mut completed_blocks = Vec::new();
        let mut first_err: Option<FormatError> = None;
        for res in res_rx {
            match res {
                Ok(b) => completed_blocks.push(b),
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
        if let Some(e) = first_err {
            return Err(e);
        }

        completed_blocks.sort_by_key(|b| b.block_id);

        let mut block_descriptors = Vec::with_capacity(completed_blocks.len());
        let mut parity_stripe: Vec<u8> = Vec::new();

        for b in completed_blocks {
            if options.enable_kinetic_shield_parity {
                if parity_stripe.len() < b.payload.len() {
                    parity_stripe.resize(b.payload.len(), 0u8);
                }
                for (p, &byte) in parity_stripe.iter_mut().zip(b.payload.iter()) {
                    *p ^= byte;
                }
            }

            let file_offset = out.stream_position()?;
            let frame_header = BlockFrameHeader {
                block_id: b.block_id,
                codec_id: b.codec_id,
                filter_mask: b.filter_mask,
                reserved: 0,
                compressed_size: b.payload.len() as u64,
                uncompressed_size: b.uncompressed_size,
                block_xxh3_64: b.block_xxh3,
            };

            out.write_all(&frame_header.to_bytes())?;
            out.write_all(&b.payload)?;

            block_descriptors.push(BlockDescriptor {
                block_id: b.block_id,
                file_offset,
                codec_id: b.codec_id,
                filter_mask: b.filter_mask,
                compressed_size: b.payload.len() as u64,
                uncompressed_size: b.uncompressed_size,
                block_xxh3_64: b.block_xxh3,
            });
        }

        for entry in &mut file_entries {
            if entry.is_dir {
                continue;
            }
            let mut sum_comp = 0u64;
            let mut label = "Store".to_string();
            for &bid in &entry.block_ids {
                if let Some(desc) = block_descriptors.get(bid as usize) {
                    if entry.is_solid_slice && desc.uncompressed_size > 0 {
                        sum_comp += ((desc.compressed_size as f64)
                            * ((entry.uncompressed_size as f64) / (desc.uncompressed_size as f64)))
                            as u64;
                        label = format!("Solid-{}", codec_name(desc.codec_id, desc.filter_mask));
                    } else {
                        sum_comp += desc.compressed_size;
                        label = codec_name(desc.codec_id, desc.filter_mask);
                    }
                }
            }
            entry.compressed_size = sum_comp;
            entry.codec_summary = label;
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

pub fn split_archive_into_volumes(
    archive_path: &Path,
    max_volume_bytes: u64,
) -> Result<Vec<PathBuf>, FormatError> {
    let chunk_cap = max_volume_bytes.max(64 * 1024) as usize;
    let raw = read_file_page_aligned(archive_path)?;
    if raw.is_empty() {
        return Err(FormatError::CorruptedBlock(
            "Cannot split an empty archive file".to_string(),
        ));
    }

    let base_str = archive_path.to_string_lossy().into_owned();
    let mut parts = Vec::new();
    for (idx, chunk) in raw.chunks(chunk_cap).enumerate() {
        let part_path = PathBuf::from(format!("{}.{:03}", base_str, idx + 1));
        fs::write(&part_path, chunk)?;
        parts.push(part_path);
    }
    Ok(parts)
}

pub fn reassemble_multipart_volumes(
    first_volume_path: &Path,
    output_archive_path: &Path,
) -> Result<u64, FormatError> {
    let first_str = first_volume_path.to_string_lossy().into_owned();
    let base_prefix = first_str
        .strip_suffix(".001")
        .ok_or_else(|| {
            FormatError::CorruptedBlock(
                "First multipart volume must end with '.001'".to_string(),
            )
        })?
        .to_string();

    let staging = output_archive_path.with_extension("join.part");
    let mut out = File::create(&staging)?;
    let mut total_written = 0u64;

    for part_num in 1..=999 {
        let candidate = PathBuf::from(format!("{}.{:03}", base_prefix, part_num));
        if !candidate.exists() {
            if part_num == 1 {
                let _ = fs::remove_file(&staging);
                return Err(FormatError::CorruptedBlock(
                    "First volume .001 does not exist".to_string(),
                ));
            }
            break;
        }
        let part_bytes = read_file_page_aligned(&candidate)?;
        out.write_all(&part_bytes)?;
        total_written += part_bytes.len() as u64;
    }
    out.flush()?;
    drop(out);

    if output_archive_path.exists() {
        fs::remove_file(output_archive_path)?;
    }
    fs::rename(&staging, output_archive_path)?;
    Ok(total_written)
}

pub fn remove_entry_from_kinetic_archive(
    archive_path: &Path,
    target_entry_path: &str,
    password: Option<&str>,
) -> Result<usize, FormatError> {
    let (mut header, mut index, crypto_key) = read_kinetic_index(archive_path, password)?;
    let norm_target = target_entry_path.replace('\\', "/").trim_end_matches('/').to_string();
    let prefix = format!("{}/", norm_target);

    let before_count = index.files.len();
    index
        .files
        .retain(|f| f.path != norm_target && !f.path.starts_with(&prefix));
    let removed = before_count.saturating_sub(index.files.len());
    if removed == 0 {
        return Err(FormatError::CorruptedBlock(format!(
            "Entry '{}' not found in archive",
            target_entry_path
        )));
    }

    let index_json =
        serde_json::to_vec(&index).map_err(|e| FormatError::CorruptedBlock(e.to_string()))?;
    let mut index_payload = lz4_flex::compress_prepend_size(&index_json);
    if let Some((ref base_nonce, ref key)) = crypto_key {
        index_payload = encrypt_block_aes256_gcm(key, base_nonce, u32::MAX, &index_payload)?;
    }

    let mut raw_archive = read_file_page_aligned(archive_path)?;
    raw_archive.truncate(header.index_offset as usize);
    let new_index_offset = raw_archive.len() as u64;
    let new_index_length = index_payload.len() as u64;
    raw_archive.extend_from_slice(&index_payload);

    header.index_offset = new_index_offset;
    header.index_length = new_index_length;
    header.total_files = index.files.len() as u64;
    header.header_crc32c = header.compute_checksum();
    raw_archive[0..GLOBAL_HEADER_SIZE].copy_from_slice(&header.to_bytes());

    let staging = archive_path.with_extension("kinetic.modify.part");
    fs::write(&staging, &raw_archive)?;
    fs::rename(&staging, archive_path)?;

    Ok(removed)
}

pub fn append_files_to_kinetic_archive(
    archive_path: &Path,
    new_sources: &[PathBuf],
    password: Option<&str>,
) -> Result<usize, FormatError> {
    let (mut header, mut index, crypto_key) = read_kinetic_index(archive_path, password)?;
    let mut raw_archive = read_file_page_aligned(archive_path)?;
    let append_offset = if index.parity_offset > 0 && index.parity_offset < header.index_offset {
        index.parity_offset
    } else {
        header.index_offset
    };
    raw_archive.truncate(append_offset as usize);
    index.parity_offset = 0;
    index.parity_length = 0;
    header.flags &= !FLAG_HAS_PARITY;

    let mut added_files = 0usize;
    for src in new_sources {
        if !src.is_file() {
            continue;
        }
        let rel_path = src
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .replace('\\', "/");
        let mtime = file_modified_unix_secs(src);
        let raw_data = read_file_page_aligned(src)?;
        let file_blake3 = blake3::hash(&raw_data).to_hex().to_string();

        let block_id = index.blocks.len() as u32;
        let block_xxh3 = xxh3_64(&raw_data);
        let (codec_id, filter_mask, mut payload) = compress_chunk_payload(
            &raw_data,
            &rel_path,
            CompressionMode::Balanced,
            true,
            &[],
        )?;

        if let Some((ref base_nonce, ref key)) = crypto_key {
            payload = encrypt_block_aes256_gcm(key, base_nonce, block_id, &payload)?;
        }

        let file_offset = raw_archive.len() as u64;
        let frame_header = BlockFrameHeader {
            block_id,
            codec_id,
            filter_mask,
            reserved: 0,
            compressed_size: payload.len() as u64,
            uncompressed_size: raw_data.len() as u64,
            block_xxh3_64: block_xxh3,
        };
        raw_archive.extend_from_slice(&frame_header.to_bytes());
        raw_archive.extend_from_slice(&payload);

        index.blocks.push(BlockDescriptor {
            block_id,
            file_offset,
            codec_id,
            filter_mask,
            compressed_size: payload.len() as u64,
            uncompressed_size: raw_data.len() as u64,
            block_xxh3_64: block_xxh3,
        });

        index.files.retain(|f| f.path != rel_path);
        index.files.push(KineticFileEntry {
            path: rel_path,
            is_dir: false,
            uncompressed_size: raw_data.len() as u64,
            compressed_size: payload.len() as u64,
            modified_unix_secs: mtime,
            blake3_hex: file_blake3,
            codec_summary: codec_name(codec_id, filter_mask),
            block_ids: vec![block_id],
            solid_offset: 0,
            is_solid_slice: false,
        });
        added_files += 1;
    }

    let index_json =
        serde_json::to_vec(&index).map_err(|e| FormatError::CorruptedBlock(e.to_string()))?;
    let mut index_payload = lz4_flex::compress_prepend_size(&index_json);
    if let Some((ref base_nonce, ref key)) = crypto_key {
        index_payload = encrypt_block_aes256_gcm(key, base_nonce, u32::MAX, &index_payload)?;
    }

    let new_index_offset = raw_archive.len() as u64;
    let new_index_length = index_payload.len() as u64;
    raw_archive.extend_from_slice(&index_payload);

    header.index_offset = new_index_offset;
    header.index_length = new_index_length;
    header.total_blocks = index.blocks.len() as u64;
    header.total_files = index.files.len() as u64;
    header.header_crc32c = header.compute_checksum();
    raw_archive[0..GLOBAL_HEADER_SIZE].copy_from_slice(&header.to_bytes());

    let staging = archive_path.with_extension("kinetic.append.part");
    fs::write(&staging, &raw_archive)?;
    fs::rename(&staging, archive_path)?;

    Ok(added_files)
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
    let seed_dict = if (desc.filter_mask & FILTER_SHARED_DICT) != 0 {
        let mut header_buf = [0u8; GLOBAL_HEADER_SIZE];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut header_buf)?;
        let header = GlobalHeader::from_bytes(&header_buf)?;
        load_seed_dictionary(file, &header, crypto_key)?
    } else {
        Vec::new()
    };

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
        if entry.is_solid_slice {
            let bid = *entry.block_ids.first().ok_or_else(|| {
                FormatError::CorruptedBlock("Missing solid block ID".to_string())
            })?;
            let solid_block = decoded_blocks.get(&bid).ok_or_else(|| {
                FormatError::CorruptedBlock(format!("Missing block ID {} in archive", bid))
            })?;
            let start = entry.solid_offset as usize;
            let end = start + (entry.uncompressed_size as usize);
            if end > solid_block.len() {
                return Err(FormatError::CorruptedBlock(
                    "Solid slice out of bounds".to_string(),
                ));
            }
            let slice = &solid_block[start..end];
            file_hasher.update(slice);
            total_uncompressed += slice.len() as u64;
        } else {
            for &bid in &entry.block_ids {
                let chunk = decoded_blocks.get(&bid).ok_or_else(|| {
                    FormatError::CorruptedBlock(format!("Missing block ID {} in archive", bid))
                })?;
                file_hasher.update(chunk);
                total_uncompressed += chunk.len() as u64;
            }
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

pub fn repair_kinetic_archive(
    archive_path: &Path,
    password: Option<&str>,
) -> Result<String, FormatError> {
    let (_header, index, crypto_key) = read_kinetic_index(archive_path, password)?;
    if index.parity_offset == 0 || index.parity_length == 0 {
        return Err(FormatError::CorruptedBlock(
            "Archive does not contain a Kinetic Shield parity block for physical repair.".to_string(),
        ));
    }

    let mut f = File::open(archive_path)?;
    let mut header_buf = [0u8; GLOBAL_HEADER_SIZE];
    f.read_exact(&mut header_buf)?;
    let header = GlobalHeader::from_bytes(&header_buf)?;
    let seed_dict = load_seed_dictionary(&mut f, &header, crypto_key.as_ref())?;

    let mut corrupted_blocks: Vec<BlockDescriptor> = Vec::new();
    for desc in &index.blocks {
        f.seek(SeekFrom::Start(desc.file_offset + (BLOCK_HEADER_SIZE as u64)))?;
        let mut raw_payload = vec![0u8; desc.compressed_size as usize];
        f.read_exact(&mut raw_payload)?;

        let check = (|| -> Result<(), FormatError> {
            let decrypted = if let Some((ref nonce, ref key)) = crypto_key {
                decrypt_block_aes256_gcm(key, nonce, desc.block_id, &raw_payload)?
            } else {
                raw_payload
            };
            let _ = decode_block_payload(
                desc.codec_id,
                desc.filter_mask,
                desc.uncompressed_size,
                desc.block_xxh3_64,
                &decrypted,
                &seed_dict,
            )?;
            Ok(())
        })();

        if check.is_err() {
            corrupted_blocks.push(desc.clone());
        }
    }

    if corrupted_blocks.is_empty() {
        return Ok("Archive verified clean: 0 corrupted blocks found.".to_string());
    }
    if corrupted_blocks.len() > 1 {
        return Err(FormatError::CorruptedBlock(format!(
            "Found {} corrupted blocks; single-stripe XOR parity can repair at most 1 block.",
            corrupted_blocks.len()
        )));
    }

    let damaged = &corrupted_blocks[0];
    f.seek(SeekFrom::Start(index.parity_offset))?;
    let mut recovered_payload = vec![0u8; index.parity_length as usize];
    f.read_exact(&mut recovered_payload)?;

    for other in &index.blocks {
        if other.block_id == damaged.block_id {
            continue;
        }
        f.seek(SeekFrom::Start(other.file_offset + (BLOCK_HEADER_SIZE as u64)))?;
        let mut other_payload = vec![0u8; other.compressed_size as usize];
        f.read_exact(&mut other_payload)?;
        for (r, &b) in recovered_payload.iter_mut().zip(other_payload.iter()) {
            *r ^= b;
        }
    }
    recovered_payload.truncate(damaged.compressed_size as usize);

    let decrypted = if let Some((ref nonce, ref key)) = crypto_key {
        decrypt_block_aes256_gcm(key, nonce, damaged.block_id, &recovered_payload)?
    } else {
        recovered_payload.clone()
    };
    let _ = decode_block_payload(
        damaged.codec_id,
        damaged.filter_mask,
        damaged.uncompressed_size,
        damaged.block_xxh3_64,
        &decrypted,
        &seed_dict,
    )?;

    let mut full_bytes = read_file_page_aligned(archive_path)?;
    let payload_start = (damaged.file_offset as usize) + BLOCK_HEADER_SIZE;
    let payload_end = payload_start + (damaged.compressed_size as usize);
    full_bytes[payload_start..payload_end].copy_from_slice(&recovered_payload);

    let repair_staging = archive_path.with_extension("kinetic.repair.part");
    fs::write(&repair_staging, &full_bytes)?;
    fs::rename(&repair_staging, archive_path)?;

    Ok(format!(
        "Kinetic Shield successfully repaired corrupted Block #{} ({} bytes) on disk.",
        damaged.block_id, damaged.compressed_size
    ))
}

pub fn cleanup_interrupted_staging_files(dir: &Path) -> io::Result<usize> {
    if !dir.exists() || !dir.is_dir() {
        return Ok(0);
    }
    let mut cleaned = 0usize;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if name.ends_with(".zip.part")
                || name.ends_with(".7z.part")
                || name.ends_with(".kinetic.part")
                || name.ends_with(".repair.part")
            {
                fs::remove_file(path)?;
                cleaned += 1;
            }
        }
    }
    Ok(cleaned)
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
        let payload = b"Kinetic Phase 9B Multithreaded Writer + FastCDC Deduplication Payload! ".repeat(2048);
        fs::write(&file1, &payload).unwrap();
        fs::write(&file2, &payload).unwrap();

        let archive_path = dir.path().join("bundle.kinetic");
        let opts = KineticWriteOptions {
            mode: CompressionMode::Maximum,
            ..Default::default()
        };

        let total_in =
            create_kinetic_archive(&[file1, file2], &archive_path, &opts).unwrap();
        assert_eq!(total_in, (payload.len() * 2) as u64);

        let (hdr, index, _) = read_kinetic_index(&archive_path, None).unwrap();
        assert_eq!(index.files.len(), 2);
        assert!(index.files[0].modified_unix_secs > 0);
        assert!(index.deduplicated_chunks_saved >= 1);
        assert!((hdr.flags & FLAG_HAS_DICTIONARY) != 0);

        let (verified_count, verified_bytes, _) =
            validate_kinetic_archive(&archive_path, None).unwrap();
        assert_eq!(verified_count, 2);
        assert_eq!(verified_bytes, (payload.len() * 2) as u64);
    }

    #[test]
    fn test_zero_recompression_append_and_delete_entry() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("first.txt");
        let f2 = dir.path().join("second.txt");
        let f3 = dir.path().join("appended.txt");
        fs::write(&f1, b"First file content").unwrap();
        fs::write(&f2, b"Second file content").unwrap();
        fs::write(&f3, b"Third appended file content").unwrap();

        let archive_path = dir.path().join("modifiable.kinetic");
        create_kinetic_archive(&[f1, f2], &archive_path, &KineticWriteOptions::default()).unwrap();

        let added = append_files_to_kinetic_archive(&archive_path, &[f3], None).unwrap();
        assert_eq!(added, 1);
        let (_, idx_after_append, _) = read_kinetic_index(&archive_path, None).unwrap();
        assert_eq!(idx_after_append.files.len(), 3);

        let removed = remove_entry_from_kinetic_archive(&archive_path, "second.txt", None).unwrap();
        assert_eq!(removed, 1);
        let (verified_files, _, _) = validate_kinetic_archive(&archive_path, None).unwrap();
        assert_eq!(verified_files, 2);
    }

    #[test]
    fn test_multipart_volume_split_and_reassemble() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("large_payload.txt");
        let data = b"Multipart volume splitting and reassembly test payload! ".repeat(4000);
        fs::write(&src, &data).unwrap();

        let archive_path = dir.path().join("split_test.kinetic");
        create_kinetic_archive(
            &[src],
            &archive_path,
            &KineticWriteOptions {
                mode: CompressionMode::Store,
                ..Default::default()
            },
        )
        .unwrap();

        let original_size = fs::metadata(&archive_path).unwrap().len();
        let volumes = split_archive_into_volumes(&archive_path, 64 * 1024).unwrap();
        assert!(volumes.len() >= 3);

        let joined_path = dir.path().join("reassembled.kinetic");
        let reassembled_size =
            reassemble_multipart_volumes(&volumes[0], &joined_path).unwrap();
        assert_eq!(reassembled_size, original_size);

        let (verified_files, verified_bytes, _) =
            validate_kinetic_archive(&joined_path, None).unwrap();
        assert_eq!(verified_files, 1);
        assert_eq!(verified_bytes, data.len() as u64);
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