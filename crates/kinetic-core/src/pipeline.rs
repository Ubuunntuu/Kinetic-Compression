use std::io::{Read, Write};
use std::sync::Arc;
use std::thread;
use crossbeam_channel::bounded;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::xxh3_64;

use crate::classifier::classify_content;
use crate::memory::MemoryArena;
use crate::progress::JobController;
use crate::{CompressionMode, EngineError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BlockCodec {
    Store = 0x00,
    Lz4 = 0x01,
    DeflateHigh = 0x02,
    BrotliUltra = 0x04,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedBlock {
    pub block_id: u32,
    pub codec: BlockCodec,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub xxh3_checksum: u64,
    pub shannon_entropy: f64,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineJobReport {
    pub total_blocks: usize,
    pub total_uncompressed_bytes: u64,
    pub total_compressed_bytes: u64,
    pub compression_ratio_percent: f64,
    pub throughput_mb_s: f64,
    pub duration_ms: u64,
    pub blake3_root_hash_hex: String,
    pub entropy_bypassed_blocks: usize,
    pub worker_threads_used: usize,
    pub peak_arena_mb: f64,
}

pub fn estimate_shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let sample_len = data.len().min(8192);
    let sample = &data[..sample_len];
    let mut counts = [0u32; 256];
    for &b in sample {
        counts[b as usize] += 1;
    }

    let total = sample_len as f64;
    let mut entropy = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = (c as f64) / total;
            entropy -= p * p.log2();
        }
    }
    (entropy * 100.0).round() / 100.0
}

fn compress_single_block(
    block_id: u32,
    raw: &[u8],
    label: &str,
    mode: CompressionMode,
    enable_entropy_bypass: bool,
) -> Result<CompressedBlock, EngineError> {
    let uncompressed_size = raw.len() as u64;
    let xxh3_checksum = xxh3_64(raw);
    let profile = classify_content(label, raw);
    let shannon_entropy = profile.shannon_entropy;

    let target_codec = match mode {
        CompressionMode::Store => BlockCodec::Store,
        CompressionMode::Fast => {
            if enable_entropy_bypass && shannon_entropy > 7.85 {
                BlockCodec::Store
            } else {
                BlockCodec::Lz4
            }
        }
        CompressionMode::Balanced => {
            if enable_entropy_bypass && shannon_entropy > 7.85 {
                BlockCodec::Store
            } else if shannon_entropy <= 5.0 {
                BlockCodec::DeflateHigh
            } else {
                BlockCodec::Lz4
            }
        }
        CompressionMode::Maximum => {
            if enable_entropy_bypass && shannon_entropy > 7.90 {
                BlockCodec::Store
            } else {
                BlockCodec::DeflateHigh
            }
        }
        CompressionMode::Ultra => {
            if enable_entropy_bypass && shannon_entropy > 7.95 {
                BlockCodec::Store
            } else {
                BlockCodec::BrotliUltra
            }
        }
        CompressionMode::Automatic => profile.recommended_codec,
    };

    let (final_codec, payload) = match target_codec {
        BlockCodec::Store => (BlockCodec::Store, raw.to_vec()),
        BlockCodec::Lz4 => {
            let comp = lz4_flex::compress_prepend_size(raw);
            if comp.len() >= raw.len() {
                (BlockCodec::Store, raw.to_vec())
            } else {
                (BlockCodec::Lz4, comp)
            }
        }
        BlockCodec::DeflateHigh => {
            let level = if mode == CompressionMode::Maximum {
                Compression::best()
            } else {
                Compression::new(6)
            };
            let mut enc = ZlibEncoder::new(Vec::with_capacity(raw.len() / 2), level);
            enc.write_all(raw)?;
            let comp = enc.finish()?;
            if comp.len() >= raw.len() {
                (BlockCodec::Store, raw.to_vec())
            } else {
                (BlockCodec::DeflateHigh, comp)
            }
        }
        BlockCodec::BrotliUltra => {
            let mut comp = Vec::with_capacity(raw.len() / 2);
            {
                let mut writer = brotli::CompressorWriter::new(&mut comp, 4096, 9, 20);
                writer.write_all(raw)?;
                writer.flush()?;
            }
            if comp.len() >= raw.len() {
                (BlockCodec::Store, raw.to_vec())
            } else {
                (BlockCodec::BrotliUltra, comp)
            }
        }
    };

    let compressed_size = payload.len() as u64;
    Ok(CompressedBlock {
        block_id,
        codec: final_codec,
        uncompressed_size,
        compressed_size,
        xxh3_checksum,
        shannon_entropy,
        payload,
    })
}

