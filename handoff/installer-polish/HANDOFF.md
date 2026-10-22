# Installer and first-run polish: handoff

Lane from the coordinator (2026-10-22): a flawless first experience on a fresh Windows PC, no admin rights.

Already on main before this lane: per-user Inno installer (HKCU file types .kopercad/.novo, upgrade cleanup of NovoElectric names), portable zip with run.cmd, GPU adapter fallback incl. software, crash report card, start-screen first-run path, What's new pill, Help > Open log folder / Copy problem report.

Done in PR "installer-polish-1": portable mode (`kopercad.portable` marker, data in `.\data`), `--safe-graphics` flag + Start menu entry + run-safe-graphics.cmd, richer About (maker, licence, release notes, third-party notices, copy version info, open log folder), uninstall asks whether to remove settings/logs (projects never deleted), CloseApplications on update, QA scenario 74.

Not done / candidates: first-run language and units pickers (no language setting exists; units are mm by convention; Dutch menus are not wanted), HiDPI proof on real Windows, code signing, winget/scoop manifest, in-app update check (offline-first, would need a decision).
Windows-only to verify by hand: see PROGRESS.md.
