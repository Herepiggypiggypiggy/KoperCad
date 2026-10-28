# Progress (append only)

2026-10-28 12:52 UTC Task received from the coordinator; split approved into C0b-1 (manager, format, UI, CLI) and C0b-2 (wasmi host).
2026-10-28 Registration PR #1574 for the novo-addons crate (skeleton, crate list row).
2026-10-28 Core crate, store, extension tables, hostile-package tests written and passing.
2026-10-28 App integration, CLI, Python, fixtures, QA scenario 560 and docs written; first full build in progress.
2026-10-28 13:50 UTC #1574 merged; branch rebased on main d1d0bde. App, CLI, Python, fixtures and QA scenario 560 done; tests, fmt, clippy green; QA passes at both sizes.
2026-10-28 14:25 UTC R2 found: install of the version a roll back left behind deleted the new folder. Fixed (removal after registry save, never the installed version), plus verify_strict, device-name ids, unlisted files fail at start, stale staging cleanup, registry lock and per-process temp name, per-version trust answer on roll back; regression tests added.
2026-10-28 15:10 UTC R1 wording: no claim of a running sandbox before the host exists (trust.rs, lib.rs, Cargo.toml, manifest.rs, docs); duplicate names compared with Unicode lowercase.
