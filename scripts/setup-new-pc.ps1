# One-time setup of the "NovoElectric (latest)" desktop shortcut on a new PC.
# Paste the whole file into a PowerShell window (no admin needed). Everything
# goes under %LOCALAPPDATA% and the user's own PATH.
#
# 1. Uses the GitHub CLI if present, otherwise unpacks a portable copy.
# 2. Signs in to GitHub in the browser (the repository is private).
# 3. Fetches the launcher scripts and creates the desktop shortcut.

& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue' # Invoke-WebRequest is very slow with it
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $Repo = 'Herepiggypiggypiggy/NovoElectric'
    $Root = Join-Path $env:LOCALAPPDATA 'NovoElectric'
    New-Item -ItemType Directory -Force -Path $Root | Out-Null

    $ghBin = Join-Path $Root 'gh\bin'
    if (Test-Path (Join-Path $ghBin 'gh.exe')) { $env:Path += ";$ghBin" }
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        Write-Host 'Installing a portable GitHub CLI for this user...'
        $rel = Invoke-RestMethod 'https://api.github.com/repos/cli/cli/releases/latest'
        $asset = $rel.assets | Where-Object { $_.name -like '*_windows_amd64.zip' } | Select-Object -First 1
        $zip = Join-Path $env:TEMP $asset.name
        Invoke-WebRequest $asset.browser_download_url -OutFile $zip -UseBasicParsing
        $ghDir = Join-Path $Root 'gh'
        Remove-Item -Recurse -Force $ghDir -ErrorAction SilentlyContinue
        Expand-Archive $zip -DestinationPath $ghDir
        Remove-Item -Force $zip
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ("$userPath" -notlike "*$ghBin*") {
            [Environment]::SetEnvironmentVariable('Path', ("$userPath".TrimEnd(';') + ";$ghBin").TrimStart(';'), 'User')
        }
        $env:Path += ";$ghBin"
    }

    # Windows PowerShell 5.1 turns redirected stderr into a terminating error
    # under 'Stop', and gh reports "not signed in" on stderr.
    $ErrorActionPreference = 'Continue'
    gh auth status *> $null
    $signedIn = $LASTEXITCODE -eq 0
    $ErrorActionPreference = 'Stop'
    if (-not $signedIn) {
        gh auth login --hostname github.com --git-protocol https --web
        if ($LASTEXITCODE -ne 0) { throw 'GitHub sign-in did not complete.' }
    }

    $tmp = Join-Path $Root 'setup'
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    foreach ($f in 'run-latest.ps1', 'install-desktop-shortcut.ps1') {
        $text = gh api -H 'Accept: application/vnd.github.raw' "repos/$Repo/contents/scripts/$f"
        if ($LASTEXITCODE -ne 0) { throw "Could not download scripts/$f from GitHub." }
        Set-Content -Encoding UTF8 -Path (Join-Path $tmp $f) -Value $text
    }
    # A child process with Bypass for this run only: the default execution
    # policy blocks script files, and changing it machine-wide needs admin.
    powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $tmp 'install-desktop-shortcut.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'Creating the shortcut failed.' }
    Write-Host 'Done. Double-click "NovoElectric (latest)" on the desktop.'
}
