# Repairs the Phase 12 edits that silently failed because "\r\n" in PowerShell is literal text.
# Run from the project root:  powershell -ExecutionPolicy Bypass -File .\repair-phase12.ps1
$ErrorActionPreference = "Stop"
$root = "C:\projects\kinetic compression"
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Edit-File([string]$Rel, [string]$Old, [string]$New, [string]$AlreadyDone) {
    $path = Join-Path $root $Rel
    $text = [System.IO.File]::ReadAllText($path).Replace("`r`n", "`n")   # normalize to LF
    if ($text.Contains($AlreadyDone)) { Write-Host "SKIP (already applied): $Rel" -ForegroundColor Yellow; return }
    if (-not $text.Contains($Old))    { throw "PATTERN NOT FOUND in $Rel :`n$Old" }
    $text = $text.Replace($Old, $New)
    [System.IO.File]::WriteAllText($path, $text, $utf8NoBom)
    Write-Host "OK: $Rel" -ForegroundColor Green
}

# state.rs: initialise the new field (the struct field itself was added; the initialiser was not)
Edit-File "src-tauri\src\state.rs" `
  "            job_controller,`n        }" `
  "            job_controller,`n            startup_archive_path: Mutex::new(None),`n        }" `
  "startup_archive_path: Mutex::new"

# ArchiveExplorer.tsx: startup hook
$hook = @'
  useEffect(() => {
    invoke<string | null>("get_startup_archive_cmd")
      .then((startupPath) => {
        if (startupPath) {
          setArchivePath(startupPath);
          loadArchive(startupPath);
        }
      })
      .catch(() => {});
  }, []);

'@.Replace("`r`n", "`n")
Edit-File "src\components\ArchiveExplorer.tsx" `
  "  useEffect(() => {`n    const closeMenu = () => setContextMenu(null);" `
  ($hook + "  useEffect(() => {`n    const closeMenu = () => setContextMenu(null);") `
  "get_startup_archive_cmd"

# App.tsx: switch to explorer on startup archive
$app = @'
  useEffect(() => {
    refreshStatus();
    invoke<string | null>("get_startup_archive_cmd")
      .then((startupPath) => {
        if (startupPath) {
          setActiveSection("explorer");
        }
      })
      .catch(() => {});
  }, [refreshStatus]);
'@.Replace("`r`n", "`n")
Edit-File "src\App.tsx" `
  "  useEffect(() => {`n    refreshStatus();`n  }, [refreshStatus]);" `
  $app `
  "get_startup_archive_cmd"

Write-Host "`nRepairs done. Now run: npm run build ; cargo test --workspace" -ForegroundColor Cyan
