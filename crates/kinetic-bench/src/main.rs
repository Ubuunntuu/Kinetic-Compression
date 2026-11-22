use std::path::Path;
use kinetic_core::{ArchiveFormat, CompressionMode, HardwareProfile, ENGINE_NAME, ENGINE_VERSION};

fn find_external_tool(candidates: &[&str]) -> Option<String> {
    for path in candidates {
        if Path::new(path).exists() {
            return Some((*path).to_string());
        }
    }
    None
}

fn main() {
    println!("==========================================================");
    println!(" {} v{} — Diagnostic & Benchmark Harness", ENGINE_NAME, ENGINE_VERSION);
    println!("==========================================================");

    let hw = HardwareProfile::detect();
    println!("Logical CPU Cores        : {}", hw.logical_cores);
    println!("Recommended Worker Pool  : {} threads", hw.recommended_threads);
    println!("Total System RAM         : {} MB", hw.total_memory_mb);
    println!("Available System RAM     : {} MB", hw.available_memory_mb);
    println!("Max Safe Memory Arena    : {} MB", hw.max_safe_memory_arena_mb);
    println!("Default Archive Format   : {:?} ({})", ArchiveFormat::default(), ArchiveFormat::default().extension());
    println!("Default Compression Mode : {:?} ({})", CompressionMode::default(), CompressionMode::default().description());

    let seven_zip = find_external_tool(&[
        r"C:\Program Files\7-Zip\7z.exe",
        r"C:\Program Files (x86)\7-Zip\7z.exe",
    ]);
    let winrar = find_external_tool(&[
        r"C:\Program Files\WinRAR\Rar.exe",
        r"C:\Program Files\WinRAR\WinRAR.exe",
        r"C:\Program Files (x86)\WinRAR\WinRAR.exe",
    ]);

    println!("----------------------------------------------------------");
    println!("7-Zip Baseline Binary    : {}", seven_zip.as_deref().unwrap_or("Not found in default Program Files"));
    println!("WinRAR Baseline Binary   : {}", winrar.as_deref().unwrap_or("Not found in default Program Files"));
    println!("==========================================================");
}
