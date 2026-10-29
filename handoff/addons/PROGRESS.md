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
2026-10-29 01:31 UTC C3 started from the coordinator's note (plans/controller-live-view.md 5b, docs/design/addons.md follow-ups).
2026-10-29 04:35 UTC #1661 merged (engine.wasm 0.68 MB, pack, CI job, 4 MiB cap; R2, R3, Main B READY). #1664 READY on 9c6b81c3 (step-budget line), held for the rc13 freeze. First CI run of #1661 failed only because QA scenario 5620 pointed at dist/; shipped-scenario check allows generated files under target/ only, so the package is built into target/addons.
2026-10-29 04:40 UTC Wizard gate PR from main: file.import_controller greyed with a reason unless the controllers add-on is on, same check on Ctrl+K and scripts; QA 1580 installs the package first.

- C3-3 (engine worker thread): `novo_addons::EngineWorker` (own thread per engine, ordered answers, wake on answer), the app sends with `addon_engine_send` and applies answers each frame. The add-on's menu command is now "Radio panel" (it clashed with the built-in Simulate > Controller panel in Ctrl+K). Tests: `crates/novo-addons/tests/host.rs` (two worker tests), `addons_tests.rs` (async button). QA 5620 and 5621 at both sizes.
