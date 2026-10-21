# Project data safety: roadmap

Scope: no way for a user to lose a drawing. Old files open, crashes restore, big projects autosave fast, damaged files give a recovered copy, undo across a save is exact.

Done (PR from `claude/project-thread-7iuj1r`):
- `novo_storage::salvage` + app open path: damaged file -> `<name>.recovered.novo` that opens, warning toast (docs/FILE-COMPATIBILITY.md promise 6).
- Damaged databases report `Corrupt` ("damaged") instead of a raw SQLite error.
- Exact f64 round trip (`serde_json` `float_roundtrip`).
- 600-page tests (autosave incremental, crash offer, salvage), undo-across-save tests.
- QA scenario `qa/scenarios/36-damaged-file-recovery.qa`.

Next, in order:
1. Salvage a damaged recovery (`.autosave`) file: today it is set aside as `.autosave.broken`; salvage could read work back from it.
2. Salvage fallback folder when the project's folder is write-protected (today: error).
3. Real-kill test of a 600-page save (compat_v0_1 does it for a small project).
4. DONE (round 3): autosave while background check jobs run, app-loop tests.
5. Fixtures written by older released builds, once releases exist (0.1 golden only).

Open questions: none for the owner.
