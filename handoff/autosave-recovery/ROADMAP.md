# Autosave and crash recovery: roadmap

Scope: recovery files in the per-user app data folder, restore offer after a crash, never overwrite
the real file silently, file version migration tests. Crates: novo-storage, novo-app backend.
Design: docs/design/autosave-recovery.md.

## Done
- #375: design note, `Session::create_in/open_in`, `recovery_path_in`, legacy beside-file fallback, tests/recovery_dir.rs, tests/upgrade.rs (committed v2 fixture, derived v1).
- App wiring: `StorageBackend::with_recovery_dir`, `%LOCALAPPDATA%\KoperCAD\recovery`, `Session::set_recovery_dir` after Save As.

## Next
1. Sweep recovery files whose project file is gone (store the project path in recovery meta).
2. Surface `.autosave.broken` files in the UI.
3. Every new migration adds a fixture in crates/novo-storage/tests/upgrade.rs.
