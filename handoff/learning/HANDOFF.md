# Learning modules: handoff

## State
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-7lpw0d` (restart from main after each merge; it is the only branch this thread may push to).
- Merged: design (#456), tracks and glossary (#474), engine `novo-learn` (#479), `novo-learn` registered (#466), walk-through #428 (not mine).
- Open: the app shell PR (this branch): `crates/novo-app/src/app/learn.rs` (state, catalog, events, facts, run, start/restart/continue, progress), `learn_ui.rs` (card, pointer ring, numbered notes, Learn window), `learn_tests.rs` (headless tests incl. anchors doc check and the app-side lint), `build.rs` embeds `assets/lessons`, `ProjectBackend::new_project_from`, `Action::Learn` + `help.learn`/`learn.*` commands (novo-workflow, with Dutch words), `assets/lessons/ui.toml` + `novo_learn::UiStrings`, `docs/design/lesson-anchors.md` (generated: `UPDATE_ANCHORS_DOC=1 cargo test -p novo-app anchors_doc`). Engine additions in the same PR: scalar `field.equals`, pin pairs in `potential`, `terminal` check, solve ops `cable`, `issue_revision`, `assign_part`, `place.turns`, `select.add`, `page_title` fact, progress file not UTF-8 set aside, folder depth cap.
- Deviations from the design (recorded in the doc): Continue works only while the lesson's own project is still open; authoring folder is the env var `KOPERCAD_LESSONS` (no `--lessons` flag yet); `zoom_at_least` is relative to the fit zoom of the page; language is English until a language setting exists (`LearnState.lang`).
- Content PRs (all drafts, not merged): #484 drawing, #488 productivity, #495 cabinet, #510 terminals. Against the real engine, #484/#488/#495 parse; #510 still has 4 things the engine lacks (`field.cable`, `rename` solve, `set_potential` solve; told that thread). All four edit `glossary.toml` and conflict there.
- Next after the shell: replay test (`solve` ops through the real command path, per step, in `learn_tests.rs`), more anchors (start panel buttons, outline, wheel, live card, inspector, parts, preferences, shortcuts sheet), the `fast-keys` conversion with the first-run thread, a problem-id registry for `problem_free`, a language setting.
- Anchors requested by content threads: drawing (wheel, live.card, pages.list, menu.canvas.place_component, outline.toggle, palette.field, dock.wire_mode, dock.xref, dock.live); productivity (issue.pill, outline.issues, inspector.part, parts.*, changes.list, compare.swap, preferences.*, shortcuts.sheet, project_settings.standard); cabinet (see `handoff/cabinet-lessons/HANDOFF.md`).

## Resume
```
git fetch origin && git checkout claude/project-thread-7lpw0d
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-learn   # once the crate exists
```
Read first: the design doc, `docs/ONBOARDING-DESIGN.md` (walk-through, PR #428), `docs/design/discoverability.md` (PR #434), `crates/novo-core/src/json.rs` (project JSON form the sample projects use).

## People
Roster: /mnt/project-files/coordination/threads.md. Coordinate with Sample project and first-run walk-through (cse_019sSSXcKhY8k5uYwY3cgSyf), Onboarding and discoverability (cse_01CpKBFnbxA5RuJyh1pGkTdZ), Workspaces (cse_01WAQSEGkMbyreCPG18tsSPc), Design system thread. Message the reviewer with the head SHA after every push; no pushes after READY.

## Decisions
- Lessons are TOML, sample projects are the existing diffable project JSON; both embedded by `novo-app/build.rs`, no Rust edit to add a lesson.
- Pure engine in a new crate `novo-learn` (depends on `novo-core` only). Crate registration is the coordinator's job (CLAUDE.md).
- Checks are events (commands, tools, panels) plus document state; lessons are replayed headless in CI with each step's `solve`.
- Progress in `%APPDATA%\KoperCAD\learn.json`, not the settings blob.
