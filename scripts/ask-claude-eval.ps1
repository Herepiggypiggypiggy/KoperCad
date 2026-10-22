# One command: run the Ask Claude co-engineer test with your own API key (about 14 scenarios,
# roughly $2 to $5 on Opus, cents on Sonnet). No admin rights. The key is read from the prompt,
# used only in this window and never written to a file or the report.
#
#   .\scripts\ask-claude-eval.ps1                 # asks for the key, runs all scenarios
#   .\scripts\ask-claude-eval.ps1 -Only create-dol-starter
#   .\scripts\ask-claude-eval.ps1 -Scripted       # no key, no cost: only checks the test setup
param(
  [string[]]$Only = @(),
  [string]$Model = "",
  [double]$Budget = 0.60,
  [switch]$Scripted,
  [string]$Out = "$env:USERPROFILE\Documents\KoperCAD-ask-claude-test"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$cli = @("$root\kopercad-cli.exe", "$PSScriptRoot\kopercad-cli.exe", "$root\target\release\kopercad-cli.exe", "$root\target\debug\kopercad-cli.exe") |
  Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $cli) { throw "kopercad-cli.exe not found. Unzip the portable build and run this script from its scripts folder." }
$hadKey = [bool]$env:ANTHROPIC_API_KEY
$argv = @("ask-claude-eval", "--out", $Out, "--budget", $Budget)
foreach ($o in $Only) { $argv += @("--only", $o) }
if ($Model) { $argv += @("--model", $Model) }
if (-not $Scripted) {
  $argv += "--real"
  if (-not $env:ANTHROPIC_API_KEY) {
    $sec = Read-Host "Anthropic API key (starts with sk-ant-, typing is hidden)" -AsSecureString
    $env:ANTHROPIC_API_KEY = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec))
  }
}
try {
  & $cli @argv
  $code = $LASTEXITCODE
} finally {
  # The key typed above lives only for this run: do not leave it in the caller's session.
  if (-not $hadKey) { Remove-Item Env:\ANTHROPIC_API_KEY -ErrorAction SilentlyContinue }
}
Write-Host "Report folder: $Out"
exit $code
