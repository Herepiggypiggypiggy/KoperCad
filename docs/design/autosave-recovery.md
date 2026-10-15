# Autosave and crash recovery

Owner: Autosave thread (novo-storage). Status: storage (#375) and app wiring implemented.

## What already exists (WS-04)

- `Session` autosaves unsaved changes to a recovery file (a small SQLite file) on an interval and
  after edits; a clean close deletes it. A recovery file left at the next open means the app stopped
  without saving.
- The recovery file records which save of which project file it was made against (`base_seq`,
  `base_file`). It is offered only on a match; a stale one is deleted, an unreadable one is renamed
  to `.autosave.broken`, never deleted.
- Recovered work is never written over the real file by itself. The user chooses Restore or
  Discard; Restore only makes the work the open document, and the file changes at the user's next
  Save. Autosave pauses while the question is open.
- Older files are upgraded by `schema::MIGRATIONS` in one transaction after a `VACUUM INTO`
  backup (`<file>.v<N>.bak`); newer files are refused untouched.

## Problem

The recovery file sits next to the project. That breaks down when the project folder is read-only,
on a network share, or synced (OneDrive/Dropbox upload every autosave and may lock the file), and
it litters the user's project folders.

## Decision

Recovery files live in a per-user folder, `<app data>\recovery` (`%LOCALAPPDATA%\NovoElectric\recovery`
on Windows; no admin rights, never `Program Files` or `HKLM`).

- File name: `<project name>-<16 hex of FNV-1a of the lowercased full path>.autosave`. The hash keeps
  same-named projects in different folders apart and is stable across runs.
- `Session::create_in` / `Session::open_in` take the folder; `Session::create` / `open` keep the old
  beside-the-file behavior, so the CLI and Python layer need no change.
- A recovery file beside the project, left by an older version, is still found and offered, and
  removed with the new one.
- If the folder cannot be created the session falls back to beside the project, so autosave never
  silently stops.
- Save As moves recovery to the new project's name; the old one is deleted.

## Safety rules (unchanged, now tested for the new location)

1. Never overwrite the user's real file from recovery without a Save.
2. Never delete a recovery file that holds undecided work (a close before the decision keeps it).
3. A recovery file for a replaced or older file is not offered.

## Migration safety

`crates/novo-storage/tests/upgrade.rs` opens a committed schema-2 file (`tests/fixtures/v2-pump.novo`)
and a derived schema-1 file: contents identical after upgrade, backup kept, read-only `read` never
writes, newer files refused, recovery from an old-version autosave survives the upgrade. Every
future migration adds its fixture there.

## App wiring

`StorageBackend::with_recovery_dir` is set from `recovery_dir()` in `novo-app/src/lib.rs`:
`%LOCALAPPDATA%\KoperCAD\recovery`. Untitled projects already live in app data and keep their
recovery file beside them. Saving an untitled project (`save_as`) switches its session to the
folder (`Session::set_recovery_dir`). No recovery folder existed under the old NovoElectric name, so
only the beside-the-project legacy location needs reading, and it still is.

## Known limits

- The path hash is lowercased (right for Windows). On a case-sensitive file system two projects in
  one folder that differ only by case would share a recovery name; the file-id check then drops the
  other's file as foreign. Very unlikely.

## Follow-ups

- UI: show where leftover `.broken` files are. Remove recovery files whose project no longer exists
  (needs the project path in the recovery meta).
