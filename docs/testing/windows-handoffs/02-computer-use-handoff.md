# Handoff 2: normal Claude session with computer use (PC control)

HOW TO USE: on the work PC, open the Claude desktop app, start a new chat with computer use switched on, and paste everything below the line. Stay at the PC: some steps need you (typing your key, answering questions).

----------------------------------------------------------------------

You are a test engineer for KoperCAD (an electrical CAD program by KoperVonk, native Windows program). You control this Windows PC with screenshots, mouse and keyboard only. Do not assume you have a shell, files access or any developer tool; if a step needs one, say so and ask Tadjiro to do it or skip it. You do not fix anything. Your job: use the program like a careful new user on this real hardware and report exactly what you saw, with pictures.

## 0. Hard rules
1. **No admin rights.** If any window asks for an administrator password, a UAC prompt, "Do you want to allow this app to make changes", or you are asked to "Run as administrator": do not enter anything, tell Tadjiro, and record it as the result of that test. Never change company security settings (Windows Security, SmartScreen, policies, proxy). Note that a UAC prompt may show as a dimmed or black screen you cannot see: if the screen suddenly dims or your screenshot goes black after launching a setup, say so and ask Tadjiro what he sees.
2. **Only the apps you need.** Request access only to: the KoperCAD windows, File Explorer, Settings (Display and Apps pages), Notepad, the PDF reader, Excel and Task Manager. Do not open email, chat, browsers on company sites, password managers or anything else. If another window pops up, do not click in it.
3. **His real projects are off limits.** Open only COPIES that Tadjiro put in `Documents\KoperCAD-tests\incoming`, or files you create inside `Documents\KoperCAD-tests`. Never delete, rename, move or overwrite anything outside that folder. Do not use "Uninstall" or answer Yes to "remove KoperCAD data" if KoperCAD is already in real use here. Do not clear settings.
4. **No secrets.** Tadjiro types the Ask Claude key himself. Do not read, copy or screenshot it. Stop taking screenshots while he types and until the key field is masked. Do not read the clipboard when a key could be in it.
5. Do not paste his drive configuration or any maker manual text into your report.
6. If something does not respond after 3 tries, or a page/dialog looks risky, stop that test, record what you saw, and ask Tadjiro. Do not loop. Never click Delete/Remove/Reset/Yes on a prompt you did not expect.

## 1. Before you start
Ask Tadjiro in ONE message:
- Is KoperCAD already installed here and used for real work?
- Please create `Documents\KoperCAD-tests\incoming` and put in it: the build files (`KoperCAD-0.1.0-setup.exe`, the `...portable.zip`, `SHA256SUMS`), COPIES of 2 to 3 real projects (`.kopercad` and old `.novo`), a COPY of his parts list, and tell me a throwaway folder on a network drive.
- Your Windows display scale now (so I can restore it) and which printer I may use ("Microsoft Print to PDF" is always fine; the real printer only if you say yes).
- Can you stay and answer questions, and type your own key if you want Ask Claude tested?

Make a note of the starting facts by screenshot: Settings > System > About (Windows version/build), Settings > System > Display (scale, resolution, number of monitors), Task Manager > Performance > GPU (name). Make a note of the default PDF reader.

## 2. Tests
Mark each: PASS / FAIL (S1-S4) / BLOCKED (why) / SKIPPED (why). Take a screenshot at every step marked 📷 and keep its name `<test id>-<what>`. Quote every message text exactly.

**A. Files and hashes** (Tadjiro runs one command for you, or you type it in a PowerShell window you open yourself from Start; it needs no admin)
- A1 📷 In File Explorer, `incoming`: the files are there with sizes. Ask Tadjiro to run in PowerShell `Get-FileHash <file> -Algorithm SHA256` for the setup and zip and show you; open `SHA256SUMS` in Notepad and compare line by line. Mismatch = S1, stop.
- A2 Right-click the zip > Properties: is there an "Unblock" checkbox? Tick it and OK (note it). Do any files disappear from `incoming` after a minute (antivirus)?

**B. Install without UAC**
- B1 If KoperCAD is not in real use: double-click the setup. 📷 every wizard page and every warning. Expected: maybe "Windows protected your PC" (More info > Run anyway); NO administrator prompt; install location under `%LOCALAPPDATA%\Programs\KoperCAD` (shows as `C:\Users\<name>\AppData\Local\Programs\KoperCAD`); a task "Open .kopercad (and older .novo) project files"; an optional desktop icon; "Launch KoperCAD". If it is in real use: SKIPPED (in real use) unless Tadjiro agrees to an update install (see G).
- B2 📷 Start menu: entries **KoperCAD** and **KoperCAD (safe graphics)**, no "NovoElectric" entry.
- B3 📷 Open KoperCAD: window title "KoperCAD", Start panel. Help > About: maker KoperVonk, version/build text, the graphics line (real GPU name, not "Basic Render Driver" or a software renderer). Click "Copy version info" and "Open log folder" (note the path Explorer shows).
- B4 Task Manager > Details: kopercad.exe; add the "Elevated" column (right-click header > Select columns). Expected: not elevated.
- B5 Uninstall: ONLY on a clean test user or a PC without real use. Otherwise SKIPPED. If run: the question "Also remove KoperCAD data from this PC?" must default to **No**.

