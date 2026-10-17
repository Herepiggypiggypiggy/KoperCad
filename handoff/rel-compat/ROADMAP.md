# REL-COMPAT roadmap

Scope: release blocker B3 (`release/v0.1-plan.md`): save-file compatibility for v0.1.0.

Done (PR 1): golden v0.1 file, old schema upgrade tests, newer-file refusal (incl. save over it, unknown record kind), kill-during-save and autosave-recovery tests, `docs/FILE-COMPATIBILITY.md`.

Slice 2 (CLI/scripting open, read-only, missing tables) is on the same branch.

Next:
- Windows hands-on (harness): open golden file and an experimental-build file on Windows, kill during save, relaunch recovery card (plan section 5). Not done by this lane.
