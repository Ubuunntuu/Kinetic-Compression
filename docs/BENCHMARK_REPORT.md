# KINETIC ENGINE — Reproducible Benchmark Report vs. WinRAR & 7-Zip

- **Engine Version**: Kinetic Engine v1.0.0
- **Hardware Profile**: 8 Logical CPU Cores (7 Worker Threads) • 32706 MB Total RAM
- **WinRAR Baseline Binary**: `C:\Program Files\WinRAR\Rar.exe`
- **7-Zip CLI Binary**: `Using built-in sevenz-rust LZMA2 baseline` (Plus native LZMA2 `.7z` baseline)

## 1. Comparative Dataset Results

| Dataset | Engine / Format | Orig (KB) | Comp (KB) | Ratio (%) | Comp (ms) | Ext (ms) | Comp (MB/s) | Ext (MB/s) | Verified |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| 1. Source Code & Text | Kinetic (.kinetic Fast) | 4095 | 17 | 0.4% | 16 | 7 | 250.00 | 571.42 | PASS |
| 1. Source Code & Text | Kinetic (.kinetic Balanced) | 4095 | 17 | 0.4% | 16 | 7 | 250.00 | 571.42 | PASS |
| 1. Source Code & Text | Kinetic (.kinetic Ultra) | 4095 | 1 | 0.0% | 22 | 8 | 181.82 | 499.99 | PASS |
| 1. Source Code & Text | Kinetic (.kinetic Auto) | 4095 | 1 | 0.0% | 21 | 8 | 190.47 | 499.99 | PASS |
| 1. Source Code & Text | Kinetic (.zip Balanced) | 4095 | 16 | 0.4% | 13 | 5 | 307.69 | 799.99 | PASS |
| 1. Source Code & Text | 7-Zip Baseline (LZMA2 .7z) | 4095 | 1 | 0.0% | 369 | 32 | 10.84 | 125.00 | PASS |
| 1. Source Code & Text | WinRAR (Rar.exe RAR5 -m3) | 4095 | 0 | 0.0% | 270 | 98 | 14.81 | 40.82 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Fast) | 4096 | 17 | 0.4% | 18 | 8 | 222.22 | 500.00 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Balanced) | 4096 | 9 | 0.2% | 21 | 8 | 190.48 | 500.00 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Ultra) | 4096 | 1 | 0.0% | 27 | 9 | 148.15 | 444.44 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Auto) | 4096 | 9 | 0.2% | 20 | 9 | 200.00 | 444.44 | PASS |
| 2. Executables & Binaries | Kinetic (.zip Balanced) | 4096 | 344 | 8.4% | 120 | 11 | 33.33 | 363.64 | PASS |
| 2. Executables & Binaries | 7-Zip Baseline (LZMA2 .7z) | 4096 | 41 | 1.0% | 2080 | 40 | 1.92 | 100.00 | PASS |
| 2. Executables & Binaries | WinRAR (Rar.exe RAR5 -m3) | 4096 | 1 | 0.0% | 158 | 97 | 25.32 | 41.24 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Fast) | 4096 | 4097 | 100.0% | 18 | 9 | 222.22 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Balanced) | 4096 | 4097 | 100.0% | 17 | 9 | 235.29 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Ultra) | 4096 | 4097 | 100.0% | 17 | 8 | 235.29 | 500.00 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Auto) | 4096 | 4097 | 100.0% | 18 | 9 | 222.22 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.zip Balanced) | 4096 | 4097 | 100.0% | 118 | 14 | 33.90 | 285.71 | PASS |
| 3. Incompressible Media | 7-Zip Baseline (LZMA2 .7z) | 4096 | 4096 | 100.0% | 959 | 33 | 4.17 | 121.21 | PASS |
| 3. Incompressible Media | WinRAR (Rar.exe RAR5 -m3) | 4096 | 4096 | 100.0% | 246 | 92 | 16.26 | 43.48 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Fast) | 4095 | 4 | 0.1% | 15 | 9 | 266.66 | 444.44 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Balanced) | 4095 | 4 | 0.1% | 14 | 9 | 285.71 | 444.44 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Ultra) | 4095 | 0 | 0.0% | 18 | 10 | 222.22 | 399.99 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Auto) | 4095 | 0 | 0.0% | 17 | 10 | 235.29 | 399.99 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.zip Balanced) | 4095 | 17 | 0.4% | 14 | 11 | 285.71 | 363.63 | PASS |
| 4. Project Backup (Duplicates) | 7-Zip Baseline (LZMA2 .7z) | 4095 | 1 | 0.0% | 322 | 35 | 12.42 | 114.28 | PASS |
| 4. Project Backup (Duplicates) | WinRAR (Rar.exe RAR5 -m3) | 4095 | 1 | 0.0% | 127 | 110 | 31.50 | 36.36 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Fast) | 834 | 46 | 5.6% | 172 | 283 | 4.74 | 2.88 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Balanced) | 834 | 44 | 5.3% | 140 | 270 | 5.82 | 3.02 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Ultra) | 834 | 43 | 5.3% | 141 | 275 | 5.78 | 2.96 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Auto) | 834 | 43 | 5.3% | 190 | 273 | 4.29 | 2.98 | PASS |
| 5. Small Files (500 Files) | Kinetic (.zip Balanced) | 834 | 162 | 19.5% | 266 | 892 | 3.06 | 0.91 | PASS |
| 5. Small Files (500 Files) | 7-Zip Baseline (LZMA2 .7z) | 834 | 61 | 7.3% | 546 | 997 | 1.49 | 0.82 | PASS |
| 5. Small Files (500 Files) | WinRAR (Rar.exe RAR5 -m3) | 834 | 115 | 13.9% | 221 | 602 | 3.69 | 1.35 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Fast) | 4095 | 1378 | 33.6% | 21 | 11 | 190.48 | 363.64 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Balanced) | 4095 | 1375 | 33.6% | 19 | 12 | 210.53 | 333.33 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Ultra) | 4095 | 1367 | 33.4% | 23 | 11 | 173.91 | 363.64 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Auto) | 4095 | 1370 | 33.5% | 22 | 11 | 181.82 | 363.64 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.zip Balanced) | 4095 | 1486 | 36.3% | 83 | 13 | 48.19 | 307.69 | PASS |
| 6. Mixed Real-World Folder | 7-Zip Baseline (LZMA2 .7z) | 4095 | 1380 | 33.7% | 1061 | 37 | 3.77 | 108.11 | PASS |
| 6. Mixed Real-World Folder | WinRAR (Rar.exe RAR5 -m3) | 4095 | 1366 | 33.4% | 165 | 98 | 24.24 | 40.82 | PASS |

## 2. Multithreaded Core Scaling (LZ4 Fast Block Pipeline)

| Worker Threads | Compression Speed (MB/s) | Extraction Speed (MB/s) | Duration (ms) | Scaling Factor |
| ---: | ---: | ---: | ---: | ---: |
| 1 Threads | 1609.33 MB/s | 1893.32 MB/s | 20 ms | 1.00x |
| 2 Threads | 1893.32 MB/s | 2299.04 MB/s | 17 ms | 1.18x |
| 4 Threads | 1788.14 MB/s | 2682.21 MB/s | 18 ms | 1.11x |
| 8 Threads | 1788.14 MB/s | 2475.89 MB/s | 18 ms | 1.11x |
