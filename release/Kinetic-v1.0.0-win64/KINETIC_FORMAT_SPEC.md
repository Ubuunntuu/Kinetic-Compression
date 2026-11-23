# KINETIC ARCHIVE FORMAT SPECIFICATION (.kinetic) — Version 1.0

## 1. Overview
The .kinetic format is a high-performance, random-access, block-multiplexed archive format designed for the Kinetic Engine.
- Magic Header: KNT1\r\n\x1a\n (64-byte global header)
- Compression Codecs: Store (0x00), LZ4 (0x01), Zstd (0x02), LZMA2 (0x03), Brotli (0x04)
- Pre-Filters: x86/ARM64 BCJ, Byte-Delta, SIMD Byte-Shuffle, Deflate Container Repack, 64KB Seed Dictionary
- Deduplication: FastCDC content-defined chunking with global block pointers
- Integrity & Security: XXH3-64 per block, BLAKE3 full-file digest, AES-256-GCM + Argon2id encryption, Parity Recovery
