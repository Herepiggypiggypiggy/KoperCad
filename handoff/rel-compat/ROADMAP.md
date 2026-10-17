# REL-COMPAT roadmap

Scope: release blocker B3 (`release/v0.1-plan.md`): save-file compatibility for v0.1.0.

Done (PR 1): golden v0.1 file, old schema upgrade tests, newer-file refusal (incl. save over it, unknown record kind), kill-during-save and autosave-recovery tests, `docs/FILE-COMPATIBILITY.md`.

Next:
- Windows hands-on (harness): open golden file and an experimental-build file on Windows, kill during save, relaunch recovery card (plan section 5). Not done by this lane.
- CLI/Python open of the golden file (`kopercad-cli`, `import kopercad`) if the harness wants it in CI.
- Read-only file and "missing generated-page table" checks of plan section 5 if not covered.
