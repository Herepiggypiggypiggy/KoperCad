# Builds NovoElectric on this PC and refreshes the "NovoElectric (latest)"
# shortcut's copy, so the newest build needs no GitHub Actions minutes.
#
#   powershell -ExecutionPolicy Bypass -File scripts\build-local.ps1 [-Ref main] [-NoLaunch]
#
# No admin rights: everything lives under %LOCALAPPDATA%\NovoElectric\local-build
# and the process environment. It uses the GNU Rust target with a portable
# MinGW-w64 (WinLibs) because the MSVC target needs the Visual Studio Build
# Tools, whose installer asks for elevation. Needs the GitHub CLI signed in
# (the repository is private); scripts\setup-new-pc.ps1 sets that up.
#
# First run downloads rustup (about 15 MB, then about 300 MB of Rust) and
# WinLibs (about 250 MB) and compiles every dependency; later runs reuse
# them and rebuild only what changed.
#
# -Ref picks the branch, tag or pull request to build: main (default),
# claude/some-branch or pull/175/head. Open pull requests are not merged into
# main here; the release built by Actions does that.

param(
    [string]$Ref = 'main',
    [switch]$NoLaunch
)

# The shortcut opens a console that would vanish on an error; keep it open.
trap {
    Write-Host "`nBuild failed: $($_.Exception.Message)" -ForegroundColor Red
    if (-not $NoLaunch) { Read-Host 'Press Enter to close' }
    exit 1
}
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$Repo = 'Herepiggypiggypiggy/NovoElectric'
$Base = Join-Path $env:LOCALAPPDATA 'NovoElectric'
$Work = Join-Path $Base 'local-build'
$Target = 'x86_64-pc-windows-gnu'
$Experimental = Join-Path $Base 'experimental'
New-Item -ItemType Directory -Force -Path $Work, $Experimental | Out-Null

$portableGh = Join-Path $Base 'gh\bin'
if (Test-Path (Join-Path $portableGh 'gh.exe')) { $env:Path += ";$portableGh" }
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw 'The GitHub CLI (gh) is missing. Run scripts\setup-new-pc.ps1 first.'
}

function Invoke-Checked([string]$what, [scriptblock]$run) {
    & $run
    if ($LASTEXITCODE -ne 0) { throw "$what failed (exit code $LASTEXITCODE)" }
}

# 1. Rust, installed for this user only and kept out of the system PATH.
$env:RUSTUP_HOME = Join-Path $Work 'rustup'
$env:CARGO_HOME = Join-Path $Work 'cargo'
$cargoBin = Join-Path $env:CARGO_HOME 'bin'
$env:Path = "$cargoBin;$env:Path"
if (-not (Test-Path (Join-Path $cargoBin 'cargo.exe'))) {
    Write-Host 'Installing Rust for this user...'
    $init = Join-Path $Work 'rustup-init.exe'
    Invoke-WebRequest 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -OutFile $init -UseBasicParsing
    Invoke-Checked 'rustup-init' { & $init -y --no-modify-path --profile minimal --default-host $Target --default-toolchain stable }
    Remove-Item -Force $init
}
else {
    Invoke-Checked 'rustup update' { rustup update stable --no-self-update }
}

# 2. A portable MinGW-w64 toolchain: the linker for the GNU target and the C
# compiler that rusqlite's bundled SQLite needs.
$mingw = Join-Path $Work 'mingw64'
if (-not (Test-Path (Join-Path $mingw 'bin\gcc.exe'))) {
    Write-Host 'Downloading a portable MinGW-w64 (WinLibs)...'
    $rel = Invoke-RestMethod 'https://api.github.com/repos/brechtsanders/winlibs_mingw/releases/latest'
    $asset = $rel.assets |
        Where-Object { $_.name -match '^winlibs-x86_64-posix-seh-gcc-[\d\.]+-mingw-w64ucrt-[\d\.]+-r\d+\.zip$' } |
        Select-Object -First 1
    if (-not $asset) { throw 'Could not find a WinLibs zip in the latest release.' }
    $zip = Join-Path $Work $asset.name
    Invoke-WebRequest $asset.browser_download_url -OutFile $zip -UseBasicParsing
    $unpack = Join-Path $Work 'mingw-unpack'
    Remove-Item -Recurse -Force $unpack -ErrorAction SilentlyContinue
    Expand-Archive -Path $zip -DestinationPath $unpack
    Remove-Item -Recurse -Force $mingw -ErrorAction SilentlyContinue
    Move-Item (Join-Path $unpack 'mingw64') $mingw
    Remove-Item -Recurse -Force $unpack, $zip
}
$env:Path = "$(Join-Path $mingw 'bin');$env:Path"

