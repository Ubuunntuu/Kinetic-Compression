use std::path::{Path, PathBuf};
use std::process::Command;
use kinetic_compat::{create_archive, extract_archive};
use kinetic_core::{ArchiveFormat, CompressionMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellIntegrationStatus {
    pub registered: bool,
    pub executable_path: String,
    pub registered_verbs: Vec<String>,
    pub message: String,
}

fn reg_add_key(key: &str, value_name: Option<&str>, data: &str) -> Result<(), String> {
    let mut cmd = Command::new("reg");
    cmd.arg("add").arg(key);
    match value_name {
        Some(v) => {
            cmd.arg("/v").arg(v);
        }
        None => {
            cmd.arg("/ve");
        }
    }
    cmd.args(["/t", "REG_SZ", "/d", data, "/f"]);

    let output = cmd.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Registry update failed for {}: {}",
            key,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn reg_delete_key(key: &str) {
    let _ = Command::new("reg").args(["delete", key, "/f"]).output();
}

pub fn query_shell_status() -> ShellIntegrationStatus {
    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from(r"C:\projects\kinetic compression\release\Kinetic-v1.0.0-win64\Kinetic.exe"))
        .to_string_lossy()
        .into_owned();

    let check = Command::new("reg")
        .args(["query", r"HKCU\Software\Classes\*\shell\KineticCompressZip"])
        .output();

    let registered = check.map(|o| o.status.success()).unwrap_or(false);

    ShellIntegrationStatus {
        registered,
        executable_path: exe_path,
        registered_verbs: if registered {
            vec![
                "Kinetic: Compress to .zip".to_string(),
                "Kinetic: Compress to .kinetic".to_string(),
                "Kinetic: Extract Here".to_string(),
                "Kinetic: Extract to Subfolder".to_string(),
            ]
        } else {
            Vec::new()
        },
        message: if registered {
            "Windows Explorer right-click context menus are active under HKCU\\Software\\Classes.".to_string()
        } else {
            "Context menus are not currently registered in Windows Explorer.".to_string()
        },
    }
}

pub fn register_explorer_context_menus() -> Result<ShellIntegrationStatus, String> {
    let release_exe = PathBuf::from(r"C:\projects\kinetic compression\release\Kinetic-v1.0.0-win64\Kinetic.exe");
    let exe_path = if release_exe.exists() {
        release_exe.to_string_lossy().into_owned()
    } else {
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned()
    };

    let targets = [r"HKCU\Software\Classes\*\shell", r"HKCU\Software\Classes\Directory\shell"];

    for base in targets {
        let zip_key = format!(r"{}\KineticCompressZip", base);
        reg_add_key(&zip_key, None, "Kinetic: Compress to .zip")?;
        reg_add_key(&zip_key, Some("Icon"), &exe_path)?;
        reg_add_key(
            &format!(r"{}\command", zip_key),
            None,
            &format!("\"{}\" --compress-zip \"%1\"", exe_path),
        )?;

        let knt_key = format!(r"{}\KineticCompressNative", base);
        reg_add_key(&knt_key, None, "Kinetic: Compress to .kinetic")?;
        reg_add_key(&knt_key, Some("Icon"), &exe_path)?;
        reg_add_key(
            &format!(r"{}\command", knt_key),
            None,
            &format!("\"{}\" --compress-kinetic \"%1\"", exe_path),
        )?;
    }

    let ext_here_key = r"HKCU\Software\Classes\*\shell\KineticExtractHere";
    reg_add_key(ext_here_key, None, "Kinetic: Extract Here")?;
    reg_add_key(ext_here_key, Some("Icon"), &exe_path)?;
    reg_add_key(
        &format!(r"{}\command", ext_here_key),
        None,
        &format!("\"{}\" --extract-here \"%1\"", exe_path),
    )?;

    let ext_sub_key = r"HKCU\Software\Classes\*\shell\KineticExtractSub";
    reg_add_key(ext_sub_key, None, "Kinetic: Extract to Subfolder")?;
    reg_add_key(ext_sub_key, Some("Icon"), &exe_path)?;
    reg_add_key(
        &format!(r"{}\command", ext_sub_key),
        None,
        &format!("\"{}\" --extract-sub \"%1\"", exe_path),
    )?;

    Ok(query_shell_status())
}

pub fn unregister_explorer_context_menus() -> Result<ShellIntegrationStatus, String> {
    let keys = [
        r"HKCU\Software\Classes\*\shell\KineticCompressZip",
        r"HKCU\Software\Classes\*\shell\KineticCompressNative",
        r"HKCU\Software\Classes\*\shell\KineticExtractHere",
        r"HKCU\Software\Classes\*\shell\KineticExtractSub",
        r"HKCU\Software\Classes\Directory\shell\KineticCompressZip",
        r"HKCU\Software\Classes\Directory\shell\KineticCompressNative",
    ];
    for k in keys {
        reg_delete_key(k);
    }
    Ok(query_shell_status())
}

pub fn try_handle_cli_invocation(args: &[String]) -> Option<i32> {
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
}