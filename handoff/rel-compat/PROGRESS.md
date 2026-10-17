2026-10-17 09:40 UTC lane started; read plan B3, storage tests already cover most engine cases
2026-10-17 10:05 UTC slice 1 written: golden v0.1, compat_v0_1.rs, UnknownRecord, refuse overwrite of newer file, docs
2026-10-17 10:30 UTC slice 2 (on the #965 branch, not yet READY): compat_edge.rs, StorageError::ReadOnly + automatic read-only open in the app, side tables restored when missing, CLI/scripting golden tests
2026-10-17 10:15 UTC CI lint (newer clippy: for_unbounded_range) fixed; kill test now waits for the child's first write before timing the kill (R2/owner flake note)
2026-10-17 10:25 UTC R2: CannotOpen only maps to ReadOnly when the file or its folder exists; test for Save as into a missing folder
