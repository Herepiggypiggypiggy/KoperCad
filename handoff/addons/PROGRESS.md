# Progress (append only)

2026-10-28 12:52 UTC Task received from the coordinator; split approved into C0b-1 (manager, format, UI, CLI) and C0b-2 (wasmi host).
2026-10-28 Registration PR #1574 for the novo-addons crate (skeleton, crate list row).
2026-10-28 Core crate, store, extension tables, hostile-package tests written and passing.
2026-10-28 App integration, CLI, Python, fixtures, QA scenario 560 and docs written; first full build in progress.
2026-10-28 13:50 UTC #1574 merged; branch rebased on main d1d0bde. App, CLI, Python, fixtures and QA scenario 560 done; tests, fmt, clippy green; QA passes at both sizes.
2026-10-28 14:25 UTC R2 found: install of the version a roll back left behind deleted the new folder. Fixed (removal after registry save, never the installed version), plus verify_strict, device-name ids, unlisted files fail at start, stale staging cleanup, registry lock and per-process temp name, per-version trust answer on roll back; regression tests added.
2026-10-28 15:10 UTC R1 wording: no claim of a running sandbox before the host exists (trust.rs, lib.rs, Cargo.toml, manifest.rs, docs); duplicate names compared with Unicode lowercase.
2026-10-28 15:00 UTC C0b-2 host, app wiring, demo-engine fixture, QA 561 written on claude/c0b-2-wasm-host; PR waits for #1587 to merge (no stacked PR).
2026-10-28 17:54 UTC #1587 merged (789bfa3, head 0a17d7f: zip host byte pinned to Unix after the Windows CI fixture failure). C0b-2 rebased on main. Main A nits applied: registry versions validated on load (a hand-edited version is dropped, never joined to a path), OS litter (Thumbs.db, .DS_Store, desktop.ini) ignored in the unlisted-file check, a registry lock older than 300 s is taken over.
2026-10-28 18:35 UTC R1 on #1632 (da78e5e9): a loop of kopercad.log calls ran for seconds because host calls cost no fuel. Fixed: each log call costs 2 000 fuel (test: at most 100 lines at 200 000 fuel; default-budget loop stops in well under 5 s). Also: lamps are kept only if one of the add-on's own panels declares the id; fuel_used counts the call that stops.