pub fn decompress_single_block(block: &CompressedBlock) -> Result<Vec<u8>, EngineError> {
    let raw = match block.codec {
        BlockCodec::Store => block.payload.clone(),
        BlockCodec::Lz4 => lz4_flex::decompress_size_prepended(&block.payload)
            .map_err(|e| EngineError::UnsupportedOperation(format!("LZ4 decode error: {}", e)))?,
        BlockCodec::DeflateHigh => {
            let mut dec = ZlibDecoder::new(&block.payload[..]);
            let mut out = Vec::with_capacity(block.uncompressed_size as usize);
            dec.read_to_end(&mut out)?;
            out
        }
        BlockCodec::BrotliUltra => {
            let mut dec = brotli::Decompressor::new(&block.payload[..], 4096);
            let mut out = Vec::with_capacity(block.uncompressed_size as usize);
            dec.read_to_end(&mut out)?;
            out
        }
    };

    let actual_xxh3 = xxh3_64(&raw);
    if actual_xxh3 != block.xxh3_checksum {
        return Err(EngineError::UnsupportedOperation(format!(
            "Block {} XXH3 integrity mismatch: expected {:#018x}, found {:#018x}",
            block.block_id, block.xxh3_checksum, actual_xxh3
        )));
    }

    Ok(raw)
}

