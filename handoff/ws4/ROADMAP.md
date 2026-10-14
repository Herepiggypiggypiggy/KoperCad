# WS-04 Project file (novo-storage): roadmap

Scope: the `.novo` SQLite project file, autosave, crash recovery and schema migrations.
Done-criterion (Phase 0): save, close and reopen gives an identical project. Met.

## Done
- PR #5 (merged 1a68be8): ProjectFile (create/open/read/save/save_changes), Session (autosave, recovery), migrations v1, 25 integration tests.
- PR #22 (merged af01a66): closes the lock-release/rename gap when replacing a file; autosave deleted only after replace succeeds.
- Record JSON stored in novo-core's `Record` serde form (tag "record", novo-core #7), kind column cross-checked on load.

## Next, in order
1. PR #33: Session::close keeps the recovery file while a recovery decision is pending. Waiting for the reviewer's READY and Windows CI; the integration lead merges it.
2. When a novo-core model change alters the record JSON, add a Migration (schema.rs, MIGRATIONS) that rewrites rows; bump nothing else.
3. Later phases (not started; no owner yet):
   - Per-page lazy load (the `records.page` index exists for this).
   - Revisions (WS-18) may need snapshots or history tables; that would mean a new migration.
   - Embedded assets (symbol libraries, images) as new tables.
   - An optional network-share mode: SMB/NFS locking is unreliable, see the file.rs docs.

## Open questions
- Should leftover `<file>.autosave.broken` files be surfaced in the UI beyond a log line? (WS-01 logs the path for now.)
