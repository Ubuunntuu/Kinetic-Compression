use std::path::{Path, PathBuf};
use std::process::Command;
use kinetic_compat::{create_archive, extract_archive, inspect_archive, repair_archive, validate_archive};
use kinetic_core::{ArchiveFormat, CompressionMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellIntegrationStatus {
    pub registered: bool,
    pub executable_path: String,
    pub registered_verbs: Vec<String>,
    pub message: String,
}

fn run_ps_script(script: &str) -> Result<(), String> {
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(())
}

pub fn query_shell_status() -> ShellIntegrationStatus {
    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from(r"C:\projects\kinetic compression\release\Kinetic-v1.0.0-win64\Kinetic.exe"))
        .to_string_lossy()
        .into_owned();

    let check = Command::new("reg")
        .args(["query", r"HKCU\Software\Classes\*\shell\KineticMenu\shell"])
        .output();

    let registered = check.map(|o| o.status.success()).unwrap_or(false);

    ShellIntegrationStatus {
        registered,
        executable_path: exe_path,
        registered_verbs: if registered {
            vec![
                "Kinetic > Open in Kinetic Archive Explorer...".to_string(),
                "Kinetic > Add to .kinetic Archive".to_string(),
                "Kinetic > Add to .zip Archive".to_string(),
                "Kinetic > Extract Here".to_string(),
                "Kinetic > Extract to Subfolder".to_string(),
            ]
        } else {
            Vec::new()
        },
        message: if registered {
            "Cascading 'Kinetic >' flyout menu and .kinetic file association are active in Windows Explorer.".to_string()
        } else {
            "Context menus are not currently registered in Windows Explorer.".to_string()
        },
    }
}

/// Removes both legacy flat menu entries and the cascading submenu entries cleanly.
pub fn unregister_explorer_context_menus() -> Result<ShellIntegrationStatus, String> {
    let keys = [
        r"HKCU\Software\Classes\*\shell\KineticCompressZip",
        r"HKCU\Software\Classes\*\shell\KineticCompressNative",
        r"HKCU\Software\Classes\*\shell\KineticExtractHere",
        r"HKCU\Software\Classes\*\shell\KineticExtractSub",
        r"HKCU\Software\Classes\*\shell\KineticMenu",
        r"HKCU\Software\Classes\Directory\shell\KineticCompressZip",
        r"HKCU\Software\Classes\Directory\shell\KineticCompressNative",
        r"HKCU\Software\Classes\Directory\shell\KineticMenu",
        r"HKCU\Software\Classes\.kinetic",
        r"HKCU\Software\Classes\Kinetic.Archive.1",
    ];
    for k in keys {
        let _ = Command::new("reg").args(["delete", k, "/f"]).output();
    }
    Ok(query_shell_status())
}

