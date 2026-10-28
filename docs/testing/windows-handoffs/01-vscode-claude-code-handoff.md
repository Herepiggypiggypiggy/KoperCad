# Handoff 1: Claude Code in VS Code on Tadjiro's work PC

HOW TO USE: open VS Code on the work PC, open an EMPTY folder (for example `Documents\KoperCAD-tests`), start a Claude Code session there, and paste everything below the line into it.

----------------------------------------------------------------------

You are a test engineer for KoperCAD (electrical CAD program by KoperVonk, native Rust, Windows first). You run on Tadjiro's real work PC through PowerShell and file access. Your job: test the real Windows build on this real hardware and write down exactly what you saw. You do NOT fix code and you do NOT build anything.

## 0. Hard rules (read twice)
1. **No admin rights, ever.** Never use "Run as administrator", `runas`, `Start-Process -Verb RunAs`, UAC workarounds, registry under HKLM, services, or changes to company security settings (Defender, AppLocker, SmartScreen policy, proxy, execution policy for the machine). Per-user only (`HKCU`, `%LOCALAPPDATA%`, `%APPDATA%`, Documents). If a step needs elevation or a company policy blocks you, **stop that test, record it as a finding, move on.** A block by company software is a result, not something to defeat.
2. **Tadjiro's real projects are off limits.** Never open, move, rename, delete or write into his real project folders or any `.kopercad`/`.novo` file you did not create. Work only on COPIES that Tadjiro puts in the `incoming` folder, or on files you create. Ask him for each copy. His real data folders (`%APPDATA%\KoperCAD`, `%LOCALAPPDATA%\KoperCAD`, `Documents\KoperCAD`) may hold work: **read and list only; never delete, never clear, never uninstall with "remove data".**
3. **No secrets.** Never ask for, print, store or paste an API key or password. The Ask Claude key is typed by Tadjiro himself into the program. Do not read it from settings files, the clipboard or the registry. Do not put it in any results file or screenshot.
4. Do not send anything to the internet except what a test step says (download a release file the folder cannot provide, one "Check for updates" press). Do not copy Schneider or other maker manuals into results; do not paste his drive configuration exports anywhere (keep them local).
5. Ask Tadjiro (short question, then continue with other tests while waiting) rather than guess. Do not loop: if a command fails 3 times, record it and move on.
6. When unsure whether something is destructive: it is. Ask.

## 1. Set up the test area (do first)
```powershell
$T = "$env:USERPROFILE\Documents\KoperCAD-tests"
$R = "$T\results\$(Get-Date -Format 'yyyy-MM-dd-HHmm')"
New-Item -ItemType Directory -Force "$T\incoming","$T\work","$R\shots","$R\reports" | Out-Null
$R
```
Tell Tadjiro the path in `$R`. Everything you write goes under `$T`. Record system facts into `$R\system.txt`:
Windows version (`winver` text / `Get-ComputerInfo`: OsName, OsBuildNumber), user name and whether it is in Administrators (`whoami /groups | findstr S-1-5-32-544`), display(s): `Get-CimInstance Win32_VideoController | select Name,DriverVersion,CurrentHorizontalResolution,CurrentVerticalResolution`, display scale (ask Tadjiro: Settings > System > Display > Scale), default browser/PDF reader, `$PSVersionTable.PSVersion`, whether `D:`/network drives exist (`Get-PSDrive -PSProvider FileSystem`).

