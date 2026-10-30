# Add-on signing for a standard Windows user: no admin rights, no network.
# A thin wrapper over `kopercad-cli addons keygen | sign | verify`; the guide is
# docs/design/addons-signing.md.
#
#   scripts\sign-addon.ps1 keygen                       # once; makes %LOCALAPPDATA%\KoperCAD\keys\koper-vonk-<year>.key
#   scripts\sign-addon.ps1 sign   FILE.kcaddon          # signs in place
#   scripts\sign-addon.ps1 verify FILE.kcaddon          # checks against addons\keys\kopervonk.pub
#   scripts\sign-addon.ps1 secret                       # copies the GitHub secret value to the clipboard
#
# The private key lives in %LOCALAPPDATA%\KoperCAD\keys (access limited to you, outside the
# repository). `sign` uses -Key, else the base64 text in $env:ADDON_SIGNING_KEY (what CI has),
# else the single *.key file in that folder. The key is never printed.
#
# Extra flags go to the CLI: -Id koper-vonk-2026, -Owner KoperVonk, -ValidFrom 2026-11-01,
# -Date 2026-11-05, -Replace.

param(
    [Parameter(Position = 0, Mandatory = $true)][ValidateSet('keygen', 'sign', 'verify', 'secret')][string]$Action,
    [Parameter(Position = 1)][string]$File,
    [string]$Key,
    [string]$Id,
    [string]$Owner = 'KoperVonk',
    [string]$ValidFrom,
    [string]$Date,
    [switch]$Replace,
    [string]$Keys
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
# Not Documents: that folder is often synced to OneDrive, and a private key must not be.
$KeyDir = Join-Path $env:LOCALAPPDATA 'KoperCAD\keys'

function Invoke-Cli {
    param([string[]]$CliArgs)
    $exe = $env:KOPERCAD_CLI
    if (-not $exe) {
        $onPath = Get-Command kopercad-cli -ErrorAction SilentlyContinue
        if ($onPath) { $exe = $onPath.Source }
    }
    if (-not $exe) {
        foreach ($sub in 'release', 'debug') {
            $candidate = Join-Path $Root "target\$sub\kopercad-cli.exe"
            if (Test-Path $candidate) { $exe = $candidate; break }
        }
    }
    if ($exe) {
        & $exe @CliArgs
    } else {
        & cargo run -q --release -p novo-cli --manifest-path (Join-Path $Root 'Cargo.toml') -- @CliArgs
    }
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

function Get-OnlyKey {
    $found = @(Get-ChildItem -Path $KeyDir -Filter *.key -ErrorAction SilentlyContinue)
    if ($found.Count -eq 1) { return $found[0].FullName }
    return $null
}

switch ($Action) {
    'keygen' {
        if (-not $Id) { $Id = 'koper-vonk-' + (Get-Date).ToUniversalTime().Year }
        $out = if ($Key) { $Key } else { Join-Path $KeyDir "$Id.key" }
        $cliArgs = @('addons', 'keygen', '--id', $Id, '--owner', $Owner, '--out', $out)
        if ($ValidFrom) { $cliArgs += @('--valid-from', $ValidFrom) }
        Invoke-Cli $cliArgs
    }
    'sign' {
        if (-not $File) { throw 'usage: sign-addon.ps1 sign FILE.kcaddon' }
        $cliArgs = @('addons', 'sign', $File)
        if ($Key) {
            $cliArgs += @('--key', $Key)
        } elseif ($env:ADDON_SIGNING_KEY) {
            $cliArgs += @('--key-env', 'ADDON_SIGNING_KEY')
        } else {
            $only = Get-OnlyKey
            if (-not $only) { throw "No key: pass -Key FILE, set ADDON_SIGNING_KEY, or keep exactly one .key in $KeyDir" }
            $cliArgs += @('--key', $only)
        }
        if ($Date) { $cliArgs += @('--date', $Date) }
        if ($Replace) { $cliArgs += '--replace' }
        Invoke-Cli $cliArgs
    }
    'verify' {
        if (-not $File) { throw 'usage: sign-addon.ps1 verify FILE.kcaddon' }
        $cliArgs = @('addons', 'verify', $File)
        $pub = if ($Keys) { $Keys } else { Join-Path $Root 'addons\keys\kopervonk.pub' }
        if (Test-Path $pub) { $cliArgs += @('--keys', $pub) }
        Invoke-Cli $cliArgs
    }
    'secret' {
        $path = if ($Key) { $Key } else { Get-OnlyKey }
        if (-not $path -or -not (Test-Path $path)) { throw "No key file: pass -Key FILE, or keep exactly one .key in $KeyDir" }
        # Base64 of the key file; goes to the clipboard, never to the screen or a file.
        $value = [Convert]::ToBase64String([IO.File]::ReadAllBytes($path))
        Set-Clipboard -Value $value
        Write-Host 'The value for the GitHub secret ADDON_SIGNING_KEY is on the clipboard (not shown).'
        Write-Host 'Paste it in GitHub (Settings > Secrets and variables > Actions), then copy something else.'
    }
}
