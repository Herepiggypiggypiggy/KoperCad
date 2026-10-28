# Testing KoperCAD on Tadjiro's own Windows PC

Two prompts to paste into a fresh Claude session. Each is self-contained (no other file is needed).

| File | Paste into | What it can do |
| --- | --- | --- |
| `01-vscode-claude-code-handoff.md` | Claude Code inside VS Code on the work PC | PowerShell, files, hashes, registry reads, installer logs, the scripted QA harness (`kopercad-cli qa run`), frame timing on the real GPU, results files and a zip |
| `02-computer-use-handoff.md` | Claude app with computer use / PC control | What a person sees: installer pages, SmartScreen, HiDPI scales, double-click, printing, real part lists, screenshots; no shell assumed |

Best result: run **both**. They overlap on purpose (install, portable, safe graphics, recovery) so you get two views, and split the rest: VS Code does hashes, registry, harness and timing; computer use does HiDPI, wizard pages, printing and Excel/PDF checks.

## Before you start (5 minutes, at the PC)
1. Create `Documents\KoperCAD-tests\incoming`.
2. Put in it: `KoperCAD-0.1.0-setup.exe`, `KoperCAD-0.1.0-rc10-portable.zip` (or the newer file), `SHA256SUMS` from the GitHub release (the draft release needs your GitHub login; download at home if the work proxy blocks it).
3. Put in **copies** (never the originals) of 2 to 3 real projects (`.kopercad` and an old `.novo`, one big) and of your parts list. Create a throwaway folder on the network drive and note its path.
4. If KoperCAD is already installed and you use it for real: the sessions will not run uninstall tests and will ask before updating. You can also back up `%APPDATA%\KoperCAD` and `%LOCALAPPDATA%\KoperCAD` by copying them.
5. Open the file, copy everything under the line `-----` and paste it into the new session. Answer its first questions. Stay nearby: it asks you to change display scale, type your Ask Claude key (only you, it never sees it) and answer "did an admin prompt appear?".

## Safety built into both prompts
No admin, no UAC workarounds, no bypassing company software control; your real projects and data folders are read-only for the tester (copies only); no uninstall with data removal on a working install; no secrets in any file; Schneider manuals and your drive exports are never copied into results.

## What comes back
A short paste block (summary + findings table, max 60 lines) that you paste into the KoperCAD project chat, plus (VS Code) a zip `Documents\KoperCAD-tests\results\KoperCAD-windows-test-<build>-<date>.zip` with the full `results.md`, screenshots, problem reports and QA reports. Severity scale S1 (data loss, crash, cannot start, admin prompt) to S4 (polish), same as the QA reports. Attach screenshots of S1 and S2 findings yourself.

## Later builds
Give the session the new build files and say "run again on this build". Both prompts have a section for re-runs that marks each earlier finding fixed, still there or new, and tests the features named in the new release notes (drives and controllers as they land: the tester calculates the expected number first, then compares).

## What the prompts reuse
`qa/pre-work/windows.md` (the 12 things only hardware can prove), `docs/FIRST-DAY.md`, `docs/WINDOWS-CLEAN-PROFILE.md`, `docs/WINDOWS-CHECK.md`, `docs/testing/qa-suite.md` and the `--control` harness. The harness is **not yet verified on Windows**: the VS Code session treats harness trouble as a finding about the harness, not the product.
