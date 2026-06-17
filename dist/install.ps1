# Installs the spcli binary and the Claude skill on Windows.
# Run it from inside the extracted package directory:
#
#   powershell -ExecutionPolicy Bypass -File .\install.ps1
#
# Optional: -InstallDir overrides where the binary goes.

param(
  [string]$InstallDir = "$env:LOCALAPPDATA\Programs\spcli"
)

$ErrorActionPreference = "Stop"
$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$SkillsDir = Join-Path $env:USERPROFILE ".claude\skills\spcli"

$binSrc = Join-Path $Here "spcli.exe"
if (-not (Test-Path $binSrc)) {
  Write-Error "spcli.exe not found next to this script ($binSrc)"
  exit 1
}

# 1) Install the binary
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item $binSrc (Join-Path $InstallDir "spcli.exe") -Force

# 2) Install the Claude skill (global)
New-Item -ItemType Directory -Force -Path $SkillsDir | Out-Null
Copy-Item (Join-Path $Here "skills\spcli\SKILL.md") (Join-Path $SkillsDir "SKILL.md") -Force
Copy-Item (Join-Path $Here "skills\spcli\reference.md") (Join-Path $SkillsDir "reference.md") -Force

Write-Host "OK Installed spcli.exe -> $InstallDir\spcli.exe"
Write-Host "OK Installed the skill -> $SkillsDir"

# 3) Ensure the install dir is on the user PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$InstallDir*") {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
  Write-Host "OK Added $InstallDir to your user PATH (restart the terminal to pick it up)."
} else {
  Write-Host "OK $InstallDir is already on your PATH."
}

Write-Host ""
Write-Host "Next: open Claude in any project and ask something like 'list my accounts'."
Write-Host "First run, Claude will ask for the login URL (https://app.alfredorivera.dev),"
Write-Host "your email, and your TOTP secret."