/// Registers a clean WinRAR-style cascading "Kinetic >" submenu for all files and folders.
pub fn register_explorer_context_menus() -> Result<ShellIntegrationStatus, String> {
    // First remove any old flat menu keys so only the single cascading "Kinetic >" menu appears
    let _ = unregister_explorer_context_menus();

    let release_exe = PathBuf::from(r"C:\projects\kinetic compression\release\Kinetic-v1.0.0-win64\Kinetic.exe");
    let exe_path = if release_exe.exists() {
        release_exe.to_string_lossy().into_owned()
    } else {
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned()
    };
    let esc_exe = exe_path.replace('\'', "''");

    let ps = format!(
        r#"
        $exe = '{esc_exe}'
        $icon = "$exe,0"

        function Add-KineticSubVerb([string]$ParentShellPath, [string]$KeyName, [string]$Label, [string]$ArgFlag) {{
            $vPath = "$ParentShellPath\$KeyName"
            $cPath = "$vPath\command"
            New-Item -Path $cPath -Force | Out-Null
            Set-ItemProperty -LiteralPath $vPath -Name 'MUIVerb' -Value $Label
            Set-ItemProperty -LiteralPath $vPath -Name 'Icon' -Value $icon
            Set-ItemProperty -LiteralPath $cPath -Name '(Default)' -Value "`"$exe`" $ArgFlag `"%1`""
        }}

        # 1. Cascading 'Kinetic >' Menu for All Files (*)
        $fileMenu = 'HKCU:\Software\Classes\*\shell\KineticMenu'
        $fileSub = "$fileMenu\shell"
        New-Item -Path $fileSub -Force | Out-Null
        Set-ItemProperty -LiteralPath $fileMenu -Name 'MUIVerb' -Value 'Kinetic'
        Set-ItemProperty -LiteralPath $fileMenu -Name 'Icon' -Value $icon
        Set-ItemProperty -LiteralPath $fileMenu -Name 'SubCommands' -Value ''

        Add-KineticSubVerb $fileSub '01_Open'           'Open in Kinetic Archive Explorer...' '--open'
        Add-KineticSubVerb $fileSub '02_AddKinetic'     'Add to .kinetic Archive'             '--compress-kinetic'
        Add-KineticSubVerb $fileSub '03_AddZip'         'Add to .zip Archive'                 '--compress-zip'
        Add-KineticSubVerb $fileSub '04_ExtractHere'    'Extract Here'                        '--extract-here'
        Add-KineticSubVerb $fileSub '05_ExtractSub'     'Extract to Subfolder'                '--extract-sub'

        # 2. Cascading 'Kinetic >' Menu for All Folders (Directory)
        $dirMenu = 'HKCU:\Software\Classes\Directory\shell\KineticMenu'
        $dirSub = "$dirMenu\shell"
        New-Item -Path $dirSub -Force | Out-Null
        Set-ItemProperty -LiteralPath $dirMenu -Name 'MUIVerb' -Value 'Kinetic'
        Set-ItemProperty -LiteralPath $dirMenu -Name 'Icon' -Value $icon
        Set-ItemProperty -LiteralPath $dirMenu -Name 'SubCommands' -Value ''

        Add-KineticSubVerb $dirSub '01_AddKinetic'      'Add to .kinetic Archive'             '--compress-kinetic'
        Add-KineticSubVerb $dirSub '02_AddZip'          'Add to .zip Archive'                 '--compress-zip'

        # 3. Double-Click .kinetic File Association
        New-Item -Path 'HKCU:\Software\Classes\.kinetic' -Force | Out-Null
        Set-ItemProperty -LiteralPath 'HKCU:\Software\Classes\.kinetic' -Name '(Default)' -Value 'Kinetic.Archive.1'
        New-Item -Path 'HKCU:\Software\Classes\Kinetic.Archive.1\DefaultIcon' -Force | Out-Null
        Set-ItemProperty -LiteralPath 'HKCU:\Software\Classes\Kinetic.Archive.1' -Name '(Default)' -Value 'Kinetic Native Archive (.kinetic)'
        Set-ItemProperty -LiteralPath 'HKCU:\Software\Classes\Kinetic.Archive.1\DefaultIcon' -Name '(Default)' -Value $icon
        New-Item -Path 'HKCU:\Software\Classes\Kinetic.Archive.1\shell\open\command' -Force | Out-Null
        Set-ItemProperty -LiteralPath 'HKCU:\Software\Classes\Kinetic.Archive.1\shell\open\command' -Name '(Default)' -Value "`"$exe`" --open `"%1`""
        "#
    );

    run_ps_script(&ps)?;
    Ok(query_shell_status())
}

pub fn extract_startup_open_archive(args: &[String]) -> Option<String> {
    if args.len() >= 3 && args[1] == "--open" {
        return Some(args[2].clone());
    }
    if args.len() == 2 && !args[1].starts_with("--") {
        let p = Path::new(&args[1]);
        if p.exists() && p.is_file() {
            return Some(args[1].clone());
        }
    }
    None
}

