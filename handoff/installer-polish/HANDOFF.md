# Installer and first-run polish: handoff

Lane from the coordinator (2026-10-22): a flawless first experience on a fresh Windows PC, no admin rights.

Already on main before this lane: per-user Inno installer (HKCU file types .kopercad/.novo, upgrade cleanup of NovoElectric names), portable zip with run.cmd, GPU adapter fallback incl. software, crash report card, start-screen first-run path, What's new pill, Help > Open log folder / Copy problem report.

Done in PR "installer-polish-1": portable mode (`kopercad.portable` marker, data in `.\data`), `--safe-graphics` flag + Start menu entry + run-safe-graphics.cmd, richer About (maker, licence, release notes, third-party notices, copy version info, open log folder), uninstall asks whether to remove settings/logs (projects never deleted), CloseApplications on update, QA scenario 76.

Not done / candidates: first-run language and units pickers (no language setting exists; units are mm by convention; Dutch menus are not wanted), HiDPI proof on real Windows, code signing, winget/scoop manifest, in-app update check (offline-first, would need a decision).
Windows-only to verify by hand: see PROGRESS.md.

## Installer polish 2 (2026-10-23)
- About > Check for updates (`update_check.rs`): manual only, asks GitHub's public release list, offers the newest stable `vX.Y.Z` newer than the crate version, links the releases page, downloads nothing. Drafts, prereleases, rc tags and non-version tags (`experimental`) are ignored. Limit: a running release candidate shares the crate version with the final release, so it is only told about a newer version, not a newer candidate. `KOPERCAD_UPDATE_FEED` (file or URL) replaces the feed for QA.
- `packaging_tests.rs` holds `novoelectric.iss` to the no-admin rule on every platform (lowest privileges, HKCU only, no Program Files/common folders, both extensions, old-name cleanup, close-on-update, uninstall default No).
- QA scenarios 210 (newer found) and 211 (failed check), pictures in /mnt/project-files/qa/installer-update-check/.
- Still not done: first-run pickers (no language setting), code signing, winget manifest, automatic (opt-in) update check at start.
- Review fixes (main reviewer, 2026-10-23): the up-to-date answer reads "No newer stable version found / Release candidates are not checked" (never "newest version"); errors keep a reason, 403/429 get the rate-limit wording; links must start `https://github.com/`; `KOPERCAD_UPDATE_FEED` takes a file or https only. QA scenario 212 pictures the up-to-date answer.

## Clean-profile pass (2026-10-26, PR #1515)
Windows installer CI now also proves: clean first start and log, per-user file types/uninstall entry/shortcuts, update over NovoElectric leftovers, silent uninstall keeps data and projects. Uninstall drops empty file-type keys and old-name data folders. Findings: /mnt/project-files/installer-clean-profile/FINDINGS.md. Still real-Windows only: wizard, data question, SmartScreen, Explorer double-click, running-copy update, HiDPI, GPU choice. Open: single-instance guard, code signing, winget.
