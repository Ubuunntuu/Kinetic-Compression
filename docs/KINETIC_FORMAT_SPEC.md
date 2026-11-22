# KINETIC ARCHIVE FORMAT SPECIFICATION (.kinetic) — Version 1.0

## 1. Overview
The `.kinetic` format is a high-performance, random-access, block-multiplexed archive format designed for the Kinetic Engine. It supports content-defined deduplication (FastCDC), per-block algorithm selection, pre-processing filters, optional pre-trained dictionaries, authenticated encryption (AES-256-GCM + Argon2id), and parity recovery records.

## 2. Binary Layout
A valid `.kinetic` v1.0 archive consists of five sequential regions:

1. **Global Archive Header** (Fixed 64 bytes)
2. **Optional Encryption & Dictionary Header** (Variable length, present if flagged)
3. **Independent Data & Deduplication Blocks** (Variable length stream)
4. **Optional Kinetic Shield Parity Blocks** (Variable length, present if flagged)
5. **Central Directory Index & Master Footer** (Variable length + Fixed 48-byte trailer)

All multi-byte integers are serialized in **Little-Endian (LE)** byte order.

## 3. Global Archive Header (64 Bytes)
| Offset | Size | Type | Field Name | Description |
| :--- | :--- | :--- | :--- | :--- |
| 0x00 | 8 B | `[u8; 8]` | `magic` | Exact bytes: `4B 4E 54 31 0D 0A 1A 0A` (`KNT1\r\n\x1a\n`) |
| 0x08 | 2 B | `u16` | `version_major` | Format major version (`1`) |
| 0x0A | 2 B | `u16` | `version_minor` | Format minor version (`0`) |
| 0x0C | 4 B | `u32` | `flags` | Bitmask: `0x01`=Encrypted, `0x02`=HasDict, `0x04`=HasParity, `0x08`=SolidMicroBlocks |
| 0x10 | 8 B | `u64` | `index_offset` | Absolute byte offset to the start of the Central Directory Index |
| 0x18 | 8 B | `u64` | `index_length` | Compressed byte length of the Central Directory Index |
| 0x20 | 8 B | `u64` | `total_blocks` | Total number of unique compressed data blocks |
| 0x28 | 8 B | `u64` | `total_files` | Total number of file and directory entries |
| 0x30 | 8 B | `u64` | `dict_offset` | Byte offset to shared seed dictionary (`0` if unused) |
| 0x38 | 4 B | `u32` | `dict_length` | Byte length of shared seed dictionary (`0` if unused) |
| 0x3C | 4 B | `u32` | `header_crc32c` | Castagnoli CRC32C of bytes `0x00..0x3B` |

## 4. Per-Block Frame Header (32 Bytes)
Each data block is independently addressable and self-verifying:

* `block_id` (`u32`): Sequential block identifier.
* `codec_id` (`u8`):
  * `0x00` = Store (Raw / Entropy Bypass)
  * `0x01` = LZ4 Frame / HC
  * `0x02` = Zstandard (Zstd)
  * `0x03` = LZMA2
  * `0x04` = Brotli
* `filter_mask` (`u8`):
  * `0x00` = None
  * `0x01` = x86 BCJ Filter
  * `0x02` = ARM64 BCJ Filter
  * `0x04` = Byte-Delta Filter
  * `0x08` = SIMD Byte-Shuffle Transpose
  * `0x10` = Deflate Container Repack
  * `0x20` = Shared Seed Dictionary Applied
* `reserved` (`u16`): Alignment padding (`0x0000`).
* `compressed_size` (`u64`): Size of payload on disk in bytes.
* `uncompressed_size` (`u64`): Original size of block payload in bytes.
* `block_xxh3_64` (`u64`): 64-bit XXH3 checksum of the uncompressed block payload.

## 5. Content-Defined Deduplication & File Entry Mapping
Each file entry in the Central Directory contains:
* UTF-8 normalized relative path (strictly validated against `..` and root prefixes).
* Original size (`u64`), modification timestamp (`i64` UTC Unix epoch nanoseconds), and OS attributes (`u32`).
* Full-file 256-bit **BLAKE3** integrity hash (`[u8; 32]`).
* **Chunk Map List**: One or more `(block_id, byte_offset_in_block, chunk_length)` tuples. Identical chunks across files point to the same `(block_id, byte_offset_in_block)`, enabling zero-overhead global deduplication.

## 6. Security & Cryptographic Specification
* **Key Derivation**: Argon2id (`m = 65536 KiB`, `t = 3`, `p = 4`, 32-byte CSPRNG salt).
* **Authenticated Encryption**: AES-256-GCM with unique 96-bit nonces derived per block (`base_iv ^ block_id`) and 128-bit authentication tags appended to each encrypted block and the Central Directory Index.