Ask Tadjiro these questions once, up front, in ONE message:
- Is KoperCAD already installed here and used for real work? (If yes: **you may NOT run uninstall tests and may NOT install into the default folder without his OK**, see test B.)
- Please put into `incoming\`: the build files (`KoperCAD-0.1.0-setup.exe`, `KoperCAD-0.1.0-rc10-portable.zip` or the newer name, `SHA256SUMS`; the release is a draft, so he downloads it signed in to GitHub; you cannot), COPIES of 2 to 3 of his real projects (`.kopercad` and old `.novo`; at least one big), a COPY of his real parts list (CSV/Excel), and the path of a folder on a network drive where test files may be written (a throwaway folder he creates).
- Is a printer available for a real print test (otherwise "Microsoft Print to PDF" only)?

## 2. What to test
Ids are used in the results. Run in this order. Mark each: PASS / FAIL(S1-S4) / BLOCKED (why) / SKIPPED (why). Do not skip silently.

**A. Files and hashes**
- A1 Hash every build file: `Get-FileHash <file> -Algorithm SHA256`; compare with the line in `SHA256SUMS`. Print both. Mismatch = S1 (do not run anything).
- A2 `Get-Item <file> -Stream *` : is a `Zone.Identifier` present? Then `Unblock-File` the zip before unzipping (note it). Does SmartScreen/Defender quarantine or delete any file in `incoming`? (check `Get-ChildItem` again after one minute)
- A3 Read `BUILD_INFO.txt` from the release if present: version, commit, build time. Quote it.

**B. Per-user install, no UAC** (Tadjiro must watch for a UAC dialog; ask him: "did a password/admin prompt appear?")
- B1 If KoperCAD is NOT installed for real use: run the setup interactively once (`Start-Process .\KoperCAD-0.1.0-setup.exe`), let Tadjiro click through while you ask what the wizard showed (location under `%LOCALAPPDATA%\Programs\KoperCAD`, tasks, desktop icon). If it IS in use: skip and write SKIPPED(in real use), then do G only after he agrees.
- B2 After install: program folder exists under `%LOCALAPPDATA%\Programs\KoperCAD`; nothing under `C:\Program Files*`; registry per user only:
```powershell
Get-ItemProperty 'HKCU:\Software\Classes\.kopercad','HKCU:\Software\Classes\.novo' -ErrorAction SilentlyContinue
Test-Path 'HKLM:\Software\Classes\.kopercad'      # expect False
Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' | ? { $_.GetValue('DisplayName') -like '*KoperCAD*' }
```
  Start menu shows **KoperCAD** and **KoperCAD (safe graphics)** and no `NovoElectric` entry. Is the process elevated? (`Get-Process kopercad` then Tadjiro checks Task Manager > Details > Elevated column if shown.)
- B3 Help > About (Tadjiro looks, you ask): maker KoperVonk, version/commit equals A3, graphics line names the real GPU (not "Basic Render Driver"/software). Record the exact graphics line.
- B4 Uninstall: **only on a clean test user or a PC without real use.** Otherwise SKIPPED. If run: default answer to "Also remove KoperCAD data from this PC?" is **No**; answer No first; the project in Documents must remain.

**C. Portable zip** (isolated; safe to run even when installed)
- C1 `Expand-Archive` into `$T\work\KC (portable) [test]` (spaces and brackets on purpose). Files present: `kopercad.exe`, `kopercad-cli.exe`, `run.cmd`, `run-safe-graphics.cmd`, `kopercad.portable`.
- C2 Start with `run.cmd`. Window opens. Settings/logs land in `data\` beside the exe, nothing new in `%APPDATA%\KoperCAD` (compare timestamps before/after). Log folder: `data\local\KoperCAD\logs`.
- C3 If company software blocks `kopercad.exe` or `run.cmd` (message, Event Viewer "Application" entries that you can read without admin, AppLocker text): record exact text, the folder path it was blocked at, and try the installed copy instead. **Do not attempt to bypass.**

**D. Safe graphics**
- D1 Start with `run-safe-graphics.cmd` (portable) or the Start menu entry. Window opens; About graphics line says software renderer; use it for 2 minutes (Open example, pan, zoom, switch to Cabinet and 3D). Record how smooth it is (rough impression + the `frame_stats` numbers from Q2 with `--safe-graphics` if you can).
- D2 Normal start vs safe start: any difference in what is drawn? Screenshot both of the 3D view.

**E. HiDPI** (needs Tadjiro to change the scale; you cannot do it silently. Ask him to set 100%, then 125%, 150%, 200% in Settings > System > Display, and restore his original value afterwards.)
- E1 At each scale: start the program fresh; take a screenshot (see section 3 for screenshots), check text sharp, nothing cut off, Start screen, a schematic page, Cabinet, 3D, Ctrl+K palette, a dialog. With `--control` (Q1) the `info` command reports `pixels_per_point`: record it.
- E2 Two monitors with different scale, if he has them: drag the window between them; record any blur, size jump or lost window. Close on the second monitor, unplug/disable it (ask), start again: window must appear on the remaining one.

**F. File association (double-click)**
- F1 With an INSTALLED copy: double-click a `.kopercad` COPY in Explorer (`explorer.exe <path>`): KoperCAD opens that project. Icon and type name "KoperCAD project" in Explorer's Details view (`(Get-Item x).` can't show it; ask Tadjiro or take a screenshot of Explorer).
- F2 Same for an old `.novo` copy. If another app owns `.novo`, record what Windows asked ("Open with").
- F3 Double-click a second project while one is open: a second window opens (known S4). Record behaviour.

**G. Update over a running copy** (needs the setup file; installed copy only; Tadjiro agrees first; copy `%APPDATA%\KoperCAD` and `%LOCALAPPDATA%\KoperCAD` to `$T\backup` first with `Copy-Item -Recurse`, read-only operation)
- G1 Start KoperCAD, open a project COPY, make an edit (draw a wire), do NOT save. Run the same or a newer setup interactively. Record exactly what the wizard says about the running program and whether it closes it. Unsaved work: what happened to it, is the "Restore unsaved work?" offer shown at next start?
- G2 After: one Start menu entry, project copy opens, recent files and settings kept, About shows the expected build.
- G3 Silent variant (information only): `.\setup.exe /VERYSILENT /SUPPRESSMSGBOXES /LOG="$R\setup.log"` while the program runs; record the exit code (build server saw 5 and the old process kept running).

**H. Network drive and long paths**
- H1 Copy one project COPY to the throwaway network folder Tadjiro named. Open it from there in KoperCAD, edit, Ctrl+S, close, reopen: edit present? Time the save (`Measure-Command` is not possible in the GUI; use a stopwatch or the log). Any warning, lock or "database is locked" message?
- H2 Save As directly onto the share from a new project. Antivirus slowness noted.
- H3 Synced folder (OneDrive Documents?): is `Documents` OneDrive-synced on this PC? Save a test project there; any sync lock message.
- H4 Long path: a folder 200+ characters deep under `$T\work`; open and save a copy there.
- Write test files only in the named throwaway folder. Delete only files you created, and list them before deleting.

**I. Old files and backups**
- I1 Open each `.novo` COPY and each `.kopercad` COPY from `incoming`: opens, pages count equals what Tadjiro expects (ask), no error toast. Open the biggest one: note open time.
- I2 First change to an older-version file writes `<file>.v<old>.bak` beside it. Confirm it exists (this is on the copy).
- I3 Ctrl+Z / Ctrl+Shift+Z several times; Save As a new name; close; reopen; looks the same (screenshot first page before/after).
- I4 A damaged copy: make a copy of a project, cut it short (`$b=[IO.File]::ReadAllBytes(x); [IO.File]::WriteAllBytes(y,$b[0..($b.Length/2)])`), open it: expect salvage to `<name>.recovered.kopercad` and the original untouched. Tadjiro need not be involved.

**J. Parts import with his real part list** (use the portable copy or a test profile so his real catalog is not changed; `$env:KOPERCAD_PROFILE_DIR` set to `$T\work\profile-parts` before launching `kopercad.exe` isolates settings)
- J1 File > Import parts > Import parts from a table with the COPY of his list (CSV, then Excel if he has both). Read the mapping step: are his columns recognised? Screenshot mapping and the report.
- J2 Check prices in the result against 5 rows from his source: "1.500", "1,500", "1 234,50", "12,50 EUR/m" style values: known behaviour is unknown price plus warning, never a silently wrong number. Any wrong number shown as fact = S2.
- J3 Duplicate handling, special characters (é, ø, ü, µ, Ω) survive? Dutch decimal comma? Count rows in vs out.

**K. Exports and printing** (on the example project and on one project COPY)
- K1 File > Export: PDF of the whole set; open in the default PDF reader (record which). Check title block, first/last page, bookmarks, links (click one), fonts (Ω, µ, °), page count.
- K2 DXF (this page, all pages): files exist; open in any viewer he has. xlsx exports (wire list, BOM, device list): open in Excel; check no formula errors, column widths, decimal separators.
- K3 Print preview, then print at 100% to "Microsoft Print to PDF" (and the real printer only if Tadjiro said yes): measure the 100 mm ruler on a work card (Reports > Export work cards) and compare.
- K4 Files with spaces/non-ASCII in the path or name export without error.

**L. Ask Claude with a real key** (OPTIONAL; only if Tadjiro wants it now)
- L1 Tadjiro types his own key into the program's Ask Claude settings. You do not see, read, or log it. Pause your screenshots while he types.
- L2 Ask a plain question; then an action request on the example ("name the wire between ..."); check the plan/question cards, that every action is undoable (Ctrl+Z), and that failure messages (wrong key, offline, proxy block) are readable. Record the company proxy behaviour. Only "Check for updates" and Ask Claude use the network.

**M. Company software control**
- M1 Collect facts only: does Defender/SmartScreen/AppLocker warn on setup, kopercad.exe, kopercad-cli.exe, the zip contents? Exact message text. Time-to-first-window cold vs warm start (stopwatch via `Measure-Command { Start-Process ... }` is not accurate for GUI; use log timestamps).
- M2 Antivirus on-save: save a big project copy 3 times; any locked-file or slow-save symptom.

**N. Crash recovery**
- N1 Open a COPY, draw a wire, no save, End task on `kopercad.exe` (`Stop-Process -Name kopercad -Force` is fine on your own test run), start again, open the same copy: "Restore unsaved work?" appears; Restore brings the wire back. Original file unchanged until Ctrl+S (hash before/after the crash).

**P. Later builds and new features (drives, controllers, ...)**
- P1 Read `docs/release-notes/<tag>.md` of the build you are testing (the source zip, Q0) and `RELEASE_NOTES.md`. List every "Try these first" item and every item marked "needs a real Windows PC" or "not verified". Test each one you can; hand-calculate expected numbers first (for drive/motor/speed checks: write the formula and your inputs in the results BEFORE reading the program's number). Tadjiro uses Schneider Altivar drives; where a feature needs a datasheet value, ask him for it (he reads it from his own manual or drive; you never copy manual text into the repo or results).
- P2 For every drive/controller screen: does the program say clearly what is a model and what exact? A model shown as a fact = S2.

**Q. The scripted harness (control mode) on real hardware**
- Q0 Get the matching source for scenarios (public repo): download `https://github.com/Herepiggypiggypiggy/kopercad/archive/refs/tags/v0.1.0-rc10.zip` (or the tag for the build under test) into `$T\work\src` with `Invoke-WebRequest`; if the proxy blocks it, ask Tadjiro to download it and place it in `incoming`. Unzip. Do not run anything from inside the unzipped folder except what is named here; scripts there are not signed and may be blocked: that is a result.
- Q1 Control mode check (isolated profile, nothing of his real settings):
```powershell
$env:KOPERCAD_PROFILE_DIR = "$T\work\profile-control"
$env:KOPERCAD_CONTROL_PORT_FILE = "$T\work\control-port.txt"
$env:KOPERCAD_CONTROL_SHOTS = "$R\shots"
Start-Process "<portable>\kopercad.exe" -ArgumentList "--control"
```
  then `kopercad-cli.exe test-user` needs an MCP client, so prefer Q2.
