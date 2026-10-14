# Installs the experimental launcher and puts "NovoElectric (latest)" on the
# desktop. Run once:  powershell -ExecutionPolicy Bypass -File scripts\install-desktop-shortcut.ps1

$ErrorActionPreference = 'Stop'
$Root = Join-Path $env:LOCALAPPDATA 'NovoElectric\experimental'
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$Launcher = Join-Path $Root 'run-latest.ps1'
Copy-Item -Force (Join-Path $PSScriptRoot 'run-latest.ps1') $Launcher

$Desktop = [Environment]::GetFolderPath('Desktop')
$Shell = New-Object -ComObject WScript.Shell
$Link = $Shell.CreateShortcut((Join-Path $Desktop 'NovoElectric (latest).lnk'))
$Link.TargetPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$Link.Arguments = "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File `"$Launcher`""
$Link.WorkingDirectory = $Root
$Link.Description = 'Start the newest experimental NovoElectric build'
$Exe = Join-Path $Root 'current\novoelectric.exe'
if (Test-Path $Exe) { $Link.IconLocation = "$Exe,0" }
$Link.Save()
Write-Host "Shortcut created on the desktop; launcher at $Launcher"
