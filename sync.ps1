
param(
    [Parameter(Mandatory=$true)]
    [string]$Message
)

$ErrorActionPreference = "Stop"

Write-Host "`n=== KINETIC GIT SYNC ===" -ForegroundColor Cyan

# Verify Git repository
git rev-parse --is-inside-work-tree

if ($LASTEXITCODE -ne 0) {
    throw "This folder is not a Git repository."
}

# Check remote
$remote = git remote get-url origin 2>$null

if ($LASTEXITCODE -ne 0) {
    throw "No GitHub remote named origin is configured."
}

Write-Host "GitHub: $remote" -ForegroundColor Green

# Stage changes
git add -A

# Check for changes
git diff --cached --quiet

if ($LASTEXITCODE -eq 0) {
    Write-Host "No changes to upload." -ForegroundColor Yellow
    exit 0
}

# Review changes
git status --short

Write-Host "`nCommit message: $Message" -ForegroundColor Cyan

$confirm = Read-Host "Commit and push these changes? (y/n)"

if ($confirm -ne "y") {
    Write-Host "Sync cancelled." -ForegroundColor Yellow
    exit 0
}

# Commit
git commit -m $Message

if ($LASTEXITCODE -ne 0) {
    throw "Git commit failed."
}

# Push
git push

if ($LASTEXITCODE -ne 0) {
    throw "Git push failed. Your local commit is preserved."
}

Write-Host "`nKinetic successfully synced to GitHub!" -ForegroundColor Green