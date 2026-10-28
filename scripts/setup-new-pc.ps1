# One-time setup of the "KoperCAD (latest)" desktop shortcut on a new PC.
# Paste the whole file into a PowerShell window (no admin needed). Everything
# goes under %LOCALAPPDATA%. The repository is public, so there is nothing to
# sign in to and no GitHub CLI to install.
#
# Fetches the launcher scripts and creates the desktop shortcut.

& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue' # Invoke-WebRequest is very slow with it
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $Repo = 'Herepiggypiggypiggy/kopercad'
    $Root = Join-Path $env:LOCALAPPDATA 'NovoElectric'
    New-Item -ItemType Directory -Force -Path $Root | Out-Null

    $tmp = Join-Path $Root 'setup'
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    foreach ($f in 'run-latest.ps1', 'install-desktop-shortcut.ps1', 'build-local.ps1') {
        try {
            Invoke-WebRequest "https://raw.githubusercontent.com/$Repo/main/scripts/$f" -OutFile (Join-Path $tmp $f) -UseBasicParsing
        } catch {
            throw "Could not download scripts/$f from GitHub: $($_.Exception.Message)"
        }
    }
    # A child process with Bypass for this run only: the default execution
    # policy blocks script files, and changing it machine-wide needs admin.
    powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $tmp 'install-desktop-shortcut.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'Creating the shortcut failed.' }
    # The installer copied the launcher into place; the downloads are no longer needed.
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    Write-Host 'Done. Double-click "KoperCAD (latest)" on the desktop.'
}
