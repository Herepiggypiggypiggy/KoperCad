# REL-DIAG handoff

Branch `claude/project-thread-ufdeh6`, one PR from main. See PROGRESS.md for the head SHA.

What exists
- Logs: `%LOCALAPPDATA%\KoperCAD\logs\kopercad.log`, rotated at 1 MiB, 3 old files (`kopercad.1.log`...). Set up by `novo_app::diag::init()` at the top of `main.rs`. If the folder cannot be made the log goes to the console as before.
- Crash: the panic hook writes `crash-YYYYMMDD-HHMMSS.txt` (message, location, thread, last command, backtrace, build, system, graphics, last 40 log lines) and a `crash-pending.txt` marker. The next start reads and deletes the marker and shows the card "KoperCAD stopped unexpectedly" (Open report, Open log folder, Copy problem report, Dismiss). Also fires for a panic on a worker thread.
- Help menu: "Open log folder" (`help.open_log_folder`), "Copy problem report" (`help.copy_problem_report`).
- Force a panic for the crash check: start with the environment variable `KOPERCAD_FORCE_PANIC=1` (panics when the app is created; the report appears, the next start without the variable shows the card).
- Build commit: `build.rs` sets `KOPERCAD_BUILD_COMMIT` from `KOPERCAD_BUILD_ID`, then `GITHUB_SHA`, then `git rev-parse`. Experimental builds are a merge of main and PRs, so their commit is the merge tree commit.
- Autosave and recovery are untouched. The recovery question comes before the crash card.

Verified: headless Linux only (unit tests, clippy, `cargo test -p novo-app -p novo-workflow`). Nobody ran the Windows exe: the folder under `%LOCALAPPDATA%`, the `cmd /C ver` line, Explorer opening the folder and Notepad opening the report need a Windows check.

Traps
- The tests share a process-wide log folder (`diag::tests::GLOBALS`).
- Release has no symbols stripped, but backtraces on Windows MSVC show function names only if the pdb is next to the exe; offsets still help.