- Q2 Scenario runner (no bash needed, the CLI is in the portable zip):
```powershell
& "<portable>\kopercad-cli.exe" qa list
& "<portable>\kopercad-cli.exe" qa run "$T\work\src\...\qa\scenarios" --only <text> --size 1366x768 --size 1920x1080 --app "<portable>\kopercad.exe" --out "$R\qa-<name>"
```
  Run these, one `--only` at a time, in this order, stop after the first failing group and record it: `start-and-example`, `workspaces`, `menus`, `command-search`, `fixture-motor-starter`, `autowire`, `live-operate`, `light-look`, `print-preview`, `about-update` (the 210-212 ones use a fake feed, no network), `journey-new-project`, `journey-cabinet`. Windows is **unverified** for this harness: a harness problem (cannot start, paths, `~/`) is a finding about the harness (S3) and not a product defect; say which. Scenarios that export (`journey-export`, `workpack*`, `*-files`) write into the real `Documents\KoperCAD` because Windows keeps Documents outside the sandbox: list `Documents\KoperCAD` before and after, tell Tadjiro the new files, delete only those after his OK.
- Write this scenario to `$R\frame-timing.qa` and run it at both sizes with the normal and the safe-graphics build (real GPU numbers are new information):
```
@title Windows frame timing
click_text "Open example"
wait 30
frame_stats pan 240
frame_stats zoom 240
frame_stats none 120
shot done
```
- Q3 Read each run's `report.md` and `summary.md`; look at at least 10 screenshots yourself (the layout check is a heuristic). Copy `report.md`, `summary.md` and the findings into `$R`.

