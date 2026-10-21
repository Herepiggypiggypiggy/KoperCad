2026-10-21 20:30 UTC salvage, damaged-file open, float round trip fix, 600-page and undo-across-save tests written; PR pending.
2026-10-21 21:20 UTC Reviewer A finding: Salvage.info_lost flag, named in summary(), test with only the info row damaged.

2026-10-21 (round 3, branch claude/data-safety-kill-save): real-kill tests of 600-page saves in `crates/novo-tests/tests/data_safety.rs`: a child process saving the 600-page project over its file is killed while the rollback journal exists (mid-write, 6 of 6 rounds); the file reopens whole as one of the two states. A child creating a new 600-page file is killed early or left to finish: no file or a whole one under the name. Limit: a process kill leaves the OS cache intact; this does not prove power-loss safety (that rests on `synchronous = FULL`).

2026-10-21 (branch claude/data-safety-kill-save-guard): follow-up to the merged kill tests, from Reviewer A's non-blocking notes: the child process is killed when the test scope ends (no orphan if an assert fails between spawn and kill), and the "kills inside a save" floor is 3 of 6 instead of 4.
