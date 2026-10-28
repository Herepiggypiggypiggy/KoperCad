# File errors and autosave timing (hardware test 2026-10-28)

- HPR-01 was already fixed on main by #1565 (the tested build 64b31c2f predates it): the project file changes only at Save, autosave writes the recovery copy, "Save changes?" before leaving; FIRST-DAY, TESTER-README, troubleshooting and DESIGN agree.
- HPR-02: `Session::autosave_wait()` returns the time left until due, so a tick that arrives a hair early no longer waits a second interval. `StorageBackend::tick` uses it.
- Save targets (`ProjectFile::create_as`): reserved device names (`novo_storage::reserved_device_name`, every platform), missing folder, Windows path too long, and unwritable folder give `CannotSave`/`ReservedName` naming the chosen file once; no SQLite text, no `.saving`.
- Open: reserved names and zero-byte files are refused before SQLite touches them (the file is no longer rewritten). A file another program holds open (Windows sharing violation) is `Locked`; the app no longer claims "opened read-only" when the read-only retry failed.
- A valid SQLite file with orphan objects is `Corrupt` ("some objects refer to others that are missing"), so Open and Recover salvage it.
- CLI: `write_export` refuses device names first (FEC-01: the project-file check opened `CON` and hung); `info` shows plain text for damaged pages (FEC-07); `calc` strips a UTF-8 BOM from the JSON and CSV inputs (FEC-08).
- Not changed on purpose: a file missing its snapshots/sidecar table still gets the empty table when opened for editing (FILE-COMPATIBILITY now says so; reading never writes).
- Windows-only behaviour (sharing violation to Locked) has no Linux test; checked by reading, needs the next hardware pass.
