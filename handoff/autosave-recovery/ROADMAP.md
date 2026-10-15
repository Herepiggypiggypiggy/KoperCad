# Autosave and crash recovery: roadmap

Scope: recovery files in the per-user app data folder, restore offer after a crash, never overwrite
the real file silently, file version migration tests. Crate: novo-storage. Design: docs/design/autosave-recovery.md.

## Done
- Design note.
- novo-storage: `Session::create_in/open_in`, `recovery_path_in`, legacy beside-file fallback, tests/recovery_dir.rs.
- tests/upgrade.rs with committed v2 fixture and derived v1 file.

## Next
1. App wiring in `novo-app/src/backend.rs` (App shell thread owns it): pass `data_dir()/recovery`.
2. Sweep recovery files whose project file is gone (store the project path in recovery meta).
3. Surface `.autosave.broken` files in the UI.
