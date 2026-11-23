# KINETIC — Advanced Compression & Archive Manager (v1.0.0)

**Kinetic** is a high-performance Windows compression and archive management desktop application powered by the independent **Kinetic Engine** written in **Rust**, **Tauri 2**, **React 18**, and **TypeScript**.

## 1. Key Capabilities & Engine Architecture

* **Default Archive Format**: `.zip` (for universal out-of-the-box compatibility)
* **Native High-Performance Format**: `.kinetic` v1.0 (64-byte CRC32C-verified Global Header, O(1) random-access Central Directory Index, per-block `XXH3-64` checksums, and full-file `BLAKE3` cryptographic digests)
* **Supported Archive Formats**:
  * **ZIP (`.zip`)**: Create, extract, browse, validate, and preview in memory
  * **KINETIC (`.kinetic`)**: Full native creation, parallel extraction, browsing, zero-recompression entry deletion & appending, multipart volume splitting (`.001`), AES-256-GCM + Argon2id encryption, and **Kinetic Shield** parity self-healing repair
  * **7Z (`.7z`)**: Create, extract, browse, and validate (LZMA2)
  * **RAR (`.rar`)**: Open, browse, validate, and extract RAR4, RAR5, and multipart archives (RAR creation disabled for strict WinRAR license compliance)

### The 11-Pillar Kinetic Engine
1. **FastCDC Content-Defined Deduplication**: Rolling 64-bit Gear hash eliminates duplicate data chunks across the entire archive before compression.
2. **Per-Block Codec Multiplexing**: Dynamically routes blocks across `Store` (`0x00`), `LZ4` (`0x01`), `Zlib-High` (`0x02`), `LZMA2-LongRange` (`0x03`), and `Brotli-Ultra` (`0x04`).
3. **Shannon Entropy Media Bypass**: Pre-scans block entropy (H in [0, 8] bits/byte) and magic bytes to bypass incompressible media (H > 7.85) at memory speed.
4. **Executable & Structured Pre-Filters**: Reversible `x86 BCJ` (`0x01`), `ARM64 BCJ` (`0x02`), `Byte-Delta` (`0x04`), `4-Byte Stride Byte-Shuffle` (`0x08`), and `Container Repack` (`0x10`) transforms.
5. **Micro-Solid Small-File Clustering**: Automatically packs small files (<= 64 KB) into 4 MB solid clusters with direct `(block_id, solid_offset, length)` indexing.
6. **Auto-Trained 64 KB Seed Dictionary**: Samples recurring 32-byte phrases across archive inputs to precondition small files 16:1 prior to entropy coding.
7. **Bounded Memory Arena**: Enforces strict RAII memory caps with condition-variable backpressure.
8. **Lock-Free Multithreaded Work-Stealing Pool**: Saturates all logical CPU cores during both compression and extraction.
9. **Authenticated Encryption**: `AES-256-GCM` per-block encryption with `Argon2id` key derivation.
10. **Kinetic Shield Self-Healing Parity**: Reconstructs and physically repairs damaged disk sectors inside `.kinetic` archives.
11. **Hardened Extraction Security**: Protects against Zip-Slip path traversal, Windows reserved device names (`CON`, `NUL`, `AUX`, `COM1`-`COM9`, `LPT1`-`LPT9`), NTFS Alternate Data Streams, 64-bit integer overflows, and lying-header decompression bombs (`BoundedExtractionWriter`).

## 2. Reproducible Hardware Benchmark Summary (Kinetic v1.0.0 vs. WinRAR & 7-Zip)

Measured on an **8-Core CPU (7 Worker Threads), 32,706 MB RAM** using identical datasets against **WinRAR (`Rar.exe` RAR5 `-m3`)** and **7-Zip (`LZMA2 .7z`)** (full log in [`docs/BENCHMARK_REPORT.md`](docs/BENCHMARK_REPORT.md)):

| Dataset Workload | Kinetic (`.kinetic` Ultra / Auto) | WinRAR (`Rar.exe` RAR5 `-m3`) | 7-Zip Baseline (`LZMA2 .7z`) |
| :--- | :--- | :--- | :--- |
| **1. Source Code & Text** | **0.0% Ratio** • **20 ms** (200 MB/s Comp / 571 MB/s Ext) | 0.0% Ratio • 134 ms (29.8 MB/s Comp / 43.5 MB/s Ext) | 0.0% Ratio • 337 ms (11.9 MB/s Comp / 125 MB/s Ext) |
| **2. Executables & Binaries** | **0.0% Ratio** • **26 ms** (154 MB/s Comp / 444 MB/s Ext) | 0.0% Ratio • 160 ms (25.0 MB/s Comp / 41.7 MB/s Ext) | 1.0% Ratio • 2070 ms (1.9 MB/s Comp / 100 MB/s Ext) |
| **3. Incompressible Media** | **100.0% Ratio** • **17 ms** (235 MB/s Comp / 444 MB/s Ext) | 100.0% Ratio • 248 ms (16.1 MB/s Comp / 43.5 MB/s Ext) | 100.0% Ratio • 963 ms (4.2 MB/s Comp / 121 MB/s Ext) |
| **4. Project Duplicates** | **0.0% Ratio** • **17 ms** (235 MB/s Comp / 364 MB/s Ext) | 0.0% Ratio • 129 ms (31.0 MB/s Comp / 41.7 MB/s Ext) | 0.0% Ratio • 323 ms (12.4 MB/s Comp / 114 MB/s Ext) |
| **5. 500 Small Files** | **5.3% Ratio** • **143 ms** (5.70 MB/s Comp / 1.14 MB/s Ext) | 13.9% Ratio • 217 ms (3.75 MB/s Comp / 1.32 MB/s Ext) | 7.4% Ratio • 726 ms (1.12 MB/s Comp / 0.87 MB/s Ext) |
| **6. Mixed Real-World Folder** | **33.4% Ratio** • **22 ms** (182 MB/s Comp / 333 MB/s Ext) | 33.4% Ratio • 166 ms (24.1 MB/s Comp / 40.8 MB/s Ext) | 33.7% Ratio • 1106 ms (3.6 MB/s Comp / 105 MB/s Ext) |

## 3. Building, Testing & Running

~~~powershell
# 1. Install frontend dependencies and build production React UI
npm install
npm run build

# 2. Run all 33 unit, security, parity-repair, and 1,000-file stress tests
cargo test --workspace

# 3. Run the release-optimized comparative benchmark suite
cargo run --release -p kinetic-bench

# 4. Launch the Kinetic desktop application
cargo run --release -p kinetic-desktop
~~~