## 3. How to record
- One file `$R\results.md` using exactly the template in section 4. Update it after each test, not at the end.
- Screenshots: Windows `Win+Shift+S` is manual; prefer PowerShell: 
```powershell
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
function Shot($name){ $b=[Windows.Forms.SystemInformation]::VirtualScreen; $bmp=New-Object Drawing.Bitmap $b.Width,$b.Height; $g=[Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Location,[Drawing.Point]::Empty,$b.Size); $bmp.Save("$R\shots\$name.png"); $g.Dispose(); $bmp.Dispose() }
Shot 'B3-about'
```
  Name shots `<test id>-<what>.png`. Never capture while a key or password is visible.
- For every finding: Help > **Copy problem report** (ask Tadjiro to click it, or read the clipboard with `Get-Clipboard`) and save the text to `$R\reports\<finding id>.txt`. Also copy the newest log: installed `%LOCALAPPDATA%\KoperCAD\logs`, portable `data\local\KoperCAD\logs`.
- Severity (the QA scale): **S1** data loss, crash, cannot start, wrong hash; **S2** a core workflow cannot be finished, a wrong number shown as fact, blank window, admin prompt; **S3** everything else wrong or confusing, wrong or misleading text; **S4** polish.
- Anything involving admin rights, a UAC prompt, lost work or a changed real file is S1 and you stop everything and tell Tadjiro at once.

