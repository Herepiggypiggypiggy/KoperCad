# Starts the newest experimental KoperCAD build.
#
# Downloads the `experimental` pre-release (main plus every open pull
# request, built by .github/workflows/experimental.yml) when it is newer than
# the local copy, then starts it. Offline or on any download error it starts
# the last copy it has. The repository is public: nothing to sign in to and no
# GitHub CLI needed, only a normal internet connection.
#
# A build made on this PC by build-local.ps1 carries a newer `built` time than
# a stale release, so it is kept until the release catches up.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue' # Invoke-WebRequest is very slow with it
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$Repo = 'Herepiggypiggypiggy/kopercad'
$ReleaseUrl = "https://github.com/$Repo/releases/download/experimental"
$Root = Join-Path $env:LOCALAPPDATA 'NovoElectric\experimental'
$Current = Join-Path $Root 'current'
$Stamp = Join-Path $Root 'build-info.json'
# Builds from before the rename (and a copy made by build-local.ps1 of that
# time) only have novoelectric.exe.
function Get-Exe {
    foreach ($name in 'kopercad.exe', 'novoelectric.exe') {
        $path = Join-Path $Current $name
        if (Test-Path $path) { return $path }
    }
    return Join-Path $Current 'kopercad.exe'
}

New-Item -ItemType Directory -Force -Path $Root | Out-Null
function Show-Error($text) {
    Add-Type -AssemblyName PresentationFramework
    [System.Windows.MessageBox]::Show($text, 'KoperCAD') | Out-Null
}

try {
    $tmp = Join-Path $Root 'download'
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null

    $infoFile = Join-Path $tmp 'build-info.json'
    Invoke-WebRequest "$ReleaseUrl/build-info.json" -OutFile $infoFile -UseBasicParsing
    $remote = Get-Content -Raw $infoFile
    $local = if (Test-Path $Stamp) { Get-Content -Raw $Stamp } else { '' }
    # Only a release built after the local copy replaces it (ISO times compare as text).
    $remoteBuilt = try { ($remote | ConvertFrom-Json).built } catch { '' }
    $localBuilt = try { ($local | ConvertFrom-Json).built } catch { '' }
    $newer = ($remote -ne $local) -and ((-not $localBuilt) -or ($remoteBuilt -gt $localBuilt))

    if ($newer -or -not (Test-Path (Get-Exe))) {
        Invoke-WebRequest "$ReleaseUrl/novoelectric-experimental-windows.zip" -OutFile (Join-Path $tmp 'novoelectric-experimental-windows.zip') -UseBasicParsing
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
        Show-Error "Could not download the experimental build and no earlier copy exists.`n`n$($_.Exception.Message)`n`nCheck the internet connection (a proxy or firewall may block github.com)."
        exit 1
    }
}

Start-Process -FilePath (Get-Exe) -WorkingDirectory $Current
