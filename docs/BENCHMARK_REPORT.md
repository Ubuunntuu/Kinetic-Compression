# KINETIC ENGINE — Reproducible Benchmark Report vs. WinRAR & 7-Zip

- **Engine Version**: Kinetic Engine v0.6.0-alpha
- **Hardware Profile**: 8 Logical CPU Cores (7 Worker Threads) • 32706 MB Total RAM
- **WinRAR Baseline Binary**: `C:\Program Files\WinRAR\Rar.exe`
- **7-Zip CLI Binary**: `Using built-in sevenz-rust LZMA2 baseline` (Plus native LZMA2 `.7z` baseline)

## 1. Comparative Dataset Results

| Dataset | Engine / Format | Orig (KB) | Comp (KB) | Ratio (%) | Comp (ms) | Ext (ms) | Comp (MB/s) | Ext (MB/s) | Verified |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| 1. Source Code & Text | Kinetic (.kinetic Fast) | 4095 | 21 | 0.5% | 16 | 7 | 250.00 | 571.42 | PASS |
| 1. Source Code & Text | Kinetic (.kinetic Balanced) | 4095 | 21 | 0.5% | 17 | 8 | 235.29 | 499.99 | PASS |
| 1. Source Code & Text | Kinetic (.kinetic Ultra) | 4095 | 1 | 0.0% | 21 | 8 | 190.47 | 499.99 | PASS |
| 1. Source Code & Text | Kinetic (.kinetic Auto) | 4095 | 1 | 0.0% | 20 | 7 | 200.00 | 571.42 | PASS |
| 1. Source Code & Text | Kinetic (.zip Balanced) | 4095 | 16 | 0.4% | 13 | 5 | 307.69 | 799.99 | PASS |
| 1. Source Code & Text | 7-Zip Baseline (LZMA2 .7z) | 4095 | 1 | 0.0% | 362 | 32 | 11.05 | 125.00 | PASS |
| 1. Source Code & Text | WinRAR (Rar.exe RAR5 -m3) | 4095 | 0 | 0.0% | 249 | 94 | 16.06 | 42.55 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Fast) | 4096 | 20 | 0.5% | 16 | 7 | 250.00 | 571.43 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Balanced) | 4096 | 11 | 0.3% | 21 | 12 | 190.48 | 333.33 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Ultra) | 4096 | 1 | 0.0% | 26 | 8 | 153.85 | 500.00 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Auto) | 4096 | 11 | 0.3% | 18 | 8 | 222.22 | 500.00 | PASS |
| 2. Executables & Binaries | Kinetic (.zip Balanced) | 4096 | 344 | 8.4% | 112 | 10 | 35.71 | 400.00 | PASS |
| 2. Executables & Binaries | 7-Zip Baseline (LZMA2 .7z) | 4096 | 41 | 1.0% | 2161 | 40 | 1.85 | 100.00 | PASS |
| 2. Executables & Binaries | WinRAR (Rar.exe RAR5 -m3) | 4096 | 1 | 0.0% | 157 | 97 | 25.48 | 41.24 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Fast) | 4096 | 5083 | 124.1% | 20 | 9 | 200.00 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Balanced) | 4096 | 5083 | 124.1% | 19 | 9 | 210.53 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Ultra) | 4096 | 5083 | 124.1% | 19 | 9 | 210.53 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Auto) | 4096 | 5083 | 124.1% | 19 | 9 | 210.53 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.zip Balanced) | 4096 | 4097 | 100.0% | 119 | 14 | 33.61 | 285.71 | PASS |
| 3. Incompressible Media | 7-Zip Baseline (LZMA2 .7z) | 4096 | 4096 | 100.0% | 983 | 33 | 4.07 | 121.21 | PASS |
| 3. Incompressible Media | WinRAR (Rar.exe RAR5 -m3) | 4096 | 4096 | 100.0% | 257 | 94 | 15.56 | 42.55 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Fast) | 4095 | 9 | 0.2% | 15 | 11 | 266.66 | 363.63 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Balanced) | 4095 | 9 | 0.2% | 14 | 10 | 285.71 | 399.99 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Ultra) | 4095 | 1 | 0.0% | 17 | 10 | 235.29 | 399.99 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Auto) | 4095 | 1 | 0.0% | 17 | 10 | 235.29 | 399.99 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.zip Balanced) | 4095 | 17 | 0.4% | 15 | 10 | 266.66 | 399.99 | PASS |
| 4. Project Backup (Duplicates) | 7-Zip Baseline (LZMA2 .7z) | 4095 | 1 | 0.0% | 322 | 36 | 12.42 | 111.11 | PASS |
| 4. Project Backup (Duplicates) | WinRAR (Rar.exe RAR5 -m3) | 4095 | 1 | 0.0% | 125 | 105 | 32.00 | 38.09 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Fast) | 834 | 101 | 12.1% | 153 | 793 | 5.32 | 1.03 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Balanced) | 834 | 109 | 13.1% | 187 | 740 | 4.36 | 1.10 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Ultra) | 834 | 105 | 12.6% | 202 | 725 | 4.03 | 1.12 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Auto) | 834 | 105 | 12.7% | 164 | 719 | 4.97 | 1.13 | PASS |
| 5. Small Files (500 Files) | Kinetic (.zip Balanced) | 834 | 162 | 19.5% | 305 | 729 | 2.67 | 1.12 | PASS |
| 5. Small Files (500 Files) | 7-Zip Baseline (LZMA2 .7z) | 834 | 61 | 7.4% | 509 | 790 | 1.60 | 1.03 | PASS |
| 5. Small Files (500 Files) | WinRAR (Rar.exe RAR5 -m3) | 834 | 115 | 13.9% | 231 | 619 | 3.53 | 1.32 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Fast) | 4095 | 1830 | 44.7% | 21 | 11 | 190.48 | 363.64 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Balanced) | 4095 | 1827 | 44.6% | 20 | 12 | 200.00 | 333.33 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Ultra) | 4095 | 1819 | 44.4% | 26 | 12 | 153.85 | 333.33 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Auto) | 4095 | 1822 | 44.5% | 22 | 11 | 181.82 | 363.64 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.zip Balanced) | 4095 | 1486 | 36.3% | 82 | 12 | 48.78 | 333.33 | PASS |
| 6. Mixed Real-World Folder | 7-Zip Baseline (LZMA2 .7z) | 4095 | 1380 | 33.7% | 1055 | 40 | 3.79 | 100.00 | PASS |
| 6. Mixed Real-World Folder | WinRAR (Rar.exe RAR5 -m3) | 4095 | 1366 | 33.4% | 200 | 168 | 20.00 | 23.81 | PASS |

## 2. Multithreaded Core Scaling (LZ4 Fast Block Pipeline)

| Worker Threads | Compression Speed (MB/s) | Extraction Speed (MB/s) | Duration (ms) | Scaling Factor |
| ---: | ---: | ---: | ---: | ---: |
| 1 Threads | 1532.69 MB/s | 1694.03 MB/s | 21 ms | 1.00x |
| 2 Threads | 1788.14 MB/s | 2475.89 MB/s | 18 ms | 1.17x |
| 4 Threads | 1893.32 MB/s | 2682.21 MB/s | 17 ms | 1.24x |
| 8 Threads | 1788.14 MB/s | 2682.21 MB/s | 18 ms | 1.17x |