pub fn compress_stream_parallel(
    input: &[u8],
    label: &str,
    mode: CompressionMode,
    worker_threads: usize,
    enable_entropy_bypass: bool,
    arena: &Arc<MemoryArena>,
    controller: &Arc<JobController>,
) -> Result<(Vec<CompressedBlock>, PipelineJobReport), EngineError> {
    let threads = worker_threads.max(1);
    let block_size = mode.target_block_size_bytes().min(4 * 1024 * 1024).max(64 * 1024);
    let total_bytes = input.len() as u64;

    controller.begin_job("Multithreaded Block Compression", total_bytes, threads);
    controller.set_current_file(label);

    let full_blake3 = blake3::hash(input).to_hex().to_string();

    let (work_tx, work_rx) = bounded::<(u32, crate::memory::ArenaBuffer)>(threads * 2);
    let (res_tx, res_rx) = bounded::<Result<CompressedBlock, String>>(threads * 2);

    let mut handles = Vec::with_capacity(threads);
    for _ in 0..threads {
        let rx = work_rx.clone();
        let tx = res_tx.clone();
        let ctrl = Arc::clone(controller);
        let label_owned = label.to_string();
        handles.push(thread::spawn(move || {
            while let Ok((block_id, arena_buf)) = rx.recv() {
                if let Err(cancel_msg) = ctrl.check_point() {
                    let _ = tx.send(Err(cancel_msg));
                    break;
                }
                match compress_single_block(
                    block_id,
                    &arena_buf.data,
                    &label_owned,
                    mode,
                    enable_entropy_bypass,
                ) {
                    Ok(block) => {
                        ctrl.record_block_progress(block.uncompressed_size, block.compressed_size);
                        if tx.send(Ok(block)).is_err() {
                            break;
                        }
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

    let mut expected_blocks = 0usize;
    for (idx, chunk) in input.chunks(block_size).enumerate() {
        if controller.is_cancelled() {
            break;
        }
        let mut arena_buf = arena.allocate(chunk.len())?;
        arena_buf.data.copy_from_slice(chunk);
        if work_tx.send((idx as u32, arena_buf)).is_err() {
            break;
        }
        expected_blocks += 1;
    }
    drop(work_tx);

    let mut blocks = Vec::with_capacity(expected_blocks);
    let mut first_error: Option<String> = None;

    for result in res_rx {
        match result {
            Ok(b) => blocks.push(b),
            Err(err_msg) => {
                if first_error.is_none() {
                    first_error = Some(err_msg);
                }
            }
        }
    }

    for h in handles {
        let _ = h.join();
    }

    if let Some(err) = first_error {
        controller.fail_job(&err);
        return Err(EngineError::UnsupportedOperation(err));
    }

    blocks.sort_by_key(|b| b.block_id);
    controller.finish_job();

    let snap = controller.snapshot(arena.allocated_bytes(), arena.peak_bytes());
    let entropy_bypassed_blocks = blocks
        .iter()
        .filter(|b| b.codec == BlockCodec::Store && mode != CompressionMode::Store)
        .count();

    let report = PipelineJobReport {
        total_blocks: blocks.len(),
        total_uncompressed_bytes: snap.processed_bytes,
        total_compressed_bytes: snap.compressed_bytes,
        compression_ratio_percent: snap.compression_ratio_percent,
        throughput_mb_s: snap.throughput_mb_s,
        duration_ms: snap.elapsed_ms.max(1),
        blake3_root_hash_hex: full_blake3,
        entropy_bypassed_blocks,
        worker_threads_used: threads,
        peak_arena_mb: snap.arena_peak_mb,
    };

    Ok((blocks, report))
}

pub fn decompress_blocks_parallel(
    blocks: &[CompressedBlock],
    worker_threads: usize,
    controller: &Arc<JobController>,
) -> Result<Vec<u8>, EngineError> {
    let threads = worker_threads.max(1);
    let total_uncompressed: u64 = blocks.iter().map(|b| b.uncompressed_size).sum();

    controller.begin_job("Multithreaded Block Extraction", total_uncompressed, threads);
    controller.set_current_file("Verifying XXH3-64 block streams...");

    let (work_tx, work_rx) = bounded::<CompressedBlock>(threads * 2);
    let (res_tx, res_rx) = bounded::<Result<(u32, Vec<u8>), String>>(threads * 2);

    let mut handles = Vec::with_capacity(threads);
    for _ in 0..threads {
        let rx = work_rx.clone();
        let tx = res_tx.clone();
        let ctrl = Arc::clone(controller);
        handles.push(thread::spawn(move || {
            while let Ok(block) = rx.recv() {
                if let Err(cancel_msg) = ctrl.check_point() {
                    let _ = tx.send(Err(cancel_msg));
                    break;
                }
                match decompress_single_block(&block) {
                    Ok(raw) => {
                        ctrl.record_block_progress(raw.len() as u64, block.compressed_size);
                        if tx.send(Ok((block.block_id, raw))).is_err() {
                            break;
                        }
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

    for b in blocks {
        if work_tx.send(b.clone()).is_err() {
            break;
        }
    }
    drop(work_tx);

    let mut decoded_chunks = Vec::with_capacity(blocks.len());
    let mut first_err: Option<String> = None;

    for res in res_rx {
        match res {
            Ok(pair) => decoded_chunks.push(pair),
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

    if let Some(err) = first_err {
        controller.fail_job(&err);
        return Err(EngineError::UnsupportedOperation(err));
    }

    decoded_chunks.sort_by_key(|(id, _)| *id);
    let mut output = Vec::with_capacity(total_uncompressed as usize);
    for (_, chunk) in decoded_chunks {
        output.extend_from_slice(&chunk);
    }

    controller.finish_job();
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_six_compression_modes_roundtrip() {
        let arena = MemoryArena::new(128);
        let controller = Arc::new(JobController::new());
        let sample = b"Kinetic Engine Phase 6 All-Mode Verification Stream (Store, Fast, Balanced, Maximum, Ultra, Automatic)! ".repeat(1024);

        let modes = [
            CompressionMode::Store,
            CompressionMode::Fast,
            CompressionMode::Balanced,
            CompressionMode::Maximum,
            CompressionMode::Ultra,
            CompressionMode::Automatic,
        ];

        for mode in modes {
            let (blocks, report) = compress_stream_parallel(
                &sample,
                "source_code.rs",
                mode,
                4,
                true,
                &arena,
                &controller,
            )
            .unwrap();

            assert_eq!(report.total_uncompressed_bytes, sample.len() as u64);
            if mode != CompressionMode::Store {
                assert!(report.total_compressed_bytes < report.total_uncompressed_bytes);
            }

            let restored = decompress_blocks_parallel(&blocks, 4, &controller).unwrap();
            assert_eq!(restored, sample);
        }
    }

    #[test]
    fn test_corrupted_block_detected_by_xxh3() {
        let arena = MemoryArena::new(64);
        let controller = Arc::new(JobController::new());
        let sample = b"Integrity check payload for XXH3-64 verification".repeat(256);

        let (mut blocks, _) = compress_stream_parallel(
            &sample,
            "integrity.bin",
            CompressionMode::Ultra,
            2,
            true,
            &arena,
            &controller,
        )
        .unwrap();

        blocks[0].xxh3_checksum ^= 0xDEAD_BEEF;
        let err = decompress_blocks_parallel(&blocks, 2, &controller);
        assert!(err.is_err());
    }
}