pub fn try_handle_cli_invocation(args: &[String]) -> Option<i32> {
    if args.len() >= 2 && args[1] == "--register-shell" {
        return match register_explorer_context_menus() {
            Ok(st) => {
                println!("{}", st.message);
                Some(0)
            }
            Err(e) => {
                eprintln!("Shell registration error: {}", e);
                Some(1)
            }
        };
    }

    if args.len() < 3 {
        return None;
    }
    let flag = args[1].as_str();
    let target = PathBuf::from(&args[2]);

    match flag {
        "--compress-zip" => {
            let out_path = derive_archive_output_path(&target, "zip");
            match create_archive(&[target], &out_path, ArchiveFormat::Zip, CompressionMode::Balanced) {
                Ok(_) => Some(0),
                Err(e) => {
                    eprintln!("Kinetic CLI ZIP compression error: {}", e);
                    Some(1)
                }
            }
        }
        "--compress-kinetic" => {
            let out_path = derive_archive_output_path(&target, "kinetic");
            match create_archive(
                &[target],
                &out_path,
                ArchiveFormat::Kinetic,
                CompressionMode::Automatic,
            ) {
                Ok(_) => Some(0),
                Err(e) => {
                    eprintln!("Kinetic CLI .kinetic compression error: {}", e);
                    Some(1)
                }
            }
        }
        "--extract-here" => {
            let dest = target
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            match extract_archive(&target, &dest, "rename") {
                Ok(_) => Some(0),
                Err(e) => {
                    eprintln!("Kinetic CLI Extract Here error: {}", e);
                    Some(1)
                }
            }
        }
        "--extract-sub" => {
            let dest = derive_extract_subfolder(&target);
            match extract_archive(&target, &dest, "overwrite") {
                Ok(_) => Some(0),
                Err(e) => {
                    eprintln!("Kinetic CLI Extract to Subfolder error: {}", e);
                    Some(1)
                }
            }
        }
        "--list" => match inspect_archive(&target) {
            Ok(info) => {
                println!(
                    "Archive: {} ({:?}, {} entries, {} bytes uncompressed)",
                    info.path, info.format, info.total_entries, info.total_uncompressed_bytes
                );
                for entry in info.entries {
                    println!(
                        "  {:<48} | {:>10} B -> {:>10} B | {}",
                        entry.path, entry.uncompressed_size, entry.compressed_size, entry.codec_name
                    );
                }
                Some(0)
            }
            Err(e) => {
                eprintln!("CLI List error: {}", e);
                Some(1)
            }
        },
        "--verify" => match validate_archive(&target) {
            Ok(rep) => {
                println!("VERIFY PASSED: {}", rep.message);
                Some(0)
            }
            Err(e) => {
                eprintln!("VERIFY FAILED: {}", e);
                Some(1)
            }
        },
        "--repair" => match repair_archive(&target, None) {
            Ok(rep) => {
                println!("REPAIR COMPLETE: {}", rep.message);
                Some(0)
            }
            Err(e) => {
                eprintln!("REPAIR FAILED: {}", e);
                Some(1)
            }
        },
        _ => None,
    }
}

pub fn derive_archive_output_path(source: &Path, ext: &str) -> PathBuf {
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let stem = if source.is_dir() {
        source
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "archive".to_string())
    } else {
        source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "archive".to_string())
    };
    parent.join(format!("{}.{}", stem, ext))
}

pub fn derive_extract_subfolder(archive_path: &Path) -> PathBuf {
    let parent = archive_path.parent().unwrap_or_else(|| Path::new("."));
    let raw_stem = archive_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "extracted".to_string());
    let clean_stem = Path::new(&raw_stem)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or(raw_stem);

    let candidate = parent.join(&clean_stem);
    if candidate.exists() && candidate.is_file() {
        parent.join(format!("{}_extracted", clean_stem))
    } else {
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cli_context_menu_compress_and_extract_roundtrip() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("explorer_click.txt");
        let payload = b"Windows Explorer Context Menu CLI Integration Test!";
        std::fs::write(&src_file, payload).unwrap();

        let args_knt = vec![
            "Kinetic.exe".to_string(),
            "--compress-kinetic".to_string(),
            src_file.to_string_lossy().into_owned(),
        ];
        assert_eq!(try_handle_cli_invocation(&args_knt), Some(0));

        let created_knt = dir.path().join("explorer_click.kinetic");
        assert!(created_knt.exists());

        let args_ext = vec![
            "Kinetic.exe".to_string(),
            "--extract-sub".to_string(),
            created_knt.to_string_lossy().into_owned(),
        ];
        assert_eq!(try_handle_cli_invocation(&args_ext), Some(0));

        let extracted_file = dir
            .path()
            .join("explorer_click")
            .join("explorer_click.txt");
        assert!(extracted_file.exists());
        assert_eq!(std::fs::read(&extracted_file).unwrap(), payload);
    }

    #[test]
    fn test_double_click_open_argument_detection() {
        let args = vec![
            "Kinetic.exe".to_string(),
            "--open".to_string(),
            r"C:\archives\backup.kinetic".to_string(),
        ];
        assert_eq!(
            extract_startup_open_archive(&args),
            Some(r"C:\archives\backup.kinetic".to_string())
        );
    }
}