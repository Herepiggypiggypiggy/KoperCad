# Starts the newest experimental KoperCAD build.
#
# Downloads the `experimental` pre-release (main plus every open pull
# request, built by .github/workflows/experimental.yml) when it is newer than
# the local copy, then starts it. Offline or on any download error it starts
# the last copy it has. Needs the GitHub CLI (`gh`) signed in, because the
# repository is private.
#
# A build made on this PC by build-local.ps1 carries a newer `built` time than
# a stale release, so it is kept until the release catches up.

$ErrorActionPreference = 'Stop'
$Repo = 'Herepiggypiggypiggy/kopercad'
$Root = Join-Path $env:LOCALAPPDATA 'NovoElectric\experimental'
$Current = Join-Path $Root 'current'
$Stamp = Join-Path $Root 'build-info.json'
# Builds from before the rename only have novoelectric.exe.
function Get-Exe {
    foreach ($name in 'kopercad.exe', 'novoelectric.exe') {
        $path = Join-Path $Current $name
        if (Test-Path $path) { return $path }
    }
    return Join-Path $Current 'kopercad.exe'
}

New-Item -ItemType Directory -Force -Path $Root | Out-Null
# The portable gh that scripts/setup-new-pc.ps1 unpacks, in case the
# shortcut starts before Explorer has picked up the new user PATH.
$PortableGh = Join-Path $env:LOCALAPPDATA 'NovoElectric\gh\bin'
if (Test-Path (Join-Path $PortableGh 'gh.exe')) { $env:Path += ";$PortableGh" }

function Show-Error($text) {
    Add-Type -AssemblyName PresentationFramework
    [System.Windows.MessageBox]::Show($text, 'KoperCAD') | Out-Null
}

try {
    $tmp = Join-Path $Root 'download'
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null

    gh release download experimental --repo $Repo --pattern 'build-info.json' --dir $tmp --clobber
    if ($LASTEXITCODE -ne 0) { throw 'gh could not reach the experimental release' }

    $remote = Get-Content -Raw (Join-Path $tmp 'build-info.json')
    $local = if (Test-Path $Stamp) { Get-Content -Raw $Stamp } else { '' }
    # Only a release built after the local copy replaces it (ISO times compare as text).
    $remoteBuilt = try { ($remote | ConvertFrom-Json).built } catch { '' }
    $localBuilt = try { ($local | ConvertFrom-Json).built } catch { '' }
    $newer = ($remote -ne $local) -and ((-not $localBuilt) -or ($remoteBuilt -gt $localBuilt))

    if ($newer -or -not (Test-Path (Get-Exe))) {
        gh release download experimental --repo $Repo --pattern 'novoelectric-experimental-windows.zip' --dir $tmp --clobber
        if ($LASTEXITCODE -ne 0) { throw 'download failed' }
        $next = Join-Path $Root 'next'
        Remove-Item -Recurse -Force $next -ErrorAction SilentlyContinue
        Expand-Archive -Path (Join-Path $tmp 'novoelectric-experimental-windows.zip') -DestinationPath $next
        Remove-Item -Recurse -Force $Current -ErrorAction SilentlyContinue
        # A running copy of the app cannot be deleted; keep it and retry next start.
        if (Test-Path $Current) { throw 'KoperCAD is running; keeping the current build' }
        Move-Item $next $Current
        Set-Content -NoNewline -Path $Stamp -Value $remote
    }
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
catch {
    if (-not (Test-Path (Get-Exe))) {
        Show-Error "Could not download the experimental build and no earlier copy exists.`n`n$($_.Exception.Message)`n`nCheck that 'gh auth status' works."
        exit 1
    }
}

Start-Process -FilePath (Get-Exe) -WorkingDirectory $Current
