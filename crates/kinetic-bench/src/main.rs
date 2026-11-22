use std::path::PathBuf;
use kinetic_bench::run_full_benchmark_suite;

fn main() {
    let root = PathBuf::from(r"C:\projects\kinetic compression");
    println!("==========================================================================================");
    println!(" KINETIC PHASE 9 — REPRODUCIBLE COMPARATIVE BENCHMARK SUITE (vs. WinRAR & 7-Zip LZMA2)");
    println!("==========================================================================================");

    match run_full_benchmark_suite(&root, 4) {
        Ok(report) => {
            println!(
                "Hardware: {} Cores ({} Threads) | RAM: {} MB | WinRAR: {}",
                report.hardware.logical_cores,
                report.hardware.recommended_threads,
                report.hardware.total_memory_mb,
                report.winrar_detected.as_deref().unwrap_or("Not Found")
            );
            println!("------------------------------------------------------------------------------------------");
            println!(
                "{:<28} | {:<27} | {:>7} | {:>8} | {:>9} | {:>9}",
                "Dataset", "Engine / Mode", "Ratio", "Comp ms", "Comp MB/s", "Ext MB/s"
            );
            println!("------------------------------------------------------------------------------------------");
            for r in &report.results {
                println!(
                    "{:<28} | {:<27} | {:>6.1}% | {:>6}ms | {:>9.2} | {:>9.2}",
                    r.dataset_name,
                    r.engine_label,
                    r.compression_ratio_percent,
                    r.compress_ms,
                    r.compress_mb_s,
                    r.extract_mb_s
                );
            }
            println!("------------------------------------------------------------------------------------------");
            println!("MULTITHREADED CORE SCALING:");
            for pt in &report.thread_scaling {
                println!(
                    "  {} Thread(s): Compress {:.2} MB/s | Extract {:.2} MB/s | Speedup: {:.2}x",
                    pt.threads, pt.compress_mb_s, pt.extract_mb_s, pt.speedup_factor
                );
            }
            println!("==========================================================================================");
            println!("Saved Reproducible Markdown Report to: {}", report.markdown_report_path);
        }
        Err(e) => {
            eprintln!("Benchmark suite failed: {}", e);
            std::process::exit(1);
        }
    }
}