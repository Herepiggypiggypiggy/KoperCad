# Project file compatibility

A KoperCAD project is one `.kopercad` file (files from before the rename end in `.novo` and still open): a SQLite database (`crates/novo-storage`). Two numbers decide whether a build can read it.

| Number | Where | Meaning |
| --- | --- | --- |
| Schema version | SQLite `user_version`; `novo_storage::schema::SCHEMA_VERSION` | Layout of the database (tables, how rows are shaped). KoperCAD 0.1 writes **3**. |
| Record format | `meta.record_format`; `novo_core::JSON_FORMAT_VERSION` | JSON shape of one object. Currently **1**. |

## Promises

1. **Newer build reads older files.** Every schema step is a migration (`schema::MIGRATIONS`). Opening an older file runs all pending steps in one transaction, so a failed upgrade leaves the file as it was. Before the first write the old file is copied to `<file>.v<old>.bak` (the newest backup per old version is kept). Reading (the CLI, Python, Open read-only) upgrades in memory and never writes.
2. **Older build refuses newer files, untouched.** A file with a higher schema version, a higher record format, or an object kind the build does not know gives a readable message ("saved by a newer version of KoperCAD ...") and is not opened for writing. Save as and New project onto such a file are refused too. No backup, recovery or temp file is left beside it.
3. **A crash or kill never leaves half a save.** A save is one SQLite transaction with a rollback journal and `synchronous=FULL`; a process killed mid-save leaves the previous save (SQLite rolls the journal back on the next open). Save as and the first save of a project write a `<file>.saving` temp file and rename it over the target, so the target is either the old file or the new one. Unsaved work is autosaved to a recovery file (app data `recovery` folder); the next start offers it again.

4. **Write-protected files open for looking.** A file marked read-only, or (on Unix) in a folder without write permission, judged from permission bits so opening creates no probe file, gives `StorageError::ReadOnly` ("... is read-only ... Open it read-only, or ... use Save as"). In the app, File > Open, Recent and drag-and-drop then open it read-only automatically (title "(read-only)", status bar "Read-only", Save disabled) with a warning that names Save as; File > Open read-only does the same on request. The CLI and `import kopercad` read such files.
5. **A damaged copy is still a drawing.** If the snapshot or sidecar table is missing the file opens and the missing table is created empty on the first write (revision snapshots or commissioning data in it are lost, the drawing is not). If `meta` or `records` is missing the file is refused with a message naming the table; it is never changed.
6. **A damaged file is never edited, and its drawing is salvaged.** A file cut short (full disk, bad copy) or with overwritten pages gives `StorageError::Corrupt` ("... is damaged: the file is incomplete or has damaged pages ..."). `novo_storage::salvage(path)` reads it read-only, object by object, keeps every object that decodes and passes the model's integrity rules (an object whose parent was lost is dropped), and writes a normal project file `<name>.recovered.<ext>` beside it (same extension as the damaged file) (`-2`, `-3` ... if taken; an earlier copy is never replaced). The report says how many objects were read back, how many were skipped and whether reading stopped at lost pages. In the app, File > Open (also Recent, drag and drop) does this itself: the recovered copy opens and a warning toast names it. Limits: objects stored in the lost pages are gone, and the salvage cannot tell what they were; a file without a SQLite header, or from a newer build, is not salvaged.
7. **A saved number comes back bit for bit.** `serde_json` is built with `float_roundtrip`; without it some values (3.9000000000000004) were read back 1 ULP different, so a 600-page project reopened as a not-quite-identical one.

## Rules for changing the format

- **Adding an optional field or a new kind of record** to the model: allowed within the current versions only when an older build can ignore it. Fields use `#[serde(default)]`; a new record kind makes an older build refuse the file (promise 2, unknown kind), which is the safe outcome. Prefer a schema bump when an older build would otherwise silently drop data on re-save.
- **Changing or removing a field, renaming a kind, or moving data between tables**: add a migration (`schema::MIGRATIONS`, one step higher) that rewrites the rows, and bump `JSON_FORMAT_VERSION` only when the JSON itself changes incompatibly. Never edit an existing migration.
- Add a golden file for the new version (below) and keep all older golden files.
- Releases: the schema version a release writes is recorded in the release notes. Once a version is released, its golden file never changes.

## Golden files and tests

`crates/novo-tests/tests/golden/v0.1-motor-starter.novo` (+ `.json`) is the file KoperCAD 0.1 writes: the motor starter, an issued revision with its snapshot, and a sidecar document. `crates/novo-tests/tests/compat_v0_1.rs` pins:

- the golden file opens, equals its JSON, survives Save as, close and reopen;
- the same project built in code still serialises to the golden JSON (a model change that alters the saved form fails here);
- schema 1 and 2 files (cut from the golden file) upgrade with a backup and lose nothing; every earlier model JSON in the repository (`format_v1.json`, all lesson projects) loads, saves and reopens;
- newer schema, and unknown record kind: readable error, bytes unchanged, no side files;
- a real child process killed at staggered moments while saving: the file always opens as the old or the new save;
- `compat_edge.rs`: the golden file through `kopercad-cli` and the scripting API (`import kopercad` calls the same functions; the Python module itself is not built in CI), newer files refused in words by both, write-protected file and folder, missing tables;
- `crates/novo-storage/tests/damaged.rs`: truncated, wiped-page and missing-parent files are refused in words and salvaged to a recovered copy that opens; floats round-trip exactly;
- `crates/novo-tests/tests/data_safety.rs`: a 600-page project saves, autosaves incrementally, is offered back after a crash and is salvaged when cut short; a crash after undoing past a save offers the undone state; save, undo, save, redo reach the file exactly;
- a child that autosaves and aborts: the app backend offers the work back on the next open.

`crates/novo-tests/tests/compat_releases.rs` reads files written by the release candidates themselves (`golden/releases/rc1`, `rc2`: motor starter, a 300-object page, every lesson's finished project): read-only with the counts the release wrote, open for editing, edit, save, reopen. A new release adds a directory beside these.

To rewrite the golden files after an **intended** format change (and only then): `KOPERCAD_REGEN_GOLDEN=1 cargo test -p novo-tests --test compat_v0_1 regenerate_golden`. For a released version, add a new golden file instead.

Verified on headless Linux only; the Windows run (rename over an open file, antivirus locking) is part of the release's Windows pass.