# 3. The source: a tarball of the ref through gh (works for the private repo;
# no git needed). tar.exe ships with Windows 10 and later.
Write-Host "Fetching $Ref..."
$src = Join-Path $Work 'src'
$archive = Join-Path $Work 'source.tar.gz'
Remove-Item -Recurse -Force $src -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $src | Out-Null
# Not `gh api ... > file`: Windows PowerShell 5.1 would re-encode the bytes.
$token = (gh auth token).Trim()
if (-not $token) { throw 'gh is not signed in. Run: gh auth login --web' }
Invoke-WebRequest "https://api.github.com/repos/$Repo/tarball/$Ref" -Headers @{ Authorization = "Bearer $token" } -OutFile $archive -UseBasicParsing
Invoke-Checked 'unpack' { tar -xzf $archive -C $src --strip-components=1 }
Remove-Item -Force $archive
# The tarball keeps commit times. Cargo skips a file that is older than its
# last build, so a change from an earlier commit would ship a stale binary.
# Stamp everything now; only the workspace crates recompile, not dependencies.
$now = Get-Date
Get-ChildItem -Recurse -Force -File $src | ForEach-Object { $_.LastWriteTime = $now }
$commit = (gh api "repos/$Repo/commits/$Ref" --jq '.sha').Trim()

# 4. Build. The target folder is kept between runs so only changes recompile.
# No LTO and 16 codegen units, as in the experimental release: the build is
# unverified anyway and links faster this way.
$env:CARGO_TARGET_DIR = Join-Path $Work 'target'
$env:CARGO_PROFILE_RELEASE_LTO = 'false'
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = '16'
Write-Host 'Building (the first run takes a while)...'
Push-Location $src
try { Invoke-Checked 'cargo build' { cargo build --release -p novo-app --target $Target } }
finally { Pop-Location }
$built = Join-Path $env:CARGO_TARGET_DIR "$Target\release\novoelectric.exe"
if (-not (Test-Path $built)) { throw 'The build finished without novoelectric.exe.' }

# 5. Put it where run-latest.ps1 starts it from, with a newer `built` time so
# the stale release does not replace it.
$stamp = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
$next = Join-Path $Experimental 'next'
Remove-Item -Recurse -Force $next -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $next | Out-Null
Copy-Item $built (Join-Path $next 'novoelectric.exe')
Set-Content -Path (Join-Path $next 'BUILD_INFO.txt') -Value "NovoElectric local build $stamp`nSource: $Ref at $commit`nBuilt on this PC with the GNU toolchain."
$current = Join-Path $Experimental 'current'
Remove-Item -Recurse -Force $current -ErrorAction SilentlyContinue
if (Test-Path $current) { throw 'NovoElectric is running; close it and run this again.' }
Move-Item $next $current
$info = @{ built = $stamp; source = 'local'; ref = $Ref; commit = $commit } | ConvertTo-Json -Compress
Set-Content -NoNewline -Path (Join-Path $Experimental 'build-info.json') -Value $info
Write-Host "Done: $Ref at $($commit.Substring(0, 7))."

if (-not $NoLaunch) {
    Start-Process -FilePath (Join-Path $current 'novoelectric.exe') -WorkingDirectory $current
}
