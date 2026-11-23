# KINETIC ENGINE — Reproducible Benchmark Report vs. WinRAR & 7-Zip

- **Engine Version**: Kinetic Engine v0.6.0-alpha
- **Hardware Profile**: 8 Logical CPU Cores (7 Worker Threads) • 32706 MB Total RAM
- **WinRAR Baseline Binary**: `C:\Program Files\WinRAR\Rar.exe`
- **7-Zip CLI Binary**: `Using built-in sevenz-rust LZMA2 baseline` (Plus native LZMA2 `.7z` baseline)

## 1. Comparative Dataset Results

| Dataset | Engine / Format | Orig (KB) | Comp (KB) | Ratio (%) | Comp (ms) | Ext (ms) | Comp (MB/s) | Ext (MB/s) | Verified |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| 1. Source Code & Text | Kinetic (.kinetic Fast) | 4095 | 17 | 0.4% | 16 | 7 | 250.00 | 571.42 | PASS |
| 1. Source Code & Text | Kinetic (.kinetic Balanced) | 4095 | 17 | 0.4% | 15 | 7 | 266.66 | 571.42 | PASS |
| 1. Source Code & Text | Kinetic (.kinetic Ultra) | 4095 | 1 | 0.0% | 20 | 7 | 200.00 | 571.42 | PASS |
| 1. Source Code & Text | Kinetic (.kinetic Auto) | 4095 | 1 | 0.0% | 20 | 8 | 200.00 | 499.99 | PASS |
| 1. Source Code & Text | Kinetic (.zip Balanced) | 4095 | 16 | 0.4% | 14 | 6 | 285.71 | 666.66 | PASS |
| 1. Source Code & Text | 7-Zip Baseline (LZMA2 .7z) | 4095 | 1 | 0.0% | 337 | 32 | 11.87 | 125.00 | PASS |
| 1. Source Code & Text | WinRAR (Rar.exe RAR5 -m3) | 4095 | 0 | 0.0% | 134 | 92 | 29.85 | 43.48 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Fast) | 4096 | 17 | 0.4% | 17 | 9 | 235.29 | 444.44 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Balanced) | 4096 | 9 | 0.2% | 20 | 8 | 200.00 | 500.00 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Ultra) | 4096 | 1 | 0.0% | 26 | 9 | 153.85 | 444.44 | PASS |
| 2. Executables & Binaries | Kinetic (.kinetic Auto) | 4096 | 9 | 0.2% | 19 | 8 | 210.53 | 500.00 | PASS |
| 2. Executables & Binaries | Kinetic (.zip Balanced) | 4096 | 344 | 8.4% | 114 | 9 | 35.09 | 444.44 | PASS |
| 2. Executables & Binaries | 7-Zip Baseline (LZMA2 .7z) | 4096 | 41 | 1.0% | 2070 | 40 | 1.93 | 100.00 | PASS |
| 2. Executables & Binaries | WinRAR (Rar.exe RAR5 -m3) | 4096 | 1 | 0.0% | 160 | 96 | 25.00 | 41.67 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Fast) | 4096 | 4097 | 100.0% | 18 | 9 | 222.22 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Balanced) | 4096 | 4097 | 100.0% | 17 | 9 | 235.29 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Ultra) | 4096 | 4097 | 100.0% | 18 | 9 | 222.22 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.kinetic Auto) | 4096 | 4097 | 100.0% | 17 | 9 | 235.29 | 444.44 | PASS |
| 3. Incompressible Media | Kinetic (.zip Balanced) | 4096 | 4097 | 100.0% | 117 | 14 | 34.19 | 285.71 | PASS |
| 3. Incompressible Media | 7-Zip Baseline (LZMA2 .7z) | 4096 | 4096 | 100.0% | 963 | 33 | 4.15 | 121.21 | PASS |
| 3. Incompressible Media | WinRAR (Rar.exe RAR5 -m3) | 4096 | 4096 | 100.0% | 248 | 92 | 16.13 | 43.48 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Fast) | 4095 | 4 | 0.1% | 14 | 11 | 285.71 | 363.63 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Balanced) | 4095 | 4 | 0.1% | 13 | 10 | 307.69 | 399.99 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Ultra) | 4095 | 0 | 0.0% | 17 | 11 | 235.29 | 363.63 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.kinetic Auto) | 4095 | 0 | 0.0% | 17 | 11 | 235.29 | 363.63 | PASS |
| 4. Project Backup (Duplicates) | Kinetic (.zip Balanced) | 4095 | 17 | 0.4% | 17 | 13 | 235.29 | 307.69 | PASS |
| 4. Project Backup (Duplicates) | 7-Zip Baseline (LZMA2 .7z) | 4095 | 1 | 0.0% | 323 | 35 | 12.38 | 114.28 | PASS |
| 4. Project Backup (Duplicates) | WinRAR (Rar.exe RAR5 -m3) | 4095 | 1 | 0.0% | 129 | 96 | 31.01 | 41.67 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Fast) | 834 | 46 | 5.6% | 137 | 714 | 5.95 | 1.14 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Balanced) | 834 | 44 | 5.3% | 183 | 724 | 4.45 | 1.13 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Ultra) | 834 | 44 | 5.3% | 153 | 724 | 5.32 | 1.13 | PASS |
| 5. Small Files (500 Files) | Kinetic (.kinetic Auto) | 834 | 43 | 5.3% | 143 | 714 | 5.70 | 1.14 | PASS |
| 5. Small Files (500 Files) | Kinetic (.zip Balanced) | 834 | 162 | 19.5% | 404 | 740 | 2.02 | 1.10 | PASS |
| 5. Small Files (500 Files) | 7-Zip Baseline (LZMA2 .7z) | 834 | 61 | 7.4% | 726 | 934 | 1.12 | 0.87 | PASS |
| 5. Small Files (500 Files) | WinRAR (Rar.exe RAR5 -m3) | 834 | 115 | 13.9% | 217 | 617 | 3.75 | 1.32 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Fast) | 4095 | 1378 | 33.6% | 20 | 12 | 200.00 | 333.33 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Balanced) | 4095 | 1375 | 33.6% | 20 | 12 | 200.00 | 333.33 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Ultra) | 4095 | 1367 | 33.4% | 22 | 12 | 181.82 | 333.33 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.kinetic Auto) | 4095 | 1370 | 33.5% | 20 | 11 | 200.00 | 363.64 | PASS |
| 6. Mixed Real-World Folder | Kinetic (.zip Balanced) | 4095 | 1486 | 36.3% | 80 | 14 | 50.00 | 285.71 | PASS |
| 6. Mixed Real-World Folder | 7-Zip Baseline (LZMA2 .7z) | 4095 | 1380 | 33.7% | 1106 | 38 | 3.62 | 105.26 | PASS |
| 6. Mixed Real-World Folder | WinRAR (Rar.exe RAR5 -m3) | 4095 | 1366 | 33.4% | 166 | 98 | 24.10 | 40.82 | PASS |

## 2. Multithreaded Core Scaling (LZ4 Fast Block Pipeline)

| Worker Threads | Compression Speed (MB/s) | Extraction Speed (MB/s) | Duration (ms) | Scaling Factor |
| ---: | ---: | ---: | ---: | ---: |
| 1 Threads | 1694.03 MB/s | 1893.32 MB/s | 19 ms | 1.00x |
| 2 Threads | 1893.32 MB/s | 2475.89 MB/s | 17 ms | 1.12x |
| 4 Threads | 1893.32 MB/s | 2682.21 MB/s | 17 ms | 1.12x |
| 8 Threads | 1893.32 MB/s | 2682.21 MB/s | 17 ms | 1.12x |