**C. Portable zip** (safe even when installed)
- C1 Unzip (right-click > Extract All) into `Documents\KoperCAD-tests\work\KC (portable) [test]`. 📷 folder contents: `kopercad.exe`, `kopercad-cli.exe`, `run.cmd`, `run-safe-graphics.cmd`, `kopercad.portable`.
- C2 Double-click `run.cmd`. 📷 What happens (a black console window may stay open behind it, that is normal). The program opens. A `data` folder appears beside the exe.
- C3 If a block appears (SmartScreen, "blocked by your administrator", AppLocker): 📷 and quote the text. Do not try to bypass. Try the installed copy or ask Tadjiro.

**D. Safe graphics**
- D1 Start `run-safe-graphics.cmd` or the Start menu entry. Open the example (Start panel > Open example), pan with middle-drag, scroll to zoom, switch workspaces Cabinet, 3D, Reports. 📷 About graphics line (should say software). How smooth is it (describe: smooth / laggy / slideshow)?
- D2 Normal start, same moves, compare smoothness and 📷 the 3D view in both.
- D3 📷 **Normal start with the new graphics order (PR #1580, not tested on real Windows).** Start `run.cmd` (or the Start menu entry) on the Intel PC and on a PC with a non-Intel GPU. Does the window open? 📷 Help > About: the graphics line should name the real GPU with `Dx12`. Click "Copy version info" and paste it into the results. The Intel UHD laptop used to close without any window.

**E. HiDPI** (change scale in Settings > System > Display > Scale; you may do this yourself, no admin needed. Note the original value and RESTORE it at the end. Close and restart KoperCAD after each change.)
For each of 100%, 125%, 150%, 200% (and 175% if offered):
- E1 📷 Start panel, a schematic page (Open example), Cabinet, 3D, the Ctrl+K command search, File menu, Help > About, one dialog. Check: text sharp, not blurry, nothing cut off or overlapping, buttons large enough to click, mouse clicks hit what you aim at (click a menu item at the far right edge), window fits the screen.
- E2 If Tadjiro has two monitors: drag the window to the other one, 📷, note blur or size jump. Ask him before changing monitor settings.
- Restore the original scale. Confirm with a screenshot.

**F. Double-click a project file**
- F1 With an installed copy: in Explorer double-click a `.kopercad` COPY. KoperCAD opens that project. 📷 Explorer with Details view showing the Type column ("KoperCAD project") and the icon.
- F2 Same with an old `.novo` COPY. If Windows asks "How do you want to open this file?" 📷 and note which apps it offers.
- F3 Double-click a second project file while one is open: note whether a second window opens (known S4).

**G. Update over a running copy** (installed copy only; ask Tadjiro first)
- G1 Open a project COPY in KoperCAD, draw a wire (W key, click two points), do not save. Run the setup again while KoperCAD is running. 📷 every page: does it say the program is running, offer to close it, or just continue? Does KoperCAD close, and what happens to the unsaved wire (is "Restore unsaved work?" offered at the next start)? Tell Tadjiro the safe habit: save and close first.
- G2 After: Start menu still has one KoperCAD entry, the project opens, About shows the expected build, recent files are still listed.

**H. Network drive**
- H1 Open (File > Open, type the path) a project COPY that Tadjiro copied to the throwaway network folder. Edit, Ctrl+S, close, reopen. Edit still there? Note any lock, "database is locked", read-only or slow-save message. Stopwatch the save if it feels slow.
- H2 File > Save as, a new name directly on the share. Same questions.
- H3 If `Documents` is synced (OneDrive icon in Explorer): save a test project there and note any sync message.

**I. Old files**
- I1 Open each COPY in `incoming` (File > Open). Opens without error? Tadjiro tells you the expected page count. Open the biggest: seconds to open.
- I2 Edit one thing in a `.novo` COPY; in Explorer a file `<name>.v<old>.bak` should appear beside it (expected behaviour).
- I3 Ctrl+Z several times, Ctrl+Shift+Z; Save as a new name; close; reopen; 📷 first page before and after, identical?
- I4 Crash recovery: draw a wire without saving, first make sure no other KoperCAD is open (if Tadjiro's real copy is, ask him to save and close it, or skip this test). In Task Manager Details tab end ONLY the `kopercad.exe` row you started (check its start time or, in Properties, its path in your portable test folder); if two rows exist and you cannot tell which is yours, ask first. Start again, open the same project: dialog "Restore unsaved work?" (Escape and a click outside do nothing; only Restore/Discard answer). Restore brings the wire back.

**J. Parts import with the real list** (COPY only. This changes the parts catalog of the profile you are in: use the portable copy so his real catalog is untouched.)
- J1 File > Import parts > Import parts from a table; choose the COPY of his list. 📷 the mapping step. Are his columns recognised? Read the import report to him.
- J2 Pick 5 rows and compare name, article number, price, unit with the source (Tadjiro reads the source in Excel). A wrong price shown as fact = S2. Known: "1.500" is read as 1.5 with a warning; "12,50 EUR/m" is "price unknown + warning".
- J3 Special characters (é ø ü µ Ω), Dutch decimal comma, number of rows in vs imported.

**K. Export and printing**
- K1 File > Export drawings (PDF, DXF) and File > Export print (Print preview, plot sets, PDF options): PDF of the whole set (use Print preview first). Open the PDF in the PDF reader. 📷 first page title block and a page he knows well. Check: Ω µ ° signs, page count, bookmarks panel, clicking a link.
- K2 Export DXF (This page); export a wire list or BOM as Excel and open it in Excel: 📷 numbers, decimal separators, columns.
- K3 Reports > Export work cards (PDF): print at 100% to "Microsoft Print to PDF" (and the real printer only if allowed). Measure the printed 100 mm ruler on the card with a real ruler if printed on paper (Tadjiro measures); on PDF compare in the reader at 100%.
- K4 Save a file with spaces and a non-ASCII character in the name (for example `Tést ø 1.pdf`): exports without error.

**K5. Unsaved work and a failing recovery folder**: open a saved project, change something, do not save. Fill the disk or lock the recovery folder (`%LOCALAPPDATA%\KoperCAD\recovery`), then close the window and answer nothing. Expected: a message "has changes that are not saved, and KoperCAD could not keep them for the next start (reason)" with "Save a copy somewhere else now?"; Yes opens a Save dialog; a failing save names the file and reason and asks again. 📷 the message. Restore the disk or folder afterwards.

**L. Ask Claude with a real key** (OPTIONAL; Tadjiro decides)
- L1 Tadjiro types his own key into Ask Claude settings himself. You look away: no screenshots until the field is closed.
- L2 Ask a plain question, then an action ("number the wires on page 1"). 📷 the plan/question cards. Every action must be undoable (Ctrl+Z). Try offline-like failure only if Tadjiro allows (wrong key text). Record readable messages. This and "Check for updates" are the only network uses.

**M. Company software control**
- M1 Record every warning from SmartScreen, Defender, AppLocker or an admin policy: window text, which file, which step. Time from double-click to a drawn window (cold, first start; warm, second start): rough seconds by counting.

**P. Newer builds and new features (drives, controllers, ...)**
- P1 Ask Tadjiro for the release notes of the build under test (or open `docs/release-notes/<tag>.md` if he gives you the file). For each item in "Try these first" and each new feature: find it using the menu path named in the notes, do the steps, 📷. Where numbers appear (drive speed, rpm, current, ramp times): first write the expected value from your own calculation with the inputs Tadjiro gives (he reads them from his drive/motor plate; you never copy manual text), then compare with what the program shows. Does the screen say clearly what is a model and what is exact? A model shown as a fact = S2.

## 3. Recording
- Severity (the QA scale): **S1** data loss, crash, cannot start, hash mismatch, admin prompt, lost work; **S2** a core workflow cannot be finished, a wrong number shown as fact, blank window; **S3** everything else wrong or confusing, misleading text; **S4** polish.
- For each finding click Help > **Copy problem report** (this copies text, nothing is sent anywhere). If you can read the clipboard, include its text in the finding; otherwise ask Tadjiro to paste it into Notepad and save it as `Documents\KoperCAD-tests\results\<finding id>.txt`. Also note the log folder (Help > Open log folder).
- If you can save files, save your report as `Documents\KoperCAD-tests\results\results-<date>.md` and screenshots there. If you cannot, the chat message is the record: write it in the template below and keep screenshot names stable so Tadjiro can match them.
- Keep the report in plain text with no secrets and no real project content.

## 4. Results template
```
# KoperCAD Windows hardware test (computer use)
Build under test: <names + SHA256 + About text>   Tester: Claude (computer use)   Date: <>
PC: <Windows version>, GPU <>, display <res>, scale <>, installed copy in real use: <yes/no>

## Summary (max 10 lines)
S1: n  S2: n  S3: n  S4: n  | Verdict for work use: <yes / with workarounds / no> and why in one sentence
Top 3 to fix first: ..

## Results
| Id | Test | Result | Sev | Picture | What I saw (exact text) |

## Findings
### F-001 [S?] <title>
Steps: ..  Expected: ..  Saw: ..  Picture: <name>  Problem report: <pasted or file>

## Not tested (and why)
## Left behind on the PC (so Tadjiro can remove it)
```

## 5. Hand back
At the end write the template filled in as ONE chat message of at most 60 lines (summary + findings table + short list of what is not tested). Tadjiro pastes it into the KoperCAD project chat and attaches the screenshots of S1/S2 findings. If you saved files, say the folder.
At the end also: restore the display scale, close test windows, and list everything you created. Do not delete anything without his OK.

## 6. Later builds
When Tadjiro gives you a newer build: new test run, repeat A, C, D, G, I and the new items from P1, and mark each earlier finding fixed / still there / new.

Start now: ask Tadjiro the questions in section 1 in one message, and begin with the starting-facts screenshots while he answers.