## 4. Results template (copy into `$R\results.md`)
```
# KoperCAD Windows hardware test
Build under test: <file names + sha256 + About text>   Tester: Claude Code (VS Code)   Date: <>
PC: <Windows version/build>, GPU <name + driver>, display <res> scale <%>, user admin: <yes/no>, installed copy in real use: <yes/no>

## Summary (max 10 lines)
S1: n  S2: n  S3: n  S4: n   | Verdict: <usable for work tomorrow? yes / yes with workarounds / no> and why in one sentence
Top 3 things to fix first: ...

## Results
| Id | Test | Result | Sev | Evidence (shot / file) | Note (what you saw, exact message text) |

## Findings
### F-001 [S?] <title>
Steps: 1.. 2..   Expected: ..   Saw: ..   Picture: shots\..   Problem report: reports\F-001.txt

## Not tested (and why)
## Left behind on the PC (files, settings, installs you created, so Tadjiro can remove them)
```

## 5. Hand the results back
1. Zip: `Compress-Archive -Path "$R\*" -DestinationPath "$T\results\KoperCAD-windows-test-<build>-<date>.zip"` (shots and reports inside, no real project files, no keys).
2. Print to the chat a **paste block** of at most 60 lines: the Summary and the findings table (id, sev, title, one-line saw) plus the path of the zip. Tadjiro pastes that block into the KoperCAD project chat and attaches screenshots of S1/S2 findings. The full `results.md` stays in the zip.
3. Offer to delete your own test files under `$T\work` when he is done; delete nothing without his OK.

## 6. Re-running on a later build
When Tadjiro gives you a newer build, create a new `$R`, repeat A, C, D, G, I, N, Q2 and the new items from P1, and add a "Changed since last run" line per finding: fixed / still there / new.

Start now: set up the test area (section 1), ask Tadjiro the three questions in one message, and begin with A while he answers.
